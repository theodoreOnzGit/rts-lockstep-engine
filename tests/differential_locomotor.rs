// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::locomotor` (`LocomotorTemplate` and its INI block,
//! `LocomotorStore`, `Locomotor` with every movement function, `LocomotorSet`) against upstream
//! `Locomotor.cpp` (commit 0a05454d), compiled UNCHANGED in `differential/wrap_locomotor.cpp`
//! (see build.rs, block `p1w6-locomotor`). Phase 1 layer 8.
//!
//! # What is real on the C++ side, and what is a stand-in
//!
//! Real: `Locomotor.cpp` in full, `Locomotor.h`, `LocomotorSet.h`, the INI reader and field
//! parsers, name keys, `XferSave`/`XferLoad`, WWMath `Matrix3D`/`Vector3`, `Trig.cpp`'s
//! `Cos`/`Sin`/`ACos`, `RandomValue.cpp`, `BaseType.h`, and (cut verbatim from upstream by
//! build.rs) the enums it names and the position/orientation/transform/height methods of
//! `Thing`, so the object the locomotor drives moves as the engine's does.
//! Stand-ins (`differential/shim_locomotor/locomotor_world.h`): the rest of `Object`,
//! `PhysicsBehavior`, the body module, `TheTerrainLogic`, `ThePartitionManager`,
//! `TheAI->pathfinder()`, `TheGlobalData`, `TheGameLogic`. They answer from a scene (`LocoEnv`)
//! the test installs for each call (analytic ground, water and pathfinder tables, physics
//! numbers) and LOG every call that crosses the cut, with its arguments. The Rust side is
//! `logic::locomotor` driven through a `LocomotorHost` that answers from the SAME tables, on the
//! translated `Thing` (`logic::thing`), and writes the same log.
//!
//! # Methodology
//!
//! 1. **INI.** Random `Locomotor` files (every field, valid and broken values, unknown fields,
//!    comments, repeated names, missing `End`, `THRUST` blocks the validator rejects; loaded as
//!    plain or as override loads) go through the real `INI::load` and `Ini::load`. Compared: the
//!    outcome (what was thrown), and the whole store (every template of every override chain,
//!    every field as bits, the override flags), including the partly filled state after a
//!    failure, and again after `LocomotorStore::reset`.
//! 2. **Movement.** Random movement PROGRAMS over many frames: a random template of every
//!    appearance and Z behaviour, a random scene (terrain slope, water, bridge layer, invalid
//!    zone, damage state, stun, gravity, mass), and per frame one of `moveTowardsPosition`
//!    (random goals, path distances shorter and longer than the real distance, blocked or not),
//!    `moveTowardsAngle`, `maintainCurrentPosition`, `setPhysicsOptions`, `startMove`, the
//!    getters for every damage condition, and the setters. Both sides get the SAME inputs each
//!    frame (a canonical simulation integrates the forces and scrubs the Rust side logged, so the
//!    trajectories are plausible: braking, turning, climbing, hovering), and after EVERY frame
//!    the following are compared bit for bit: the whole call log (every force, scrub, turning,
//!    status, model condition, transform change, and every terrain / partition / pathfinder
//!    query with its arguments), the results, the object (transform, cached position and angle,
//!    status bits) and the locomotor (all 16 saved data members). A mismatch ends that program
//!    and is reported with the frame, the operation and the first differing record.
//! 3. **Locomotor xfer.** `Locomotor::xfer` through the real `XferSave` (bytes) and `XferLoad`
//!    (the state of a locomotor made fresh and loaded), sampled during the programs.
//! 4. **LocomotorSet.** Random sets (templates added in random order, `findLocomotor` for every
//!    surface mask, valid surfaces, downhill flag) and `xferSelfAndCurLocoPtr` save/load.
//!
//! No tolerance anywhere: every comparison is on raw bits (a NaN pair with different payload is
//! counted, not failed, as in `support::Check`), and a float that differs is reported with its
//! ULP distance.
//!
//! **Planted bugs.** Each comparison was also run against a deliberately wrong translation
//! (edited into `src/logic/locomotor.rs`, run, then removed); the results are in the Results
//! section.
//!
//! # Results
//!
//! Recorded by running `cargo test --release --features differential --test
//! differential_locomotor -- --nocapture --test-threads=1` (the `DIFF`, `COVERAGE` and `PLANTED`
//! lines).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::{PathfindLayer, LAYER_GROUND, LAYER_WALL};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, TYPE_TABLE};
use rts_lockstep_engine::common::kind_of::{KindOfMaskType, KindOfType, KINDOF_DOZER, KINDOF_PROJECTILE};
use rts_lockstep_engine::common::model_state::ModelConditionFlagType;
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::ObjectStatusTypes;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::trig;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::differential::LocoEnv as Env;
use rts_lockstep_engine::logic::locomotor::*;
use rts_lockstep_engine::logic::module::body_module::BodyDamageType;
use rts_lockstep_engine::logic::terrain_logic::Underwater;
use rts_lockstep_engine::logic::thing::{Thing, ThingContext, ThingTemplateView};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::sync::Arc;
use support::{ulp_distance, Gen};

// ---------------------------------------------------------------------------
// Log codes: the numbers of `locomotor_world.h`
// ---------------------------------------------------------------------------

const L_TRANSFORM: u32 = 1;
const L_SET_STATUS: u32 = 2;
const L_SET_MODEL_COND: u32 = 3;
const L_CLEAR_MODEL_COND: u32 = 4;
const L_APPLY_FORCE: u32 = 5;
const L_SCRUB: u32 = 6;
const L_SET_TURNING: u32 = 7;
const L_SET_EXTRA_FRICTION: u32 = 8;
const L_SET_AIRBORNE_FRICTION: u32 = 9;
const L_SET_STICK: u32 = 10;
const L_GROUND_HEIGHT: u32 = 20;
const L_LAYER_HEIGHT: u32 = 21;
const L_UNDERWATER: u32 = 22;
const L_HIGHEST_LAYER: u32 = 23;
const L_STRUCT_HEIGHT: u32 = 24;
const L_VALID_TERRAIN: u32 = 25;

const OBJECT_STATUS_BRAKING: i32 = 15;
const OBJECT_STATUS_DECK_HEIGHT_OFFSET: i32 = 29;

fn cstr(s: &str) -> CString {
    CString::new(s).expect("no NUL")
}

// ---------------------------------------------------------------------------
// The Rust side of the stand-ins
// ---------------------------------------------------------------------------

struct TplView;

impl ThingTemplateView for TplView {
    fn is_kind_of(&self, _t: KindOfType) -> bool {
        false
    }
    fn is_kind_of_multi(&self, _a: &KindOfMaskType, _b: &KindOfMaskType) -> bool {
        false
    }
    fn is_any_kind_of(&self, _a: &KindOfMaskType) -> bool {
        false
    }
}

/// `TheTerrainLogic` and `TheGlobalData` of the Thing: the scene's tables.
struct TCtx<'a>(&'a Env);

impl ThingContext for TCtx<'_> {
    fn ground_height(&self, x: f32, y: f32) -> f32 {
        self.0.ga * x + self.0.gb * y + self.0.gc
    }
    fn underwater_level(&self, x: f32, _y: f32) -> Option<f32> {
        if x < self.0.wx {
            Some(self.0.wz)
        } else {
            None
        }
    }
    fn align_on_terrain(&self, _o: f32, _p: &Coord3D, _s: bool) -> Matrix3D {
        panic!("no KINDOF_STICK_TO_TERRAIN_SLOPE in this scene")
    }
    fn gravity(&self) -> f32 {
        self.0.gravity
    }
}

/// The object, its physics behaviour and the world, answering from the scene and logging every
/// call, as `wrap_locomotor.cpp` does.
struct Host {
    thing: Thing<TplView>,
    env: Env,
    status: u64,
    log: Vec<u32>,
}

impl Host {
    fn new() -> Self {
        Host { thing: Thing::new(Some(Arc::new(TplView))).unwrap(), env: Env::default(), status: 0, log: Vec::new() }
    }

    fn l(&mut self, code: u32, w: &[u32]) {
        self.log.push(code);
        self.log.push(w.len() as u32);
        self.log.extend_from_slice(w);
    }

    /// The owner's reaction to a transform change (`reactToTransformChange`): logged with the
    /// NEW transform, cached position and angle.
    fn react(&mut self) {
        let m = self.thing.transform_matrix().to_array();
        let p = *self.thing.position();
        let mut w: Vec<u32> = m.iter().map(|f| f.to_bits()).collect();
        w.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), self.thing.orientation().to_bits()]);
        self.l(L_TRANSFORM, &w);
    }

    fn ground(&self, x: f32, y: f32) -> f32 {
        self.env.ga * x + self.env.gb * y + self.env.gc
    }
}

