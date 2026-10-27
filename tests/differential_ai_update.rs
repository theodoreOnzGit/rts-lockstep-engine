// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of layer 10 batches A2, A3 and A4, `logic::ai::ai_update` (`AIUpdateInterface`'s members) and
//! `logic::ai::ai_states` (its state machine, first states and update), against upstream `AIUpdate.cpp` and `AIStates.cpp` (commit
//! 0a05454d) in `differential/wrap_ai.cpp`: their members cut verbatim,
//! with `Locomotor.cpp` compiled UNCHANGED and the paths of `AIPathfind.cpp` (batch P1's cut), so that the AI drives the REAL
//! locomotors along REAL paths on both sides.
//!
//! # What is real on the C++ side, and what is a stand-in
//!
//! Real: the members under test (the constructor, the locomotor sets and their choice, the locomotor's goal and `doLocomotor`,
//! the movement queries, the path's bookkeeping, the sleep, the waypoint queue, the setters), `AIUpdate.h`'s data members,
//! `Locomotor.cpp` (the store, the INI block, every locomotor's movement), `Path` and `PathNode`, the pathfinder's
//! `chooseBestLocomotorForPosition` over a map of real cells, the INI reader, name keys, the logic random values, `Thing`'s
//! position and height code. Stand-ins (`differential/shim_ai/ai_world.h`): the rest of the object, its physics behaviour and
//! body, the terrain, the partition manager, the AI's update module (the wake frame), its group, and the pathfinder's members not
//! compiled here (`validMovementTerrain`, `validMovementPosition`, `updateLayer`).
//! They answer from a scene (`AiEnv`) and log every call that crosses the cut; the Rust side is `AiUpdate` driven through an
//! [`AiHost`] that answers from the same scene, on the translated `Thing`, `Locomotor`, `LocomotorSet` and `Path`, and writes the
//! same log.
//!
//! # Methodology
//!
//! Each case: two to four random `Locomotor` templates (plausible ones of every appearance, read by both INI readers), an AI
//! whose data names random locomotor sets of them (the normal set mostly, sometimes the upgraded one, sometimes none), a random
//! scene (terrain slope, water, map of cells with cliffs, water, rubble and obstacles, pinches, the hash's rates, damage state,
//! kinds, disabled states, a group or none), the object somewhere on or off the map; then a program of 60 operations: `doLocomotor`
//! (the most frequent), each locomotor goal, new paths (appended nodes, optimized or ground-optimized or not), locomotor set
//! choices and upgrades, the distance to the goal, the movement queries, the waypoint queue, the sleep, the final position, the
//! death, moving the object, and setting the state the machine and the collisions will set (final position and its flag,
//! blocked, the blocked and bump speeds, in-update, waiting for a path, the path's extra distance). Batch A3 adds half the
//! operations: the crate check, the victim (set, get, its position), the attitude, the state type, the ignored obstacle, the enter
//! target, the mood matrix and its action adjustments, waking to target, the mood check times, `canAutoAcquireWhileStealthed`,
//! `getNextMoodTarget` (by the AI or not, during idle or not), the morale bonus, `isIdle`/`isAttacking`/`isBusy`/
//! `isClearingMines`/`isMovingAwayFrom`/`isMoving`, `notifyCrate`, the object's status bits and the move-out-of-the-way ids; over a
//! scene that adds the other objects (which exist, which have an AI), the controlling player (none, human, computer) and its
//! upgrades, the team's common target, the ability to attack (the hash), stealth, a container, the last damage, `TheAI`'s data,
//! vision range and closest enemy, the current weapon's anti mask, three behavior modules' hordes, and the state machine's answers
//! (batch A4: the real machine's, see below). The frame advances and the
//! physics numbers change between operations. After every operation, compared: its result, its whole call log (every force,
//! scrub, status, transform change, terrain, partition and pathfinder query, wake, group, line test, with arguments), and a dump
//! of the AI (every data member the batch sets, the locomotor set with every locomotor's saved state, the path node by node with
//! its cache), the object and the wake frame. A mismatch ends the case.
//!
//! Batch A4 replaces the stand-in machine with the REAL one: `StateMachine.cpp` unchanged and `AIStateMachine` (AIStates.cpp's
//! machine members cut verbatim, its constructor edited so that each state class not yet translated is a placeholder of that id,
//! logged and answering from the hash; the idle, dead, wait and busy states verbatim) against `ai_states`. The machine is made at
//! the object's creation (`onObjectCreated`; one case in ten later, by an operation, the operations that need it held back until
//! then). A third of the operations are the machine's: `setState` (often the translated states, else any id, the undefined
//! ones included), `setTemporaryState`, `clear`, `resetToDefaultState`, the goal object and position, lock and unlock, the goal
//! path, a save (`xferSnapshot`, its bytes compared) and a load of the last save; and the AI's `update` (the machine, the path
//! queue, the turrets' updates, the death check, `doLocomotor`). The scene adds the pathfinder's goal, the disabled types the
//! update and the idle state ask, the repulsor, the vision range, the rates of the placeholder states and turret updates. The
//! dump adds the machine (its state, sleep, default, goal, lock, temporary state, goal path, the idle state's members).
//!
//! # Results
//!
//! See the `# Results` section of the test function's doc comment.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::{real_to_int, real_to_int_floor, Coord3D};
use rts_lockstep_engine::common::disabled_types::{
    DisabledType, DISABLED_EMP, DISABLED_HACKED, DISABLED_HELD, DISABLED_PARALYZED, DISABLED_SUBDUED, DISABLED_UNMANNED,
};
use rts_lockstep_engine::common::game_type::{ObjectId, PathfindLayer, LAYER_GROUND, LAYER_WALL};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, TYPE_TABLE};
use rts_lockstep_engine::common::game_common::{AbleToAttackType, PlayerType};
use rts_lockstep_engine::common::kind_of::{
    KindOfMaskType, KindOfType, KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT, KINDOF_DOZER, KINDOF_IGNORED_IN_GUI, KINDOF_IMMOBILE, KINDOF_INFANTRY,
    KINDOF_PRODUCED_AT_HELIPAD, KINDOF_PROJECTILE, KINDOF_CAN_BE_REPULSED,
};
use rts_lockstep_engine::logic::ai::ai_update_types::{
    CanAttackResultValue, CommandSourceType, ATTACKRESULT_NOT_POSSIBLE, ATTACKRESULT_POSSIBLE, ATTACKRESULT_POSSIBLE_AFTER_MOVING,
};
use rts_lockstep_engine::logic::damage::DamageType;
use rts_lockstep_engine::common::model_state::ModelConditionFlagType;
use rts_lockstep_engine::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use rts_lockstep_engine::common::object_status_types::ObjectStatusTypes;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::state_machine::{ObjectRef, StateContext, StateExitType, StateId, StateReturn};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::differential::AiEnv as Env;
use rts_lockstep_engine::logic::ai::ai_states::{self as states, AiState, AiStateHost, AiUpdateModule};
use rts_lockstep_engine::logic::ai::ai_update::{AiHost, AiUpdate, LocoGoalType};
use rts_lockstep_engine::logic::ai::ai_update_data::AiUpdateModuleData;
use rts_lockstep_engine::logic::ai::movement::valid_locomotor_surfaces_for_cell_type;
use rts_lockstep_engine::logic::ai::path::{Path, PathHost};
use rts_lockstep_engine::logic::locomotor::*;
use rts_lockstep_engine::logic::module::body_module::BodyDamageType;
use rts_lockstep_engine::logic::module::update_module::{update_sleep, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use rts_lockstep_engine::logic::terrain_logic::Underwater;
use rts_lockstep_engine::logic::thing::{Thing, ThingContext, ThingTemplateView};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use std::cell::RefCell;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::sync::Arc;
use support::{Check, Gen};

// ---------------------------------------------------------------------------
// Log codes: the numbers of `ai_world.h`
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
const L_SET_WAKE: u32 = 40;
const L_GROUP_SPEED: u32 = 41;
const L_UPDATE_LAYER: u32 = 42;
const L_VALID_POSITION: u32 = 43;
const L_DISTANCE_SQUARED: u32 = 44;
const L_LINE_PASSABLE: u32 = 45;
const L_GROUND_PASSABLE: u32 = 46;
const L_LAYER_FOR: u32 = 47;
const L_EFFECTIVELY_DEAD: u32 = 48;
const L_ADD_TARGETER: u32 = 60;
const L_ABLE_TO_ATTACK: u32 = 61;
const L_VISION_RANGE: u32 = 62;
const L_CLOSEST_ENEMY: u32 = 63;
const L_SET_BONUS: u32 = 64;
const L_CLEAR_BONUS: u32 = 65;
const L_PF_GOAL_POSITION: u32 = 70;
const L_PF_SNAP: u32 = 71;
const L_PF_UPDATE_GOAL: u32 = 72;
const L_PF_QUEUE: u32 = 73;
const L_TURRET_UPDATE: u32 = 74;
const L_AI_COMMAND: u32 = 75;
const L_REPULSOR: u32 = 76;
const L_CLEAR_SET_MODEL: u32 = 77;
const L_SCRIPT_NOTIFY: u32 = 78;
const L_STATE_ENTER: u32 = 79;
const L_STATE_UPDATE: u32 = 80;
const L_STATE_EXIT: u32 = 81;

/// The machine's xfer file of the C++ side (`ref_ai_a4_xfer_path`).
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/ai_a4.bin");

/// The map of `wrap_ai.cpp` (`MAP_W` x `MAP_H` cells from the origin).
const MAP_W: i32 = 24;
const MAP_H: i32 = 16;
/// `PathfindCell::CellType`.
const CELL_CLEAR: u8 = 0;
const CELL_WATER: u8 = 1;
const CELL_CLIFF: u8 = 2;
const CELL_RUBBLE: u8 = 3;
const CELL_OBSTACLE: u8 = 4;
const CELL_IMPASSABLE: u8 = 6;

fn cstr(s: &str) -> CString {
    CString::new(s).expect("no NUL")
}

/// Batch P1's hash (`p1Hash` of `wrap_ai.cpp`).
fn p1_hash(seed: u32, tag: u32, words: &[u32]) -> u32 {
    let mut x = seed ^ tag.wrapping_mul(0x85eb_ca6b);
    for &w in words {
        x ^= w;
        x = x.wrapping_mul(0x9E37_79B1);
        x ^= x >> 15;
    }
    x
}

fn pick(h: u32, rate: u32) -> bool {
    (h >> 8) % 100 < rate
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

/// `TheTerrainLogic` and `TheGlobalData` of the Thing: the scene's tables. The ground-height queries are recorded (the C++
/// logs those the Thing makes for `getHeightAboveTerrain`).
struct TCtx<'a> {
    env: &'a Env,
    queries: RefCell<Vec<(f32, f32)>>,
}

impl<'a> TCtx<'a> {
    fn new(env: &'a Env) -> Self {
        TCtx { env, queries: RefCell::new(Vec::new()) }
    }
}

impl ThingContext for TCtx<'_> {
    fn ground_height(&self, x: f32, y: f32) -> f32 {
        self.queries.borrow_mut().push((x, y));
        let e = &self.env.loco;
        e.ga * x + e.gb * y + e.gc
    }
    fn underwater_level(&self, x: f32, _y: f32) -> Option<f32> {
        if x < self.env.loco.wx {
            Some(self.env.loco.wz)
        } else {
            None
        }
    }
    fn align_on_terrain(&self, _o: f32, _p: &Coord3D, _s: bool) -> Matrix3D {
        panic!("no KINDOF_STICK_TO_TERRAIN_SLOPE in this scene")
    }
    fn gravity(&self) -> f32 {
        self.env.loco.gravity
    }
}

