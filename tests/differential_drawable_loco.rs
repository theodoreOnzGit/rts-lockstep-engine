#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable.cpp (DrawableLocoInfo::DrawableLocoInfo, calcPhysicsXform,
//   calcPhysicsXformThrust, calcPhysicsXformHoverOrWings, calcPhysicsXformTreads, calcPhysicsXformWheels,
//   calcPhysicsXformMotorcycle)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Drawable.h (TWheelInfo, DrawableLocoInfo, PhysicsXformInfo)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #182 item 2: the drawable's client physics models (`src/client/drawable_loco.rs`, #41) against the upstream
//! text compiled verbatim (`differential/wrap_drawable_loco.cpp`, which lists the cuts, the stand-ins and the words).
//!
//! # Methodology
//!
//! 20,000 random sequences of 1 to 60 frames. Each sequence has a locomotor template (the appearance one of the nine,
//! 85 % one of the five with a model: wheels, treads, hover, thrust, wings, motorcycle; the 24 constants in their INI
//! ranges, each of the optional ones 0 a fair share of the time, 1 in 100 words from `Gen::mixed`: huge, tiny,
//! infinite and NaN), changed for a new one in 3 % of frames, and the object's geometry (major, minor and bounding
//! radii). Each frame: what the drawable has (no object 3 %, no AI 2 %, no current locomotor 3 %, no physics 4 %, else
//! all), the position and terrain height (airborne 15 %, then 0-20 above), a unit direction, acceleration, velocity
//! (its z 0, tiny or -10..10), its magnitude (the vector's or any), motive 70 %, turning -1/0/1, the forward speed (the
//! velocity along the direction, or any), the terrain normal (flat 20 %, else tilted up to ~39 degrees), the locomotor
//! speed (0 one in 20), an overlapped object 35 % (near, its radius and height, shrubbery, low overlappable, infantry,
//! crushed front or back each at random) and a previous overlap 40 %. Both sides start `InitRandom(seed)` (the port's
//! `RandomValues::new(seed)`, the client stream for the bounce and the treads' roughness).
//!
//! The port runs [`DrawableLocoInfo::calc_physics_xform`] (the dispatch `Drawable::calc_physics_xform` calls) on facts
//! built from the words as `physics_xform_facts` builds them (the overlap through [`OverlapFacts::of`], the same
//! shrubbery filter and flatness); the C++ runs the cut `calcPhysicsXform`. Compared every frame, bit for bit (NaN on
//! both sides as equal): the answer, the four totals, whether `m_locoInfo` exists, its 14 `Real`s and its
//! `TWheelInfo` (5 `Real`s, 2 `Int`s); and the client random stream's six words at the end of each sequence.
//!
//! # Predictions (written before the first run, `issue182i2_predictions.md`)
//!
//! Defects expected in the #41 translation, from reading upstream under the shim's math.h (MSVC's C semantics: `sin`,
//! `cos`, `sqrt`, `atan2`, `fabs` of a float are the double functions, the expression continuing in double): (1) the
//! hover's rudder and elevator correction (`DEG * sin(mod += RATE)`, double) computed in float: 60-90 % of the
//! hover/wings sequences; (2) the hover's `atan2(vz, sqrt(...))` in float: within those; (3) the wheels' `m_totalZ +=
//! fabs(h)/divisor` in float: 5-25 % of the wheels sequences (only past PI/8 of pitch); thrust, treads, the new
//! motorcycle and the dispatch 0; the random state differing in < 50 sequences. After the fixes: 0.
//!
//! # Results (2026-11-13)
//!
//! - **First build:** clean. **First run** (the #41 translation, the motorcycle new): 8,688 of 20,000 sequences
//!   mismatched, 77,203 words (611,459 frames); the first mismatch of every one in a hover/wings frame (7,035: totals
//!   pitch and yaw, `m_pitch`) or a wheels frame (1,653: total Z); thrust, treads, motorcycle, the dispatch and the
//!   random state 0, as predicted. Each defect measured alone (the other two fixed): the rudder/elevator `sin`/`cos` in
//!   float 7,125 sequences (43,089 words, max ULP 11,529 on totals near zero); the climb's `atan2(vz, sqrt(...))` in
//!   float 1,240 (31,379 words: `m_pitch` drifts on through the spring); the wheels' `m_totalZ += fabs(h)/divisor` in
//!   float 1,904 (3,483 words, max ULP 1); the wheels' airborne `fabs(ph)/4 + fabs(rh)/4` in float 0 (kept in double,
//!   as upstream's text reads). **Prediction misses:** the wheels' count (I predicted 200-1,000; the generator's tilted
//!   terrain puts the pitch past PI/8 of the ground's far more often than I guessed: not measured further), and my
//!   sequence-count estimate for hover/wings (I counted ~4,200 sequences; 6 of the 9 appearances share 85 %, so 5,700
//!   start with one, and template changes add more).
//! - **After the fixes** (`drawable_loco.rs`'s module docs): 0 mismatches over 20,000 sequences (611,459 frames); then,
//!   with the thrust's wobble on a 1/64 grid one template in five (so plant 9891's equal case is reachable: the first
//!   plant run did not catch it): **0 mismatches** over 20,000 sequences, 607,059 frames (476,504 answered), 16,430,593
//!   words, max ULP 0; 590,619 words NaN on both sides (the 1-in-100 `Gen::mixed` constants), 96,614 of them with
//!   different payload bits (counted, not failed, as `Check::f32` does); the client random state equal in every sequence.
//! - **Plants**, mismatching sequences of 20,000 (words): 9891 200 (18,936); 9892 3,627 (56,228); 9893 2,577 (135,244);
//!   9894 5,162 (365,422); 9895 6,838 (500,524); 9896 4,160 (314,075); 9897 3,106 (202,575); 9898 6,911 (113,376); 9899
//!   3,433 (464,374); 9900 2,463 (334,205); 9901 2,948 (209,345); 9902 4,112 (322,444); 9903 3,892 (468,666); 9904
//!   3,924 (39,140); 9905 2,929 (375,450); 9906 3,556 (688,511); 9907 3,481 (17,357); 9908 4,233 (109,033); 9909 2,201
//!   (102,884); 9910 2,043 (60,935); 9911 4,820 (118,113); 9912 2,812 (371,249); 9913 3,614 (11,221); 9914 745 (5,028);
//!   9915 4,837 (1,535,085). **25 of 25 caught.** The first 9914 (the motorcycle's airborne branch not copying the rear
//!   left offset to the right) was not caught, and cannot be: the ground block, which upstream runs in the air too,
//!   copies it again in the same call; it was replaced by the airborne branch's height test inverted.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints `PLANTED` and passes when caught):
//!
//! | id | planted bug |
//! |---|---|
//! | 9891 | thrust: the wobble turns at `m_pitch > MAX_WOBBLE`, not `>=` |
//! | 9892 | thrust: the total yaw is not set |
//! | 9893 | thrust: the roll does not accumulate (the total roll is `THRUST_ROLL`) |
//! | 9894 | hover: the roll ignores `UNIFORM_AXIAL_DAMPING` |
//! | 9895 | hover: the lateral velocity's `-dir.y` sign dropped |
//! | 9896 | hover: the climb pitch is added, not subtracted |
//! | 9897 | hover: `TINY_DZ` tests `vel.x` |
//! | 9898 | hover: the rudder's `sin` of the modulator before it advances |
//! | 9899 | treads: no `OVERLAP_SHRINK_FACTOR` |
//! | 9900 | treads: flat when crushed front *or* back (`OverlapFacts::of`) |
//! | 9901 | treads: the leave-overlap kick without a previous overlap |
//! | 9902 | treads: a positive pitch rate is not halved |
//! | 9903 | treads: the fake Z falls by 0.1 a draw, not 0.2 |
//! | 9904 | treads: the total Z after the fake Z physics, not before |
//! | 9905 | wheels and motorcycle: the bounce's cases 1 and 2 swapped |
//! | 9906 | wheels: no return in the air |
//! | 9907 | wheels and motorcycle: `m_framesAirborne` never takes the counter |
//! | 9908 | wheels and motorcycle: the wheel angle not reversed moving backwards |
//! | 9909 | wheels: the suspension damped going up, not down |
//! | 9910 | wheels: no clamp at the maximum extension |
//! | 9911 | motorcycle: the total roll unclamped |
//! | 9912 | motorcycle: in the air the springs pull towards the ground's pitch, not level |
//! | 9913 | motorcycle: the height added in the air too |
//! | 9914 | motorcycle: the airborne rear wheel's height test inverted |
//! | 9915 | dispatch: `LOCO_WINGS` has no model |