impl LocomotorGlobals for Host {
    fn gravity(&self) -> f32 {
        self.env.gravity
    }
    fn movement_penalty_damage_state(&self) -> BodyDamageType {
        self.env.penalty
    }
}

impl LocomotorHost for Host {
    fn has_physics(&self) -> bool {
        self.env.has_physics != 0
    }
    fn position(&self) -> Coord3D {
        *self.thing.position()
    }
    fn set_position(&mut self, pos: &Coord3D) {
        let _ = self.thing.set_position(&TCtx(&self.env), Some(pos)).unwrap();
        self.react();
    }
    fn orientation(&self) -> f32 {
        self.thing.orientation()
    }
    fn set_orientation(&mut self, angle: f32) {
        let _ = self.thing.set_orientation(&TCtx(&self.env), angle).unwrap();
        self.react();
    }
    fn unit_direction_vector_2d(&mut self) -> Coord3D {
        self.thing.unit_direction_vector_2d()
    }
    fn transform_matrix(&self) -> Matrix3D {
        *self.thing.transform_matrix()
    }
    fn set_transform_matrix(&mut self, mtx: &Matrix3D) {
        let _ = self.thing.set_transform_matrix(mtx);
        self.react();
    }
    fn layer(&self) -> PathfindLayer {
        self.env.layer
    }
    fn is_kind_of(&self, kind: KindOfType) -> bool {
        (kind == KINDOF_PROJECTILE && self.env.kind_mask & 1 != 0) || (kind == KINDOF_DOZER && self.env.kind_mask & 2 != 0)
    }
    fn status_test(&self, status: ObjectStatusTypes) -> bool {
        (self.status >> status) & 1 != 0
    }
    fn set_status(&mut self, status: ObjectStatusTypes, set: bool) {
        self.l(L_SET_STATUS, &[status as u32, u32::from(set)]);
        if set {
            self.status |= 1 << status;
        } else {
            self.status &= !(1 << status);
        }
    }
    fn carrier_deck_height(&self) -> f32 {
        self.env.deck_height
    }
    fn major_radius(&self) -> f32 {
        self.env.major_radius
    }
    fn bounding_circle_radius(&self) -> f32 {
        self.env.bounding_circle_radius
    }
    fn is_disabled_held(&self) -> bool {
        self.env.disabled_held != 0
    }
    fn set_model_condition_state(&mut self, condition: ModelConditionFlagType) {
        self.l(L_SET_MODEL_COND, &[condition as u32]);
    }
    fn clear_model_condition_state(&mut self, condition: ModelConditionFlagType) {
        self.l(L_CLEAR_MODEL_COND, &[condition as u32]);
    }
    fn is_above_terrain(&mut self) -> bool {
        self.thing.is_above_terrain(&TCtx(&self.env))
    }
    fn damage_state(&self) -> BodyDamageType {
        self.env.body_state
    }
    fn is_stunned(&self) -> bool {
        self.env.stunned != 0
    }
    fn apply_motive_force(&mut self, force: &Coord3D) {
        self.l(L_APPLY_FORCE, &[force.x.to_bits(), force.y.to_bits(), force.z.to_bits()]);
    }
    fn scrub_velocity_2d(&mut self, desired_speed: f32) {
        self.l(L_SCRUB, &[desired_speed.to_bits()]);
    }
    fn set_turning(&mut self, turning: PhysicsTurningType) {
        self.l(L_SET_TURNING, &[turning as i32 as u32]);
    }
    fn velocity(&self) -> Coord3D {
        Coord3D { x: self.env.vel[0], y: self.env.vel[1], z: self.env.vel[2] }
    }
    fn velocity_magnitude(&self) -> f32 {
        self.env.vel_mag
    }
    fn forward_speed_2d(&self) -> f32 {
        self.env.fwd2d
    }
    fn forward_speed_3d(&self) -> f32 {
        self.env.fwd3d
    }
    fn mass(&self) -> f32 {
        self.env.mass
    }
    fn is_motive(&self) -> bool {
        self.env.motive != 0
    }
    fn set_extra_friction(&mut self, friction: f32) {
        self.l(L_SET_EXTRA_FRICTION, &[friction.to_bits()]);
    }
    fn set_allow_airborne_friction(&mut self, allow: bool) {
        self.l(L_SET_AIRBORNE_FRICTION, &[u32::from(allow)]);
    }
    fn set_stick_to_ground(&mut self, stick: bool) {
        self.l(L_SET_STICK, &[u32::from(stick)]);
    }
    fn frame(&self) -> u32 {
        self.env.frame
    }
    fn ground_height(&mut self, x: f32, y: f32) -> f32 {
        self.l(L_GROUND_HEIGHT, &[x.to_bits(), y.to_bits()]);
        self.ground(x, y)
    }
    fn layer_height(&mut self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        self.l(L_LAYER_HEIGHT, &[x.to_bits(), y.to_bits(), layer as u32, 1]);
        let mut h = self.ground(x, y);
        if layer != LAYER_GROUND {
            h += self.env.layer_offset;
        }
        h
    }
    fn layer_height_unclipped(&mut self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        self.l(L_LAYER_HEIGHT, &[x.to_bits(), y.to_bits(), layer as u32, 0]);
        let mut h = self.ground(x, y);
        if layer != LAYER_GROUND {
            h += self.env.layer_offset;
        }
        h + 0.5
    }
    fn underwater(&mut self, x: f32, y: f32) -> Underwater {
        self.l(L_UNDERWATER, &[x.to_bits(), y.to_bits()]);
        let terrain_z = self.ground(x, y);
        if x < self.env.wx {
            Underwater { underwater: true, water_z: Some(self.env.wz), terrain_z }
        } else {
            Underwater { underwater: false, water_z: None, terrain_z }
        }
    }
    fn highest_layer_for_destination(&mut self, pos: &Coord3D) -> PathfindLayer {
        self.l(L_HIGHEST_LAYER, &[pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
        self.env.highest_layer
    }
    fn ground_or_structure_height(&mut self, x: f32, y: f32) -> f32 {
        self.l(L_STRUCT_HEIGHT, &[x.to_bits(), y.to_bits()]);
        self.ground(x, y) + self.env.struct_offset
    }
    fn valid_movement_terrain(&mut self, layer: PathfindLayer, _loco: &Locomotor, pos: &Coord3D) -> bool {
        self.l(L_VALID_TERRAIN, &[layer as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
        let b = self.env.inv_box;
        !(self.env.inv_enabled != 0 && pos.x >= b[0] && pos.x <= b[1] && pos.y >= b[2] && pos.y <= b[3])
    }
}

// ---------------------------------------------------------------------------
// Comparison helpers
// ---------------------------------------------------------------------------

/// Counts one comparison of two arrays of words (floats as bits or integers): `float_at` says
/// which positions are floats (for the ULP distance of a miss).
struct WordCheck {
    name: String,
    compared: u64,
    mismatches: u64,
    nan_only: u64,
    max_ulp: u64,
    examples: Vec<String>,
}

impl WordCheck {
    fn new(name: &str) -> Self {
        WordCheck { name: name.to_string(), compared: 0, mismatches: 0, nan_only: 0, max_ulp: 0, examples: Vec::new() }
    }

    /// One comparison (one frame's array): true if equal.
    fn words(&mut self, rust: &[u32], cpp: &[u32], float_at: impl Fn(usize) -> bool, ctx: impl FnOnce() -> String) -> bool {
        self.compared += 1;
        if rust == cpp {
            return true;
        }
        let n = rust.len().min(cpp.len());
        let mut first = None;
        let mut real_miss = rust.len() != cpp.len();
        for i in 0..n {
            if rust[i] != cpp[i] {
                if float_at(i) {
                    let (a, b) = (f32::from_bits(rust[i]), f32::from_bits(cpp[i]));
                    if a.is_nan() && b.is_nan() {
                        continue;
                    }
                    if !a.is_nan() && !b.is_nan() {
                        self.max_ulp = self.max_ulp.max(ulp_distance(a, b));
                    }
                }
                real_miss = true;
                if first.is_none() {
                    first = Some(i);
                }
            }
        }
        if !real_miss {
            self.nan_only += 1;
            return true;
        }
        self.mismatches += 1;
        if self.examples.len() < 6 {
            let at = first.unwrap_or(n);
            self.examples.push(format!(
                "{}: first difference at word {at} (rust {:#010x?} c++ {:#010x?}; lengths {} / {})",
                ctx(),
                rust.get(at),
                cpp.get(at),
                rust.len(),
                cpp.len()
            ));
        }
        false
    }

    fn finish(self) {
        println!(
            "DIFF {:<44} compared {:>9}  mismatches {:>6}  (NaN-payload-only {}, max ULP {})",
            self.name, self.compared, self.mismatches, self.nan_only, self.max_ulp
        );
        assert!(self.compared > 0, "{}: nothing compared", self.name);
        assert_eq!(self.mismatches, 0, "{}: {} of {} differ from the C++ reference; first: {:#?}", self.name, self.mismatches, self.compared, self.examples);
    }
}

/// Which words of a log record are floats: everything but the integer arguments.
fn log_float_mask(code: u32, i: usize) -> bool {
    match code {
        L_TRANSFORM | L_APPLY_FORCE | L_SCRUB | L_SET_EXTRA_FRICTION | L_GROUND_HEIGHT | L_UNDERWATER | L_HIGHEST_LAYER | L_STRUCT_HEIGHT => true,
        L_LAYER_HEIGHT => i < 2,
        L_VALID_TERRAIN => i >= 1,
        _ => false,
    }
}

/// Compare two logs record by record; on a miss describe the first differing record. The ULP
/// of a differing float word is reported through `check`.
fn compare_logs(check: &mut WordCheck, rust: &[u32], cpp: &[u32], ctx: impl Fn() -> String) -> bool {
    check.compared += 1;
    if rust == cpp {
        return true;
    }
    // walk the records
    let (mut i, mut j) = (0usize, 0usize);
    let mut nan_only = true;
    let mut desc = String::new();
    while i < rust.len() || j < cpp.len() {
        if i >= rust.len() || j >= cpp.len() {
            nan_only = false;
            desc = format!("one log ends early (rust {} words, c++ {})", rust.len(), cpp.len());
            break;
        }
        let (rc, rn) = (rust[i], rust[i + 1] as usize);
        let (cc, cn) = (cpp[j], cpp[j + 1] as usize);
        let (rw, cw) = (&rust[i + 2..i + 2 + rn], &cpp[j + 2..j + 2 + cn]);
        if rc != cc || rn != cn {
            nan_only = false;
            desc = format!("record codes differ: rust {rc} {rw:x?} / c++ {cc} {cw:x?}");
            break;
        }
        if rw != cw {
            for (k, (a, b)) in rw.iter().zip(cw.iter()).enumerate() {
                if a == b {
                    continue;
                }
                if log_float_mask(rc, k) {
                    let (fa, fb) = (f32::from_bits(*a), f32::from_bits(*b));
                    if fa.is_nan() && fb.is_nan() {
                        continue;
                    }
                    if !fa.is_nan() && !fb.is_nan() {
                        check.max_ulp = check.max_ulp.max(ulp_distance(fa, fb));
                    }
                }
                nan_only = false;
                if desc.is_empty() {
                    desc = format!("record {rc} word {k}: rust {a:#010x} ({}) c++ {b:#010x} ({})", f32::from_bits(*a), f32::from_bits(*b));
                }
            }
            if !nan_only {
                break;
            }
        }
        i += 2 + rn;
        j += 2 + cn;
    }
    if nan_only {
        check.nan_only += 1;
        return true;
    }
    check.mismatches += 1;
    if check.examples.len() < 6 {
        check.examples.push(format!("{}: {desc}", ctx()));
    }
    false
}

// ---------------------------------------------------------------------------
// INI: generator, Rust side, dumps
// ---------------------------------------------------------------------------

struct IniCtx {
    store: LocomotorStore,
    names: NameKeyGenerator,
}

fn h_locomotor(ini: &mut Ini<'_>, c: &mut IniCtx) -> IniResult<()> {
    c.store.parse_locomotor_template_definition(ini, &mut c.names)
}

fn h_stub(_: &mut Ini<'_>, _: &mut IniCtx) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}

/// The block table: `Locomotor` is translated; the other handlers throw `ERROR_BUG`, as the
/// stubs of wrap_ini.cpp do.
fn blocks() -> Vec<BlockParse<IniCtx>> {
    TYPE_TABLE.iter().map(|&(token, _, _)| BlockParse { token, parse: if token == "Locomotor" { h_locomotor } else { h_stub } }).collect()
}

impl IniCtx {
    fn new() -> Self {
        let mut names = NameKeyGenerator::new();
        names.reset();
        IniCtx { store: LocomotorStore::new(), names }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    status: i32,
    detail: Vec<u8>,
}

fn rust_outcome(r: IniResult<()>) -> Outcome {
    let (status, detail) = match r {
        Ok(()) => (0, Vec::new()),
        Err(e) => match e.thrown() {
            None => panic!("the generator produced an input that is undefined upstream: {e:?}"),
            Some(Thrown::Exception(m)) => (1, m),
            Some(Thrown::Status(c)) => (2, c.to_le_bytes().to_vec()),
            Some(Thrown::ErrorCode(c)) => (3, c.to_le_bytes().to_vec()),
            Some(Thrown::OutOfRange) => (4, Vec::new()),
        },
    };
    Outcome { status, detail }
}

/// `INI::load` of `data` on the C++ side (wrap_ini.cpp's hooks), into `TheLocomotorStore`.
fn cpp_load(name: &str, data: &[u8], load_type: IniLoadType) -> Outcome {
    let cname = cstr(name);
    let mut log = vec![0u8; 16];
    let mut msg = vec![0u8; 4096];
    let (mut log_len, mut crc, mut code) = (0i32, 0u32, 0u32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const c_char, data.len() as c_int);
        cpp::ref_ini_load_typed(cname.as_ptr(), load_type as c_int, 0, log.as_mut_ptr(), log.len() as c_int, &mut log_len, &mut crc, msg.as_mut_ptr() as *mut c_char, msg.len() as c_int, &mut code)
    };
    assert_eq!(log_len, 0, "the real block handlers log nothing");
    let detail = match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    };
    Outcome { status, detail }
}

fn put_str(v: &mut Vec<u32>, s: &[u8]) {
    v.push(s.len() as u32);
    v.extend(s.iter().map(|&b| u32::from(b)));
}

fn put_template(v: &mut Vec<u32>, t: &LocomotorTemplate) {
    let r = |v: &mut Vec<u32>, x: f32| v.push(x.to_bits());
    let b = |v: &mut Vec<u32>, x: bool| v.push(u32::from(x));
    put_str(v, &t.name);
    v.push(t.surfaces as u32);
    r(v, t.max_speed);
    r(v, t.max_speed_damaged);
    r(v, t.min_speed);
    r(v, t.max_turn_rate);
    r(v, t.max_turn_rate_damaged);
    r(v, t.acceleration);
    r(v, t.acceleration_damaged);
    r(v, t.lift);
    r(v, t.lift_damaged);
    r(v, t.braking);
    r(v, t.min_turn_speed);
    r(v, t.preferred_height);
    r(v, t.preferred_height_damping);
    r(v, t.circling_radius);
    r(v, t.speed_limit_z);
    r(v, t.extra_2d_friction);
    r(v, t.max_thrust_angle);
    v.push(t.behavior_z as i32 as u32);
    v.push(t.appearance as i32 as u32);
    v.push(t.move_priority as i32 as u32);
    r(v, t.accel_pitch_limit);
    r(v, t.decel_pitch_limit);
    r(v, t.bounce_kick);
    r(v, t.pitch_stiffness);
    r(v, t.roll_stiffness);
    r(v, t.pitch_damping);
    r(v, t.roll_damping);
    r(v, t.pitch_by_z_vel_coef);
    r(v, t.thrust_roll);
    r(v, t.wobble_rate);
    r(v, t.min_wobble);
    r(v, t.max_wobble);
    r(v, t.forward_vel_coef);
    r(v, t.lateral_vel_coef);
    r(v, t.forward_accel_coef);
    r(v, t.lateral_accel_coef);
    r(v, t.uniform_axial_damping);
    r(v, t.turn_pivot_offset);
    v.push(t.airborne_targeting_height as u32);
    r(v, t.close_enough_dist);
    b(v, t.is_close_enough_dist_3d);
    r(v, t.ultra_accurate_slide_into_place_factor);
    b(v, t.locomotor_works_when_dead);
    b(v, t.allow_motive_force_while_airborne);
    b(v, t.apply_2d_friction_when_airborne);
    b(v, t.downhill_only);
    b(v, t.stick_to_ground);
    b(v, t.can_move_backward);
    b(v, t.has_suspension);
    r(v, t.maximum_wheel_extension);
    r(v, t.maximum_wheel_compression);
    r(v, t.wheel_turn_angle);
    r(v, t.wander_width_factor);
    r(v, t.wander_length_factor);
    r(v, t.wander_about_point_radius);
    r(v, t.rudder_correction_degree);
    r(v, t.rudder_correction_rate);
    r(v, t.elevator_correction_degree);
    r(v, t.elevator_correction_rate);
}

/// The dump of `ref_loco_dump_store`.
fn rust_store_dump(store: &LocomotorStore) -> Vec<u32> {
    let mut v = vec![store.len() as u32];
    for (key, head) in store.chains() {
        v.push(key.0);
        let mut chain = Vec::new();
        let mut node = Some(head);
        while let Some(n) = node {
            chain.push(n);
            node = n.get_next_override();
        }
        v.push(chain.len() as u32);
        for n in chain {
            v.push(u32::from(n.is_override()));
            put_template(&mut v, &n.data);
        }
    }
    v
}

fn cpp_store_dump() -> Vec<u32> {
    let mut buf = vec![0u32; 1 << 14];
    loop {
        let n = unsafe { cpp::ref_loco_dump_store(buf.as_mut_ptr(), buf.len() as c_int) } as usize;
        if n <= buf.len() {
            buf.truncate(n);
            return buf;
        }
        buf = vec![0u32; n];
    }
}

/// Every word of a store dump is compared as an integer or a float alike (a miss is reported
/// with the ULP distance if both look like floats): no per-word typing is kept.
fn any_float(_: usize) -> bool {
    true
}

const APPEARANCES: [&str; 9] = LOCO_APPEARANCE_NAMES;
const Z_NAMES: [&str; 8] = LOCO_Z_NAMES;
const PRIORITIES: [&str; 3] = LOCO_PRIORITY_NAMES;

/// A random decimal real, moderate magnitude, never undefined upstream.
fn real_text(g: &mut Gen) -> String {
    match g.below(10) {
        0 => "0".to_string(),
        1 => format!("{}", g.below(1000)),
        2 => format!("-{}.{}", g.below(100), g.below(1000)),
        3 => format!("{}.{:03}", g.below(100000), g.below(1000)),
        _ => format!("{:.4}", g.uniform(-200.0, 400.0)),
    }
}

/// The value text of an INI field of the given kind. `bad` selects a broken value.
fn value_text(g: &mut Gen, kind: &str, bad: bool) -> String {
    if bad {
        return match g.below(5) {
            0 => "xyz".to_string(),
            1 => String::new(),
            2 => "true".to_string(), // scanBool takes yes and no only
            3 => "1".to_string(),
            _ => "??".to_string(),
        };
    }
    match kind {
        "real" => real_text(g),
        "int" => match g.below(5) {
            0 => "0".to_string(),
            1 => format!("-{}", g.below(100)),
            2 => "2147483647".to_string(),
            _ => format!("{}", g.below(5000)),
        },
        "bool" => ["Yes", "No", "yes", "NO", "YES", "no"][g.below(6) as usize].to_string(),
        "surfaces" => {
            let names = LOCOMOTOR_SURFACE_TYPE_NAMES;
            match g.below(6) {
                0 => "NONE".to_string(),
                1 => format!("+{}", names[g.below(5) as usize]),
                2 => format!("-{} +{}", names[g.below(5) as usize], names[g.below(5) as usize]),
                _ => {
                    let mut s = Vec::new();
                    for _ in 0..1 + g.below(4) {
                        s.push(names[g.below(5) as usize]);
                    }
                    s.join(" ")
                }
            }
        }
        "appearance" => APPEARANCES[g.below(9) as usize].to_string(),
        "z" => Z_NAMES[g.below(8) as usize].to_string(),
        "priority" => PRIORITIES[g.below(3) as usize].to_string(),
        _ => unreachable!(),
    }
}

/// Every field of the table: its INI name and value kind.
const FIELDS: [(&str, &str); 60] = [
    ("Surfaces", "surfaces"),
    ("Speed", "real"),
    ("SpeedDamaged", "real"),
    ("TurnRate", "real"),
    ("TurnRateDamaged", "real"),
    ("Acceleration", "real"),
    ("AccelerationDamaged", "real"),
    ("Lift", "real"),
    ("LiftDamaged", "real"),
    ("Braking", "real"),
    ("MinSpeed", "real"),
    ("MinTurnSpeed", "real"),
    ("PreferredHeight", "real"),
    ("PreferredHeightDamping", "real"),
    ("CirclingRadius", "real"),
    ("Extra2DFriction", "real"),
    ("SpeedLimitZ", "real"),
    ("MaxThrustAngle", "real"),
    ("ZAxisBehavior", "z"),
    ("Appearance", "appearance"),
    ("GroupMovementPriority", "priority"),
    ("AccelerationPitchLimit", "real"),
    ("DecelerationPitchLimit", "real"),
    ("BounceAmount", "real"),
    ("PitchStiffness", "real"),
    ("RollStiffness", "real"),
    ("PitchDamping", "real"),
    ("RollDamping", "real"),
    ("ThrustRoll", "real"),
    ("ThrustWobbleRate", "real"),
    ("ThrustMinWobble", "real"),
    ("ThrustMaxWobble", "real"),
    ("PitchInDirectionOfZVelFactor", "real"),
    ("ForwardVelocityPitchFactor", "real"),
    ("LateralVelocityRollFactor", "real"),
    ("ForwardAccelerationPitchFactor", "real"),
    ("LateralAccelerationRollFactor", "real"),
    ("UniformAxialDamping", "real"),
    ("TurnPivotOffset", "real"),
    ("Apply2DFrictionWhenAirborne", "bool"),
    ("DownhillOnly", "bool"),
    ("AllowAirborneMotiveForce", "bool"),
    ("LocomotorWorksWhenDead", "bool"),
    ("AirborneTargetingHeight", "int"),
    ("StickToGround", "bool"),
    ("CanMoveBackwards", "bool"),
    ("HasSuspension", "bool"),
    ("FrontWheelTurnAngle", "real"),
    ("MaximumWheelExtension", "real"),
    ("MaximumWheelCompression", "real"),
    ("CloseEnoughDist", "real"),
    ("CloseEnoughDist3D", "bool"),
    ("SlideIntoPlaceTime", "real"),
    ("WanderWidthFactor", "real"),
    ("WanderLengthFactor", "real"),
    ("WanderAboutPointRadius", "real"),
    ("RudderCorrectionDegree", "real"),
    ("RudderCorrectionRate", "real"),
    ("ElevatorCorrectionDegree", "real"),
    ("ElevatorCorrectionRate", "real"),
];

const NAMES: [&str; 6] = ["Tank", "tank", "Heli", "Boat", "Jet", "Guy"];

/// One random `Locomotor` block (possibly broken).
fn gen_block(g: &mut Gen, out: &mut String) {
    let name = NAMES[g.below(NAMES.len() as u32) as usize];
    if g.below(80) == 0 {
        out.push_str("Locomotor\n");
        return;
    }
    out.push_str(&format!("Locomotor {name}\n"));
    let mut lines = 0;
    for _ in 0..g.below(10) {
        let (f, kind) = FIELDS[g.below(FIELDS.len() as u32) as usize];
        let bad = g.below(70) == 0;
        let sep = ["= ", " = ", "=", "\t= "][g.below(4) as usize];
        let field = if g.below(10) == 0 { f.to_uppercase() } else { f.to_string() };
        out.push_str(&format!("  {field} {sep}{}\n", value_text(g, kind, bad)));
        lines += 1;
        match g.below(120) {
            0..=3 => out.push_str("  ; a comment\n"),
            4..=6 => out.push('\n'),
            7 => out.push_str("  Wibble = 3\n"),
            _ => {}
        }
    }
    // THRUST blocks the validator rejects
    if g.below(30) == 0 {
        out.push_str("  Appearance = THRUST\n");
        out.push_str(&format!("  Lift = {}\n", g.below(3)));
        if g.below(2) == 0 {
            out.push_str("  ZAxisBehavior = SEA_LEVEL\n");
        }
    }
    if g.below(20) == 0 {
        out.push_str("  Appearance = WINGS\n");
        out.push_str("  MinSpeed = 0\n  MinTurnSpeed = -1\n");
    }
    let _ = lines;
    if g.below(120) != 0 {
        out.push_str("End\n");
    }
}

fn gen_ini(g: &mut Gen) -> String {
    let mut s = String::new();
    for _ in 0..1 + g.below(4) {
        match g.below(60) {
            0 => s.push_str("AIData\nEnd\n"), // a block of a stubbed handler: ERROR_BUG
            1 => s.push_str("Nonsense Block\nEnd\n"),
            _ => gen_block(g, &mut s),
        }
    }
    s
}

#[test]
fn locomotor_ini_matches_upstream() {
    let _lock = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x10C0_0001);
    let table = blocks();
    let mut outcomes = WordCheck::new("Locomotor INI outcome");
    let mut dumps = WordCheck::new("Locomotor INI store");
    let mut resets = WordCheck::new("Locomotor store reset");
    let mut statuses: BTreeMap<i32, u64> = BTreeMap::new();
    let (mut templates, mut chains, mut overrides_made, mut skipped_resets) = (0u64, 0u64, 0u64, 0u64);
    for case in 0..4000 {
        unsafe { cpp::ref_loco_reset() };
        let mut ctx = IniCtx::new();
        // up to three files: plain loads, then override loads
        for round in 0..1 + g.below(3) {
            let load_type = if (round == 0 && g.below(3) != 0) || g.below(4) == 0 { IniLoadType::Overwrite } else { IniLoadType::CreateOverrides };
            let text = gen_ini(&mut g);
            let r = Ini::load(b"l.ini", text.as_bytes(), load_type, None, &table, &mut ctx);
            let rust = rust_outcome(r);
            let cpp_out = cpp_load("l.ini", text.as_bytes(), load_type);
            *statuses.entry(rust.status).or_default() += 1;
            if std::env::var("LOCO_DEBUG").is_ok() && case < 12 { println!("case {case} status {} {}\n{text}", rust.status, String::from_utf8_lossy(&rust.detail)); }
            let ctxs = || format!("case {case} round {round} ({load_type:?}) text:\n{text}");
            outcomes.words(&[rust.status as u32], &[cpp_out.status as u32], |_| false, ctxs);
            outcomes.words(&rust.detail.iter().map(|&b| u32::from(b)).collect::<Vec<_>>(), &cpp_out.detail.iter().map(|&b| u32::from(b)).collect::<Vec<_>>(), |_| false, ctxs);
            let (rd, cd) = (rust_store_dump(&ctx.store), cpp_store_dump());
            dumps.words(&rd, &cd, any_float, ctxs);
        }
        templates += ctx.store.len() as u64;
        for (_, head) in ctx.store.chains() {
            let mut n = 0;
            let mut node = Some(head);
            while let Some(x) = node {
                n += 1;
                node = x.get_next_override();
            }
            chains += 1;
            overrides_made += n - 1;
        }
        // LocomotorStore::reset. Upstream is undefined when a template was itself made by an
        // override load: reset() erases it from the map and the loop goes on with the erased
        // iterator (a read of freed memory, then a second deleteOverrides on a deleted object).
        // The Rust removes it; that is covered by a unit test, not compared here.
        if ctx.store.chains().all(|(_, head)| !head.is_override()) {
            ctx.store.reset();
            unsafe { cpp::ref_loco_store_reset() };
            resets.words(&rust_store_dump(&ctx.store), &cpp_store_dump(), any_float, || format!("case {case} after reset"));
        } else {
            skipped_resets += 1;
        }
    }
    println!("COVERAGE INI: outcomes by status {statuses:?}; {templates} templates left in stores, {chains} chains, {overrides_made} override nodes; {skipped_resets} resets skipped (undefined upstream)");
    assert!(statuses.get(&0).copied().unwrap_or(0) > 1000, "enough successful loads");
    assert!(statuses.keys().count() >= 3, "several kinds of failure");
    assert!(overrides_made > 100, "override chains were exercised");
    outcomes.finish();
    dumps.finish();
    resets.finish();
}

// ---------------------------------------------------------------------------
// Movement programs
// ---------------------------------------------------------------------------

/// What the generator knows about a template it wrote.
#[derive(Clone, Debug)]
struct Meta {
    appearance: usize,
    z: usize,
    air: bool,
    max_speed: f32,
}

/// A plausible template of the given appearance (INI text).
fn gen_template(g: &mut Gen, name: &str, appearance: usize) -> (String, Meta) {
    let mut s = format!("Locomotor {name}\n");
    let mut put = |k: &str, v: String| s.push_str(&format!("  {k} = {v}\n"));
    let flyer = matches!(appearance, 4 | 5) || (appearance == 7 && g.below(2) == 0);
    let speed = g.uniform(20.0, 160.0);
    put("Appearance", APPEARANCES[appearance].to_string());
    put("Surfaces", if flyer { "AIR".to_string() } else if appearance == 3 { "GROUND WATER".to_string() } else { "GROUND CLIFF".to_string() });
    put("Speed", format!("{speed:.3}"));
    if g.below(2) == 0 {
        put("SpeedDamaged", format!("{:.3}", speed * g.uniform(0.2, 1.0)));
    }
    put("TurnRate", format!("{:.3}", g.uniform(30.0, 400.0)));
    if g.below(3) == 0 {
        put("TurnRateDamaged", format!("{:.3}", g.uniform(10.0, 200.0)));
    }
    put("Acceleration", format!("{:.3}", g.uniform(20.0, 400.0)));
    if g.below(3) == 0 {
        put("AccelerationDamaged", format!("{:.3}", g.uniform(10.0, 200.0)));
    }
    put("Braking", format!("{:.3}", g.uniform(20.0, 400.0)));
    // LEGS and CLIMBER must not have a minimum speed: moveTowardsPosition -> Legs -> moveTowardsAngle ->
    // moveTowardsPosition would not end (a stack overflow upstream)
    let min_speed = match appearance {
        0 | 6 => 0.0,
        4 => g.uniform(5.0, 30.0),
        5 => g.uniform(20.0, 60.0),
        _ => {
            if g.below(4) == 0 {
                g.uniform(0.0, 20.0)
            } else {
                0.0
            }
        }
    };
    put("MinSpeed", format!("{min_speed:.3}"));
    if g.below(2) == 0 || matches!(appearance, 1 | 8 | 5) {
        put("MinTurnSpeed", format!("{:.3}", g.uniform(0.0, 80.0)));
    }
    let z = if appearance == 4 {
        0 // THRUST: no Z behaviour, no lift (the validator rejects them)
    } else if flyer {
        [2usize, 3, 7, 2, 3, 4, 5, 6][g.below(8) as usize]
    } else if g.below(3) == 0 {
        g.below(8) as usize
    } else {
        0
    };
    put("ZAxisBehavior", Z_NAMES[z].to_string());
    let preferred_height = if z != 0 || appearance == 4 { g.uniform(0.0, 60.0) } else { 0.0 };
    if preferred_height != 0.0 || g.below(3) == 0 {
        put("PreferredHeight", format!("{preferred_height:.3}"));
    }
    if appearance != 4 && (z == 2 || z == 3 || z == 7 || flyer) {
        put("Lift", format!("{:.3}", g.uniform(90.0, 500.0)));
        if g.below(3) == 0 {
            put("LiftDamaged", format!("{:.3}", g.uniform(0.0, 300.0)));
        }
    }
    if g.below(2) == 0 {
        put("PreferredHeightDamping", format!("{:.3}", g.uniform(0.1, 1.0)));
    }
    if g.below(4) == 0 {
        put("SpeedLimitZ", format!("{:.3}", g.uniform(10.0, 200.0)));
    }
    if appearance == 4 {
        put("MaxThrustAngle", format!("{:.3}", g.uniform(0.0, 90.0)));
    }
    if appearance == 5 && g.below(2) == 0 {
        put("CirclingRadius", format!("{:.3}", g.uniform(-80.0, 80.0)));
    }
    if g.below(3) == 0 {
        put("TurnPivotOffset", format!("{:.3}", g.uniform(-1.0, 1.0)));
    }
    if g.below(4) == 0 {
        put("CanMoveBackwards", "Yes".to_string());
    }
    if g.below(3) == 0 {
        put("DownhillOnly", "Yes".to_string());
    }
    if g.below(3) == 0 {
        put("AllowAirborneMotiveForce", "Yes".to_string());
    }
    if g.below(3) == 0 {
        put("WanderWidthFactor", format!("{:.3}", g.uniform(0.0, 2.0)));
        put("WanderLengthFactor", format!("{:.3}", g.uniform(0.3, 3.0)));
    }
    if g.below(3) == 0 {
        put("SlideIntoPlaceTime", format!("{:.1}", g.uniform(0.0, 2000.0)));
    }
    if g.below(3) == 0 {
        put("Extra2DFriction", format!("{:.3}", g.uniform(0.0, 3.0)));
    }
    if g.below(3) == 0 {
        put("CloseEnoughDist", format!("{:.3}", g.uniform(0.0, 12.0)));
    }
    if g.below(6) == 0 {
        put("CloseEnoughDist3D", "Yes".to_string());
    }
    if g.below(5) == 0 {
        put("Apply2DFrictionWhenAirborne", "Yes".to_string());
    }
    if g.below(5) == 0 {
        put("StickToGround", "Yes".to_string());
    }
    s.push_str("End\n");
    let f = speed * (1.0 / 30.0);
    (s, Meta { appearance, z, air: flyer, max_speed: f })
}

fn finite3(v: &[f32; 3]) -> bool {
    v.iter().all(|x| x.is_finite() && x.abs() < 1.0e6)
}

/// `Locomotor`'s data members in `xfer`'s order, from the bytes the Rust xfer writes.
fn rust_state_words(loco: &mut Locomotor) -> Vec<u32> {
    let mut save = XferSave::new();
    save.open(b"loco").unwrap();
    loco.xfer_state(&mut save).unwrap();
    save.close().unwrap();
    let b = save.bytes();
    assert_eq!(b.len(), 1 + 64, "version byte and 16 words");
    b[1..].chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect()
}

fn cpp_state_words(handle: c_int) -> Vec<u32> {
    let mut w = vec![0u32; 16];
    unsafe { cpp::ref_loco_state(handle, w.as_mut_ptr()) };
    w
}

fn state_is_float(i: usize) -> bool {
    !(i == 0 || i == 11)
}

fn rust_obj_words(h: &Host) -> Vec<u32> {
    let mut w: Vec<u32> = h.thing.transform_matrix().to_array().iter().map(|f| f.to_bits()).collect();
    let p = h.thing.position();
    w.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), h.thing.orientation().to_bits()]);
    w.push(h.status as u32);
    w.push((h.status >> 32) as u32);
    w
}