/// The object, its physics behaviour, its world and its subsystems, answering from the scene and logging every call, as
/// `wrap_ai.cpp` does.
struct Host {
    thing: Thing<TplView>,
    env: Env,
    status: u64,
    log: Vec<u32>,
    /// The AI module's next call frame (`g_nextCallFrame`).
    next_call_frame: u32,
    /// The map's cells: type and pinch.
    map: Vec<(u8, bool)>,
    store: LocomotorStore,
    names: NameKeyGenerator,
    rng: RandomValues,
    /// Batch A4: the AIs whose machines are running (the AI's state, lent while its machine runs).
    ai_stack: Vec<AiUpdate>,
}

impl Host {
    fn l(&mut self, code: u32, w: &[u32]) {
        self.log.push(code);
        self.log.push(w.len() as u32);
        self.log.extend_from_slice(w);
    }

    /// The owner's reaction to a transform change (`reactToTransformChange`): logged with the NEW transform, cached position
    /// and angle.
    fn react(&mut self) {
        let m = self.thing.transform_matrix().to_array();
        let p = *self.thing.position();
        let mut w: Vec<u32> = m.iter().map(|f| f.to_bits()).collect();
        w.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), self.thing.orientation().to_bits()]);
        self.l(L_TRANSFORM, &w);
    }

    fn ground(&self, x: f32, y: f32) -> f32 {
        let e = &self.env.loco;
        e.ga * x + e.gb * y + e.gc
    }

    /// `makeMap()`: each cell's type and pinch from the hash of its coordinates.
    fn make_map(&mut self) {
        const OTHERS: [u8; 6] = [CELL_CLEAR, CELL_CLEAR, CELL_WATER, CELL_RUBBLE, CELL_OBSTACLE, CELL_IMPASSABLE];
        let seed = self.env.seed;
        self.map.clear();
        for x in 0..MAP_W {
            for y in 0..MAP_H {
                let h = p1_hash(seed, 5, &[x as u32, y as u32]);
                let ty = if pick(h, self.env.cliff_rate) { CELL_CLIFF } else { OTHERS[((p1_hash(seed, 8, &[h]) >> 8) % 6) as usize] };
                self.map.push((ty, pick(p1_hash(seed, 6, &[h]), self.env.pinch_rate)));
            }
        }
    }

    /// `Pathfinder::getCell(layer, x, y)` (every layer's cell is the ground's here): the cell's index in the map, or `None`.
    fn cell(&self, x: i32, y: i32) -> Option<usize> {
        if (0..MAP_W).contains(&x) && (0..MAP_H).contains(&y) {
            Some((x * MAP_H + y) as usize)
        } else {
            None
        }
    }
}

impl LocomotorGlobals for Host {
    fn gravity(&self) -> f32 {
        self.env.loco.gravity
    }
    fn movement_penalty_damage_state(&self) -> BodyDamageType {
        self.env.loco.penalty
    }
}