mod support;

use petir::Real;
use rts_lockstep_engine::client::drawable_loco::{DrawableLocoInfo, LocoFacts, OverlapFacts, PhysicsXformFacts, XformObjectFacts};
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::locomotor::LocomotorAppearance;
use support::{Check, Gen};

/// Words per frame (see the wrapper) and per recorded frame.
const IN_WORDS: usize = 63;
const OUT_WORDS: usize = 27;
const SEQUENCES: usize = 20_000;

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn fb(v: f32) -> i32 {
    v.to_bits() as i32
}
fn bf(w: i32) -> f32 {
    f32::from_bits(w as u32)
}

/// A parameter word: 1 in 100 from `Gen::mixed`, else `v`.
fn param(g: &mut Gen, v: f32) -> i32 {
    fb(if g.below(100) == 0 { g.mixed() } else { v })
}

/// `zero` per cent 0, else uniform in [lo, hi).
fn maybe(g: &mut Gen, zero: u32, lo: f32, hi: f32) -> f32 {
    if g.below(100) < zero { 0.0 } else { g.uniform(lo, hi) }
}

/// The locomotor's 25 words: the appearance, then the 24 constants in the wrapper's order.
fn template(g: &mut Gen) -> [i32; 25] {
    let appearance = if g.below(100) < 85 { [1, 2, 3, 4, 5, 8][g.below(6) as usize] } else { [0, 6, 7][g.below(3) as usize] };
    let f: [f32; 24] = [
        g.uniform(0.0, 0.6),                           // accelPitchLimit
        g.uniform(0.0, 0.6),                           // decelPitchLimit
        maybe(g, 20, 0.0, 0.2),                        // bounceKick
        g.uniform(0.0, 0.5),                           // pitchStiffness
        g.uniform(0.0, 0.5),                           // rollStiffness
        g.uniform(0.0, 1.0),                           // pitchDamping
        g.uniform(0.0, 1.0),                           // rollDamping
        maybe(g, 50, -2.0, 2.0),                       // pitchByZVelCoef
        maybe(g, 50, -0.2, 0.2),                       // thrustRoll
        maybe(g, 35, 0.001, 0.2),                      // wobbleRate
        g.uniform(0.0, 0.6),                           // maxWobble
        g.uniform(-0.6, 0.0),                          // minWobble
        maybe(g, 20, -0.1, 0.1),                       // forwardVelCoef
        maybe(g, 20, -0.1, 0.1),                       // lateralVelCoef
        maybe(g, 20, 0.0, 0.5),                        // forwardAccelCoef
        maybe(g, 20, 0.0, 0.5),                        // lateralAccelCoef
        if g.below(100) < 30 { 1.0 } else { g.uniform(0.0, 1.0) }, // uniformAxialDamping
        0.0,                                           // hasSuspension (a word, below)
        g.uniform(-3.0, 0.0),                          // maxWheelExtension
        g.uniform(0.0, 0.8),                           // wheelTurnAngle
        maybe(g, 25, -0.3, 0.3),                       // rudderCorrectionDegree
        g.uniform(0.0, 0.5),                           // rudderCorrectionRate
        maybe(g, 25, -0.3, 0.3),                       // elevatorCorrectionDegree
        g.uniform(0.0, 0.5),                           // elevatorCorrectionRate
    ];
    let mut w = [0i32; 25];
    w[0] = appearance;
    for (i, v) in f.iter().enumerate() {
        w[1 + i] = if i == 17 { g.below(2) as i32 } else { param(g, *v) };
    }
    // 1 in 5: the wobble on a grid of 1/64 (rate 1..8, max 0..32, min -32..0 sixty-fourths), so the thrust's pitch
    // lands exactly on its limits (`m_pitch >= MAX_WOBBLE`, `<= MIN_WOBBLE`: the equal case is reachable)
    if g.below(5) == 0 {
        w[1 + 9] = fb((1 + g.below(8)) as f32 / 64.0);
        w[1 + 10] = fb(g.below(33) as f32 / 64.0);
        w[1 + 11] = fb(-(g.below(33) as f32) / 64.0);
    }
    w
}

