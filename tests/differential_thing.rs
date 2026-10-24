// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::thing` (`Thing`) against upstream
//! `Common/Thing/Thing.cpp` (commit 0a05454d), compiled unchanged in
//! `differential/wrap_thing.cpp` on the stand-ins of `differential/shim_thing`
//! (see build.rs). Phase 1 layer 5.
//!
//! # Methodology
//!
//! A **program** is a random template (a kind mask, which decides whether the
//! thing sticks to the terrain slope), a random terrain (a plane for the
//! ground and the water, and the gravity) and 1 to 14 operations on one
//! `Thing`. The same program runs in the Rust and in the C++. After every
//! operation both sides write the same words: the operation's own results
//! (the transform after a setter, the position, direction vectors, heights,
//! booleans), then the base state (the twelve transform entries, the cached
//! position and orientation), then the number of transform reactions the
//! operation caused and, for each reaction, the old transform, position and
//! angle it was handed. Reading the heights and direction vectors fills their
//! caches, and the next operations then take the cache-valid paths, so the
//! incremental `setPositionZ` update and the flag clearing of the setters are
//! exercised as they run upstream: the programs are not "read everything after
//! each step", because the cache-invalid paths are also reached in programs
//! that read a height only sometimes.
//!
//! Operations, with their upstream calls:
//!
//! | code | operation | upstream |
//! |---|---|---|
//! | 0 | `setPosition` | `setPosition(&pos)` |
//! | 1 | `setPositionZ` | `setPositionZ(z)` |
//! | 2 | `setOrientation` | `setOrientation(angle)` |
//! | 3 | `setTransformMatrix` | `setTransformMatrix(&m)` |
//! | 4 | `transformPoint` | `transformPoint(&in, &out)` |
//! | 5 | `transformPoint` with NULL input | `transformPoint(NULL, &out)` |
//! | 6 | `transformPoint` with NULL output | `transformPoint(&in, NULL)` |
//! | 7 | `convertBonePosToWorldPos` | both outputs |
//! | 8, 9 | the unit direction vectors | `getUnitDirectionVector2D/3D` |
//! | 10, 11 | the heights above terrain and water | `getHeightAboveTerrain[OrWater]` |
//! | 12-14 | the significance and above-terrain tests | `isSignificantlyAboveTerrain`, `isAboveTerrain[OrWater]` |
//! | 15-17 | the kind-of queries, forwarded to the template | `isKindOf`, `isKindOfMulti`, `isAnyKindOf` |
//!
//! Terrain (cut point 2 of `logic::thing`): `ground(x, y) = ga*x + gb*y + gc`;
//! a point with `x < wx` is under water at height `wz`; `alignOnTerrain`
//! rotates about Z by the orientation, translated to the position, at the
//! ground height when the thing sticks. Both sides compute these with the same
//! float operations in the same order (the C++ is built with
//! `-ffp-contract=off`, so no fused multiply-add on either side).
//!
//! **Undefined upstream inputs** are not sent to the C++ (it would crash or
//! never return). They are counted instead, per kind, and the Rust must
//! return an error for each:
//!
//! - an angle that `normalizeAngle` never reduces (`try_normalize_angle`
//!   errs): upstream never returns from `setOrientation`. The generator draws
//!   such an angle, counts it, and draws another for the same operation.
//! - `Thing(NULL)`, `setPosition(NULL)`: checked by `thing_undefined_inputs`.
//!
//! Comparisons are on bits (`support::Check`). When both sides are NaN with
//! different payloads the pair is counted as NaN-payload-only, as the other
//! differential tests do.
//!
//! **Planted bugs.** Each comparison was also run against one deliberately
//! wrong translation (see the results), to show that the programs can tell a
//! right answer from a plausible wrong one. The bugs were planted by hand in
//! `src/logic/thing.rs`, the run was observed to fail, and the bug was
//! removed; the record is in the doc comment of the test that ran them.
//!
//! # Results
//!
//! See the `DIFF` lines of `cargo test --release --features differential
//! --test differential_thing -- --nocapture`, and the summary in
//! `docs/phase1-porting-graph.md`.