impl LocomotorHost for Host {
    fn has_physics(&self) -> bool {
        self.env.loco.has_physics != 0
    }
    fn position(&self) -> Coord3D {
        *self.thing.position()
    }
    fn set_position(&mut self, pos: &Coord3D) {
        let _ = self.thing.set_position(&TCtx::new(&self.env), Some(pos)).unwrap();
        self.react();
    }
    fn orientation(&self) -> f32 {
        self.thing.orientation()
    }
    fn set_orientation(&mut self, angle: f32) {
        let _ = self.thing.set_orientation(&TCtx::new(&self.env), angle).unwrap();
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
        self.env.loco.layer
    }
    fn is_kind_of(&self, kind: KindOfType) -> bool {
        let m = self.env.loco.kind_mask;
        (kind == KINDOF_PROJECTILE && m & 1 != 0)
            || (kind == KINDOF_DOZER && m & 2 != 0)
            || (kind == KINDOF_IMMOBILE && m & 4 != 0)
            || (kind == KINDOF_PRODUCED_AT_HELIPAD && m & 8 != 0)
            || (kind == KINDOF_INFANTRY && m & 16 != 0)
            || (kind == KINDOF_IGNORED_IN_GUI && m & 32 != 0)
            || (kind == KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT && m & 64 != 0)
            || (kind == KINDOF_CAN_BE_REPULSED && m & 128 != 0)
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
        self.env.loco.deck_height
    }
    fn major_radius(&self) -> f32 {
        self.env.loco.major_radius
    }
    fn bounding_circle_radius(&self) -> f32 {
        self.env.loco.bounding_circle_radius
    }
    fn is_disabled_held(&self) -> bool {
        self.env.loco.disabled_held != 0
    }
    fn set_model_condition_state(&mut self, condition: ModelConditionFlagType) {
        self.l(L_SET_MODEL_COND, &[condition as u32]);
    }
    fn clear_model_condition_state(&mut self, condition: ModelConditionFlagType) {
        self.l(L_CLEAR_MODEL_COND, &[condition as u32]);
    }
    fn is_above_terrain(&mut self) -> bool {
        // quiet: the Thing machinery's query (as wrap_ai.cpp's isAboveTerrain)
        self.thing.is_above_terrain(&TCtx::new(&self.env))
    }
    fn damage_state(&self) -> BodyDamageType {
        self.env.loco.body_state
    }
    fn is_stunned(&self) -> bool {
        self.env.loco.stunned != 0
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
        let v = self.env.loco.vel;
        Coord3D { x: v[0], y: v[1], z: v[2] }
    }
    fn velocity_magnitude(&self) -> f32 {
        self.env.loco.vel_mag
    }
    fn forward_speed_2d(&self) -> f32 {
        self.env.loco.fwd2d
    }
    fn forward_speed_3d(&self) -> f32 {
        self.env.loco.fwd3d
    }
    fn mass(&self) -> f32 {
        self.env.loco.mass
    }
    fn is_motive(&self) -> bool {
        self.env.loco.motive != 0
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
        self.env.loco.frame
    }
    fn ground_height(&mut self, x: f32, y: f32) -> f32 {
        self.l(L_GROUND_HEIGHT, &[x.to_bits(), y.to_bits()]);
        self.ground(x, y)
    }
    fn layer_height(&mut self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        self.l(L_LAYER_HEIGHT, &[x.to_bits(), y.to_bits(), layer as u32, 1]);
        let mut h = self.ground(x, y);
        if layer != LAYER_GROUND {
            h += self.env.loco.layer_offset;
        }
        h
    }
    fn layer_height_unclipped(&mut self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        self.l(L_LAYER_HEIGHT, &[x.to_bits(), y.to_bits(), layer as u32, 0]);
        let mut h = self.ground(x, y);
        if layer != LAYER_GROUND {
            h += self.env.loco.layer_offset;
        }
        h + 0.5
    }
    fn underwater(&mut self, x: f32, y: f32) -> Underwater {
        self.l(L_UNDERWATER, &[x.to_bits(), y.to_bits()]);
        let terrain_z = self.ground(x, y);
        if x < self.env.loco.wx {
            Underwater { underwater: true, water_z: Some(self.env.loco.wz), terrain_z }
        } else {
            Underwater { underwater: false, water_z: None, terrain_z }
        }
    }
    fn highest_layer_for_destination(&mut self, pos: &Coord3D) -> PathfindLayer {
        self.l(L_HIGHEST_LAYER, &[pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
        self.env.loco.highest_layer
    }
    fn ground_or_structure_height(&mut self, x: f32, y: f32) -> f32 {
        self.l(L_STRUCT_HEIGHT, &[x.to_bits(), y.to_bits()]);
        self.ground(x, y) + self.env.loco.struct_offset
    }
    fn valid_movement_terrain(&mut self, layer: PathfindLayer, _loco: &Locomotor, pos: &Coord3D) -> bool {
        self.l(L_VALID_TERRAIN, &[layer as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
        let b = self.env.loco.inv_box;
        !(self.env.loco.inv_enabled != 0 && pos.x >= b[0] && pos.x <= b[1] && pos.y >= b[2] && pos.y <= b[3])
    }
}

impl PathHost for Host {
    fn is_line_passable(&mut self, obj: ObjectId, surfaces: LocomotorSurfaceTypeMask, layer: PathfindLayer, start: &Coord3D, end: &Coord3D, blocked: bool, allow_pinched: bool) -> bool {
        let b = |f: f32| f.to_bits();
        let w = [obj as u32, surfaces as u32, layer as u32, b(start.x), b(start.y), b(end.x), b(end.y), u32::from(blocked), u32::from(allow_pinched)];
        let r = pick(p1_hash(self.env.seed, 2, &w), self.env.line_rate);
        self.l(L_LINE_PASSABLE, &[w[1], w[2], w[3], w[4], w[5], w[6], w[7], w[8], u32::from(r)]);
        r
    }
    fn is_ground_path_passable(&mut self, crusher: bool, start: &Coord3D, layer: PathfindLayer, end: &Coord3D, diameter: i32) -> bool {
        let b = |f: f32| f.to_bits();
        let w = [u32::from(crusher), b(start.x), b(start.y), layer as u32, b(end.x), b(end.y), diameter as u32];
        let r = pick(p1_hash(self.env.seed, 3, &w), self.env.ground_rate);
        let mut lw = w.to_vec();
        lw.push(u32::from(r));
        self.l(L_GROUND_PASSABLE, &lw);
        r
    }
    fn cell_type_and_pinched(&mut self, _layer: PathfindLayer, pos: &Coord3D) -> Option<(i32, bool)> {
        // Pathfinder::getCell(layer, pos), inline: worldToCell (floor), NULL on an overflow; a layer's cell is the ground's
        let x = real_to_int_floor(pos.x / 10.0);
        let y = real_to_int_floor(pos.y / 10.0);
        self.cell(x, y).map(|c| (i32::from(self.map[c].0), self.map[c].1))
    }
    fn layer_for_destination(&mut self, pos: &Coord3D) -> PathfindLayer {
        let b = |f: f32| f.to_bits();
        let h = p1_hash(self.env.seed, 1, &[b(pos.x), b(pos.y), b(pos.z)]);
        let layer = (1 + (h >> 8) % 3) as PathfindLayer;
        self.l(L_LAYER_FOR, &[b(pos.x), b(pos.y), b(pos.z), layer as u32]);
        layer
    }
    fn object_layer(&mut self, _obj: ObjectId) -> PathfindLayer {
        self.env.loco.layer
    }
}

impl AiHost for Host {
    fn object_id(&self) -> ObjectId {
        self.env.object_id
    }
    fn is_disabled_by_type(&self, t: DisabledType) -> bool {
        (t == DISABLED_HELD && self.env.loco.disabled_held != 0)
            || (t == DISABLED_UNMANNED && self.env.disabled_unmanned != 0)
            || ([DISABLED_PARALYZED, DISABLED_EMP, DISABLED_SUBDUED, DISABLED_HACKED].contains(&t) && (self.env.disabled_mask >> t) & 1 != 0)
    }
    fn crusher_level(&self) -> i32 {
        i32::from(self.env.crusher_level as u8)
    }
    fn physics_allow_to_fall(&self) -> Option<bool> {
        (self.env.loco.has_physics != 0).then_some(self.env.allow_to_fall != 0)
    }
    fn height_above_terrain(&mut self) -> f32 {
        let ctx = TCtx::new(&self.env);
        let h = self.thing.height_above_terrain(&ctx);
        let queries = ctx.queries.into_inner();
        for (x, y) in queries {
            self.l(L_GROUND_HEIGHT, &[x.to_bits(), y.to_bits()]);
        }
        h
    }
    fn set_effectively_dead(&mut self, dead: bool) {
        self.l(L_EFFECTIVELY_DEAD, &[u32::from(dead)]);
    }
    fn wake_frame(&self) -> UpdateSleepTime {
        let now = self.env.loco.frame;
        if self.next_call_frame > now {
            update_sleep(self.next_call_frame - now)
        } else {
            UPDATE_SLEEP_NONE
        }
    }
    fn set_wake_frame(&mut self, wake_delay: UpdateSleepTime) {
        self.l(L_SET_WAKE, &[wake_delay.0]);
        self.next_call_frame = self.env.loco.frame.wrapping_add(wake_delay.0);
    }
    fn group_recompute_speed(&mut self) {
        if self.env.has_group != 0 {
            self.l(L_GROUP_SPEED, &[]);
        }
    }
    fn pf_surfaces_for_position(&mut self, _layer: PathfindLayer, pos: &Coord3D) -> LocomotorSurfaceTypeMask {
        let x = real_to_int_floor(pos.x / 10.0);
        let y = real_to_int_floor(pos.y / 10.0);
        let t = self.cell(x, y).map_or(CELL_CLEAR, |c| self.map[c].0);
        valid_locomotor_surfaces_for_cell_type(t)
    }
    fn pf_update_layer(&mut self, layer: PathfindLayer) {
        self.l(L_UPDATE_LAYER, &[layer as u32]);
    }
    fn pf_valid_movement_position(&mut self, is_crusher: bool, _layer: PathfindLayer, surfaces: LocomotorSurfaceTypeMask, pos: &Coord3D) -> bool {
        // the header's inlines: REAL_TO_INT (truncation) of the position over the cell size, then getCell(layer, x, y)
        let x = real_to_int(pos.x / 10.0);
        let y = real_to_int(pos.y / 10.0);
        let cell = self.cell(x, y).map_or(u32::MAX, |c| c as u32);
        let r = pick(p1_hash(self.env.seed, 7, &[u32::from(is_crusher), surfaces as u32, cell]), self.env.valid_pos_rate);
        self.l(L_VALID_POSITION, &[u32::from(is_crusher), surfaces as u32, cell, u32::from(r)]);
        r
    }
    fn distance_squared_3d(&mut self, dest: &Coord3D) -> f32 {
        let p = *self.thing.position();
        let (dx, dy, dz) = (dest.x - p.x, dest.y - p.y, dest.z - p.z);
        let d = dx * dx + dy * dy + dz * dz;
        self.l(L_DISTANCE_SQUARED, &[dest.x.to_bits(), dest.y.to_bits(), dest.z.to_bits(), d.to_bits()]);
        d
    }
    fn add_locomotor(&mut self, set: &mut LocomotorSet, template: NameKeyType) {
        let frame = self.env.loco.frame;
        set.add_locomotor(&self.store, template, &mut self.rng, frame).expect("the data names the store's templates");
    }

    // ---- batch A3 ----
    fn object_exists(&self, id: ObjectId) -> bool {
        (1..32).contains(&id) && (self.env.exist_mask >> id) & 1 != 0
    }
    fn ai_add_targeter(&mut self, target: ObjectId, targeter: ObjectId, add: bool) {
        if self.object_exists(target) && (self.env.ai_mask >> target) & 1 != 0 {
            self.l(L_ADD_TARGETER, &[1, targeter as u32, u32::from(add)]);
        }
    }
    fn is_effectively_dead(&self) -> bool {
        self.env.effectively_dead != 0
    }
    fn controlling_player_type(&self) -> Option<PlayerType> {
        match self.env.player_type {
            0 => Some(PlayerType::Human),
            1 => Some(PlayerType::Computer),
            _ => None,
        }
    }
    fn team_attack_common_target(&self) -> bool {
        self.env.team_attack_common != 0
    }
    fn team_target_object(&self) -> ObjectId {
        let id = self.env.team_target;
        if self.object_exists(id) { id } else { 0 }
    }
    fn able_to_attack_specific_object(&mut self, t: AbleToAttackType, target: ObjectId, command_source: CommandSourceType) -> CanAttackResultValue {
        let h = p1_hash(self.env.seed, 9, &[t as u32, target as u32, command_source as u32]);
        let r = if pick(h, self.env.able_rate) {
            ATTACKRESULT_POSSIBLE
        } else if pick(p1_hash(self.env.seed, 10, &[h]), self.env.able_rate) {
            ATTACKRESULT_POSSIBLE_AFTER_MOVING
        } else {
            ATTACKRESULT_NOT_POSSIBLE
        };
        self.l(L_ABLE_TO_ATTACK, &[t as u32, target as u32, command_source as u32, r as u32]);
        r
    }
    fn stealth_granted_by_special_power(&self) -> Option<bool> {
        (self.env.stealth >= 0).then_some(self.env.stealth > 0)
    }
    fn contained_by(&self) -> ObjectId {
        let id = self.env.container_id;
        if self.object_exists(id) { id } else { 0 }
    }
    fn container_passenger_allowed_to_fire(&self) -> bool {
        self.env.container_allows_fire != 0
    }
    fn container_bounding_circle_radius(&self) -> f32 {
        self.env.container_radius
    }
    fn last_damage_type_and_source(&self) -> (DamageType, ObjectId) {
        (self.env.last_damage_type, self.env.last_damage_source)
    }
    fn ai_data_force_idle_frames(&self) -> u32 {
        self.env.force_idle_frames as u32
    }
    fn ai_data_attack_uses_line_of_sight(&self) -> bool {
        self.env.attack_uses_los != 0
    }
    fn ai_data_attack_ignore_insignificant_buildings(&self) -> bool {
        self.env.ignore_insignificant != 0
    }
    fn adjusted_vision_range_for_object(&mut self, factors: i32) -> f32 {
        let id = self.env.object_id;
        self.l(L_VISION_RANGE, &[id as u32, factors as u32]);
        self.env.vision_range
    }
    fn find_closest_enemy(&mut self, range: f32, qualifiers: u32, info: Option<usize>) -> ObjectId {
        let id = self.env.object_id;
        self.l(L_CLOSEST_ENEMY, &[id as u32, range.to_bits(), qualifiers, u32::from(info.is_some())]);
        let e = self.env.closest_enemy;
        if self.object_exists(e) { e } else { 0 }
    }
    fn current_weapon_anti_mask(&self) -> Option<i32> {
        (self.env.weapon_anti >= 0).then_some(self.env.weapon_anti)
    }
    fn player_has_upgrade_complete(&self, name: &str) -> bool {
        self.env.player_type >= 0
            && match name {
                "Upgrade_Nationalism" => self.env.upgrades & 1 != 0,
                "Upgrade_Fanaticism" => self.env.upgrades & 2 != 0,
                _ => false,
            }
    }
    fn horde_update_interfaces(&self) -> Vec<(bool, bool)> {
        let b = self.env.horde;
        (0..3).filter(|i| (b >> i) & 1 != 0).map(|i| ((b >> (3 + i)) & 1 != 0, (b >> (6 + i)) & 1 != 0)).collect()
    }
    fn set_weapon_bonus_condition(&mut self, c: i32) {
        self.l(L_SET_BONUS, &[c as u32]);
    }
    fn clear_weapon_bonus_condition(&mut self, c: i32) {
        self.l(L_CLEAR_BONUS, &[c as u32]);
    }
    fn logic_random_value(&mut self, lo: i32, hi: i32) -> i32 {
        self.rng.game_logic_random_value(lo, hi)
    }
}

impl StateContext for Host {
    fn frame(&self) -> u32 {
        self.env.loco.frame
    }
    /// The scene's other objects (never moved: at the origin, as `wrap_ai.cpp`'s `g_others`).
    fn find_object(&self, id: ObjectId) -> Option<ObjectRef> {
        self.object_exists(id).then(|| ObjectRef { id, position: Coord3D::default() })
    }
}

impl Host {
    /// A placeholder state's answer (`placeholderAnswer` of `wrap_ai.cpp`): the hash of the frame, its id and the callback.
    fn placeholder_answer(&self, id: StateId, which: u32) -> StateReturn {
        let h = p1_hash(self.env.seed, 11, &[self.env.loco.frame, id, which]);
        if pick(h, self.env.state_rate) {
            return StateReturn::CONTINUE;
        }
        match (h >> 16) % 4 {
            0 => StateReturn::SUCCESS,
            1 => StateReturn::FAILURE,
            2 => StateReturn::sleep(1 + (h >> 20) % 40),
            _ => StateReturn::SLEEP_FOREVER,
        }
    }
}

impl AiStateHost for Host {
    fn ai_stack(&mut self) -> &mut Vec<AiUpdate> {
        &mut self.ai_stack
    }
    fn pf_goal_position(&mut self) -> Option<Coord3D> {
        let has = self.env.pf_has_goal;
        self.l(L_PF_GOAL_POSITION, &[has as u32]);
        let g = self.env.pf_goal;
        (has != 0).then_some(Coord3D { x: g[0], y: g[1], z: g[2] })
    }
    fn pf_snap_position(&mut self, pos: &mut Coord3D) {
        self.l(L_PF_SNAP, &[pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
        pos.x = (pos.x / 10.0).floor() * 10.0 + 10.0 / 2.0;
        pos.y = (pos.y / 10.0).floor() * 10.0 + 10.0 / 2.0;
    }
    fn pf_update_goal(&mut self, pos: &Coord3D, layer: PathfindLayer) {
        self.l(L_PF_UPDATE_GOAL, &[pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), layer as u32]);
    }
    fn pf_queue_for_path(&mut self) {
        let id = self.env.object_id;
        self.l(L_PF_QUEUE, &[id as u32]);
    }
    fn turret_update(&mut self, which: usize) -> UpdateSleepTime {
        let h = p1_hash(self.env.seed, 12, &[self.env.loco.frame, which as u32]);
        let r = if pick(h, self.env.turret_rate) {
            UPDATE_SLEEP_NONE
        } else if (h >> 16) & 1 != 0 {
            update_sleep(1 + (h >> 20) % 60)
        } else {
            UPDATE_SLEEP_FOREVER
        };
        self.l(L_TURRET_UPDATE, &[which as u32, r.0]);
        r
    }
    fn ai_do_command(&mut self, cmd: i32, source: CommandSourceType, obj: ObjectId, int_value: i32) {
        self.l(L_AI_COMMAND, &[cmd as u32, source as u32, obj as u32, int_value as u32]);
    }
    fn find_closest_repulsor(&mut self, range: f32) -> ObjectId {
        let id = self.env.object_id;
        self.l(L_REPULSOR, &[id as u32, range.to_bits()]);
        let r = self.env.repulsor;
        if self.object_exists(r) { r } else { 0 }
    }
    fn object_vision_range(&self) -> f32 {
        self.env.obj_vision_range
    }
    fn clear_and_set_model_condition_flags(&mut self, clear: &[ModelConditionFlagType], set: ModelConditionFlagType) {
        let mut w: Vec<u32> = clear.iter().map(|&c| c as u32).collect();
        w.sort_unstable();
        w.dedup();
        w.push(u32::MAX);
        w.push(set as u32);
        self.l(L_CLEAR_SET_MODEL, &w);
    }
    fn notify_of_object_creation_or_destruction(&mut self) {
        self.l(L_SCRIPT_NOTIFY, &[]);
    }
    fn untranslated_on_enter(&mut self, id: StateId) -> StateReturn {
        let r = self.placeholder_answer(id, 0);
        self.l(L_STATE_ENTER, &[id, r.0 as u32]);
        r
    }
    fn untranslated_update(&mut self, id: StateId) -> StateReturn {
        let r = self.placeholder_answer(id, 1);
        self.l(L_STATE_UPDATE, &[id, r.0 as u32]);
        r
    }
    fn untranslated_on_exit(&mut self, id: StateId, status: StateExitType) {
        self.l(L_STATE_EXIT, &[id, status as u32]);
    }
}

// ---------------------------------------------------------------------------
// The INI side
// ---------------------------------------------------------------------------

struct IniCtx<'a> {
    store: &'a mut LocomotorStore,
    names: &'a mut NameKeyGenerator,
}

fn h_locomotor(ini: &mut Ini<'_>, c: &mut IniCtx<'_>) -> IniResult<()> {
    c.store.parse_locomotor_template_definition(ini, c.names)
}

fn h_stub(_: &mut Ini<'_>, _: &mut IniCtx<'_>) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}

fn blocks<'a>() -> Vec<BlockParse<IniCtx<'a>>> {
    TYPE_TABLE.iter().map(|&(token, _, _)| BlockParse { token, parse: if token == "Locomotor" { h_locomotor } else { h_stub } }).collect()
}

/// A plausible template of the given appearance (INI text), as the Locomotor test writes them.
fn gen_template(g: &mut Gen, name: &str, appearance: usize) -> String {
    let mut s = format!("Locomotor {name}\n");
    let mut put = |k: &str, v: String| s.push_str(&format!("  {k} = {v}\n"));
    let flyer = matches!(appearance, 4 | 5) || (appearance == 7 && g.below(2) == 0);
    let speed = g.uniform(20.0, 160.0);
    put("Appearance", LOCO_APPEARANCE_NAMES[appearance].to_string());
    put(
        "Surfaces",
        if flyer {
            "AIR".to_string()
        } else {
            ["GROUND CLIFF", "GROUND WATER", "GROUND", "WATER", "RUBBLE GROUND", "CLIFF"][g.below(6) as usize].to_string()
        },
    );
    put("Speed", format!("{speed:.3}"));
    if g.below(2) == 0 {
        put("SpeedDamaged", format!("{:.3}", speed * g.uniform(0.2, 1.0)));
    }
    put("TurnRate", format!("{:.3}", g.uniform(30.0, 400.0)));
    put("Acceleration", format!("{:.3}", g.uniform(20.0, 400.0)));
    put("Braking", format!("{:.3}", g.uniform(20.0, 400.0)));
    // LEGS and CLIMBER must not have a minimum speed (a stack overflow upstream, see the Locomotor test)
    let min_speed = match appearance {
        0 | 6 => 0.0,
        4 => g.uniform(5.0, 30.0),
        5 => g.uniform(20.0, 60.0),
        _ => 0.0,
    };
    put("MinSpeed", format!("{min_speed:.3}"));
    if matches!(appearance, 1 | 8 | 5) {
        put("MinTurnSpeed", format!("{:.3}", g.uniform(0.0, 80.0)));
    }
    let z = if appearance == 4 {
        0
    } else if flyer {
        [2usize, 3, 7, 2, 3, 4, 5, 6][g.below(8) as usize]
    } else {
        0
    };
    put("ZAxisBehavior", LOCO_Z_NAMES[z].to_string());
    if z != 0 || appearance == 4 {
        put("PreferredHeight", format!("{:.3}", g.uniform(0.0, 60.0)));
    }
    if appearance != 4 && (z == 2 || z == 3 || z == 7 || flyer) {
        put("Lift", format!("{:.3}", g.uniform(90.0, 500.0)));
    }
    if appearance == 4 {
        put("MaxThrustAngle", format!("{:.3}", g.uniform(0.0, 90.0)));
    }
    if g.below(3) == 0 {
        put("WanderWidthFactor", format!("{:.3}", g.uniform(0.0, 2.0)));
        put("WanderLengthFactor", format!("{:.3}", g.uniform(0.3, 3.0)));
    }
    if g.below(3) == 0 {
        put("CloseEnoughDist", format!("{:.3}", g.uniform(0.0, 12.0)));
    }
    if g.below(4) == 0 {
        put("CloseEnoughDist3D", "Yes".to_string());
    }
    if g.below(4) == 0 {
        put("LocomotorWorksWhenDead", "Yes".to_string());
    }
    if g.below(3) == 0 {
        put("AirborneTargetingHeight", format!("{}", g.below(40)));
    }
    if g.below(4) == 0 {
        put("CanMoveBackwards", "Yes".to_string());
    }
    if g.below(6) == 0 {
        put("DownhillOnly", "Yes".to_string());
    }
    s.push_str("End\n");
    s
}

// ---------------------------------------------------------------------------
// The dumps (the format of ref_ai_a2_dump)
// ---------------------------------------------------------------------------

fn put_pos(v: &mut Vec<u32>, p: &Coord3D) {
    v.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
}

/// The driver's `putPath` (batch P1's).
fn put_path(v: &mut Vec<u32>, p: &Path) {
    let list = p.list();
    let index = |n: Option<usize>| -> u32 {
        match n {
            None => u32::MAX,
            Some(i) => list.iter().position(|&x| x == i).map_or(1000, |k| k as u32),
        }
    };
    v.push(list.len() as u32);
    for &i in &list {
        let n = p.node(i);
        let pos = n.position();
        v.extend([pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), n.layer() as u32, u32::from(n.can_optimize()), n.id as u32]);
        let (opt, dir, dist) = n.next_optimized();
        v.extend([index(opt), dist.to_bits(), dir.x.to_bits(), dir.y.to_bits(), index(n.previous())]);
    }
    v.push(index(p.last_node()));
    v.push(u32::from(p.is_optimized()));
    v.push(u32::from(p.blocked_by_ally()));
    let (valid, countdown, cin, cout, recent) = p.cpop_state();
    v.extend([u32::from(valid), countdown as u32, cin.x.to_bits(), cin.y.to_bits(), cin.z.to_bits(), cout.dist_along_path.to_bits()]);
    v.extend([cout.pos_on_path.x.to_bits(), cout.pos_on_path.y.to_bits(), cout.pos_on_path.z.to_bits(), cout.layer as u32, index(recent)]);
}

/// A locomotor: its template's key, then its data members in `xfer`'s order (from the bytes the Rust xfer writes).
fn put_loco(v: &mut Vec<u32>, names: &mut NameKeyGenerator, loco: &Locomotor) {
    v.push(names.name_to_key(loco.get_template_name()).0);
    let mut copy = loco.clone();
    let mut save = XferSave::new();
    save.open(b"loco").unwrap();
    copy.xfer_state(&mut save).unwrap();
    save.close().unwrap();
    let b = save.bytes();
    assert_eq!(b.len(), 1 + 64, "version byte and 16 words");
    v.extend(b[1..].chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())));
}