fn cpp_obj_words() -> Vec<u32> {
    let mut o = [0u32; 23];
    unsafe { cpp::ref_loco_obj_state(o.as_mut_ptr()) };
    let mut w: Vec<u32> = o[0..16].to_vec();
    w.push(o[21]);
    w.push(o[22]);
    w
}

fn obj_is_float(i: usize) -> bool {
    i < 16
}

fn cpp_call(handle: c_int, op: c_int, env: &Env, fa: &[f32; 8], ia: &[c_int; 4]) -> (Vec<u32>, [u32; 16], [c_int; 4]) {
    let mut rf = [0u32; 16];
    let mut ri = [0 as c_int; 4];
    let mut log = vec![0u32; 1 << 12];
    let mut n: c_int = 0;
    unsafe { cpp::ref_loco_call(handle, op, env, fa.as_ptr(), ia.as_ptr(), rf.as_mut_ptr(), ri.as_mut_ptr(), log.as_mut_ptr(), log.len() as c_int, &mut n) };
    if n as usize > log.len() {
        log = vec![0u32; n as usize];
        unsafe { cpp::ref_loco_call(handle, op, env, fa.as_ptr(), ia.as_ptr(), rf.as_mut_ptr(), ri.as_mut_ptr(), log.as_mut_ptr(), log.len() as c_int, &mut n) };
        panic!("a call must not be repeated: the log outgrew its buffer ({n} words)");
    }
    log.truncate(n as usize);
    (log, rf, ri)
}