/// One frame's 63 words.
fn frame(g: &mut Gen, loco: &[i32; 25], radii: [f32; 3]) -> Vec<i32> {
    let mut w = Vec::with_capacity(IN_WORDS);
    w.push(match g.below(100) {
        0..=2 => 0,
        3..=4 => 1,
        5..=7 => 2,
        8..=11 => 3,
        _ => 4,
    });
    w.extend_from_slice(loco);
    let height = g.uniform(0.0, 50.0);
    let airborne = g.below(100) < 15;
    let pos = [g.uniform(0.0, 1000.0), g.uniform(0.0, 1000.0), height + if airborne { g.uniform(0.0, 20.0) } else { g.uniform(-0.2, 0.5) }];
    let angle = g.uniform(0.0, 6.2831855);
    let dir = [Real::cos(angle), Real::sin(angle), 0.0f32];
    let accel = [g.uniform(-5.0, 5.0), g.uniform(-5.0, 5.0), g.uniform(-2.0, 2.0)];
    let s = if g.below(2) == 0 { 1.0 } else { 0.1 };
    let vz = match g.below(10) {
        0..=3 => 0.0,
        4..=6 => g.uniform(-0.002, 0.002),
        _ => g.uniform(-10.0, 10.0),
    };
    let vel = [g.uniform(-30.0, 30.0) * s, g.uniform(-30.0, 30.0) * s, vz];
    let mag = if g.below(10) < 8 { Real::sqrt(vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]) } else { g.uniform(0.0, 40.0) };
    let fwd = if g.below(10) < 7 { dir[0] * vel[0] + dir[1] * vel[1] } else { g.uniform(-20.0, 20.0) };
    let (nx, ny) = if g.below(5) == 0 { (0.0, 0.0) } else { (g.uniform(-0.8, 0.8), g.uniform(-0.8, 0.8)) };
    let nl: f32 = Real::sqrt(nx * nx + ny * ny + 1.0);
    let normal = [nx / nl, ny / nl, 1.0 / nl];
    for v in pos.iter().chain(&dir).chain(&accel).chain(&vel) {
        w.push(fb(*v));
    }
    w.push(fb(mag));
    w.push(i32::from(g.below(10) < 7));
    w.push(g.below(3) as i32 - 1);
    w.push(fb(fwd));
    w.push(fb(height));
    for v in normal {
        w.push(fb(v));
    }
    w.push(i32::from(airborne));
    for r in radii {
        w.push(fb(r));
    }
    w.push(fb(if g.below(20) == 0 { 0.0 } else { g.uniform(1.0, 40.0) }));
    // the overlap
    let near = radii[2] * 1.5;
    w.push(i32::from(g.below(100) < 35));
    for v in [pos[0] + g.uniform(-near, near), pos[1] + g.uniform(-near, near), pos[2]] {
        w.push(fb(v));
    }
    w.push(fb(g.uniform(0.5, 15.0)));
    w.push(fb(g.uniform(0.0, 10.0)));
    for p in [10, 15, 15, 30, 30] {
        w.push(i32::from(g.below(100) < p));
    }
    w.push(i32::from(g.below(100) < 40));
    assert_eq!(w.len(), IN_WORDS);
    w
}