/// `ref_ai_a2_dump`. `bug` plants a wrong dump: 1 forgets the blocked frames.
fn rust_dump(module: &AiUpdateModule<Host>, h: &mut Host, bug: u32) -> Vec<u32> {
    let ai = module.ai();
    let mut v = vec![
        ai.prior_waypoint_id,
        ai.current_waypoint_id,
        ai.next_enemy_scan_time,
        ai.current_victim_id as u32,
        ai.desired_speed.to_bits(),
        ai.last_command_source as u32,
        ai.guard_mode as u32,
        ai.guard_target_type[0] as u32,
        ai.guard_target_type[1] as u32,
    ];
    put_pos(&mut v, &ai.location_to_guard);
    v.push(ai.object_to_guard as u32);
    for p in &ai.waypoint_queue {
        put_pos(&mut v, p);
    }
    v.push(ai.waypoint_count as u32);
    v.push(ai.waypoint_index as u32);
    v.push(ai.requested_victim_id as u32);
    put_pos(&mut v, &ai.requested_destination);
    put_pos(&mut v, &ai.requested_destination2);
    v.push(ai.path_timestamp);
    v.push(ai.ignore_obstacle_id as u32);
    v.push(ai.path_extra_distance.to_bits());
    v.extend([ai.pathfind_goal_cell.x as u32, ai.pathfind_goal_cell.y as u32, ai.pathfind_cur_cell.x as u32, ai.pathfind_cur_cell.y as u32]);
    v.push(if bug == 1 { 0 } else { ai.blocked_frames as u32 });
    v.push(ai.cur_max_blocked_speed.to_bits());
    v.push(ai.bump_speed_limit.to_bits());
    v.push(ai.ignore_collisions_until);
    v.push(ai.queue_for_path_frame);
    put_pos(&mut v, &ai.final_position);
    v.push(ai.repulsor1 as u32);
    v.push(ai.repulsor2 as u32);
    v.push(ai.next_goal_path_index as u32);
    v.push(ai.move_out_of_way1 as u32);
    v.push(ai.move_out_of_way2 as u32);
    let set = &ai.locomotor_set;
    v.push(set.len() as u32);
    for i in 0..set.len() {
        put_loco(&mut v, &mut h.names, set.get(i).unwrap());
    }
    v.push(ai.cur_locomotor.map_or(u32::MAX, |i| i as u32));
    v.push(set.get_valid_surfaces() as u32);
    v.push(u32::from(set.is_downhill_only()));
    v.push(ai.cur_locomotor_set as u32);
    v.push(ai.locomotor_goal_type as u32);
    put_pos(&mut v, &ai.locomotor_goal_data);
    v.push(ai.turret_sync_flag as u32);
    v.push(ai.attitude as u32);
    v.push(ai.next_mood_check_time);
    v.push(ai.crate_created as u32);
    v.push(ai.tmp_int as u32);
    for f in [
        ai.do_final_position,
        ai.waiting_for_path,
        ai.is_attack_path,
        ai.is_final_goal,
        ai.is_approach_path,
        ai.is_safe_path,
        ai.movement_complete,
        ai.is_moving,
        ai.is_blocked,
        ai.is_blocked_and_stuck,
        ai.upgraded_locomotors,
        ai.can_path_through_units,
        ai.randomly_offset_mood_check,
        ai.is_ai_dead,
        ai.is_recruitable,
        ai.executing_waypoint_queue,
        ai.retry_path,
        ai.allowed_to_chase,
        ai.is_in_update,
        ai.fix_loco_in_post_process,
    ] {
        v.push(u32::from(f));
    }
    v.push(u32::from(ai.path.is_some()));
    if let Some(p) = &ai.path {
        put_path(&mut v, p);
    }
    v.extend(h.thing.transform_matrix().to_array().iter().map(|f| f.to_bits()));
    put_pos(&mut v, h.thing.position());
    v.push(h.thing.orientation().to_bits());
    v.push(h.status as u32);
    v.push((h.status >> 32) as u32);
    v.push(h.next_call_frame);
    // batch A4: the machine
    v.push(u32::from(module.machine.is_some()));
    if let Some(sm) = &module.machine {
        v.extend([sm.get_current_state_id(), sm.peek_sleep_till(), sm.default_state_id(), sm.goal_object_id() as u32]);
        put_pos(&mut v, sm.goal_position());
        v.extend([u32::from(sm.is_locked()), u32::from(sm.is_default_state_inited())]);
        let x = &ai.machine;
        v.extend([x.temporary_state, x.temporary_state_frame_end, x.goal_path.len() as u32]);
        for p in &x.goal_path {
            put_pos(&mut v, p);
        }
        match sm.state(states_ai_idle()) {
            Some(AiState::Idle { initial_sleep_offset, should_look_for_targets, inited }) => {
                v.extend([u32::from(*initial_sleep_offset), u32::from(*should_look_for_targets), u32::from(*inited)]);
            }
            _ => panic!("the idle state"),
        }
    }
    v
}