fn cpp_obj(op: c_int, fa: &[f32; 12], ia: &[c_int; 2]) {
    unsafe { cpp::ref_loco_obj(op, fa.as_ptr(), ia.as_ptr()) };
}

/// Tallies of what the programs reached.
#[derive(Default)]
struct Coverage {
    counts: BTreeMap<String, u64>,
}

impl Coverage {
    fn add(&mut self, k: impl Into<String>) {
        *self.counts.entry(k.into()).or_default() += 1;
    }
}

struct Program {
    seed: u64,
    appearance: usize,
}

/// The canonical physics the programs integrate with: the forces and scrubs of the Rust log.
struct Sim {
    vel: [f32; 3],
    mass: f32,
}

impl Sim {
    /// One frame of physics after the locomotor ran: accelerate by the motive forces, scrub,
    /// gravity, friction, move, land.
    fn integrate(&mut self, host: &mut Host, log: &[u32], gravity: f32, friction: f32) -> Coord3D {
        let mut acc = [0.0f32; 3];
        let mut i = 0;
        while i < log.len() {
            let (code, n) = (log[i], log[i + 1] as usize);
            let w = &log[i + 2..i + 2 + n];
            match code {
                L_APPLY_FORCE => {
                    for (a, &word) in acc.iter_mut().zip(w.iter()) {
                        *a += f32::from_bits(word) / self.mass;
                    }
                }
                L_SCRUB => {
                    let v = f32::from_bits(w[0]);
                    let s = (self.vel[0] * self.vel[0] + self.vel[1] * self.vel[1]).sqrt();
                    if v == 0.0 {
                        self.vel[0] = 0.0;
                        self.vel[1] = 0.0;
                    } else if s > v && s > 0.0 {
                        self.vel[0] *= v / s;
                        self.vel[1] *= v / s;
                    }
                }
                _ => {}
            }
            i += 2 + n;
        }
        for (v, a) in self.vel.iter_mut().zip(acc.iter()) {
            *v += a;
        }
        self.vel[2] += gravity;
        self.vel[0] *= friction;
        self.vel[1] *= friction;
        let p = host.position();
        let mut np = Coord3D { x: p.x + self.vel[0], y: p.y + self.vel[1], z: p.z + self.vel[2] };
        let ground = host.ground(np.x, np.y);
        if np.z < ground {
            np.z = ground;
            if self.vel[2] < 0.0 {
                self.vel[2] = 0.0;
            }
        }
        np
    }
}