/// A sequence's input words: [seed, frame count, frames...].
fn sequence(g: &mut Gen) -> Vec<i32> {
    let frames = 1 + g.below(60) as usize;
    let mut input = vec![g.u32() as i32, frames as i32];
    let mut loco = template(g);
    let radii = [g.uniform(1.0, 20.0), g.uniform(1.0, 15.0), g.uniform(1.0, 25.0)];
    for _ in 0..frames {
        if g.below(100) < 3 {
            loco = template(g);
        }
        input.extend(frame(g, &loco, radii));
    }
    input
}

fn c3(w: &[i32]) -> Coord3D {
    Coord3D { x: bf(w[0]), y: bf(w[1]), z: bf(w[2]) }
}

/// The port's facts for one frame, as `physics_xform_facts` gathers them from the World.
fn facts(p: &[i32]) -> PhysicsXformFacts {
    let kind = p[0];
    let f = |i: usize| bf(p[2 + i]);
    let locomotor = (kind >= 3).then(|| LocoFacts {
        appearance: LocomotorAppearance::from_index(p[1]).expect("an appearance"),
        accel_pitch_limit: f(0),
        decel_pitch_limit: f(1),
        bounce_kick: f(2),
        pitch_stiffness: f(3),
        roll_stiffness: f(4),
        pitch_damping: f(5),
        roll_damping: f(6),
        pitch_by_z_vel_coef: f(7),
        thrust_roll: f(8),
        wobble_rate: f(9),
        max_wobble: f(10),
        min_wobble: f(11),
        forward_vel_coef: f(12),
        lateral_vel_coef: f(13),
        forward_accel_coef: f(14),
        lateral_accel_coef: f(15),
        uniform_axial_damping: f(16),
        has_suspension: p[2 + 17] != 0,
        max_wheel_extension: f(18),
        wheel_turn_angle: f(19),
        rudder_correction_degree: f(20),
        rudder_correction_rate: f(21),
        elevator_correction_degree: f(22),
        elevator_correction_rate: f(23),
        is_moving_backwards: false,
    });
    let o = &p[26..];
    let v = &o[25..];
    let object = (kind >= 4).then(|| XformObjectFacts {
        position: c3(&o[0..]),
        dir: c3(&o[3..]),
        accel: c3(&o[6..]),
        vel: c3(&o[9..]),
        velocity_magnitude: bf(o[12]),
        is_motive: o[13] != 0,
        turning: o[14],
        forward_speed_2d: bf(o[15]),
        layer_height: bf(o[16]),
        normal: c3(&o[17..]),
        airborne: o[20] != 0,
        major_radius: bf(o[21]),
        minor_radius: bf(o[22]),
        bounding_circle_radius: bf(o[23]),
        cur_locomotor_speed: bf(o[24]),
        overlap: if v[0] != 0 { OverlapFacts::of(c3(&v[1..]), bf(v[4]), bf(v[5]), [v[6] != 0, v[7] != 0, v[8] != 0], (v[9] != 0, v[10] != 0)) } else { None },
        had_previous_overlap: v[11] != 0,
    });
    PhysicsXformFacts { apply: true, locomotor, object }
}