fn states_ai_idle() -> StateId {
    rts_lockstep_engine::logic::ai::ai_update_types::AI_IDLE
}

fn cpp_dump() -> Vec<u32> {
    let mut buf = vec![0u32; 4096];
    let n = unsafe { cpp::ref_ai_a2_dump(buf.as_mut_ptr(), buf.len() as c_int) };
    assert!(n as usize <= buf.len(), "dump too long");
    buf.truncate(n as usize);
    buf
}

// ---------------------------------------------------------------------------
// The programs
// ---------------------------------------------------------------------------

/// The scene of a case.
fn gen_scene(g: &mut Gen) -> Env {
    let mut e = Env::default();
    let l = &mut e.loco;
    // (batch A4: sometimes the first frames, where the idle state's first update pops the object to its goal)
    l.frame = if g.below(10) == 0 { g.below(2) } else { g.below(5000) };
    l.gravity = if g.below(4) == 0 { -1.0 } else { -60.0 * (1.0 / 30.0) * (1.0 / 30.0) };
    l.penalty = 1 + g.below(2) as i32;
    l.body_state = g.below(4) as i32;
    l.has_physics = i32::from(g.below(8) != 0);
    l.mass = g.uniform(1.0, 60.0);
    l.layer = if g.below(5) == 0 { 1 + g.below(3) as i32 } else { LAYER_GROUND };
    l.kind_mask = (g.below(16) & if g.below(3) == 0 { 15 } else { 10 }) as i32;
    l.deck_height = g.uniform(0.0, 8.0);
    l.major_radius = g.uniform(1.0, 20.0);
    l.bounding_circle_radius = g.uniform(1.0, 25.0);
    l.disabled_held = i32::from(g.below(10) == 0);
    l.ga = g.uniform(-0.25, 0.25);
    l.gb = g.uniform(-0.25, 0.25);
    l.gc = g.uniform(-10.0, 10.0);
    l.wx = if g.below(3) == 0 { g.uniform(-20.0, 200.0) } else { -1.0e9 };
    l.wz = g.uniform(-5.0, 15.0);
    l.layer_offset = g.uniform(-4.0, 12.0);
    l.struct_offset = g.uniform(0.0, 20.0);
    l.highest_layer = if g.below(2) == 0 { LAYER_GROUND } else { LAYER_WALL };
    if g.below(3) == 0 {
        l.inv_enabled = 1;
        let (cx, cy) = (g.uniform(0.0, 240.0), g.uniform(0.0, 160.0));
        let (w, h) = (g.uniform(5.0, 60.0), g.uniform(5.0, 60.0));
        l.inv_box = [cx - w, cx + w, cy - h, cy + h];
    }
    e.object_id = 1 + g.below(200) as i32;
    e.crusher_level = g.below(3) as i32;
    e.disabled_unmanned = i32::from(g.below(8) == 0);
    e.allow_to_fall = i32::from(g.below(4) == 0);
    e.has_group = i32::from(g.below(2) == 0);
    e.seed = g.u32();
    e.line_rate = g.below(101);
    e.ground_rate = g.below(101);
    e.cliff_rate = g.below(40);
    e.pinch_rate = g.below(30);
    e.valid_pos_rate = g.below(101);
    // batch A3
    e.loco.kind_mask |= (g.below(8) as i32) << 4;
    e.effectively_dead = i32::from(g.below(10) == 0);
    e.player_type = [-1, 0, 1, 1, 0, 1, 0, 1, 1, 0][g.below(10) as usize];
    e.team_attack_common = i32::from(g.below(2) == 0);
    e.team_target = g.below(32) as i32;
    e.able_rate = g.below(101);
    e.stealth = g.below(3) as i32 - 1;
    e.container_id = if g.below(3) == 0 { g.below(32) as i32 } else { 0 };
    e.container_allows_fire = i32::from(g.below(2) == 0);
    e.container_radius = g.uniform(0.0, 30.0);
    e.last_damage_type = if g.below(4) == 0 { 10 } else { g.below(24) as i32 };
    e.last_damage_source = g.below(32) as i32;
    e.force_idle_frames = g.below(60) as i32;
    e.attack_uses_los = i32::from(g.below(2) == 0);
    e.ignore_insignificant = i32::from(g.below(2) == 0);
    e.vision_range = if g.below(8) == 0 { -g.uniform(0.0, 10.0) } else { g.uniform(0.0, 300.0) };
    e.closest_enemy = g.below(32) as i32;
    e.weapon_anti = if g.below(4) == 0 { -1 } else { g.below(256) as i32 };
    e.upgrades = g.below(4) as i32;
    e.horde = g.below(512) as i32;
    e.exist_mask = g.u32() & !1;
    e.ai_mask = g.u32();
    e.has_machine = i32::from(g.below(10) != 0);
    e.turrets = g.below(4) as i32;
    // batch A4
    e.loco.kind_mask |= i32::from(g.below(3) == 0) << 7;
    gen_a4(g, &mut e);
    e.turret_rate = g.below(101);
    e.state_rate = g.below(101);
    e
}

/// Batch A4's scene: the pathfinder's goal, the disabled types the AI's update and idle state ask, the repulsor, the vision range.
fn gen_a4(g: &mut Gen, e: &mut Env) {
    e.pf_has_goal = i32::from(g.below(3) != 0);
    e.pf_goal = [g.uniform(-20.0, 260.0), g.uniform(-20.0, 180.0), g.uniform(-5.0, 30.0)];
    if g.below(4) == 0 {
        // near the object's cell or not (the update's pop to the goal is within a cell)
        e.pf_goal[2] = g.uniform(-5.0, 30.0);
    }
    e.disabled_mask = if g.below(4) == 0 { [1 << DISABLED_PARALYZED, 1 << DISABLED_EMP, 1 << DISABLED_SUBDUED, 1 << DISABLED_HACKED][g.below(4) as usize] } else { 0 };
    e.repulsor = if g.below(3) == 0 { g.below(32) as i32 } else { 0 };
    e.obj_vision_range = g.uniform(0.0, 300.0);
}

/// The physics numbers and the frame between operations.
fn perturb(g: &mut Gen, e: &mut Env) {
    let l = &mut e.loco;
    l.frame += g.below(3);
    l.vel = [g.uniform(-3.0, 3.0), g.uniform(-3.0, 3.0), g.uniform(-1.0, 1.0)];
    l.vel_mag = (l.vel[0] * l.vel[0] + l.vel[1] * l.vel[1] + l.vel[2] * l.vel[2]).sqrt();
    l.fwd2d = g.uniform(-2.0, 5.0);
    l.fwd3d = l.fwd2d + g.uniform(-0.5, 0.5);
    l.stunned = i32::from(g.below(20) == 0);
    l.motive = i32::from(g.below(3) != 0);
    if g.below(10) == 0 {
        l.body_state = g.below(4) as i32;
    }
    if g.below(15) == 0 {
        e.disabled_unmanned ^= 1;
    }
    if g.below(15) == 0 {
        l.disabled_held ^= 1;
    }
    // batch A4
    if g.below(6) == 0 {
        gen_a4(g, e);
    }
    if g.below(10) == 0 {
        e.state_rate = g.below(101);
        e.turret_rate = g.below(101);
    }
    // batch A3
    if g.below(10) == 0 {
        e.closest_enemy = g.below(32) as i32;
        e.team_target = g.below(32) as i32;
        e.able_rate = g.below(101);
    }
    if g.below(15) == 0 {
        e.effectively_dead ^= 1;
    }
    if g.below(10) == 0 {
        e.horde = g.below(512) as i32;
        e.upgrades = g.below(4) as i32;
    }
    if g.below(12) == 0 {
        e.player_type = [-1, 0, 1, 1][g.below(4) as usize];
    }
}

fn gen_point(g: &mut Gen) -> [f32; 3] {
    [g.uniform(-20.0, 260.0), g.uniform(-20.0, 180.0), g.uniform(-5.0, 40.0)]
}