#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::try_normalize_angle;
use rts_lockstep_engine::common::kind_of::{
    KindOfMaskType, KindOfType, KINDOF_COUNT, KINDOF_STICK_TO_TERRAIN_SLOPE,
};
use rts_lockstep_engine::common::trig;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::thing::{
    Thing, ThingContext, ThingError, ThingTemplateView, TransformChange,
};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use std::sync::Arc;
use support::{Check, Gen};

/// Number of kinds (`KINDOF_COUNT`, 116 with ALLOW_SURRENDER undefined).
const KIND_COUNT: u32 = KINDOF_COUNT as u32;
/// Programs per run.
const PROGRAMS: usize = 4000;
/// Operation codes (see the table in the module doc).
const OP_NAMES: [&str; 18] = [
    "setPosition",
    "setPositionZ",
    "setOrientation",
    "setTransformMatrix",
    "transformPoint",
    "transformPoint(NULL in)",
    "transformPoint(NULL out)",
    "convertBonePosToWorldPos",
    "getUnitDirectionVector2D",
    "getUnitDirectionVector3D",
    "getHeightAboveTerrain",
    "getHeightAboveTerrainOrWater",
    "isSignificantlyAboveTerrain",
    "isAboveTerrain",
    "isAboveTerrainOrWater",
    "isKindOf",
    "isKindOfMulti",
    "isAnyKindOf",
];

// ---------------------------------------------------------------------------
// The stand-ins, on the Rust side
// ---------------------------------------------------------------------------

/// The template of a program: the kinds it has (the C++ `ThingTemplate` of
/// shim_thing holds the same mask, and the same three queries).
struct StubTemplate {
    kinds: KindOfMaskType,
}

impl ThingTemplateView for StubTemplate {
    fn is_kind_of(&self, t: KindOfType) -> bool {
        self.kinds.test(t)
    }
    fn is_kind_of_multi(
        &self,
        must_be_set: &KindOfMaskType,
        must_be_clear: &KindOfMaskType,
    ) -> bool {
        self.kinds.test_set_and_clear(must_be_set, must_be_clear)
    }
    fn is_any_kind_of(&self, any_kind_of: &KindOfMaskType) -> bool {
        self.kinds.test_for_any(any_kind_of)
    }
}

/// The terrain of a program, as `wrap_thing.cpp` sets it up.
#[derive(Clone, Copy, Debug)]
struct PlaneTerrain {
    ga: f32,
    gb: f32,
    gc: f32,
    wx: f32,
    wz: f32,
    gravity: f32,
}

impl ThingContext for PlaneTerrain {
    fn ground_height(&self, x: f32, y: f32) -> f32 {
        self.ga * x + self.gb * y + self.gc
    }
    fn underwater_level(&self, x: f32, y: f32) -> Option<f32> {
        let _ = y;
        if x < self.wx {
            Some(self.wz)
        } else {
            None
        }
    }
    fn align_on_terrain(&self, orientation: f32, pos: &Coord3D, stick_to_ground: bool) -> Matrix3D {
        let z = if stick_to_ground {
            self.ground_height(pos.x, pos.y)
        } else {
            pos.z
        };
        let c = trig::cos(orientation);
        let s = trig::sin(orientation);
        Matrix3D::new(c, -s, 0.0, pos.x, s, c, 0.0, pos.y, 0.0, 0.0, 1.0, z)
    }
    fn gravity(&self) -> f32 {
        self.gravity
    }
}

// ---------------------------------------------------------------------------
// Programs
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct Op {
    code: i32,
    a1: i32,
    a2: i32,
    p: [f32; 16],
}

#[derive(Debug)]
struct Program {
    /// Kind bits, two words (`KINDOF_COUNT` = 116 bits).
    kinds: [u64; 2],
    terrain: [f32; 6],
    ops: Vec<Op>,
}

/// Counts of the undefined inputs the generator drew and kept out of the C++.
#[derive(Default)]
struct Undefined {
    /// `setOrientation` angles that `normalizeAngle` never reduces.
    hanging_angles: u64,
}

fn mat(g: &mut Gen) -> [f32; 12] {
    if g.below(4) == 0 {
        // An orthonormal rotation with a translation, as most game matrices are.
        let mut m = Matrix3D::identity();
        m.rotate_z(g.uniform(-4.0, 4.0));
        m.rotate_y(g.uniform(-4.0, 4.0));
        m.rotate_x(g.uniform(-4.0, 4.0));
        m.set_translation(&rts_lockstep_engine::ww::vector3::Vector3::new(
            g.geom(),
            g.geom(),
            g.geom(),
        ));
        m.to_array()
    } else {
        g.vec::<12>(Gen::geom)
    }
}