/// The port's run of one sequence: its 27 words a frame and the client stream's state at the end.
fn port(input: &[i32]) -> (Vec<i32>, [u32; 6]) {
    let mut random = RandomValues::new(input[0] as u32);
    let mut slot: Option<DrawableLocoInfo> = None;
    let mut out = Vec::new();
    for p in input[2..].chunks(IN_WORDS) {
        let info = DrawableLocoInfo::calc_physics_xform(&mut slot, &facts(p), &mut random);
        out.push(i32::from(info.is_some()));
        let i = info.unwrap_or_default();
        out.extend([i.total_pitch, i.total_roll, i.total_yaw, i.total_z].map(fb));
        out.push(i32::from(slot.is_some()));
        let li = slot.unwrap_or(DrawableLocoInfo {
            wobble: 0.0,
            ..Default::default()
        });
        out.extend(
            [
                li.pitch,
                li.pitch_rate,
                li.roll,
                li.roll_rate,
                li.yaw,
                li.acceleration_pitch,
                li.acceleration_pitch_rate,
                li.acceleration_roll,
                li.acceleration_roll_rate,
                li.overlap_z_vel,
                li.overlap_z,
                li.wobble,
                li.yaw_modulator,
                li.pitch_modulator,
            ]
            .map(fb),
        );
        let w = li.wheel_info;
        out.extend([w.front_left_height_offset, w.front_right_height_offset, w.rear_left_height_offset, w.rear_right_height_offset, w.wheel_angle].map(fb));
        out.push(w.frames_airborne_counter);
        out.push(w.frames_airborne);
    }
    (out, random.client)
}