/// One operation: the op number, its floats and its ints (`ref_ai_a2_op`'s).
fn gen_op(g: &mut Gen, e: &Env, waypoint_heavy: bool, moving_away: (i32, i32), has_machine: bool, has_saved: bool) -> (i32, Vec<f32>, Vec<i32>) {
    let mut fa = vec![0.0f32; 64];
    let mut ia = vec![0i32; 64];
    let p = gen_point(g);
    fa[..3].copy_from_slice(&p);
    let op = if waypoint_heavy && g.below(3) == 0 {
        [14, 14, 14, 15, 16][g.below(5) as usize]
    } else if g.below(25) == 0 {
        32
    } else if g.below(3) == 0 {
        // batch A4: the machine, the AI's update and creation
        match g.below(100) {
            0..=19 => 59,
            20..=29 => 60,
            30..=34 => 61,
            35..=39 => 62,
            40..=45 => 63,
            46..=51 => 64,
            52..=79 => 65,
            80..=82 => 66,
            83..=85 => 67,
            86..=90 => 68,
            91..=94 => 69,
            95..=97 => 70,
            _ => 71,
        }
    } else if g.below(2) == 0 {
        // batch A3's members
        33 + g.below(26) as i32
    } else {
        match g.below(100) {
        0..=29 => 0,
        30..=33 => 1,
        34..=35 => 2,
        36..=40 => 3,
        41..=44 => 4,
        45..=47 => 5,
        48..=50 => 6,
        51..=55 => 7,
        56..=57 => 8,
        58 => 9,
        59 => 10,
        60..=61 => 11,
        62..=63 => 12,
        64 => 13,
        65..=66 => 14,
        67 => 15,
        68 => 16,
        69 => 17,
        70 => 18,
        71 => 19,
        72..=78 => 20,
        79 => 21,
        80 => 22,
        81 => 23,
        82 => 24,
        83 => 25,
        84 => 26,
        85..=89 => 27,
        90..=91 => 28,
        92..=93 => 29,
        94 => 30,
        _ => 31,
        }
    };
    // the operations that need the AI's machine: none before onObjectCreated makes it (upstream dereferences it); a load needs a save
    let needs_machine = matches!(op, 7 | 36 | 38 | 41 | 43 | 44 | 48 | 50..=52 | 54 | 55 | 59..=65 | 67..=71);
    let op = if needs_machine && !has_machine { 66 } else if op == 71 && !has_saved { 70 } else { op };
    match op {
        1 => ia[0] = [0, 0, 1, 2, 3, 4, 5, 6, 7, -1, 8][g.below(11) as usize],
        2 => ia[0] = g.below(2) as i32,
        5 => fa[0] = g.uniform(-4.0, 4.0),
        12 => ia[0] = [0, 1, 2, 5, 30, 200, -3][g.below(7) as usize],
        20 => {
            let n = 1 + g.below(6) as usize;
            for i in 0..n {
                let q = if i == 0 || g.below(3) != 0 { gen_point(g) } else { [fa[3 * i - 3] + g.uniform(-30.0, 30.0), fa[3 * i - 2] + g.uniform(-30.0, 30.0), fa[3 * i - 1]] };
                fa[3 * i..3 * i + 3].copy_from_slice(&q);
                ia[1 + i] = if g.below(6) == 0 { 1 + g.below(3) as i32 } else { LAYER_GROUND };
            }
            ia[0] = n as i32;
            ia[1 + n] = g.below(3) as i32;
            ia[2 + n] = if ia[1 + n] == 1 { [1, 9, 3, 8, 31][g.below(5) as usize] } else { g.below(2) as i32 };
            ia[3 + n] = if ia[1 + n] == 1 { g.below(2) as i32 } else { [0, 1, 2, 3][g.below(4) as usize] };
        }
        22 => ia[0] = [1, 2, 4, 8, 16, 3, 9][g.below(7) as usize],
        24 => ia[0] = g.below(6) as i32 - 2,
        25 => ia[0] = g.below(300) as i32,
        26 => ia[0] = g.below(5) as i32,
        27 => {
            ia[0] = i32::from(g.below(2) == 0);
            ia[1] = i32::from(g.below(3) == 0);
            ia[2] = i32::from(g.below(6) == 0);
            ia[3] = i32::from(g.below(5) == 0);
            fa[3] = g.uniform(0.0, 4.0);
            fa[4] = if g.below(3) == 0 { 999_999.0 } else { g.uniform(0.0, 6.0) };
            fa[5] = if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, 60.0) };
            fa[6] = if g.below(3) == 0 { 999_999.0 } else { g.uniform(0.0, 5.0) };
            if g.below(3) == 0 {
                // close to the object (the final position's "darn close" pop)
                fa[0] = -1.0e30;
            }
        }
        31 => fa[3] = g.uniform(-3.2, 3.2),
        32 => ia[0] = [0, 1, 2, 3, 10, 40, 1000][g.below(7) as usize],
        34 | 39 | 56 => ia[0] = if g.below(4) == 0 { 0 } else { g.below(32) as i32 },
        43 => ia[0] = g.below(5) as i32,
        46 => ia[0] = e.loco.frame as i32 + g.below(200) as i32 - 100,
        48 => {
            // upstream reads the controlling player unchecked when called by the AI (a crash without one): not generated
            ia[0] = i32::from(e.player_type >= 0 && g.below(3) != 0);
            ia[1] = i32::from(g.below(2) == 0);
        }
        54 => {
            // an object that exists (upstream asks it its id)
            let ids: Vec<i32> = (1..32).filter(|i| (e.exist_mask >> i) & 1 != 0).collect();
            ia[0] = if ids.is_empty() { 0 } else { ids[g.below(ids.len() as u32) as usize] };
            if g.below(2) == 0 {
                // one of the move-out-of-the-way ids (op 58), if it exists
                let id = [moving_away.0, moving_away.1][g.below(2) as usize];
                if (e.exist_mask >> id) & 1 != 0 && id != 0 {
                    ia[0] = id;
                }
            }
        }
        57 => {
            ia[0] = [16, 23, 24, 7, 2][g.below(5) as usize];
            ia[1] = g.below(2) as i32;
        }
        58 => {
            ia[0] = g.below(32) as i32;
            ia[1] = g.below(32) as i32;
        }
        // batch A4: the translated states often (idle, dead, wait, busy), else any id (22, 29, 36 and above 43 are not the
        // machine's: upstream's internalGetState gives the default state)
        59 | 60 => {
            ia[0] = if g.below(2) == 0 { [0, 0, 13, 8, 41, 15, 21][g.below(7) as usize] } else { g.below(46) as i32 };
            if op == 60 {
                if g.below(2) == 0 {
                    ia[0] = 23;
                }
                ia[1] = [0, 1, 5, 30, 1799, 1800, 1801, 5000, -5][g.below(9) as usize];
            }
        }
        63 => ia[0] = if g.below(4) == 0 { 0 } else { g.below(32) as i32 },
        67 => ia[0] = i32::from(g.below(3) == 0),
        68 => {
            if g.below(2) == 0 {
                // a repeated point (addToGoalPath skips the path's last point)
                fa[..3].copy_from_slice(&[10.0, 20.0, 30.0]);
            }
        }
        69 => ia[0] = g.below(8) as i32 - 1,
        _ => {}
    }
    (op, fa, ia)
}

/// The Rust side of `ref_ai_a2_op`. `bug` plants a wrong translation in the result: 2 flips a boolean result.
fn rust_op(module: &mut AiUpdateModule<Host>, h: &mut Host, op: i32, fa: &[f32], ia: &[i32], bug: u32, saved: &mut Vec<u8>) -> Vec<u32> {
    let p = Coord3D { x: fa[0], y: fa[1], z: fa[2] };
    let b = |x: bool| vec![u32::from(x ^ (bug == 2))];
    if op >= 59 {
        return rust_op_a4(module, h, op, &p, ia, saved);
    }
    let ai = module.ai.as_mut().expect("the AI");
    let m = module.machine.as_ref();
    match op {
        0 => vec![ai.do_locomotor(h).0],
        1 => b(ai.choose_locomotor_set(h, ia[0])),
        2 => {
            ai.set_locomotor_upgrade(h, ia[0] != 0);
            vec![]
        }
        3 => {
            ai.set_locomotor_goal_position_on_path();
            vec![]
        }
        4 => {
            ai.set_locomotor_goal_position_explicit(&p);
            vec![]
        }
        5 => {
            ai.set_locomotor_goal_orientation(fa[0]);
            vec![]
        }
        6 => {
            ai.set_locomotor_goal_none();
            vec![]
        }
        7 => vec![ai.get_locomotor_distance_to_goal(h, m.unwrap()).to_bits()],
        8 => b(ai.is_doing_ground_movement(h)),
        9 => b(ai.is_aircraft_that_adjusts_destination()),
        10 => b(ai.get_treat_as_aircraft_for_loco_dist_to_goal(h)),
        11 => b(ai.is_valid_locomotor_position(h, &p)),
        12 => {
            ai.set_queue_for_path_time(h, ia[0]);
            vec![]
        }
        13 => {
            ai.wake_up_now(h);
            vec![]
        }
        14 => b(ai.queue_waypoint(&p)),
        15 => {
            ai.execute_waypoint_queue();
            vec![]
        }
        16 => {
            ai.clear_waypoint_queue();
            vec![]
        }
        17 => {
            ai.destroy_path();
            vec![]
        }
        18 => {
            ai.friend_starting_move();
            vec![]
        }
        19 => {
            ai.friend_ending_move();
            vec![]
        }
        20 => {
            let n = ia[0] as usize;
            let mut path = Path::new();
            for i in 0..n {
                path.append_node(&Coord3D { x: fa[3 * i], y: fa[3 * i + 1], z: fa[3 * i + 2] }, ia[1 + i]);
            }
            let id = h.env.object_id;
            match ia[1 + n] {
                1 => path.optimize(h, id, ia[2 + n], ia[3 + n] != 0),
                2 => path.optimize_ground_path(h, ia[2 + n] != 0, ia[3 + n]),
                _ => {}
            }
            ai.friend_set_path(Some(path));
            vec![]
        }
        21 => vec![ai.get_cur_locomotor_speed(h).to_bits()],
        22 => b(ai.has_locomotor_for_surface(ia[0])),
        23 => {
            ai.mark_as_dead(h);
            vec![]
        }
        24 => {
            ai.set_attitude(ia[0]);
            vec![]
        }
        25 => {
            ai.ignore_obstacle_id(ia[0]);
            vec![]
        }
        26 => {
            ai.set_last_command_source(ia[0]);
            vec![]
        }
        27 => {
            ai.final_position = p;
            ai.do_final_position = ia[0] != 0;
            ai.is_blocked = ia[1] != 0;
            ai.cur_max_blocked_speed = fa[3];
            ai.desired_speed = fa[4];
            ai.is_in_update = ia[2] != 0;
            ai.waiting_for_path = ia[3] != 0;
            ai.path_extra_distance = fa[5];
            ai.bump_speed_limit = fa[6];
            vec![]
        }
        28 => {
            ai.set_final_position(&p);
            vec![]
        }
        29 => {
            ai.choose_good_locomotor_from_current_set(h);
            vec![]
        }
        30 => {
            ai.friend_notify_state_machine_changed(h);
            vec![]
        }
        31 => {
            h.set_position(&p);
            h.set_orientation(fa[3]);
            vec![]
        }
        32 => {
            h.next_call_frame = h.env.loco.frame.wrapping_add(ia[0] as u32);
            vec![]
        }
        // batch A3 (an object argument is ia[0]'s object, none if the scene has none)
        33 => vec![ai.check_for_crate_to_pickup(h) as u32],
        34 => {
            let v = if h.object_exists(ia[0]) { ia[0] } else { 0 };
            ai.set_current_victim(h, v);
            vec![]
        }
        35 => vec![ai.get_current_victim(h) as u32],
        36 => match ai.get_current_victim_pos(h, m.unwrap()) {
            Some(p) => vec![1, p.x.to_bits(), p.y.to_bits(), p.z.to_bits()],
            None => vec![0],
        },
        37 => vec![ai.get_attitude() as u32],
        38 => vec![ai.get_ai_state_type(m.unwrap())],
        39 => {
            let o = if h.object_exists(ia[0]) { ia[0] } else { 0 };
            ai.ignore_obstacle(o);
            vec![]
        }
        40 => vec![ai.get_ignored_obstacle_id() as u32],
        41 => vec![ai.get_enter_target(h, m.unwrap()) as u32],
        42 => vec![ai.get_mood_matrix_value(h, m)],
        43 => vec![ai.get_mood_matrix_action_adjustment(h, m.unwrap(), ia[0])],
        44 => {
            ai.wake_up_and_attempt_to_target(h, m.unwrap());
            vec![]
        }
        45 => {
            ai.reset_next_mood_check_time(h);
            vec![]
        }
        46 => {
            ai.set_next_mood_check_time(ia[0] as u32);
            vec![]
        }
        47 => b(ai.can_auto_acquire_while_stealthed(h)),
        48 => vec![ai.get_next_mood_target(h, m.unwrap(), ia[0] != 0, ia[1] != 0) as u32],
        49 => {
            ai.evaluate_morale_bonus(h);
            vec![]
        }
        50 => b(ai.is_idle(m.unwrap())),
        51 => b(ai.is_attacking(m.unwrap())),
        52 => b(ai.is_busy(m.unwrap())),
        53 => b(ai.is_clearing_mines(h)),
        54 => {
            if h.object_exists(ia[0]) {
                b(ai.is_moving_away_from(ia[0]))
            } else {
                vec![]
            }
        }
        55 => b(ai.is_moving(m.unwrap())),
        56 => {
            ai.notify_crate(ia[0]);
            vec![]
        }
        57 => {
            if ia[1] != 0 {
                h.status |= 1 << ia[0];
            } else {
                h.status &= !(1 << ia[0]);
            }
            vec![]
        }
        58 => {
            ai.move_out_of_way1 = ia[0];
            ai.move_out_of_way2 = ia[1];
            vec![]
        }
        _ => unreachable!(),
    }
}