fn gen_angle(g: &mut Gen, und: &mut Undefined) -> f32 {
    loop {
        let a = if g.below(4) == 0 {
            g.mixed()
        } else {
            g.uniform(-4.0, 4.0)
        };
        if try_normalize_angle(a).is_ok() {
            return a;
        }
        und.hanging_angles += 1;
    }
}

fn gen_op(g: &mut Gen, und: &mut Undefined) -> Op {
    let code = g.below(18) as i32;
    let mut op = Op {
        code,
        a1: 0,
        a2: 0,
        p: [0.0; 16],
    };
    op.p = g.vec::<16>(Gen::geom);
    match code {
        2 => op.p[0] = gen_angle(g, und),
        3 => {
            let m = mat(g);
            op.p[..12].copy_from_slice(&m);
        }
        7 => {
            // bone position and bone transform
            op.p = g.vec::<16>(Gen::geom);
        }
        15 => op.a1 = g.below(KIND_COUNT) as i32,
        16 => {
            op.a1 = if g.below(5) == 0 {
                -1
            } else {
                g.below(KIND_COUNT) as i32
            };
            op.a2 = if g.below(5) == 0 {
                -1
            } else {
                g.below(KIND_COUNT) as i32
            };
        }
        17 => {
            op.a1 = if g.below(5) == 0 {
                -1
            } else {
                g.below(KIND_COUNT) as i32
            }
        }
        _ => {}
    }
    op
}

fn gen_program(g: &mut Gen, und: &mut Undefined) -> Program {
    let mut kind_idx: Vec<i32> = Vec::new();
    for _ in 0..g.below(6) {
        kind_idx.push(g.below(KIND_COUNT) as i32);
    }
    // half the programs stick to the terrain slope
    if g.below(2) == 0 {
        kind_idx.push(KINDOF_STICK_TO_TERRAIN_SLOPE);
    }
    let mask = KindOfMaskType::from_indices(&kind_idx);
    let words = mask.words();
    let terrain = [
        if g.below(4) == 0 { g.mixed() } else { g.geom() },
        if g.below(4) == 0 { g.mixed() } else { g.geom() },
        g.geom(),
        g.geom(),
        g.geom(),
        g.uniform(-2.0, 0.0),
    ];
    let n = 1 + g.below(14) as usize;
    let ops = (0..n).map(|_| gen_op(g, und)).collect();
    Program {
        kinds: [words[0], words[1]],
        terrain,
        ops,
    }
}

// ---------------------------------------------------------------------------
// The two interpreters (same words, same order)
// ---------------------------------------------------------------------------

/// The words of one program, and for each operation its code and the range
/// of words it wrote.
struct Trace {
    words: Vec<u32>,
    segments: Vec<(i32, std::ops::Range<usize>)>,
}

fn bits(f: f32) -> u32 {
    f.to_bits()
}

fn arr<const N: usize>(s: &[f32]) -> [f32; N] {
    let mut a = [0.0f32; N];
    a.copy_from_slice(&s[..N]);
    a
}

fn push_matrix(w: &mut Vec<u32>, m: &Matrix3D) {
    w.extend(m.to_array().iter().map(|&f| bits(f)));
}

fn push_coord(w: &mut Vec<u32>, c: &Coord3D) {
    w.extend([bits(c.x), bits(c.y), bits(c.z)]);
}