/// The scene constants of a program.
fn gen_scene(g: &mut Gen, meta: &Meta) -> Env {
    let mut e = Env {
        gravity: if g.below(4) == 0 { -1.0 } else { -60.0 * (1.0 / 30.0) * (1.0 / 30.0) },
        penalty: 1 + g.below(2) as i32,
        has_physics: 1,
        mass: g.uniform(1.0, 60.0),
        layer: LAYER_GROUND,
        kind_mask: g.below(4) as i32,
        ..Env::default()
    };
    if g.below(4) != 0 {
        e.kind_mask &= !1; // projectiles are rarer
    }
    e.deck_height = g.uniform(0.0, 8.0);
    e.major_radius = g.uniform(1.0, 20.0);
    e.bounding_circle_radius = g.uniform(1.0, 25.0);
    e.ga = g.uniform(-0.25, 0.25);
    e.gb = g.uniform(-0.25, 0.25);
    e.gc = g.uniform(-10.0, 10.0);
    e.wx = if g.below(3) == 0 { g.uniform(-150.0, 150.0) } else { -1.0e9 };
    e.wz = g.uniform(-5.0, 15.0);
    e.layer_offset = g.uniform(-4.0, 12.0);
    e.struct_offset = g.uniform(0.0, 20.0);
    e.highest_layer = if g.below(2) == 0 { LAYER_GROUND } else { LAYER_WALL };
    if !meta.air && g.below(2) == 0 {
        e.inv_enabled = 1;
        let (cx, cy) = (g.uniform(-150.0, 150.0), g.uniform(-150.0, 150.0));
        let (w, h) = (g.uniform(5.0, 60.0), g.uniform(5.0, 60.0));
        e.inv_box = [cx - w, cx + w, cy - h, cy + h];
    }
    e
}