/// Batch A4's operations (`ref_ai_a2_op`'s 59..71): the machine, the AI's update and creation.
fn rust_op_a4(module: &mut AiUpdateModule<Host>, h: &mut Host, op: i32, p: &Coord3D, ia: &[i32], saved: &mut Vec<u8>) -> Vec<u32> {
    let ret = |r: StateReturn| vec![r.0 as u32];
    match op {
        59 => ret(module.run(h, |sm, h| states::aism_set_state(sm, h, ia[0] as StateId)).expect("setState")),
        60 => ret(module.run(h, |sm, h| states::aism_set_temporary_state(sm, h, ia[0] as StateId, ia[1])).expect("setTemporaryState")),
        61 => {
            module.run(h, states::aism_clear).expect("clear");
            vec![]
        }
        62 => ret(module.run(h, states::aism_reset_to_default_state).expect("resetToDefaultState")),
        63 => {
            let o = h.find_object(ia[0]);
            module.machine.as_mut().expect("the machine").set_goal_object(o.as_ref());
            vec![]
        }
        64 => {
            module.machine.as_mut().expect("the machine").set_goal_position(Some(p));
            vec![]
        }
        65 => vec![module.update(h).expect("update").0],
        66 => {
            module.on_object_created(h).expect("onObjectCreated");
            vec![]
        }
        67 => {
            let sm = module.machine.as_mut().expect("the machine");
            if ia[0] != 0 {
                sm.lock();
            } else {
                sm.unlock();
            }
            vec![]
        }
        68 => {
            states::aism_add_to_goal_path(&mut module.ai_mut().machine, p);
            vec![]
        }
        69 => match states::aism_goal_path_position(&module.ai().machine, ia[0]) {
            Some(q) => vec![1, q.x.to_bits(), q.y.to_bits(), q.z.to_bits()],
            None => vec![0],
        },
        70 => {
            let mut x = XferSave::new();
            x.open(b"ai_a4.bin").expect("open");
            let (sm, ai) = (module.machine.as_mut().expect("the machine"), module.ai.as_mut().expect("the AI"));
            let st = states::aism_xfer(sm, &mut ai.machine, &mut x, |_| panic!("no goal waypoint"), |_| panic!("no goal waypoint")).is_err();
            let _ = x.close();
            *saved = x.bytes().to_vec();
            let mut r = vec![u32::from(st), saved.len() as u32];
            r.extend(saved.iter().map(|&c| u32::from(c)));
            r
        }
        71 => {
            let mut x = XferLoad::new(saved.clone());
            x.open(b"ai_a4.bin").expect("open");
            let (sm, ai) = (module.machine.as_mut().expect("the machine"), module.ai.as_mut().expect("the AI"));
            let st = states::aism_xfer(sm, &mut ai.machine, &mut x, |_| panic!("no goal waypoint"), |_| panic!("no goal waypoint")).is_err();
            let _ = x.close();
            vec![u32::from(st)]
        }
        _ => unreachable!(),
    }
}

/// The comparisons and the coverage.
struct Checks {
    construction: Check,
    results: Check,
    results_bug: Check,
    log: Check,
    dump: Check,
    dump_bug: Check,
    ops: [u64; 72],
    sleeps: [u64; 2],
    goals_moved: [u64; 4],
    loco_changes: u64,
    final_pops: u64,
    paths_followed: u64,
    cases: u64,
    ended_early: u64,
    /// batch A3: mood targets found (`getNextMoodTarget` answering an object), victims found, enter targets found
    mood_targets: u64,
    victims: u64,
    enter_targets: u64,
    /// batch A4: the log records of codes 70..=81 (the pathfinder's goal, snap, update goal and queue, the turrets' updates, the
    /// commands, the repulsor, the dead state's model condition and script notice, the placeholder states' callbacks); the
    /// current state after an operation (idle, dead, wait, busy, another); temporary states set (continuing); saves loaded
    a4_logs: [u64; 12],
    a4_states: [u64; 5],
    temporary_states: u64,
    loads: u64,
}

impl Checks {
    fn new() -> Self {
        Checks {
            construction: Check::new("AIUpdateInterface construction (log and dump)"),
            results: Check::new("AI operation results"),
            results_bug: Check::new("AI operation results"),
            log: Check::new("AI call logs (every cut-point call)"),
            dump: Check::new("AI, locomotors, path, object after each operation"),
            dump_bug: Check::new("AI blocked frames"),
            ops: [0; 72],
            sleeps: [0; 2],
            goals_moved: [0; 4],
            loco_changes: 0,
            final_pops: 0,
            paths_followed: 0,
            cases: 0,
            ended_early: 0,
            mood_targets: 0,
            victims: 0,
            enter_targets: 0,
            a4_logs: [0; 12],
            a4_states: [0; 5],
            temporary_states: 0,
            loads: 0,
        }
    }
}