fn run_rust(p: &Program) -> Trace {
    let mask = KindOfMaskType::from_words([p.kinds[0], p.kinds[1], 0, 0]);
    let ctx = PlaneTerrain {
        ga: p.terrain[0],
        gb: p.terrain[1],
        gc: p.terrain[2],
        wx: p.terrain[3],
        wz: p.terrain[4],
        gravity: p.terrain[5],
    };
    let template = Arc::new(StubTemplate { kinds: mask });
    let mut thing = Thing::new(Some(template)).expect("a template");
    let mut last = TransformChange {
        old_transform: Matrix3D::identity(),
        old_position: Coord3D::default(),
        old_angle: 0.0,
    };
    let mut words = Vec::new();
    let mut segments = Vec::new();
    for op in &p.ops {
        let start = words.len();
        let q = op.p;
        let mut change: Option<TransformChange> = None;
        match op.code {
            0 => {
                let pos = Coord3D {
                    x: q[0],
                    y: q[1],
                    z: q[2],
                };
                change = Some(thing.set_position(&ctx, Some(&pos)).expect("a position"));
            }
            1 => change = Some(thing.set_position_z(&ctx, q[0])),
            2 => {
                change = Some(
                    thing
                        .set_orientation(&ctx, q[0])
                        .expect("a finite-normalising angle"),
                )
            }
            3 => {
                let m = Matrix3D::from_array(arr::<12>(&q));
                change = Some(thing.set_transform_matrix(&m));
            }
            4 => {
                let input = Coord3D {
                    x: q[0],
                    y: q[1],
                    z: q[2],
                };
                let mut out = Coord3D::default();
                thing.transform_point(Some(&input), Some(&mut out));
                push_coord(&mut words, &out);
            }
            5 => {
                let mut out = Coord3D::default();
                thing.transform_point(None, Some(&mut out));
                push_coord(&mut words, &out);
            }
            6 => {
                let input = Coord3D {
                    x: q[0],
                    y: q[1],
                    z: q[2],
                };
                thing.transform_point(Some(&input), None);
            }
            7 => {
                let bone_pos = Coord3D {
                    x: q[0],
                    y: q[1],
                    z: q[2],
                };
                let bone_tm = Matrix3D::from_array(arr::<12>(&q[3..]));
                let (wp, wt) = thing.convert_bone_pos_to_world_pos(&bone_pos, &bone_tm);
                push_coord(&mut words, &wp);
                push_matrix(&mut words, &wt);
            }
            8 => push_coord(&mut words, &thing.unit_direction_vector_2d()),
            9 => push_coord(&mut words, &thing.unit_direction_vector_3d()),
            10 => words.push(bits(thing.height_above_terrain(&ctx))),
            11 => words.push(bits(thing.height_above_terrain_or_water(&ctx))),
            12 => words.push(u32::from(thing.is_significantly_above_terrain(&ctx))),
            13 => words.push(u32::from(thing.is_above_terrain(&ctx))),
            14 => words.push(u32::from(thing.is_above_terrain_or_water(&ctx))),
            15 => words.push(u32::from(thing.is_kind_of(op.a1))),
            16 => {
                let set = if op.a1 >= 0 {
                    KindOfMaskType::from_indices(&[op.a1])
                } else {
                    KindOfMaskType::new()
                };
                let clr = if op.a2 >= 0 {
                    KindOfMaskType::from_indices(&[op.a2])
                } else {
                    KindOfMaskType::new()
                };
                words.push(u32::from(thing.is_kind_of_multi(&set, &clr)));
            }
            17 => {
                let any = if op.a1 >= 0 {
                    KindOfMaskType::from_indices(&[op.a1])
                } else {
                    KindOfMaskType::new()
                };
                words.push(u32::from(thing.is_any_kind_of(&any)));
            }
            c => panic!("unknown op {c}"),
        }
        if let Some(c) = change {
            last = c;
        }
        // the base state
        push_matrix(&mut words, thing.transform_matrix());
        push_coord(&mut words, thing.position());
        words.push(bits(thing.orientation()));
        // the reactions
        let delta = u32::from(change.is_some());
        words.push(delta);
        if delta > 0 {
            push_matrix(&mut words, &last.old_transform);
            push_coord(&mut words, &last.old_position);
            words.push(bits(last.old_angle));
        }
        segments.push((op.code, start..words.len()));
    }
    Trace { words, segments }
}

fn run_cpp(p: &Program) -> Vec<u32> {
    let mut ops = Vec::with_capacity(3 * p.ops.len());
    let mut params = Vec::with_capacity(16 * p.ops.len());
    for op in &p.ops {
        ops.extend([op.code, op.a1, op.a2]);
        params.extend_from_slice(&op.p);
    }
    let cap = 64 * p.ops.len() + 64;
    let mut out = vec![0u32; cap];
    let n = unsafe {
        cpp::ref_thing_run(
            p.kinds.as_ptr(),
            p.terrain.as_ptr(),
            p.ops.len() as i32,
            ops.as_ptr(),
            params.as_ptr(),
            out.as_mut_ptr(),
            cap as i32,
        )
    };
    assert!(n >= 0, "ref_thing_run returned {n}");
    out.truncate(n as usize);
    out
}