/// One program: a locomotor of a random template of `appearance`, driven for `frames` frames in
/// a random scene. Returns false on a mismatch (after recording it).
#[allow(clippy::too_many_arguments)]
fn run_program(p: &Program, frames: usize, checks: &mut Checks, cov: &mut Coverage) -> bool {
    let mut g = Gen::new(p.seed);
    let name = "T";
    let (ini, meta) = gen_template(&mut g, name, p.appearance);

    // the template through both INI readers
    unsafe { cpp::ref_loco_reset() };
    let mut ictx = IniCtx::new();
    let table = blocks();
    Ini::load(b"t.ini", ini.as_bytes(), IniLoadType::Overwrite, None, &table, &mut ictx).unwrap_or_else(|e| panic!("generated template rejected: {e:?}\n{ini}"));
    let o = cpp_load("t.ini", ini.as_bytes(), IniLoadType::Overwrite);
    assert_eq!(o.status, 0, "generated template rejected by C++:\n{ini}");
    checks.template.words(&rust_store_dump(&ictx.store), &cpp_store_dump(), any_float, || format!("seed {:#x}\n{ini}", p.seed));
    let key = ictx.names.name_to_key(name.as_bytes());
    assert_eq!(unsafe { cpp::ref_loco_name_key(cstr(name).as_ptr()) }, key.0);

    // the locomotor: the constructor draws three logic random values
    let seed = g.u32();
    let start_frame = g.below(3000);
    unsafe { cpp::ref_init_game_logic_random(seed) };
    let mut rng = RandomValues::default();
    rng.init_game_logic_random(seed);
    let mut loco = ictx.store.new_locomotor(key, &mut rng, start_frame).unwrap();
    let handle = unsafe { cpp::ref_loco_new(cstr(name).as_ptr(), start_frame) };
    assert!(handle >= 0);
    let ok = checks.state.words(&rust_state_words(&mut loco), &cpp_state_words(handle), state_is_float, || format!("seed {:#x} after construction", p.seed));
    if !ok {
        return false;
    }

    // the scene and the object
    let mut host = Host::new();
    host.env = gen_scene(&mut g, &meta);
    let base = host.env;
    let (px, py) = (g.uniform(-100.0, 100.0), g.uniform(-100.0, 100.0));
    let ground0 = host.ground(px, py);
    let z0 = if meta.air { ground0 + g.uniform(0.0, 80.0) } else { ground0 };
    let mut m = Matrix3D::identity();
    m.translate(px, py, z0);
    m.rotate_z(g.uniform(-3.1, 3.1));
    let _ = host.thing.set_transform_matrix(&m);
    let mut fa12 = [0.0f32; 12];
    fa12.copy_from_slice(&m.to_array());
    cpp_obj(0, &fa12, &[0, 0]);
    host.log.clear();

    let mut sim = Sim { vel: [0.0; 3], mass: base.mass };
    let mut goal = Coord3D { x: px + g.uniform(-300.0, 300.0), y: py + g.uniform(-300.0, 300.0), z: z0 + g.uniform(-5.0, 30.0) };
    let mut frame = start_frame;
    let mut body_state = 0i32;
    let gravity = base.gravity;
    let mut stuck = 0;
    for step in 0..frames {
        frame += 1;
        // the world changes now and then
        if g.below(40) == 0 {
            goal = Coord3D { x: px + g.uniform(-300.0, 300.0), y: py + g.uniform(-300.0, 300.0), z: z0 + g.uniform(-5.0, 30.0) };
            if g.below(3) == 0 {
                let p = host.position();
                goal = Coord3D { x: p.x + g.uniform(-8.0, 8.0), y: p.y + g.uniform(-8.0, 8.0), z: p.z + g.uniform(-2.0, 2.0) };
            }
        }
        if g.below(30) == 0 {
            body_state = g.below(4) as i32;
        }
        let pos = host.position();
        let dir = Coord3D { x: trig::cos(host.thing.orientation()), y: trig::sin(host.thing.orientation()), z: 0.0 };
        let xv = host.thing.transform_matrix().get_x_vector();
        let v = sim.vel;
        let mut env = base;
        env.frame = frame;
        env.body_state = body_state;
        env.stunned = i32::from(g.below(40) == 0);
        env.has_physics = i32::from(g.below(300) != 0);
        env.vel = v;
        env.vel_mag = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        env.fwd2d = v[0] * dir.x + v[1] * dir.y;
        env.fwd3d = v[0] * xv.x + v[1] * xv.y + v[2] * xv.z;
        env.motive = i32::from(env.vel_mag > 0.01);
        env.layer = if g.below(25) == 0 { LAYER_WALL } else { LAYER_GROUND };
        env.disabled_held = i32::from(g.below(20) == 0);
        if g.below(15) == 0 {
            env.highest_layer = if g.below(2) == 0 { LAYER_GROUND } else { LAYER_WALL };
        }
        let _ = pos;

        // the operation
        let r = g.below(100);
        let mut fa = [0.0f32; 8];
        let mut ia = [0 as c_int; 4];
        let op: c_int = match r {
            0..=54 => {
                let p = host.position();
                let real = ((goal.x - p.x) * (goal.x - p.x) + (goal.y - p.y) * (goal.y - p.y)).sqrt();
                let on_path = match g.below(5) {
                    0 => real * g.uniform(0.2, 1.0),
                    1 => 99999.0,
                    2 => real,
                    _ => real * g.uniform(1.0, 2.5) + g.uniform(0.0, 20.0),
                };
                let desired = match g.below(5) {
                    0 => 99999.0,
                    1 => 0.0,
                    _ => meta.max_speed * g.uniform(0.1, 1.6),
                };
                fa[0] = goal.x;
                fa[1] = goal.y;
                fa[2] = goal.z;
                fa[3] = on_path;
                fa[4] = desired;
                ia[0] = i32::from(g.below(8) == 0);
                0
            }
            55..=62 => {
                fa[0] = g.uniform(-std::f32::consts::PI, std::f32::consts::PI);
                1
            }
            63..=80 => 2,
            81..=84 => 3,
            85..=87 => 4,
            88..=90 => {
                ia[0] = g.below(4) as i32;
                5
            }
            _ => {
                let which = 10 + g.below(13) as c_int;
                fa[0] = match which {
                    10..=14 => {
                        if g.below(2) == 0 {
                            g.uniform(0.0, 3.0)
                        } else {
                            99999.0
                        }
                    }
                    16 => g.uniform(0.0, 10.0),
                    18 => g.uniform(0.0, 80.0),
                    _ => 0.0,
                };
                ia[0] = g.below(2) as i32;
                which
            }
        };
        // statuses the engine sets from outside now and then
        if g.below(25) == 0 {
            let (bit, set) = (if g.below(2) == 0 { OBJECT_STATUS_BRAKING } else { OBJECT_STATUS_DECK_HEIGHT_OFFSET }, g.below(2) as i32);
            host.set_status(bit, set != 0);
            host.log.clear();
            cpp_obj(3, &[0.0; 12], &[bit, set]);
        }

        host.env = env;
        host.log.clear();
        let undefined_before = loco.undefined_inputs();
        let mut blocked_out = 0;
        let mut rf_r = [0u32; 16];
        let mut ri0 = 0i32;
        match op {
            0 => {
                let mut blocked = ia[0] != 0;
                loco.loco_update_move_towards_position(&mut host, &Coord3D { x: fa[0], y: fa[1], z: fa[2] }, fa[3], fa[4], &mut blocked);
                blocked_out = i32::from(blocked);
                ri0 = blocked_out;
            }
            1 => loco.loco_update_move_towards_angle(&mut host, fa[0]),
            2 => ri0 = i32::from(loco.loco_update_maintain_current_position(&mut host)),
            3 => loco.set_physics_options(&mut host),
            4 => loco.start_move(frame),
            5 => {
                let c = ia[0];
                rf_r[0] = loco.get_max_speed_for_condition(&host, c).to_bits();
                rf_r[1] = loco.get_max_turn_rate(&host, c).to_bits();
                rf_r[2] = loco.get_max_acceleration(&host, c).to_bits();
                rf_r[3] = loco.get_max_lift(&host, c).to_bits();
                rf_r[4] = loco.get_braking().to_bits();
                let (rad, t) = loco.calc_min_turn_radius(&host, c);
                rf_r[5] = rad.to_bits();
                rf_r[6] = t.to_bits();
            }
            10 => loco.set_max_lift(fa[0]),
            11 => loco.set_max_speed(fa[0]),
            12 => loco.set_max_acceleration(fa[0]),
            13 => loco.set_max_braking(fa[0]),
            14 => loco.set_max_turn_rate(fa[0]),
            15 => loco.set_allow_invalid_position(ia[0] != 0),
            16 => loco.set_close_enough_dist(fa[0]),
            17 => loco.set_close_enough_dist_3d(ia[0] != 0),
            18 => loco.set_preferred_height(fa[0]),
            19 => loco.set_use_precise_z_pos(ia[0] != 0),
            20 => loco.set_no_slow_down_as_approaching_dest(ia[0] != 0),
            21 => loco.set_ultra_accurate(ia[0] != 0),
            22 => loco.restore_preferred_height_from_template(),
            _ => unreachable!(),
        }
        let rust_log = std::mem::take(&mut host.log);
        if loco.undefined_inputs() != undefined_before {
            // upstream read an uninitialised variable in this call (see Locomotor's module docs):
            // its result is whatever the stack held, so the program ends here, uncompared
            cov.add("program ended: undefined upstream (uninitialised relAngle)");
            return true;
        }
        let (cpp_log, rf_c, ri_c) = cpp_call(handle, op, &env, &fa, &ia);
        let ctxs = || format!("seed {:#x} (appearance {}, z {}) step {step} op {op} fa {fa:?} ia {ia:?}\n{ini}", p.seed, meta.appearance, meta.z);

        if !compare_logs(&mut checks.log, &rust_log, &cpp_log, ctxs) {
            return false;
        }
        let ret_ok = if op == 5 {
            checks.results.words(&rf_r[..7], &rf_c[..7], |_| true, ctxs)
        } else if op == 0 || op == 2 {
            checks.results.words(&[ri0 as u32], &[ri_c[0] as u32], |_| false, ctxs)
        } else {
            true
        };
        let _ = blocked_out;
        let obj_ok = checks.object.words(&rust_obj_words(&host), &cpp_obj_words(), obj_is_float, ctxs);
        let st_ok = checks.state.words(&rust_state_words(&mut loco), &cpp_state_words(handle), state_is_float, ctxs);
        if !(ret_ok && obj_ok && st_ok) {
            return false;
        }

        // coverage
        cov.add(format!("op {op}"));
        if op == 0 || op == 1 || op == 2 {
            cov.add(format!("appearance {} op {op}", meta.appearance));
            cov.add(format!("z {} op {op}", meta.z));
        }
        let mut i = 0;
        while i < rust_log.len() {
            cov.add(format!("log code {}", rust_log[i]));
            i += 2 + rust_log[i + 1] as usize;
        }
        let fl = loco.flags();
        for b in 0..12 {
            if fl & (1 << b) != 0 {
                cov.add(format!("flag {b}"));
            }
        }
        if host.status & (1 << OBJECT_STATUS_BRAKING) != 0 {
            cov.add("status braking");
        }
        if ri0 != 0 && op == 0 {
            cov.add("still blocked");
        }

        // sampled xfer: the bytes of the real XferSave against the Rust xfer, then a load
        if step % 17 == 0 {
            let path = std::env::temp_dir().join(format!("loco_xfer_{}_{}.bin", std::process::id(), p.seed));
            let cpath = cstr(path.to_str().unwrap());
            let mut out = vec![0u8; 256];
            let mut len = 0;
            let st = unsafe { cpp::ref_loco_xfer_save(handle, cpath.as_ptr(), out.as_mut_ptr(), out.len() as c_int, &mut len) };
            assert_eq!(st, 0);
            let mut save = XferSave::new();
            save.open(path.to_str().unwrap().as_bytes()).unwrap();
            loco.xfer_state(&mut save).unwrap();
            save.close().unwrap();
            let rb = save.bytes().to_vec();
            let cb = out[..len as usize].to_vec();
            // the identifier is not in the file: the bytes are the data
            checks.xfer_bytes.words(&rb.iter().map(|&b| u32::from(b)).collect::<Vec<_>>(), &cb.iter().map(|&b| u32::from(b)).collect::<Vec<_>>(), |_| false, ctxs);
            // load into a fresh locomotor on both sides
            let mut rng2 = RandomValues::default();
            rng2.init_game_logic_random(seed ^ 1);
            let mut fresh = ictx.store.new_locomotor(key, &mut rng2, 7).unwrap();
            let mut load = XferLoad::new(cb.clone());
            load.open(b"x").unwrap();
            rts_lockstep_engine::common::xfer::Snapshot::xfer(&mut fresh, &mut load).unwrap();
            load.close().unwrap();
            unsafe { cpp::ref_init_game_logic_random(seed ^ 1) };
            let mut nh: c_int = -1;
            let st = unsafe { cpp::ref_loco_xfer_load(cstr(name).as_ptr(), 7, cpath.as_ptr(), &mut nh) };
            assert_eq!(st, 0);
            checks.xfer_load.words(&rust_state_words(&mut fresh), &cpp_state_words(nh), state_is_float, ctxs);
            // the loaded state is the saved one
            assert_eq!(rust_state_words(&mut fresh), rust_state_words(&mut loco));
            let _ = std::fs::remove_file(&path);
            // the C++ RNG goes back to where this program left it (nothing further draws)
        }

        // the getters that need no scene, sampled
        if step % 29 == 0 {
            let mut gc = [0u32; 48];
            unsafe { cpp::ref_loco_getters(handle, gc.as_mut_ptr()) };
            let gr = rust_getters(&loco);
            checks.getters.words(&gr, &gc[..gr.len()], |_| true, ctxs);
        }

        // physics: integrate on the Rust side, give both sides the new position
        if env.has_physics != 0 {
            let friction = if (host.position().z - host.ground(host.position().x, host.position().y)) < 0.5 { 0.96 } else { 0.995 };
            sim.integrate_and_move(&mut host, &rust_log, gravity, friction, &mut stuck);
            let np = host.position();
            if !finite3(&sim.vel) || !finite3(&[np.x, np.y, np.z]) || !host.thing.orientation().is_finite() {
                cov.add("program ended: not finite");
                return true;
            }
            let mut fa12 = [0.0f32; 12];
            fa12[0] = np.x;
            fa12[1] = np.y;
            fa12[2] = np.z;
            cpp_obj(1, &fa12, &[0, 0]);
            host.log.clear();
        }
    }
    true
}