/// One case (see the module docs). Returns false if it ended at a mismatch.
fn run_case(seed: u64, ck: &mut Checks) -> bool {
    let mut g = Gen::new(seed);
    ck.cases += 1;
    // the templates, through both INI readers
    let n_templates = 2 + g.below(3) as usize;
    let mut ini = String::new();
    for i in 0..n_templates {
        let appearance = g.below(9) as usize;
        ini.push_str(&gen_template(&mut g, &format!("Loco{i}"), appearance));
    }
    let mut host = Host {
        thing: Thing::new(Some(Arc::new(TplView))).unwrap(),
        env: gen_scene(&mut g),
        status: 0,
        log: Vec::new(),
        next_call_frame: 0,
        map: Vec::new(),
        store: LocomotorStore::new(),
        names: NameKeyGenerator::new(),
        rng: RandomValues::default(),
        ai_stack: Vec::new(),
    };
    host.names.reset();
    unsafe { cpp::ref_ai_a2_reset() };
    let xfer_path = cstr(XFER_FILE_PATH);
    unsafe { cpp::ref_ai_a4_xfer_path(xfer_path.as_ptr()) };
    let mut saved: Vec<u8> = Vec::new();
    let mut has_saved = false;
    {
        let mut ctx = IniCtx { store: &mut host.store, names: &mut host.names };
        let table = blocks();
        Ini::load(b"a2.ini", ini.as_bytes(), IniLoadType::Overwrite, None, &table, &mut ctx).unwrap_or_else(|e| panic!("template rejected: {e:?}\n{ini}"));
    }
    let name = cstr("a2.ini");
    unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(name.as_ptr(), ini.as_ptr() as *const c_char, ini.len() as c_int);
        assert_eq!(cpp::ref_ai_a2_load(name.as_ptr(), IniLoadType::Overwrite as c_int), 0, "C++ rejected the templates:\n{ini}");
    }
    let keys: Vec<NameKeyType> = (0..n_templates).map(|i| host.names.name_to_key(format!("Loco{i}").as_bytes())).collect();
    for (i, k) in keys.iter().enumerate() {
        assert_eq!(unsafe { cpp::ref_loco_name_key(cstr(&format!("Loco{i}")).as_ptr()) }, k.0, "the same keys");
    }

    // the data's locomotor sets
    let mut data = AiUpdateModuleData::default();
    let mut sets_words = vec![0u32];
    {
        let mut map = data.locomotor_templates.write().unwrap();
        for set in [0, 1, 2, 3, 4, 5, 6, 7] {
            let wanted = match set {
                0 => g.below(8) != 0,
                1 => g.below(3) == 0,
                _ => g.below(5) == 0,
            };
            if !wanted {
                continue;
            }
            let n = g.below(4) as usize;
            let chosen: Vec<NameKeyType> = (0..n).map(|_| keys[g.below(keys.len() as u32) as usize]).collect();
            sets_words[0] += 1;
            sets_words.push(set as u32);
            sets_words.push(n as u32);
            sets_words.extend(chosen.iter().map(|k| k.0));
            map.insert(set, chosen);
        }
    }
    // batch A3: the auto-acquire bits, the mood check rate, the turrets
    data.auto_acquire_enemies_when_idle = g.below(32);
    data.mood_attack_check_rate = [0, 1, 60, 61, g.below(300)][g.below(5) as usize];
    sets_words.extend([data.auto_acquire_enemies_when_idle, data.mood_attack_check_rate]);
    for i in 0..2 {
        if (host.env.turrets >> i) & 1 != 0 {
            data.turret_data[i] = Some(Arc::new(rts_lockstep_engine::logic::ai::turret::TurretAiData::default()));
        }
    }
    let data = Arc::new(data);

    // the object and the AI: the constructor draws the locomotors' random values
    let start = [g.uniform(-20.0, 260.0), g.uniform(-20.0, 180.0), 0.0];
    let start_pos = Coord3D { x: start[0], y: start[1], z: host.ground(start[0], start[1]) + if g.below(3) == 0 { g.uniform(0.0, 50.0) } else { 0.0 } };
    let angle = g.uniform(-3.1, 3.1);
    let rng_seed = g.u32();
    let _rng_lock = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { cpp::ref_init_game_logic_random(rng_seed) };
    host.rng.init_game_logic_random(rng_seed);
    host.make_map();
    let _ = host.thing.set_position(&TCtx::new(&host.env), Some(&start_pos)).unwrap();
    let _ = host.thing.set_orientation(&TCtx::new(&host.env), angle).unwrap();
    let mut module = AiUpdateModule::new(&mut host, data);
    if host.env.has_machine != 0 {
        // batch A4: the object's creation makes the machine
        module.on_object_created(&mut host).expect("onObjectCreated");
    }
    let mut clog = vec![0u32; 1 << 16];
    let pos = [start_pos.x, start_pos.y, start_pos.z];
    let n = unsafe { cpp::ref_ai_a2_new(sets_words.as_ptr(), &host.env, pos.as_ptr(), angle, clog.as_mut_ptr(), clog.len() as c_int) };
    clog.truncate(n as usize);
    let rlog = std::mem::take(&mut host.log);
    let rd = rust_dump(&module, &mut host, 0);
    let cd = cpp_dump();
    let ok = rlog == clog && rd == cd;
    if !ok && std::env::var("AI_A2_DEBUG").is_ok() {
        first_diff("construction log", &rlog, &clog);
        first_diff("construction dump", &rd, &cd);
    }
    ck.construction.eq((rlog, rd), (clog, cd), || format!("seed {seed:#x}: construction\n{ini}"));
    if !ok {
        ck.ended_early += 1;
        return false;
    }

    let waypoint_heavy = g.below(8) == 0;
    for step in 0..60 {
        perturb(&mut g, &mut host.env);
        let moving_away = (module.ai().move_out_of_way1, module.ai().move_out_of_way2);
        let (op, mut fa, ia) = gen_op(&mut g, &host.env, waypoint_heavy, moving_away, module.machine.is_some(), has_saved);
        has_saved |= op == 70;
        if op == 27 && fa[0] == -1.0e30 {
            let p = *host.thing.position();
            fa[0] = p.x + g.uniform(-0.6, 0.6);
            fa[1] = p.y + g.uniform(-0.6, 0.6);
            fa[2] = p.z;
        }
        ck.ops[op as usize] += 1;
        let before_loco = module.ai().cur_locomotor;
        let goal = module.ai().locomotor_goal_type;
        let before_pos = *host.thing.position();
        let r = rust_op(&mut module, &mut host, op, &fa, &ia, 0, &mut saved);
        let rlog = std::mem::take(&mut host.log);
        let mut out = vec![0u32; 1 << 16];
        let mut log = vec![0u32; 1 << 16];
        let mut log_len = 0;
        let nr = unsafe { cpp::ref_ai_a2_op(op, fa.as_ptr(), ia.as_ptr(), &host.env, out.as_mut_ptr(), log.as_mut_ptr(), log.len() as c_int, &mut log_len) };
        out.truncate(nr as usize);
        assert!(log_len as usize <= log.len(), "log too long");
        log.truncate(log_len as usize);
        let tag = || format!("seed {seed:#x} step {step} op {op} fa {:?} ia {:?}", &fa[..8], &ia[..8]);
        let mut ok = r == out;
        ck.results.eq(r.clone(), out.clone(), tag);
        if r.len() == 1 && r[0] <= 1 && matches!(op, 1 | 8..=11 | 14 | 22 | 47 | 50..=55) {
            ck.results_bug.eq(rust_op_bug_view(&r), out.clone(), tag);
        }
        ok &= rlog == log;
        let (host_log_copy, log_copy) = (rlog.clone(), log.clone());
        ck.log.eq(rlog, log, tag);
        let rd = rust_dump(&module, &mut host, 0);
        let cd = cpp_dump();
        ok &= rd == cd;
        if !ok && std::env::var("AI_A2_DEBUG").is_ok() {
            println!("{}", tag());
            first_diff("result", &r, &out);
            first_diff("log", &host_log_copy, &log_copy);
            first_diff("dump", &rd, &cd);
        }
        ck.dump_bug.eq(rust_dump(&module, &mut host, 1), cd.clone(), tag);
        ck.dump.eq(rd, cd, tag);
        if op == 0 {
            ck.sleeps[usize::from(r[0] != 1)] += 1;
            if *host.thing.position() != before_pos || module.ai().blocked_frames > 0 {
                ck.goals_moved[goal as usize] += 1;
            }
            if goal == LocoGoalType::PositionOnPath && module.ai().path.is_some() {
                ck.paths_followed += 1;
            }
            if goal == LocoGoalType::None && *host.thing.position() != before_pos {
                ck.final_pops += 1;
            }
        }
        // batch A4's coverage
        let mut k = 0;
        while k + 1 < host_log_copy.len() {
            let (code, n) = (host_log_copy[k], host_log_copy[k + 1] as usize);
            if (70..=81).contains(&code) {
                ck.a4_logs[(code - 70) as usize] += 1;
            }
            k += 2 + n;
        }
        if let Some(sm) = &module.machine {
            ck.a4_states[match sm.get_current_state_id() {
                0 => 0,
                13 => 1,
                8 => 2,
                41 => 3,
                _ => 4,
            }] += 1;
        }
        match op {
            60 if r[0] == 0 => ck.temporary_states += 1,
            71 => ck.loads += 1,
            48 if r[0] != 0 => ck.mood_targets += 1,
            35 if r[0] != 0 => ck.victims += 1,
            41 if r[0] != 0 => ck.enter_targets += 1,
            _ => {}
        }
        if module.ai().cur_locomotor != before_loco {
            ck.loco_changes += 1;
        }
        if !ok {
            ck.ended_early += 1;
            return false;
        }
    }
    true
}

/// Prints where two word arrays first differ (`AI_A2_DEBUG`).
fn first_diff(what: &str, r: &[u32], c: &[u32]) {
    if r == c {
        return;
    }
    let i = r.iter().zip(c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
    let lo = i.saturating_sub(6);
    println!("{what}: lengths {} / {}, first difference at {i}", r.len(), c.len());
    println!("  rust {:?}", &r[lo..(i + 8).min(r.len())]);
    println!("  cpp  {:?}", &c[lo..(i + 8).min(c.len())]);
}

/// A boolean result flipped (the planted bug of the results).
fn rust_op_bug_view(r: &[u32]) -> Vec<u32> {
    r.iter().map(|x| x ^ 1).collect()
}

/// # Results (2026-10-27, batch A4; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 10,000 cases)
///
/// - 10,000 constructions and 600,000 operations, a third of them batch A4's (51,290 updates, 36,661 `setState`, 18,306
///   `setTemporaryState` of which 11,147 continued, 7,886 saves and 1,239 loads): **0 mismatches** in the results, the call logs and
///   the dumps. After an operation the machine was idle 298,017 times, dead 58,367, waiting 25,084, busy 24,299, in a placeholder
///   state 192,942. Reached through the states and the update: 4,773 dead entries (the model condition cleared and set, the script
///   notice), 4,813 repulsor checks, 434 commands from the idle state (crate pick-ups and mood attacks), 8,869 goal updates, 921
///   snaps, 1,540 path queues, 11,648 turret updates, 36,321 placeholder `onEnter`s, 11,485 `update`s and 32,589 `onExit`s.
/// - Planted bugs: in the Rust source plants 3240-3247, each caught: the AI not told of a changed state, the temporary state's
///   minute not clamped, the idle state not inited on entry, its sleep offset not kept, the dead state's model condition list one
///   short, the dead infantry's slowing factor, the attack flag of the attack states, the goal path's repeated point. A2's and
///   A3's plants re-run against this mix: all caught (3201 by a panic, as before; 3210 at 10,000 cases) but 3222 (the get-repaired
///   state as an enter target), which the base machine cannot reach (it defines no `AI_GET_REPAIRED` state; subclasses' machines
///   do) and which A3's run caught.
/// - The first run found no mismatch; one harness defect (the result buffer of 16 words, which a save overflowed).
///
/// # Results (2026-10-27, batch A3; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 10,000 cases)
///
/// - 10,000 constructions and 600,000 operations, half of them batch A3's (about 10,600 of each of its 26): **0 mismatches** in the
///   results, the call logs and the dumps. Reached: 1,359 mood targets, 1,489 victims, 1,582 enter targets.
/// - Planted bugs: the result flip (124,159 of 124,159), the blocked frames (18,240 of 600,000); in the Rust source plants
///   3220-3229, each caught: the crate looked up by its id (upstream clears it first), the targeter not told, the get-repaired state
///   not an enter target, the turret ignored by the mood matrix, an aggressive move adjusted as alert, the random mood offset not
///   asked for, its range one wider, the human's unfogged qualifier dropped, the horde's nationalism veto ignored, the second
///   move-out-of-the-way id ignored (3229 survived the first run: nothing set those ids; op 58 sets them). A2's plants 3200-3214
///   re-run against this mix: all caught.
/// - Upstream defects kept: `checkForCrateToPickup` clears the crate's id before looking it up, so it never finds the crate;
///   `getNextMoodTarget` reads the controlling player unchecked when called by the AI (not generated without one).
///
/// # Results (2026-10-27, batch A2; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 10,000 cases)
///
/// - 10,000 constructions and 600,000 operations (165,208 `doLocomotor`, 38,505 new paths, 27,911 distances to the goal, ...):
///   **0 mismatches** in the results, the call logs and the dumps. No case ended early.
/// - Reached: `doLocomotor` sleeping forever 40,441 times and not 124,767; the object moved by the goal none (the final position's
///   approach and pop, 5,803 moves), on the path (3,651; 22,027 frames following a path), explicit (5,462) and angle (1,017); the
///   current locomotor changed 3,614 times.
/// - Planted bugs: a boolean result flipped (86,905 of 86,905) and the blocked frames left out of the dump (21,177 of 600,000), and
///   in the Rust source plants 3200-3214, each caught (3201 by a panic: the changed locomotor is then none): the upgraded set
///   ignored, the previous locomotor not kept, the ultra-accurate flag not reset, `wakeUpNow` without effect, wings not adjusting
///   the destination, the bump decay, the final position's "darn close" distance, the held check, the hover rule of the distance
///   to the goal, `setQueueForPathTime`'s `>= UPDATE_SLEEP_NONE`, the blocked frames' reset, the current locomotor kept across a
///   new set, a waypoint queue one shorter, the dead executing the queue, the airborne-target height. 3203, 3209 and 3212
///   survived the first run (nothing set a wake frame beyond the next frame, no queue filled); op 32 (the update's sleep) and the
///   waypoint-heavy programs were added for them.
#[test]
fn ai_update_a2_matches_upstream() {
    let cases: u64 = std::env::var("AI_A2_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(10_000);
    let mut ck = Checks::new();
    for case in 0..cases {
        run_case(0xA2_0000 + case, &mut ck);
    }
    println!(
        "COVERAGE: {} cases ({} ended at a mismatch); ops {:?}; doLocomotor sleeps none/longer {:?}; moved by goal none/path/explicit/angle {:?}; \
         path frames {}; final-position moves {}; locomotor changes {}; mood targets {}, victims {}, enter targets {}; \
         A4 log records 70..=81 {:?}; current state idle/dead/wait/busy/other {:?}; temporary states {}; loads {}",
        ck.cases, ck.ended_early, ck.ops, ck.sleeps, ck.goals_moved, ck.paths_followed, ck.final_pops, ck.loco_changes, ck.mood_targets, ck.victims,
        ck.enter_targets, ck.a4_logs, ck.a4_states, ck.temporary_states, ck.loads
    );
    ck.construction.finish();
    ck.results.finish();
    ck.log.finish();
    ck.dump.finish();
    ck.results_bug.finish_planted();
    ck.dump_bug.finish_planted();
}