// ---------------------------------------------------------------------------
// The comparison
// ---------------------------------------------------------------------------

#[test]
fn thing_programs_match_cpp() {
    let mut g = Gen::new(0x7417);
    let mut und = Undefined::default();
    let mut checks: Vec<Check> = OP_NAMES
        .iter()
        .map(|n| Check::new(&format!("Thing {n}")))
        .collect();
    let mut lengths = Check::new("Thing words per program");
    let mut programs = 0u64;
    let mut ops_run = 0u64;
    for i in 0..PROGRAMS {
        let p = gen_program(&mut g, &mut und);
        let trace = run_rust(&p);
        let cppw = run_cpp(&p);
        programs += 1;
        ops_run += p.ops.len() as u64;
        lengths.eq(trace.words.len(), cppw.len(), || {
            format!("program {i}: {p:?}")
        });
        if trace.words.len() != cppw.len() {
            continue;
        }
        for (j, (code, range)) in trace.segments.iter().enumerate() {
            let c = &mut checks[*code as usize];
            for k in range.clone() {
                let r = f32::from_bits(trace.words[k]);
                let cc = f32::from_bits(cppw[k]);
                c.f32(r, cc, || {
                    format!("program {i} op {j} (code {code}) word {k}: {p:?}")
                });
            }
        }
    }
    println!("DIFF programs {programs}, operations {ops_run}");
    println!(
        "DIFF undefined inputs kept out of the C++: {} hanging angles (Rust reports them as errors)",
        und.hanging_angles
    );
    lengths.finish();
    for c in checks {
        c.finish();
    }
}

/// The undefined inputs: each must come back as an error, with the thing
/// unchanged where the operation would have written it.
#[test]
fn thing_undefined_inputs_are_errors() {
    let mut n_null_template = 0u64;
    let mut n_null_position = 0u64;
    let mut n_hanging = 0u64;
    let mut unchanged = 0u64;

    let none: Option<Arc<StubTemplate>> = None;
    assert_eq!(Thing::new(none).err(), Some(ThingError::NoTemplate));
    n_null_template += 1;

    let ctx = PlaneTerrain {
        ga: 0.5,
        gb: -0.25,
        gc: 3.0,
        wx: -10.0,
        wz: 2.0,
        gravity: -1.0,
    };
    let mut thing = Thing::new(Some(Arc::new(StubTemplate {
        kinds: KindOfMaskType::new(),
    })))
    .expect("template");
    let before = (
        *thing.transform_matrix(),
        *thing.position(),
        thing.orientation(),
    );
    assert_eq!(
        thing.set_position(&ctx, None).err(),
        Some(ThingError::NullPosition)
    );
    n_null_position += 1;
    let after = (
        *thing.transform_matrix(),
        *thing.position(),
        thing.orientation(),
    );
    if before == after {
        unchanged += 1;
    }

    let mut g = Gen::new(0x7419);
    for _ in 0..1000 {
        // angles that no finite subtraction of 2*PI reduces, and infinities
        let sign = if g.below(2) == 0 { 1.0f32 } else { -1.0 };
        let a = if g.below(2) == 0 {
            sign * f32::INFINITY
        } else {
            sign * 1.0e10 * g.uniform(1.0, 2.0)
        };
        let before = (
            *thing.transform_matrix(),
            *thing.position(),
            thing.orientation(),
        );
        match thing.set_orientation(&ctx, a) {
            Err(ThingError::AngleNeverNormalizes) => n_hanging += 1,
            other => panic!("angle {a}: expected AngleNeverNormalizes, got {other:?}"),
        }
        let after = (
            *thing.transform_matrix(),
            *thing.position(),
            thing.orientation(),
        );
        if before == after {
            unchanged += 1;
        }
    }
    println!(
        "DIFF undefined inputs: NULL template {n_null_template}, NULL position {n_null_position}, \
         hanging angles {n_hanging}; the thing unchanged in {unchanged} of {}",
        n_null_position + n_hanging
    );
    assert_eq!(
        unchanged,
        n_null_position + n_hanging,
        "an error changed the thing"
    );
}