impl Sim {
    fn integrate_and_move(&mut self, host: &mut Host, log: &[u32], gravity: f32, friction: f32, _stuck: &mut i32) {
        let np = self.integrate(host, log, gravity, friction);
        host.set_position(&np);
    }
}

/// The inline getters of `Locomotor.h`, in the order of `ref_loco_getters`.
fn rust_getters(l: &Locomotor) -> Vec<u32> {
    let f = |x: f32| x.to_bits();
    let b = |x: bool| u32::from(x);
    vec![
        l.get_appearance() as i32 as u32,
        l.get_move_priority() as i32 as u32,
        l.get_legal_surfaces() as u32,
        f(l.get_min_speed()),
        f(l.get_accel_pitch_limit()),
        f(l.get_decel_pitch_limit()),
        f(l.get_bounce_kick()),
        f(l.get_pitch_stiffness()),
        f(l.get_roll_stiffness()),
        f(l.get_pitch_damping()),
        f(l.get_roll_damping()),
        f(l.get_pitch_by_z_vel_coef()),
        f(l.get_thrust_roll()),
        f(l.get_wobble_rate()),
        f(l.get_max_wobble()),
        f(l.get_min_wobble()),
        f(l.get_forward_vel_coef()),
        f(l.get_lateral_vel_coef()),
        f(l.get_forward_accel_coef()),
        f(l.get_lateral_accel_coef()),
        f(l.get_uniform_axial_damping()),
        f(l.get_turn_pivot_offset()),
        b(l.get_apply_2d_friction_when_airborne()),
        b(l.get_is_downhill_only()),
        b(l.get_allow_motive_force_while_airborne()),
        l.get_airborne_targeting_height() as u32,
        b(l.get_locomotor_works_when_dead()),
        b(l.get_stick_to_ground()),
        f(l.get_close_enough_dist()),
        b(l.is_close_enough_dist_3d()),
        b(l.has_suspension()),
        b(l.can_move_backwards()),
        f(l.get_max_wheel_extension()),
        f(l.get_max_wheel_compression()),
        f(l.get_wheel_turn_angle()),
        f(l.get_rudder_correction_degree()),
        f(l.get_rudder_correction_rate()),
        f(l.get_elevator_correction_degree()),
        f(l.get_elevator_correction_rate()),
        f(l.get_wander_width_factor()),
        f(l.get_wander_about_point_radius()),
        f(l.get_preferred_height()),
        f(l.get_preferred_height_damping()),
        b(l.is_ultra_accurate()),
        b(l.is_moving_backwards()),
        b(l.is_invalid_position_allowed()),
        l.donut_timer(),
    ]
}