/// The C++ run: its words and the client stream's state at the end.
fn reference(input: &[i32]) -> (Vec<i32>, [u32; 6]) {
    let frames = input[1] as usize;
    let mut out = vec![0i32; frames * OUT_WORDS];
    let n = unsafe { cpp::ref_drawable_loco_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    let mut state = [0u32; 6];
    unsafe { cpp::ref_get_state(1, state.as_mut_ptr()) };
    (out, state)
}

const FIELDS: [&str; OUT_WORDS] = [
    "answer", "totalPitch", "totalRoll", "totalYaw", "totalZ", "locoInfo", "m_pitch", "m_pitchRate", "m_roll",
    "m_rollRate", "m_yaw", "m_accelerationPitch", "m_accelerationPitchRate", "m_accelerationRoll",
    "m_accelerationRollRate", "m_overlapZVel", "m_overlapZ", "m_wobble", "m_yawModulator", "m_pitchModulator",
    "frontLeft", "frontRight", "rearLeft", "rearRight", "wheelAngle", "framesAirborneCounter", "framesAirborne",
];

const APPEARANCES: [&str; 9] = ["LEGS_TWO", "WHEELS_FOUR", "TREADS", "HOVER", "THRUST", "WINGS", "CLIMBER", "OTHER", "MOTORCYCLE"];

#[test]
fn the_physics_models_match_upstream() {
    let _lock = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x182_0002);
    let mut check = Check::new("Drawable calcPhysicsXform models");
    let mut by_field = [0u64; OUT_WORDS];
    let mut first_field = [0u64; OUT_WORDS];
    let mut seqs_by_app = [0u64; 9];
    let mut bad_by_app = [0u64; 9];
    let mut random_bad = 0u64;
    let mut bad_seqs = 0u64;
    let (mut frames, mut answered, mut nan_words) = (0u64, 0u64, 0u64);
    for case in 0..SEQUENCES {
        let input = sequence(&mut g);
        seqs_by_app[input[2 + 1] as usize] += 1;
        let mut first_bad: Option<(usize, usize)> = None;
        let (r, rs) = port(&input);
        let (c, cs) = reference(&input);
        check.eq(r.len(), c.len(), || format!("case {case}: word count"));
        let before = check.mismatches;
        for (k, (a, b)) in r.iter().zip(&c).enumerate() {
            let field = k % OUT_WORDS;
            if field == 0 {
                frames += 1;
                answered += *a as u64;
            }
            let m0 = check.mismatches;
            if matches!(field, 0 | 5 | 25 | 26) {
                check.eq(*a, *b, || format!("case {case} frame {} {}", k / OUT_WORDS, FIELDS[field]));
            } else {
                if bf(*a).is_nan() && bf(*b).is_nan() {
                    nan_words += 1;
                }
                check.f32(bf(*a), bf(*b), || format!("case {case} frame {} {} ({})", k / OUT_WORDS, FIELDS[field], APPEARANCES[input[2 + (k / OUT_WORDS) * IN_WORDS + 1] as usize]));
            }
            if check.mismatches > m0 {
                by_field[field] += 1;
                first_bad.get_or_insert((input[2 + (k / OUT_WORDS) * IN_WORDS + 1] as usize, field));
            }
        }
        let m0 = check.mismatches;
        check.eq(rs, cs, || format!("case {case}: the client random state"));
        if check.mismatches > m0 {
            random_bad += 1;
        }
        if check.mismatches > before {
            bad_seqs += 1;
            if let Some((app, field)) = first_bad {
                bad_by_app[app] += 1;
                first_field[field] += 1;
            }
        }
    }
    println!("sequences {SEQUENCES}, frames {frames}, answered {answered}, NaN words {nan_words}");
    println!("mismatching sequences {bad_seqs} (random state differs in {random_bad})");
    for (i, name) in APPEARANCES.iter().enumerate() {
        println!("  {name:<11} sequences starting with it {:>6}  mismatching sequences whose first mismatch is in its frame {:>6}", seqs_by_app[i], bad_by_app[i]);
    }
    for (i, name) in FIELDS.iter().enumerate() {
        if by_field[i] > 0 {
            println!("  field {name:<24} mismatching words {:>8}  first mismatch of a sequence {:>6}", by_field[i], first_field[i]);
        }
    }
    if planted() {
        check.finish_planted();
    } else {
        check.finish();
    }
}