/// All comparisons of the movement programs.
struct Checks {
    template: WordCheck,
    log: WordCheck,
    results: WordCheck,
    object: WordCheck,
    state: WordCheck,
    xfer_bytes: WordCheck,
    xfer_load: WordCheck,
    getters: WordCheck,
}

impl Checks {
    fn new() -> Self {
        Checks {
            template: WordCheck::new("Locomotor program template"),
            log: WordCheck::new("Locomotor call logs (every cut-point call)"),
            results: WordCheck::new("Locomotor results (blocked, getters)"),
            object: WordCheck::new("Object after each call"),
            state: WordCheck::new("Locomotor state after each call"),
            xfer_bytes: WordCheck::new("Locomotor::xfer bytes (XferSave)"),
            xfer_load: WordCheck::new("Locomotor::xfer load (XferLoad)"),
            getters: WordCheck::new("Locomotor inline getters"),
        }
    }

    fn finish(self) {
        self.template.finish();
        self.log.finish();
        self.results.finish();
        self.object.finish();
        self.state.finish();
        self.xfer_bytes.finish();
        self.xfer_load.finish();
        self.getters.finish();
    }
}

fn run_all_programs(programs_per_appearance: usize, frames: usize, seed: u64) -> (Checks, Coverage, usize) {
    let mut checks = Checks::new();
    let mut cov = Coverage::default();
    let mut total_frames = 0usize;
    let mut failed = 0;
    for n in 0..programs_per_appearance {
        for appearance in 0..9 {
            let p = Program { seed: seed ^ ((n as u64) << 8) ^ (appearance as u64) << 40 | 1, appearance };
            if !run_program(&p, frames, &mut checks, &mut cov) {
                failed += 1;
            } else {
                total_frames += frames;
            }
        }
    }
    let _ = failed;
    (checks, cov, total_frames)
}

#[test]
fn locomotor_movement_matches_upstream() {
    let _lock = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (checks, cov, frames) = run_all_programs(70, 220, 0x10C0_2000);
    println!("COVERAGE movement: {frames} frames of {} programs", 70 * 9);
    for (k, v) in &cov.counts {
        println!("COVERAGE   {k:<28} {v}");
    }
    // the programs must have reached the interesting corners
    for need in [
        "log code 5", "log code 6", "log code 7", "log code 2", "log code 1", "log code 20", "log code 21", "log code 22", "log code 23", "log code 24", "log code 25",
        "flag 0", "flag 7", "flag 8", "flag 9", "flag 5", "status braking", "still blocked",
    ] {
        assert!(cov.counts.get(need).copied().unwrap_or(0) > 0, "never reached: {need}");
    }
    checks.finish();
}
