// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of layer 10 batches A2 to A7, `logic::ai::ai_update` (`AIUpdateInterface`'s members) and
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
//! Batch A5 adds the commands: a quarter of the operations are `aiDoCommand` of random parms (any command, its source, an object,
//! the team, a waypoint, a polygon, an int value, a command button, coords; what upstream dereferences unchecked is always given)
//! and `doQuickExit`, over a scene that adds the map's extent, the team's members, the action manager's answers, the other AIs'
//! paths, the move-away path and quick path rates, the weapon's continue range and contact flag, the partition's closest
//! attackable object, the position finder, the container, the special modules and the command set. The dump adds the area to
//! guard, the goal waypoint and the goal squad.
//!
//! Batch A6 adds the paths: the four requests, `doPathfind` (with a stand-in `PathfindServicesInterface` whose searches make a
//! path from the hash or none), `setPathFromWaypoint` over linked waypoints, `computePath`, `computeAttackPath` (with or without a
//! victim or its position), the quick paths, `friend_setGoalObject`, `isQuickPathAvailable` and the current locomotor's
//! ultra-accurate flag; the scene adds the pathfinding extent, the searches', answers' and weapon ranges' rates, the repulsed
//! distance, the waypoints' links and bridges. The dump adds the pathfinder's ignored obstacle.
//!
//! Batch A7 makes the move states real (setState draws them often); the scene adds the template's move sounds, the formation,
//! the group's speed and the objects' height above their position. The dump adds the four move states' members.
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
    KINDOF_PRODUCED_AT_HELIPAD, KINDOF_PROJECTILE, KINDOF_CAN_BE_REPULSED, KINDOF_BRIDGE, KINDOF_PORTABLE_STRUCTURE, KINDOF_CAN_ATTACK,
    KINDOF_MINE, KINDOF_SPAWNS_ARE_THE_WEAPONS, KINDOF_STRUCTURE, KINDOF_AIRCRAFT, KINDOF_DONT_AUTO_CRUSH_INFANTRY,
    KINDOF_CAN_RAPPEL,
};
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::WeaponSlotType;
use rts_lockstep_engine::logic::ai::ai_attack_states::AttackFrame;
use rts_lockstep_engine::logic::weapon_types::WeaponStatus;
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
use rts_lockstep_engine::logic::ai::ai_commands::AiCommandParms;
use rts_lockstep_engine::logic::ai::ai_paths;
use rts_lockstep_engine::logic::ai::ai_states::{self as states, AiState, AiStateHost, AiUpdateModule};
use rts_lockstep_engine::logic::ai::ai_waypoint_states::WaypointPathData;
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
const L_REPULSOR: u32 = 76;
const L_CLEAR_SET_MODEL: u32 = 77;
const L_SCRIPT_NOTIFY: u32 = 78;
const L_STATE_ENTER: u32 = 79;
const L_STATE_UPDATE: u32 = 80;
const L_STATE_EXIT: u32 = 81;
const L_EDGE_POINT: u32 = 82;
const L_ACTION: u32 = 83;
const L_MOVE_AWAY_PATH: u32 = 84;
const L_MOVE_ALLIES: u32 = 85;
const L_PATH_EXISTS: u32 = 86;
const L_MAX_SHOTS: u32 = 87;
const L_CLOSEST_OBJECT: u32 = 88;
const L_FIND_POSITION: u32 = 89;
const L_CONTAIN: u32 = 90;
const L_MODULE: u32 = 91;
const L_SERVICE: u32 = 92;
const L_PF_ANSWER: u32 = 93;
const L_WEAPON_RANGE: u32 = 94;
const L_AUDIO_ADD: u32 = 95;
const L_AUDIO_REMOVE: u32 = 96;
const L_TEAM_ACTIVE: u32 = 97;
const L_DESTROY_OBJECT: u32 = 98;
const L_TEAM_WAYPOINT: u32 = 99;
const L_GROUP_CENTER: u32 = 100;
const L_TEAM_GROUP: u32 = 101;
const L_SKIRMISH: u32 = 102;
const L_ATTACK: u32 = 103;
/// Batch A15: the rappel state's calls (`LOG_RAPPEL`).
const L_RAPPEL: u32 = 104;

/// The machine's xfer file of the C++ side (`ref_ai_a4_xfer_path`): one per process, so that runs in parallel (the planted bugs)
/// do not load each other's saves.
fn xfer_file_path() -> String {
    format!("{}/ai_a4_{}.bin", env!("CARGO_TARGET_TMPDIR"), std::process::id())
}

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
    /// Batch A6: the pathfinder's ignored obstacle (`m_ignoreObstacleID`).
    pf_ignore: ObjectId,
    /// Batch A7: TheAudio's next handle (from 1 in each case).
    next_audio: u32,
    /// Batch A9: the team's current waypoint.
    team_waypoint: Option<u32>,
    /// Batch A10a: the attack frames lent while attack machines run; the current weapon's max shot count.
    attack_frames: Vec<AttackFrame>,
    max_shots: i32,
    /// The other objects whose height above the terrain this operation has asked (wrap_ai.cpp makes them anew every operation;
    /// a second question is answered from the object's cache, unlogged).
    others_height_asked: u32,
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
            || (kind == KINDOF_BRIDGE && m & 256 != 0)
            || (kind == KINDOF_PORTABLE_STRUCTURE && m & 512 != 0)
            || (kind == KINDOF_CAN_ATTACK && m & 1024 != 0)
            || (kind == KINDOF_MINE && m & 2048 != 0)
            || (kind == KINDOF_SPAWNS_ARE_THE_WEAPONS && m & 4096 != 0)
            || (kind == KINDOF_STRUCTURE && m & 8192 != 0)
            || (kind == KINDOF_AIRCRAFT && m & 16384 != 0)
            || (kind == KINDOF_DONT_AUTO_CRUSH_INFANTRY && m & 32768 != 0)
            || (kind == KINDOF_CAN_RAPPEL && m & 65536 != 0)
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
    fn find_closest_enemy(&mut self, range: f32, qualifiers: u32, info: Option<usize>, area: Option<i32>) -> ObjectId {
        let id = self.env.object_id;
        let (info, area) = (info.map_or(0, |i| i as u32 + 1), area.map_or(0, |a| a as u32 + 1));
        self.l(L_CLOSEST_ENEMY, &[id as u32, range.to_bits(), qualifiers, info, area]);
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
    /// `TheAudio->addAudioEvent(event)`: logged with the sound, its object and the new handle.
    fn audio_add(&mut self, k: u32) -> u32 {
        let h = self.next_audio;
        self.next_audio += 1;
        let id = self.env.object_id as u32;
        self.l(L_AUDIO_ADD, &[k, id, h]);
        h
    }

    /// A search's path (`standinPath` of `wrap_ai.cpp`): from the object to `to` offset by the hash, sometimes blocked by an
    /// ally; or none.
    fn standin_path(&mut self, kind: u32, to: &Coord3D, words: &[u32]) -> Option<Path> {
        let h = p1_hash(self.env.seed, 17 + kind, words);
        let found = pick(h, self.env.service_rate);
        let mut w = vec![kind];
        w.extend_from_slice(words);
        w.push(u32::from(found));
        self.l(L_SERVICE, &w);
        if !found {
            return None;
        }
        let a = *self.thing.position();
        let mut b = *to;
        b.x += (((h >> 12) % 9) as i32 - 4) as f32;
        b.y += (((h >> 16) % 9) as i32 - 4) as f32;
        let mut path = Path::new();
        path.append_node(&a, LAYER_GROUND);
        path.append_node(&b, LAYER_GROUND);
        if (h >> 28) & 1 != 0 {
            path.set_blocked_by_ally(true);
        }
        Some(path)
    }

    /// The pathfinder's answers (`pfAnswer` of `wrap_ai.cpp`).
    fn pf_answer(&mut self, kind: u32, words: &[u32]) -> bool {
        let r = pick(p1_hash(self.env.seed, 30 + kind, words), self.env.pf_answer_rate);
        let mut w = vec![kind];
        w.extend_from_slice(words);
        w.push(u32::from(r));
        self.l(L_PF_ANSWER, &w);
        r
    }

    /// Batch A10a: the objects' answers (`objectAnswer` of `wrap_ai.cpp`).
    fn object_answer(&self, kind: u32, words: &[u32]) -> bool {
        pick(p1_hash(self.env.seed, 60 + kind, words), self.env.object_rate)
    }

    /// The weapon's range answers (`rangeAnswer` of `wrap_ai.cpp`).
    fn range_answer(&mut self, kind: u32, words: &[u32]) -> bool {
        let r = pick(p1_hash(self.env.seed, 40 + kind, words), self.env.range_rate);
        let mut w = vec![kind];
        w.extend_from_slice(words);
        w.push(u32::from(r));
        self.l(L_WEAPON_RANGE, &w);
        r
    }

    /// The action manager's answer (`actionAnswer` of `wrap_ai.cpp`): the hash of the question, the target and the source.
    fn action_answer(&mut self, kind: u32, target: ObjectId, src: CommandSourceType) -> bool {
        let id = if self.object_exists(target) { target as u32 } else { 0 };
        let r = pick(p1_hash(self.env.seed, 13, &[kind, id, src as u32]), self.env.action_rate);
        self.l(L_ACTION, &[kind, id, src as u32, u32::from(r)]);
        r
    }

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
    // ---- batch A5
    fn is_mobile(&self) -> bool {
        self.env.mobile != 0
    }
    fn partition_cell_size(&self) -> f32 {
        self.env.partition_cell_size
    }
    fn is_significantly_above_terrain(&mut self) -> bool {
        // Thing's: `getHeightAboveTerrain() > -(3*3)*TheGlobalData->m_gravity`
        let g = self.env.loco.gravity;
        self.height_above_terrain() > (-(3 * 3)) as f32 * g
    }
    fn terrain_extent(&self) -> (Coord3D, Coord3D) {
        (Coord3D::default(), Coord3D { x: self.env.extent_hi[0], y: self.env.extent_hi[1], z: 0.0 })
    }
    fn terrain_find_closest_edge_point(&mut self, pos: &Coord3D) -> Coord3D {
        self.l(L_EDGE_POINT, &[pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
        let mut r = *pos;
        if r.x < 0.0 {
            r.x = 0.0;
        }
        if r.x > self.env.extent_hi[0] {
            r.x = self.env.extent_hi[0];
        }
        if r.y < 0.0 {
            r.y = 0.0;
        }
        if r.y > self.env.extent_hi[1] {
            r.y = self.env.extent_hi[1];
        }
        r
    }
    fn team_member_ids(&mut self, _team: u32) -> Vec<ObjectId> {
        (1..32).filter(|i| (self.env.team_mask >> i) & 1 != 0).collect()
    }
    fn action_can_get_healed_at(&mut self, depot: ObjectId, src: CommandSourceType) -> bool {
        self.action_answer(1, depot, src)
    }
    fn action_can_get_repaired_at(&mut self, depot: ObjectId, src: CommandSourceType) -> bool {
        self.action_answer(0, depot, src)
    }
    fn action_can_enter_object(&mut self, target: ObjectId, src: CommandSourceType) -> bool {
        self.action_answer(2, target, src)
    }
    fn object_is_disabled_by_type(&self, _id: ObjectId, t: DisabledType) -> bool {
        // (the scene's disabled types are every object's)
        self.is_disabled_by_type(t)
    }
    fn object_ai_has_path(&self, id: ObjectId) -> bool {
        self.object_exists(id) && (self.env.ai_mask >> id) & 1 != 0 && self.env.other_ai_path != 0
    }
    fn pf_move_away_from_path(&mut self, unit: ObjectId, other: ObjectId, other_has_path: bool) -> Option<Path> {
        let can_path = u32::from(self.ai_stack.last().expect("a running AI").can_path_through_units);
        let h = p1_hash(self.env.seed, 14, &[unit as u32, other as u32, u32::from(other_has_path), can_path]);
        let found = pick(h, self.env.move_away_rate);
        self.l(L_MOVE_AWAY_PATH, &[unit as u32, other as u32, u32::from(other_has_path), can_path, u32::from(found)]);
        if !found {
            return None;
        }
        let a = *self.thing.position();
        let mut b = a;
        b.x += (((h >> 12) % 41) as i32 - 20) as f32;
        b.y += (((h >> 20) % 41) as i32 - 20) as f32;
        let mut path = Path::new();
        path.append_node(&a, LAYER_GROUND);
        path.append_node(&b, LAYER_GROUND);
        Some(path)
    }
    fn pf_move_allies(&mut self, path: &Path) {
        let n = path.list().len() as u32;
        self.l(L_MOVE_ALLIES, &[n]);
    }
    fn is_path_available(&mut self, d: &Coord3D) -> bool {
        let r = pick(p1_hash(self.env.seed, 15, &[d.x.to_bits(), d.y.to_bits()]), self.env.path_exists_rate);
        self.l(L_PATH_EXISTS, &[d.x.to_bits(), d.y.to_bits(), d.z.to_bits(), u32::from(r)]);
        r
    }
    fn weapon_set_max_shot_count(&mut self, n: i32) {
        if self.env.weapon_anti >= 0 {
            self.l(L_MAX_SHOTS, &[n as u32]);
            self.max_shots = n;
        }
    }
    fn weapon_continue_attack_range(&self) -> f32 {
        if self.env.weapon_anti >= 0 { self.env.continue_range } else { 0.0 }
    }
    fn weapon_is_contact(&self) -> bool {
        self.env.weapon_anti >= 0 && self.env.contact_weapon != 0
    }
    fn partition_closest_attack_target(&mut self, pos: &Coord3D, range: f32, src: CommandSourceType) -> ObjectId {
        self.l(L_CLOSEST_OBJECT, &[pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), range.to_bits(), 0, src as u32]);
        let c = self.env.closest_attack;
        if self.object_exists(c) { c } else { 0 }
    }
    fn find_position_around(&mut self, pos: &Coord3D, min_radius: f32, max_radius: f32) -> Option<Coord3D> {
        let r = pick(p1_hash(self.env.seed, 16, &[pos.x.to_bits(), pos.y.to_bits()]), self.env.find_pos_rate);
        let id = self.env.object_id as u32;
        self.l(L_FIND_POSITION, &[pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), min_radius.to_bits(), max_radius.to_bits(), id, u32::from(r)]);
        r.then_some(Coord3D { x: pos.x + 5.0, y: pos.y + 5.0, z: pos.z })
    }
    fn has_contain(&self) -> bool {
        self.env.has_contain != 0
    }
    fn contain_idle_passengers(&mut self, _src: CommandSourceType) {
        // (the stand-in container lists no passengers)
    }
    fn contain_remove_all_contained(&mut self) {
        self.l(L_CONTAIN, &[0, 0]);
    }
    fn contain_mark_all_passengers_detected(&mut self) {
        self.l(L_CONTAIN, &[1]);
    }
    fn contain_order_all_passengers_to_exit(&mut self, src: CommandSourceType, instantly: bool) {
        self.l(L_CONTAIN, &[2, src as u32, u32::from(instantly)]);
    }
    fn deliver_payload_via_module_data(&mut self, pos: &Coord3D) -> bool {
        if self.env.modules & 1 == 0 {
            return false;
        }
        self.l(L_MODULE, &[0, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
        true
    }
    fn go_prone(&mut self) {
        if self.env.modules & 2 != 0 {
            self.l(L_MODULE, &[1]);
        }
    }
    fn hack_internet(&mut self) {
        if self.env.modules & 4 != 0 {
            self.l(L_MODULE, &[2]);
        }
    }
    fn polygon_center(&mut self, polygon: i32) -> Coord3D {
        Coord3D { x: 10.0 * polygon as f32, y: 20.0 * polygon as f32, z: 0.0 }
    }
    fn command_set_button_matches(&mut self, button: u32) -> (u32, i32) {
        let n = if self.env.has_command_set != 0 { self.env.command_slots.iter().filter(|&&k| k == button as i32).count() as u32 } else { 0 };
        (n, self.env.button_types[button as usize])
    }
    // ---- batch A6
    fn waypoint_location(&mut self, way: u32) -> Coord3D {
        Coord3D { x: 15.0 * way as f32, y: 25.0 * way as f32, z: 1.0 }
    }
    fn waypoint_link0(&mut self, way: u32) -> Option<u32> {
        self.waypoint_link(way, 0)
    }
    fn pf_get_aircraft_path(&mut self, d: &Coord3D) -> Option<Path> {
        self.standin_path(5, d, &[d.x.to_bits(), d.y.to_bits(), d.z.to_bits()])
    }
    fn terrain_max_pathfind_extent(&mut self) -> (Coord3D, Coord3D) {
        (Coord3D::default(), Coord3D { x: self.env.pf_extent_hi[0], y: self.env.pf_extent_hi[1], z: 0.0 })
    }
    fn pf_move_allies_away_from_destination(&mut self, d: &Coord3D) {
        self.pf_answer(0, &[d.x.to_bits(), d.y.to_bits(), d.z.to_bits()]);
    }
    fn pf_set_ignore_obstacle_id(&mut self, id: ObjectId) {
        self.pf_ignore = id;
    }
    fn pf_patch_path(&mut self, path: Option<&Path>, blocked: bool) -> Option<Path> {
        let pos = *self.thing.position();
        self.standin_path(3, &pos, &[u32::from(path.is_some()), u32::from(blocked)])
    }
    fn pf_find_path(&mut self, d: &Coord3D) -> Option<Path> {
        self.standin_path(0, d, &[d.x.to_bits(), d.y.to_bits(), d.z.to_bits()])
    }
    fn pf_find_closest_path(&mut self, d: &Coord3D, blocked: bool, cost: f32, move_allies: bool) -> Option<Path> {
        self.standin_path(1, d, &[d.x.to_bits(), d.y.to_bits(), d.z.to_bits(), u32::from(blocked), cost.to_bits(), u32::from(move_allies)])
    }
    fn pf_find_safe_path(&mut self, p1: &Coord3D, p2: &Coord3D, radius: f32) -> Option<Path> {
        let pos = *self.thing.position();
        self.standin_path(4, &pos, &[p1.x.to_bits(), p1.y.to_bits(), p2.x.to_bits(), p2.y.to_bits(), radius.to_bits()])
    }
    fn pf_find_attack_path(&mut self, victim: ObjectId, vp: &Coord3D) -> Option<Path> {
        self.standin_path(2, vp, &[victim as u32, vp.x.to_bits(), vp.y.to_bits(), vp.z.to_bits()])
    }
    fn pf_adjust_to_possible_destination(&mut self, d: &mut Coord3D) {
        let id = self.env.object_id as u32;
        if self.pf_answer(1, &[id, d.x.to_bits(), d.y.to_bits(), d.z.to_bits()]) {
            d.x += 2.0;
            d.y -= 2.0;
        }
    }
    fn pf_adjust_destination(&mut self, d: &mut Coord3D) -> bool {
        let r = self.pf_answer(2, &[d.x.to_bits(), d.y.to_bits(), d.z.to_bits(), 0]);
        if r {
            d.x -= 3.0;
            d.y += 3.0;
        }
        r
    }
    fn pf_quick_path_exists_for_ui(&mut self, d: &Coord3D) -> bool {
        self.pf_answer(3, &[d.x.to_bits(), d.y.to_bits(), d.z.to_bits()])
    }
    fn pf_is_attack_view_blocked(&mut self, victim: ObjectId, vp: &Coord3D) -> bool {
        let p = *self.thing.position();
        self.pf_answer(4, &[victim as u32, p.x.to_bits(), p.y.to_bits(), vp.x.to_bits(), vp.y.to_bits()])
    }
    fn ai_data_repulsed_distance(&self) -> f32 {
        self.env.repulsed_distance
    }
    fn weapon_is_within_attack_range_of_object(&mut self, victim: ObjectId) -> bool {
        self.range_answer(0, &[victim as u32])
    }
    fn weapon_is_within_attack_range_of_position(&mut self, p: &Coord3D) -> bool {
        self.range_answer(1, &[p.x.to_bits(), p.y.to_bits(), p.z.to_bits()])
    }
    fn weapon_compute_approach_target(&mut self, victim: ObjectId, pos: &Coord3D) -> Coord3D {
        let s = *self.thing.position();
        let mut r = *pos;
        r.x += (s.x - pos.x) * 0.25;
        r.y += (s.y - pos.y) * 0.25;
        self.range_answer(3, &[victim as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), 0.0f32.to_bits()]);
        r
    }
    fn weapon_is_goal_pos_within_attack_range(&mut self, g: &Coord3D, victim: ObjectId, vp: &Coord3D) -> bool {
        self.range_answer(2, &[g.x.to_bits(), g.y.to_bits(), g.z.to_bits(), victim as u32, vp.x.to_bits(), vp.y.to_bits()])
    }
    fn object_is_significantly_above_terrain(&mut self, id: ObjectId) -> bool {
        // another object of the scene: at the origin, its height asked anew (wrap_ai.cpp resets them every operation; then cached)
        let mut t = Thing::new(Some(Arc::new(TplView))).unwrap();
        let ctx = TCtx::new(&self.env);
        let height = t.height_above_terrain(&ctx);
        let queries = ctx.queries.into_inner();
        let bit = 1u32 << (id as u32 & 31);
        if self.others_height_asked & bit == 0 {
            self.others_height_asked |= bit;
            for (x, y) in queries {
                self.l(L_GROUND_HEIGHT, &[x.to_bits(), y.to_bits()]);
            }
        }
        height > (-(3 * 3)) as f32 * self.env.loco.gravity
    }
    fn object_is_kind_of(&self, _id: ObjectId, kind: KindOfType) -> bool {
        // (the scene's kinds are every object's)
        self.is_kind_of(kind)
    }
    fn bridge_attack_points(&mut self, bridge: ObjectId) -> (Coord3D, Coord3D) {
        let i = bridge as f32;
        (Coord3D { x: 7.0 * i, y: 3.0 * i, z: 0.0 }, Coord3D { x: 3.0 * i, y: 7.0 * i, z: 0.0 })
    }
    fn distance_squared_bounding_sphere_3d(&mut self, pos: &Coord3D) -> f32 {
        self.distance_squared_3d(pos)
    }
    // ---- batch A7
    fn pf_cell_at(&mut self, _layer: PathfindLayer, x: i32, y: i32) -> Option<(u8, bool)> {
        // (the scene's layers have no cells of their own: the ground's)
        self.cell(x, y).map(|c| self.map[c])
    }
    fn pf_snap_closest_goal_position(&mut self, p: &mut Coord3D) {
        if self.pf_answer(5, &[p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]) {
            p.x += 1.0;
        }
    }
    fn pf_remove_goal(&mut self) {
        self.pf_answer(6, &[]);
    }
    fn start_move_sound(&mut self, damaged: bool) -> Option<u32> {
        // the template's sounds: bit k of the scene's set is "sk" not empty
        let has = |k: i32| (self.env.sounds >> k) & 1 != 0;
        let (start, looped) = if damaged { (1, 3) } else { (0, 2) };
        if has(start) {
            self.audio_add(start as u32);
            None
        } else if has(looped) {
            Some(self.audio_add(looped as u32))
        } else {
            None
        }
    }
    fn remove_audio_event(&mut self, handle: u32) {
        self.l(L_AUDIO_REMOVE, &[handle]);
    }
    // ---- batch A10a
    fn attack_stack(&mut self) -> &mut Vec<AttackFrame> {
        &mut self.attack_frames
    }
    fn object_contained_by(&self, _id: ObjectId) -> ObjectId {
        0 // (the other objects are in nothing)
    }
    fn ai_data_crushes_infantry(&self) -> bool {
        self.env.crushes_infantry != 0
    }
    fn can_crush_or_squish(&mut self, victim: ObjectId) -> bool {
        // (TEST_CRUSH_OR_SQUISH)
        self.object_answer(2, &[self.env.object_id as u32, victim as u32, 2])
    }
    fn current_weapon_slot(&self) -> Option<WeaponSlotType> {
        (self.env.weapon_anti >= 0).then(|| slot_of(self.env.cur_slot))
    }
    fn weapon_slot_present(&self, slot: WeaponSlotType) -> bool {
        (self.env.slot_mask >> slot as i32) & 1 != 0
    }
    fn object_is_airborne_target(&mut self, id: ObjectId) -> bool {
        self.object_answer(0, &[id as u32])
    }
    fn is_able_to_attack(&self) -> bool {
        self.env.able_to_attack != 0
    }
    fn is_out_of_ammo(&self) -> bool {
        self.env.out_of_ammo != 0
    }
    fn is_cur_weapon_locked(&self) -> bool {
        self.env.cur_weapon_locked != 0
    }
    fn has_any_weapon(&self) -> bool {
        self.env.has_any_weapon != 0
    }
    fn turret_holds_current_weapon(&self, which: usize) -> bool {
        (self.env.turret_cur_weapon >> which) & 1 != 0
    }
    fn turret_set_target_object(&mut self, which: usize, o: Option<ObjectId>, force: bool) {
        self.l(L_ATTACK, &[14, which as u32, o.unwrap_or(0) as u32, u32::from(force)]);
    }
    fn turret_set_target_position(&mut self, which: usize, pos: Option<&Coord3D>) {
        match pos {
            Some(p) => self.l(L_ATTACK, &[15, which as u32, p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]),
            None => self.l(L_ATTACK, &[15, which as u32]),
        }
    }
    fn turret_turn_rate(&self, which: usize) -> f32 {
        self.env.turret_turn_rate[which]
    }
    fn choose_best_weapon_for_target(&mut self, target: Option<ObjectId>, criteria: i32, src: CommandSourceType) -> bool {
        let id = target.unwrap_or(0) as u32;
        let r = self.object_answer(6, &[id, criteria as u32, src as u32]);
        self.l(L_ATTACK, &[8, id, criteria as u32, src as u32, u32::from(r)]);
        r
    }
    fn adjust_model_condition_for_weapon_status(&mut self) {
        self.l(L_ATTACK, &[9]);
    }
    fn clear_leech_range_mode_for_all_weapons(&mut self) {
        self.l(L_ATTACK, &[10]);
    }
    fn object_is_effectively_dead(&self, _id: ObjectId) -> bool {
        self.env.effectively_dead != 0 // (the scene's flag is every object's)
    }
    fn object_is_destroyed(&mut self, id: ObjectId) -> bool {
        self.object_answer(1, &[id as u32])
    }
    fn object_team(&self, id: ObjectId) -> Option<u32> {
        // (one of two teams, by the hash of the id and the scene's epoch: wrap_ai.cpp's)
        Some((p1_hash(self.env.seed, 68, &[id as u32, self.env.team_epoch as u32]) >> 8) & 1)
    }
    fn object_test_status(&self, _id: ObjectId, _bit: ObjectStatusTypes) -> bool {
        false // (only the AI's own object has statuses)
    }
    fn object_has_contain(&self, _id: ObjectId) -> bool {
        true // (every other object has the scene's contain)
    }
    fn object_relationship(&mut self, that: ObjectId) -> Relationship {
        match p1_hash(self.env.seed, 63, &[self.env.object_id as u32, that as u32]) % 3 {
            0 => Relationship::Enemies,
            1 => Relationship::Neutral,
            _ => Relationship::Allies,
        }
    }
    fn set_status_bits(&mut self, bits: &[ObjectStatusTypes], set: bool) {
        // (the stand-in logs the lowest bit of the mask)
        let low = *bits.iter().min().expect("a bit");
        self.l(L_SET_STATUS, &[low as u32, u32::from(set)]);
        for &b in bits {
            if set {
                self.status |= 1 << b;
            } else {
                self.status &= !(1 << b);
            }
        }
    }
    fn set_team_target_object(&mut self, target: ObjectId) {
        self.l(L_ATTACK, &[11, target as u32]);
    }
    fn contain_is_garrisonable(&self) -> bool {
        self.env.garrisonable != 0
    }
    fn contain_count(&self) -> u32 {
        self.env.contain_count as u32
    }
    fn contain_is_enclosing_container_for_me(&mut self) -> bool {
        self.object_answer(4, &[self.env.object_id as u32])
    }
    fn contain_attempt_best_fire_point_at_object(&mut self, victim: ObjectId) -> bool {
        let r = self.object_answer(5, &[victim as u32]);
        self.l(L_ATTACK, &[12, victim as u32, u32::from(r)]);
        r
    }
    fn contain_attempt_best_fire_point_at_position(&mut self, p: &Coord3D) -> bool {
        let r = self.object_answer(5, &[p.x.to_bits(), p.y.to_bits()]);
        self.l(L_ATTACK, &[12, p.x.to_bits(), p.y.to_bits(), u32::from(r)]);
        r
    }
    fn relative_angle_2d_to_object(&mut self, other: ObjectId) -> f32 {
        self.l(L_ATTACK, &[13, other as u32]);
        self.env.rel_angle
    }
    fn relative_angle_2d_to_position(&mut self, p: &Coord3D) -> f32 {
        self.l(L_ATTACK, &[13, p.x.to_bits(), p.y.to_bits()]);
        self.env.rel_angle
    }
    fn weapon_max_shot_count(&self) -> i32 {
        self.max_shots
    }
    fn weapon_aim_delta(&self) -> f32 {
        self.env.aim_delta
    }
    fn weapon_status_now(&self, _slot: WeaponSlotType) -> WeaponStatus {
        match self.env.weapon_status {
            0 => WeaponStatus::ReadyToFire,
            1 => WeaponStatus::OutOfAmmo,
            2 => WeaponStatus::BetweenFiringShots,
            3 => WeaponStatus::ReloadingClip,
            _ => WeaponStatus::PreAttack,
        }
    }
    fn weapon_has_leech_range_now(&self, _slot: WeaponSlotType) -> bool {
        self.env.leech_range != 0
    }
    fn weapon_fire_at(&mut self, slot: WeaponSlotType, p: &Coord3D) -> bool {
        let r = self.object_answer(7, &[slot as u32, p.x.to_bits(), p.y.to_bits()]);
        self.l(L_ATTACK, &[1, slot as u32, p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), u32::from(r)]);
        r
    }
    fn weapon_set_pre_attack_finished(&mut self, slot: WeaponSlotType, frame: u32) {
        self.l(L_ATTACK, &[0, slot as u32, frame]);
    }
    fn set_firing_condition_for_current_weapon(&mut self) {
        self.l(L_ATTACK, &[5]);
    }
    fn pre_fire_current_weapon(&mut self, victim: Option<ObjectId>) {
        self.l(L_ATTACK, &[4, victim.unwrap_or(0) as u32]);
    }
    fn fire_current_weapon_at_object(&mut self, victim: Option<ObjectId>) {
        self.l(L_ATTACK, &[6, victim.unwrap_or(0) as u32]);
    }
    fn fire_current_weapon_at_position(&mut self, p: &Coord3D) {
        self.l(L_ATTACK, &[7, p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
    }
    fn release_weapon_lock(&mut self, lock: i32) {
        self.l(L_ATTACK, &[2, lock as u32]);
    }
    // ---- batch A12
    fn units_should_hunt(&self) -> Option<bool> {
        (self.env.player_type >= 0).then_some(self.env.units_hunt != 0)
    }
    // ---- batch A13
    fn contain_wants_to_enter_or_exit(&mut self, _container: ObjectId, wants: i32) {
        let id = self.env.object_id as u32;
        self.l(L_CONTAIN, &[3, id, wants as u32]);
    }
    fn contain_has_exit_interface(&mut self, _container: ObjectId) -> bool {
        self.env.exit_flags & 1 != 0
    }
    fn exit_is_busy(&mut self, _container: ObjectId) -> bool {
        self.env.exit_flags & 2 != 0
    }
    fn exit_reserve_door(&mut self, _container: ObjectId) -> i32 {
        let id = self.env.object_id as u32;
        self.l(L_CONTAIN, &[5, id, self.env.exit_door as u32]);
        self.env.exit_door
    }
    fn exit_object_via_door(&mut self, _container: ObjectId, door: i32) {
        let id = self.env.object_id as u32;
        self.l(L_CONTAIN, &[6, id, door as u32]);
    }
    fn object_ai_free_to_exit(&mut self, other: ObjectId) -> Option<i32> {
        if !self.object_exists(other) || (self.env.ai_mask >> other) & 1 == 0 {
            return None;
        }
        let id = self.env.object_id as u32;
        let r = p1_hash(self.env.seed, 71, &[id, self.env.loco.frame]) % 3;
        self.l(L_CONTAIN, &[4, id, r]);
        Some(r as i32)
    }
    // ---- batch A14
    fn object_is_above_terrain(&mut self, id: ObjectId) -> bool {
        pick(p1_hash(self.env.seed, 73, &[id as u32]), 50)
    }
    fn action_can_attack_object(&mut self, target: ObjectId, src: CommandSourceType) -> CanAttackResultValue {
        if self.action_answer(3, target, src) {
            return ATTACKRESULT_POSSIBLE;
        }
        if self.action_answer(4, target, src) { ATTACKRESULT_POSSIBLE_AFTER_MOVING } else { ATTACKRESULT_NOT_POSSIBLE }
    }
    fn object_geometry_radii(&mut self, _id: ObjectId) -> (f32, f32, bool) {
        (self.env.minor_radius, self.env.loco.major_radius, self.env.geom_box != 0)
    }
    fn contain_add_to_contain(&mut self, _container: ObjectId) {
        let id = self.env.object_id as u32;
        self.l(L_CONTAIN, &[7, id]);
    }
    // ---- batch A15
    fn contain_items(&mut self, _container: ObjectId) -> Vec<ObjectId> {
        (1..32).filter(|&i| (self.env.contained_mask >> i) & 1 != 0 && (self.env.ai_mask >> i) & 1 == 0 && self.object_exists(i)).collect()
    }
    fn contain_remove_from_contain(&mut self, _container: ObjectId, obj: ObjectId) {
        self.l(L_RAPPEL, &[7, obj as u32]);
    }
    fn contain_is_valid_container_for_me(&mut self, _container: ObjectId) -> bool {
        let r = pick(p1_hash(self.env.seed, 74, &[self.env.object_id as u32, 1]), 50);
        self.l(L_RAPPEL, &[6, 1, u32::from(r)]);
        r
    }
    fn score_the_kill(&mut self, victim: ObjectId) {
        self.l(L_RAPPEL, &[3, victim as u32]);
    }
    fn object_kill(&mut self, id: ObjectId) {
        self.l(L_RAPPEL, &[4, id as u32]);
    }
    fn kill_self(&mut self) {
        let id = self.env.object_id as u32;
        self.l(L_RAPPEL, &[4, id]);
    }
    fn do_fx_combat_drop_kill(&mut self, bldg: ObjectId) {
        self.l(L_RAPPEL, &[5, bldg as u32]);
    }
    fn physics_reset_dynamic(&mut self) {
        self.l(L_RAPPEL, &[0]);
    }
    fn physics_scrub_velocity_z(&mut self, rate: f32) {
        self.l(L_RAPPEL, &[1, rate.to_bits()]);
    }
    fn object_set_layer(&mut self, layer: PathfindLayer) {
        self.l(L_RAPPEL, &[2, layer as u32]);
    }
    fn object_orientation(&mut self, _id: ObjectId) -> f32 {
        0.0 // (the other objects' cached angle)
    }
    fn object_position(&mut self, _id: ObjectId) -> Coord3D {
        Coord3D::default() // (the other objects' cached position)
    }
    fn object_bounding_circle_radius(&mut self, _id: ObjectId) -> f32 {
        self.env.container_radius
    }
    fn logic_random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.rng.game_logic_random_value_real(lo, hi)
    }
    fn find_position_around_no_source(&mut self, pos: &Coord3D, min_radius: f32, max_radius: f32) -> Option<Coord3D> {
        let r = pick(p1_hash(self.env.seed, 16, &[pos.x.to_bits(), pos.y.to_bits()]), self.env.find_pos_rate);
        self.l(L_FIND_POSITION, &[pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), min_radius.to_bits(), max_radius.to_bits(), 0, u32::from(r)]);
        r.then_some(Coord3D { x: pos.x + 5.0, y: pos.y + 5.0, z: pos.z })
    }
    fn attack_priority(&mut self, info: usize, obj: ObjectId) -> i32 {
        let r = p1_hash(self.env.seed, 70, &[info as u32, obj as u32]) % 4;
        self.l(L_ATTACK, &[16, info as u32, obj as u32, r]);
        r as i32
    }
    fn notify_firing_tracker_shot_fired(&mut self, slot: WeaponSlotType, victim: ObjectId) {
        self.l(L_ATTACK, &[3, slot as u32, victim as u32]);
    }
    // ---- batch A10b
    fn weapon_is_too_close(&mut self, victim: ObjectId) -> bool {
        self.range_answer(3, &[victim as u32])
    }
    fn object_forward_speed_2d(&self, _id: ObjectId) -> f32 {
        self.env.loco.fwd2d // (the scene's physics is every object's)
    }
    fn object_unit_direction_vector_2d(&mut self, _id: ObjectId) -> Coord3D {
        // another object of the scene: unturned
        Thing::new(Some(Arc::new(TplView))).unwrap().unit_direction_vector_2d()
    }
    fn object_center_position(&mut self, _id: ObjectId, pos: &Coord3D) -> Coord3D {
        Coord3D { z: pos.z + self.env.max_height_above_position * 0.5, ..*pos }
    }
    fn object_template_is_kind_of(&self, _id: ObjectId, _kind: KindOfType) -> bool {
        false // (the stand-in template's)
    }
    // ---- batch A9
    fn waypoint_num_links(&mut self, way: u32) -> i32 {
        if !(way < 7 && (self.env.waypoint_links >> way) & 1 != 0) {
            return 0;
        }
        1 + ((self.env.waypoint_links2 >> way) & 1) as i32
    }
    fn waypoint_link(&mut self, way: u32, which: i32) -> Option<u32> {
        match which {
            0 => (way < 7 && (self.env.waypoint_links >> way) & 1 != 0).then_some(way + 1),
            1 => (self.waypoint_num_links(way) == 2).then_some((way + 5) % 8),
            _ => None,
        }
    }
    fn pf_is_point_on_wall(&mut self, p: &Coord3D) -> bool {
        self.pf_answer(7, &[p.x.to_bits(), p.y.to_bits(), p.z.to_bits()])
    }
    fn pf_wall_height(&self) -> f32 {
        self.env.wall_height
    }
    fn group_center(&mut self) -> Coord3D {
        let c = self.env.group_center;
        self.l(L_GROUP_CENTER, &[0, c[0].to_bits(), c[1].to_bits(), c[2].to_bits()]);
        Coord3D { x: c[0], y: c[1], z: c[2] }
    }
    fn team_current_waypoint(&mut self) -> Option<u32> {
        self.team_waypoint
    }
    fn team_set_current_waypoint(&mut self, way: Option<u32>) {
        self.l(L_TEAM_WAYPOINT, &[way.unwrap_or(u32::MAX)]);
        self.team_waypoint = way;
    }
    fn is_skirmish_ai_player(&mut self) -> bool {
        assert!(self.env.player_type >= 0, "upstream reads the controlling player unchecked");
        let s = self.env.skirmish;
        self.l(L_SKIRMISH, &[s as u32]);
        s != 0
    }
    fn team_group_center_and_count(&mut self) -> (Coord3D, i32) {
        self.l(L_TEAM_GROUP, &[]);
        // the object's position moved by the scene's offset (wrap_ai.cpp's)
        let (p, o) = (*self.thing.position(), self.env.team_center);
        let c = Coord3D { x: p.x + o[0], y: p.y + o[1], z: p.z + o[2] };
        self.l(L_GROUP_CENTER, &[1, c.x.to_bits(), c.y.to_bits(), c.z.to_bits()]);
        (c, self.env.team_count)
    }
    fn ai_data_skirmish_group_fudge_value(&self) -> f32 {
        self.env.skirmish_fudge
    }
    // ---- batch A8
    fn team_set_active(&mut self) {
        self.l(L_TEAM_ACTIVE, &[]);
    }
    fn destroy_object(&mut self) {
        let id = self.object_id() as u32;
        self.l(L_DESTROY_OBJECT, &[id]);
    }
    fn formation_id(&self) -> i32 {
        self.env.formation_id
    }
    fn group_speed(&mut self) -> Option<f32> {
        (self.env.has_group != 0).then_some(self.env.group_speed)
    }
    fn object_max_height_above_position(&self, _id: ObjectId) -> f32 {
        self.env.max_height_above_position
    }
    fn object_has_physics(&self, _id: ObjectId) -> bool {
        self.env.loco.has_physics != 0
    }
    fn object_velocity_magnitude(&self, _id: ObjectId) -> f32 {
        self.env.loco.vel_mag
    }
    fn object_unit_direction_vector_3d(&mut self, _id: ObjectId) -> Coord3D {
        // another object of the scene: at the origin, unturned (wrap_ai.cpp resets them every operation)
        Thing::new(Some(Arc::new(TplView))).unwrap().unit_direction_vector_3d()
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
        put("WanderAboutPointRadius", format!("{:.3}", g.uniform(0.0, 60.0)));
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
    v.push(ai.area_to_guard.map_or(u32::MAX, |a| a as u32));
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
    // batch A6: the pathfinder's ignored obstacle
    v.push(h.pf_ignore as u32);
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
        v.push(x.goal_waypoint.unwrap_or(u32::MAX));
        v.push(u32::from(x.goal_squad.is_some()));
        if let Some(q) = &x.goal_squad {
            v.push(q.object_ids.len() as u32);
            v.extend(q.object_ids.iter().map(|&i| i as u32));
        }
        // batch A7: the move states' members
        for id in [1u32, 23, 24, 39] {
            match sm.state(id) {
                Some(AiState::MoveTo { mv, .. }) | Some(AiState::MoveOutOfTheWay { mv }) => put_move(&mut v, mv),
                Some(AiState::MoveAndTighten { mv, ok_to_repath_times, check_for_path })
                | Some(AiState::MoveAwayFromRepulsors { mv, ok_to_repath_times, check_for_path }) => {
                    put_move(&mut v, mv);
                    v.extend([*ok_to_repath_times as u32, u32::from(*check_for_path)]);
                }
                _ => panic!("a move state"),
            }
        }
        // batch A8: the follow-path, move-and-evacuate and move-and-delete states' members
        for id in [6u32, 7, 25, 26, 27] {
            match sm.state(id) {
                Some(AiState::FollowPath { mv, index, adjust_final, adjust_final_override, retry_count }) => {
                    put_move(&mut v, mv);
                    v.extend([*index as u32, u32::from(*adjust_final), u32::from(*adjust_final_override), *retry_count as u32]);
                }
                Some(AiState::MoveAndEvacuate { mv, origin }) => {
                    put_move(&mut v, mv);
                    put_pos(&mut v, origin);
                }
                Some(AiState::MoveAndDelete { mv, append_goal_position }) => {
                    put_move(&mut v, mv);
                    v.push(u32::from(*append_goal_position));
                }
                _ => panic!("an A8 move state"),
            }
        }
        // batch A9: the team's waypoint; the waypoint, wander and panic states' members
        v.push(h.team_waypoint.unwrap_or(u32::MAX));
        v.push(module.ai().completed_waypoint.unwrap_or(u32::MAX));
        let way = |w: &Option<u32>| w.unwrap_or(u32::MAX);
        let put_wp = |v: &mut Vec<u32>, wp: &WaypointPathData| {
            v.extend([wp.group_offset.x.to_bits(), wp.group_offset.y.to_bits(), wp.angle.to_bits(), wp.frames_sleeping as u32]);
            v.extend([way(&wp.current), way(&wp.prior), u32::from(wp.append_goal_position)]);
        };
        for id in [2u32, 3, 4, 5, 18, 19, 40] {
            match sm.state(id) {
                Some(AiState::FollowWaypointPath { mv, wp, as_group, is_follow, .. }) => {
                    put_move(&mut v, mv);
                    put_wp(&mut v, wp);
                    v.extend([u32::from(*as_group), u32::from(*is_follow)]);
                }
                Some(AiState::FollowWaypointPathExact { mv, last, as_group }) => {
                    put_move(&mut v, mv);
                    v.extend([way(last), u32::from(*as_group)]);
                }
                Some(AiState::Wander { mv, wp, wait_frames, timer }) | Some(AiState::Panic { mv, wp, wait_frames, timer }) => {
                    put_move(&mut v, mv);
                    put_wp(&mut v, wp);
                    v.extend([*wait_frames as u32, *timer as u32]);
                }
                Some(AiState::WanderInPlace { mv, origin, wait_frames, timer }) => {
                    put_move(&mut v, mv);
                    put_pos(&mut v, origin);
                    v.extend([*wait_frames as u32, *timer as u32]);
                }
                _ => panic!("an A9 state"),
            }
        }
        // batch A10a: the attack states, their machines and their aim states
        for id in [9u32, 10, 11, 12] {
            match sm.state(id) {
                Some(AiState::Attack { machine, original_victim_pos, victim_team, locked_weapon_on_enter, .. }) => {
                    v.push(u32::from(machine.is_some()));
                    put_pos(&mut v, original_victim_pos);
                    v.push(u32::from(victim_team.is_some()));
                    v.push(locked_weapon_on_enter.map(|s| s as u32).unwrap_or(u32::MAX));
                    if let Some(m) = machine {
                        v.extend([m.get_current_state_id(), m.goal_object_id() as u32]);
                        put_pos(&mut v, m.goal_position());
                        v.push(m.peek_sleep_till());
                        match m.state(2) {
                            Some(AiState::AttackAim { can_turn_in_place, set_locomotor, .. }) => {
                                v.extend([u32::from(*can_turn_in_place), u32::from(*set_locomotor)]);
                            }
                            _ => panic!("the aim state"),
                        }
                        // batch A10b: the chase states' members (when the machine has them)
                        for cid in [0u32, 1] {
                            if let Some(AiState::AttackApproach { mv, chase } | AiState::AttackPursue { mv, chase }) = m.state(cid) {
                                put_move(&mut v, mv);
                                put_pos(&mut v, &chase.prev_victim_pos);
                                v.extend([chase.approach_timestamp, u32::from(chase.follow), u32::from(chase.attacking_object)]);
                                v.extend([u32::from(chase.stop_if_in_range), u32::from(chase.initial_approach), u32::from(chase.force_attacking)]);
                            }
                        }
                    }
                }
                _ => panic!("an attack state"),
            }
        }
        // batch A11: the attack-move state (its machine: state, goal, its crate and attack states) and the crate state
        match sm.state(30) {
            Some(AiState::AttackMove { mv, frame_to_sleep_until, retry_count, command_src, machine: Some(m) }) => {
                put_move(&mut v, mv);
                v.extend([*frame_to_sleep_until, *retry_count as u32, *command_src as u32]);
                v.extend([m.get_current_state_id(), m.goal_object_id() as u32, m.peek_sleep_till()]);
                put_pos(&mut v, m.goal_position());
                match m.state(38) {
                    Some(AiState::PickUpCrate { mv, delay_counter }) => {
                        put_move(&mut v, mv);
                        v.push(*delay_counter as u32);
                    }
                    _ => panic!("the attack-move machine's crate state"),
                }
                match m.state(10) {
                    Some(AiState::Attack { machine, original_victim_pos, .. }) => {
                        v.push(u32::from(machine.is_some()));
                        put_pos(&mut v, original_victim_pos);
                        if let Some(am) = machine {
                            v.extend([am.get_current_state_id(), am.goal_object_id() as u32]);
                        }
                    }
                    _ => panic!("the attack-move machine's attack state"),
                }
            }
            _ => panic!("the attack-move state"),
        }
        match sm.state(38) {
            Some(AiState::PickUpCrate { mv, delay_counter }) => {
                put_move(&mut v, mv);
                v.push(*delay_counter as u32);
            }
            _ => panic!("the crate state"),
        }
        // batch A12: the hunt and attack-area states (their machines: state, goal, sleep, goal position, the crate state, the attack
        // state's machine)
        for id in [17u32, 28] {
            match sm.state(id) {
                Some(AiState::Hunt { machine, next_enemy_scan_time, .. }) => {
                    v.extend([*next_enemy_scan_time, u32::from(machine.is_some())]);
                    if let Some(m) = machine {
                        v.extend([m.get_current_state_id(), m.goal_object_id() as u32, m.peek_sleep_till()]);
                        put_pos(&mut v, m.goal_position());
                        match m.state(38) {
                            Some(AiState::PickUpCrate { mv, delay_counter }) => {
                                put_move(&mut v, mv);
                                v.push(*delay_counter as u32);
                            }
                            _ => panic!("the hunt machine's crate state"),
                        }
                        match m.state(10) {
                            Some(AiState::Attack { machine, original_victim_pos, .. }) => {
                                v.push(u32::from(machine.is_some()));
                                put_pos(&mut v, original_victim_pos);
                                if let Some(am) = machine {
                                    v.extend([am.get_current_state_id(), am.goal_object_id() as u32]);
                                }
                            }
                            _ => panic!("the hunt machine's attack state"),
                        }
                    }
                }
                _ => panic!("a hunt state"),
            }
        }
        // batch A12b: the attack-follow-waypoint states (the waypoint path's members, their machines)
        for id in [31u32, 32] {
            match sm.state(id) {
                Some(AiState::FollowWaypointPath { mv, wp, as_group, is_follow, machine: Some(m) }) => {
                    put_move(&mut v, mv);
                    put_wp(&mut v, wp);
                    v.extend([u32::from(*as_group), u32::from(*is_follow)]);
                    v.extend([m.get_current_state_id(), m.goal_object_id() as u32, m.peek_sleep_till()]);
                    put_pos(&mut v, m.goal_position());
                    match m.state(38) {
                        Some(AiState::PickUpCrate { mv, delay_counter }) => {
                            put_move(&mut v, mv);
                            v.push(*delay_counter as u32);
                        }
                        _ => panic!("the attack-follow machine's crate state"),
                    }
                    match m.state(10) {
                        Some(AiState::Attack { machine, original_victim_pos, .. }) => {
                            v.push(u32::from(machine.is_some()));
                            put_pos(&mut v, original_victim_pos);
                            if let Some(am) = machine {
                                v.extend([am.get_current_state_id(), am.goal_object_id() as u32]);
                            }
                        }
                        _ => panic!("the attack-follow machine's attack state"),
                    }
                }
                _ => panic!("an attack-follow-waypoint state"),
            }
        }
        // batch A15: the rappel state's members
        match sm.state(35) {
            Some(AiState::Rappel(d)) => v.extend([d.rappel_rate.to_bits(), d.dest_z.to_bits(), u32::from(d.target_is_bldg)]),
            _ => panic!("the rappel state"),
        }
        // batch A14: the enter state's move members and entry to clear
        match sm.state(15) {
            Some(AiState::Enter { mv, entry_to_clear }) => {
                put_move(&mut v, mv);
                v.push(*entry_to_clear as u32);
            }
            _ => panic!("the enter state"),
        }
        // batch A13: the exit states' entries to clear, the face states' turn-in-place flags
        for id in [37u32, 42] {
            match sm.state(id) {
                Some(AiState::Exit { entry_to_clear, .. }) => v.push(*entry_to_clear as u32),
                _ => panic!("an exit state"),
            }
        }
        for id in [33u32, 34] {
            match sm.state(id) {
                Some(AiState::Face { can_turn_in_place, .. }) => v.push(u32::from(*can_turn_in_place)),
                _ => panic!("a face state"),
            }
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

/// A move state's members (the driver's `putMove`).
fn put_move(v: &mut Vec<u32>, m: &rts_lockstep_engine::logic::ai::ai_move_states::MoveToData) {
    put_pos(v, &m.goal_position);
    v.push(m.goal_layer as u32);
    put_pos(v, &m.path_goal_position);
    v.extend([m.ambient_playing_handle, m.path_timestamp, m.blocked_repath_timestamp]);
    v.extend([u32::from(m.adjust_destinations), u32::from(m.waiting_for_path), u32::from(m.try_one_more_repath)]);
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
    // (batch A10b: a quarter early, below the chase states' recompute time)
    l.frame = if g.below(4) == 0 { g.below(3) } else { g.below(5000) };
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
    // (32 and up: never one of the other objects, 1..31, whose ids the AI's own would otherwise share)
    e.object_id = 32 + g.below(200) as i32;
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
    // (batch A10b: a quarter of the scenes have the main turret, for the pursuit)
    if g.below(4) == 0 {
        e.turrets |= 1;
    }
    // batch A4
    e.loco.kind_mask |= i32::from(g.below(3) == 0) << 7;
    gen_a4(g, &mut e);
    e.turret_rate = g.below(101);
    e.state_rate = g.below(101);
    // batch A5
    e.mobile = i32::from(g.below(8) != 0);
    e.partition_cell_size = [40.0, 10.0, 25.0][g.below(3) as usize];
    e.extent_hi = [g.uniform(100.0, 300.0), g.uniform(80.0, 250.0)];
    e.team_mask = g.u32() & !1 & if g.below(3) == 0 { 0 } else { u32::MAX };
    e.other_ai_path = g.below(3).min(1);
    e.contact_weapon = i32::from(g.below(4) == 0);
    e.has_contain = i32::from(g.below(3) == 0);
    e.modules = g.below(8) as i32;
    e.has_command_set = i32::from(g.below(4) != 0);
    for k in e.command_slots.iter_mut() {
        *k = if g.below(4) == 0 { g.below(4) as i32 } else { -1 };
    }
    for t in e.button_types.iter_mut() {
        *t = [12, 25, 3, 12, 25, 0][g.below(6) as usize];
    }
    gen_a5(g, &mut e);
    // batch A6
    e.loco.kind_mask |= i32::from(g.below(6) == 0) << 8;
    e.pf_extent_hi = [g.uniform(100.0, 300.0), g.uniform(80.0, 250.0)];
    e.waypoint_links = g.u32();
    e.repulsed_distance = g.uniform(0.0, 100.0);
    gen_a6(g, &mut e);
    // batch A7
    e.sounds = g.below(16) as i32;
    e.formation_id = if g.below(3) == 0 { 1 + g.below(3) as i32 } else { 0 };
    e.group_speed = g.uniform(0.5, 6.0);
    e.max_height_above_position = g.uniform(0.0, 30.0);
    // batch A9
    e.waypoint_links2 = g.u32();
    e.team_waypoint = g.below(9) as i32 - 1;
    gen_a9(g, &mut e);
    // batch A10a
    e.loco.kind_mask |= (g.below(128) as i32 & if g.below(3) == 0 { 127 } else { 0 }) << 9;
    e.max_shots_init = [0x7fff_ffff, 1, 0, 3][g.below(4) as usize];
    e.has_any_weapon = i32::from(g.below(10) != 0);
    e.crushes_infantry = i32::from(g.below(2) == 0);
    e.turret_turn_rate = [if g.below(2) == 0 { 0.0 } else { g.uniform(0.1, 2.0) }, if g.below(2) == 0 { 0.0 } else { g.uniform(0.1, 2.0) }];
    gen_a10(g, &mut e);
    e
}

/// Batch A10a's answers that change between operations: the weapons, the object flags, the contain, the angle, the turrets.
fn gen_a10(g: &mut Gen, e: &mut Env) {
    e.weapon_status = [0, 0, 0, 4, 4, 1, 2, 3][g.below(8) as usize];
    e.leech_range = i32::from(g.below(6) == 0);
    e.aim_delta = g.uniform(0.0, 0.1);
    e.cur_slot = g.below(3) as i32;
    e.slot_mask = g.below(8) as i32;
    e.able_to_attack = i32::from(g.below(8) != 0);
    e.out_of_ammo = i32::from(g.below(12) == 0);
    e.cur_weapon_locked = i32::from(g.below(5) == 0);
    e.object_rate = g.below(101);
    e.garrisonable = i32::from(g.below(3) == 0);
    e.contain_count = g.below(3) as i32;
    e.rel_angle = if g.below(2) == 0 { g.uniform(-0.05, 0.05) } else { g.uniform(-1.0, 1.0) };
    e.turret_cur_weapon = g.below(4) as i32;
    e.team_epoch = g.below(3) as i32;
    e.units_hunt = i32::from(g.below(2) == 0);
    e.exit_flags = if g.below(4) == 0 { g.below(4) as i32 } else { 1 };
    e.exit_door = g.below(6) as i32 - 2;
    // (a third of the time large: the enter state's arrival test, against a goal at the origin)
    e.minor_radius = if g.below(3) == 0 { g.uniform(200.0, 600.0) } else { g.uniform(0.0, 30.0) };
    e.geom_box = i32::from(g.below(2) == 0);
    e.contained_mask = if g.below(2) == 0 { 0 } else { g.u32() };
    e.loco.kind_mask |= i32::from(g.below(2) == 0) << 16; // (KINDOF_CAN_RAPPEL)
    // (a quarter: the main turret holds the current weapon, for the pursuit)
    if g.below(4) == 0 {
        e.turret_cur_weapon |= 1;
    }
}

/// Batch A9's answers that change between operations: the wall's height, the groups' centres, the skirmish player and its fudge.
fn gen_a9(g: &mut Gen, e: &mut Env) {
    e.wall_height = g.uniform(0.0, 40.0);
    e.group_center = gen_point(g);
    e.skirmish = i32::from(g.below(2) == 0);
    // (an offset from the object's position, so that the skirmish bail-out's distance is near its limit)
    e.team_center = [g.uniform(-15.0, 15.0), g.uniform(-15.0, 15.0), g.uniform(-30.0, 30.0)];
    e.team_count = g.below(7) as i32;
    e.skirmish_fudge = g.uniform(0.0, 20.0);
}

/// Batch A6's rates (the searches, the pathfinder's answers, the weapon's ranges).
fn gen_a6(g: &mut Gen, e: &mut Env) {
    e.service_rate = g.below(101);
    e.pf_answer_rate = g.below(101);
    e.range_rate = g.below(101);
}

/// Batch A5's answers that change between operations: the rates, the continue range, the closest attackable object.
fn gen_a5(g: &mut Gen, e: &mut Env) {
    e.action_rate = g.below(101);
    e.move_away_rate = g.below(101);
    e.path_exists_rate = g.below(101);
    e.find_pos_rate = g.below(101);
    // (batch A10b: half with a continue range, three quarters with a closest object: the fire state's continued attack)
    e.continue_range = if g.below(2) == 0 { g.uniform(1.0, 200.0) } else { 0.0 };
    e.closest_attack = if g.below(4) != 0 { g.below(32) as i32 } else { 0 };
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
    // batch A5
    if g.below(6) == 0 {
        gen_a5(g, e);
    }
    // batch A6
    if g.below(6) == 0 {
        gen_a6(g, e);
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
    // batch A9
    if g.below(8) == 0 {
        gen_a9(g, e);
    }
    // batch A10a
    if g.below(6) == 0 {
        gen_a10(g, e);
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
    } else if g.below(40) == 0 {
        // batch A12: setAttackInfo
        87
    } else if g.below(5) == 0 {
        // batch A6: the paths
        [74, 74, 75, 76, 77, 78, 78, 78, 79, 80, 80, 81, 81, 82, 83, 84, 85, 86][g.below(18) as usize]
    } else if g.below(4) == 0 {
        // batch A5: a command (and the quick exit)
        if g.below(15) == 0 { 73 } else { 72 }
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
    let needs_machine = matches!(op, 7 | 36 | 38 | 41 | 43 | 44 | 48 | 50..=52 | 54 | 55 | 59..=65 | 67..=86);
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
            ia[0] = if g.below(2) == 0 { [0, 0, 13, 8, 41, 15, 21, 1, 1, 23, 24, 39, 6, 7, 25, 26, 27, 2, 3, 4, 5, 18, 19, 40, 3, 18, 9, 10, 11, 12, 10, 10, 2, 2, 30, 30, 38, 17, 17, 28, 28, 31, 31, 32, 32, 33, 34, 37, 42, 35, 35][g.below(51) as usize] } else { g.below(46) as i32 };
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
        // batch A6
        74 | 86 => ia[0] = g.below(2) as i32,
        87 => ia[0] = g.below(3) as i32 - 1,
        75 | 77 | 81 | 82 => {
            ia[0] = g.below(32) as i32;
            // (upstream dereferences a contact weapon's victim position)
            ia[1] = i32::from(g.below(3) != 0 || e.contact_weapon != 0 || ia[0] == 0);
        }
        79 => {
            ia[0] = g.below(9) as i32 - 1;
            fa[3] = g.uniform(-10.0, 10.0);
            fa[4] = g.uniform(-10.0, 10.0);
        }
        // batch A5: a command's parms (see ref_ai_a2_op): any command (55, 8 and 32 are unhandled), its source, an object, a team, a
        // waypoint, a polygon, an int value, a button and coords; what upstream dereferences unchecked is always given
        72 | 73 => {
            let cmd = if g.below(5) == 0 { g.below(56) as i32 } else { [0, 1, 5, 11, 14, 29, 30, 48, 49, 9, 54, 41, 40, 23, 21, 25, 6, 7, 46, 47, 42, 43, 44, 15, 15, 18, 18, 33, 33, 16, 16, 17, 17, 25, 25, 53, 53, 35, 35, 36, 36, 37, 37][g.below(43) as usize] };
            ia[0] = cmd;
            ia[1] = g.below(4) as i32;
            let ids: Vec<i32> = (1..32).filter(|i| (e.exist_mask >> i) & 1 != 0).collect();
            let some_object = |g: &mut Gen| if ids.is_empty() { 0 } else { ids[g.below(ids.len() as u32) as usize] };
            ia[2] = if g.below(4) == 0 { g.below(32) as i32 } else { some_object(g) };
            if matches!(cmd, 30 | 40 | 48) {
                ia[2] = some_object(g);
            }
            ia[3] = if g.below(3) == 0 { -1 } else { 0 };
            ia[4] = g.below(9) as i32 - 1;
            ia[5] = if cmd == 31 { g.below(8) as i32 } else { g.below(9) as i32 - 1 };
            ia[6] = [0, 1, 2, 3, -1, 5, 0x7fff_ffff][g.below(7) as usize];
            ia[7] = g.below(5) as i32 - 1;
            ia[8] = g.below(5) as i32;
            for i in 0..ia[8] as usize {
                let q = gen_point(g);
                fa[3 + 3 * i..6 + 3 * i].copy_from_slice(&q);
            }
            if g.below(3) == 0 {
                // off the map (the player's goal clipping, the guard's edge point)
                fa[0] = g.uniform(-100.0, 400.0);
                fa[1] = g.uniform(-100.0, 400.0);
            }
        }
        _ => {}
    }
    (op, fa, ia)
}

/// The Rust side of `ref_ai_a2_op`. `bug` plants a wrong translation in the result: 2 flips a boolean result.
fn rust_op(module: &mut AiUpdateModule<Host>, h: &mut Host, op: i32, fa: &[f32], ia: &[i32], bug: u32, saved: &mut Vec<u8>) -> Vec<u32> {
    let p = Coord3D { x: fa[0], y: fa[1], z: fa[2] };
    let b = |x: bool| vec![u32::from(x ^ (bug == 2))];
    if op >= 59 {
        return rust_op_a4(module, h, op, fa, ia, saved);
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

/// A weapon slot of the scene's.
fn slot_of(k: i32) -> WeaponSlotType {
    match k {
        0 => WeaponSlotType::Primary,
        1 => WeaponSlotType::Secondary,
        _ => WeaponSlotType::Tertiary,
    }
}

/// The waypoints' names (`wrap_ai.cpp`'s stand-in: "wp" and the index).
fn waypoint_name(i: u32) -> Vec<u8> {
    format!("wp{i}").into_bytes()
}

/// `TheTerrainLogic->getWaypointByName` of the stand-in.
fn waypoint_by_name(name: &[u8]) -> Option<u32> {
    let s = std::str::from_utf8(name).ok()?;
    let i: u32 = s.strip_prefix("wp")?.parse().ok()?;
    (i < 8).then_some(i)
}

/// Batch A4's operations (`ref_ai_a2_op`'s 59..71): the machine, the AI's update and creation.
fn rust_op_a4(module: &mut AiUpdateModule<Host>, h: &mut Host, op: i32, fa: &[f32], ia: &[i32], saved: &mut Vec<u8>) -> Vec<u32> {
    let p = &Coord3D { x: fa[0], y: fa[1], z: fa[2] };
    let coords: Vec<Coord3D> = (0..ia[8].max(0) as usize).map(|i| Coord3D { x: fa[3 + 3 * i], y: fa[4 + 3 * i], z: fa[5 + 3 * i] }).collect();
    let coords = &coords[..];
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
            let st = states::aism_xfer(sm, &mut ai.machine, &mut x, waypoint_name, waypoint_by_name).is_err();
            let _ = x.close();
            *saved = x.bytes().to_vec();
            let mut r = vec![u32::from(st), saved.len() as u32];
            r.extend(saved.iter().map(|&c| u32::from(c)));
            r
        }
        // batch A6: the paths
        74 => {
            module.run(h, |_, h| ai_paths::request_path(h, p, ia[0] != 0));
            vec![]
        }
        75 => {
            module.run(h, |_, h| ai_paths::request_attack_path(h, ia[0], p));
            vec![]
        }
        76 => {
            module.run(h, |_, h| ai_paths::request_approach_path(h, p));
            vec![]
        }
        77 => {
            module.run(h, |_, h| ai_paths::request_safe_path(h, ia[0]));
            vec![]
        }
        78 => {
            module.do_pathfind(h);
            vec![]
        }
        79 => {
            let way = (ia[0] >= 0).then_some(ia[0] as u32);
            module.run(h, |_, h| ai_paths::set_path_from_waypoint(h, way, fa[3], fa[4]));
            vec![]
        }
        80 => vec![u32::from(module.run(h, |sm, h| ai_paths::compute_path(sm, h, p)))],
        81 => {
            let victim = if h.object_exists(ia[0]) { ia[0] } else { 0 };
            let pos = (ia[1] != 0).then_some(p);
            vec![u32::from(module.run(h, |sm, h| ai_paths::compute_attack_path(sm, h, victim, pos)))]
        }
        82 => {
            module.run(h, |sm, h| ai_paths::friend_set_goal_object(sm, h, ia[0]));
            vec![]
        }
        83 => vec![u32::from(module.run(h, |_, h| ai_paths::is_quick_path_available(h, p)))],
        84 => vec![u32::from(module.run(h, |_, h| ai_paths::can_compute_quick_path(h)))],
        85 => vec![u32::from(module.run(h, |_, h| ai_paths::compute_quick_path(h, p)))],
        87 => {
            module.ai_mut().attack_info = (ia[0] >= 0).then_some(ia[0] as usize);
            vec![]
        }
        86 => {
            let a = module.ai_mut();
            if let Some(cur) = a.cur_locomotor {
                a.locomotor_set.get_mut(cur).expect("the locomotor").set_ultra_accurate(ia[0] != 0);
            }
            vec![]
        }
        72 => {
            let mut parms = AiCommandParms::new(ia[0], ia[1]);
            parms.pos = *p;
            parms.obj = if h.object_exists(ia[2]) { ia[2] } else { 0 };
            parms.team = (ia[3] >= 0).then_some(ia[3] as u32);
            parms.waypoint = (ia[4] >= 0).then_some(ia[4] as u32);
            parms.polygon = (ia[5] >= 0).then_some(ia[5]);
            parms.int_value = ia[6];
            parms.command_button = (ia[7] >= 0).then_some(ia[7] as u32);
            parms.coords = coords.to_vec();
            module.ai_do_command(h, &parms).expect("aiDoCommand");
            vec![]
        }
        73 => {
            module.run(h, |sm, h| rts_lockstep_engine::logic::ai::ai_commands::do_quick_exit(sm, h, coords)).expect("doQuickExit");
            vec![]
        }
        71 => {
            let mut x = XferLoad::new(saved.clone());
            x.open(b"ai_a4.bin").expect("open");
            let (sm, ai) = (module.machine.as_mut().expect("the machine"), module.ai.as_mut().expect("the AI"));
            let st = states::aism_xfer(sm, &mut ai.machine, &mut x, waypoint_name, waypoint_by_name).is_err();
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
    ops: [u64; 88],
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
    a4_logs: [u64; 34],
    a4_states: [u64; 26],
    temporary_states: u64,
    /// Batch A9: cases ended before an operation upstream would make undefined (a waypoint state without a locomotor).
    a9_undefined: u64,
    /// Batch A10a: the attack machine's state after an operation (chase, approach, aim, fire), and LOG_ATTACK's kinds.
    attack_inner: [u64; 4],
    attack_logs: [u64; 17],
    /// Batch A11: the attack-move machine's state after an operation in the attack-move state (idle, crate, attack).
    attack_move_inner: [u64; 3],
    /// Batch A12: the hunt and attack-area machines' state after an operation in those states (attack, crate, idle).
    hunt_inner: [u64; 3],
    /// Batch A12b: the attack-follow-waypoint machines' state after an operation in those states (idle, crate, attack).
    follow_inner: [u64; 3],
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
            ops: [0; 88],
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
            a4_logs: [0; 34],
            a4_states: [0; 26],
            temporary_states: 0,
            a9_undefined: 0,
            attack_inner: [0; 4],
            attack_logs: [0; 17],
            attack_move_inner: [0; 3],
            hunt_inner: [0; 3],
            follow_inner: [0; 3],
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
        pf_ignore: 0,
        next_audio: 1,
        team_waypoint: None,
        attack_frames: Vec::new(),
        max_shots: 0,
        others_height_asked: 0,
    };
    host.max_shots = host.env.max_shots_init;
    host.team_waypoint = (host.env.team_waypoint >= 0).then_some(host.env.team_waypoint as u32);
    host.names.reset();
    // batch A5: the modules the commands look up, first (as ref_ai_a2_reset)
    for name in ["DeliverPayloadAIUpdate", "ProneUpdate", "HackInternetAIUpdate"] {
        host.names.name_to_key(name.as_bytes());
    }
    unsafe { cpp::ref_ai_a2_reset() };
    let xfer_path = cstr(&xfer_file_path());
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
    // batch A5: the forbidden player commands
    data.forbid_player_commands = g.below(5) == 0;
    sets_words.push(u32::from(data.forbid_player_commands));
    // batch A10a: the linked turrets
    data.turrets_linked = g.below(3) == 0;
    sets_words.push(u32::from(data.turrets_linked));
    for i in 0..2 {
        if (host.env.turrets >> i) & 1 != 0 {
            data.turret_data[i] = Some(Arc::new(rts_lockstep_engine::logic::ai::turret::TurretAiData::default()));
        }
    }
    let data = Arc::new(data);

    // the object and the AI: the constructor draws the locomotors' random values
    // (batch A10b: a quarter at negative x, where the other objects, at the origin facing +x, move away: pursued)
    let start = [if g.below(4) == 0 { g.uniform(-60.0, -1.0) } else { g.uniform(-20.0, 260.0) }, g.uniform(-20.0, 180.0), 0.0];
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
        // batch A9: an as-team waypoint state reads the controlling player unchecked (kept while one is in the machine); batch
        // A10b: so do the attack machine's chase states (kept while an attack state is in the machine)
        let attacking = module.machine.as_ref().is_some_and(|sm| {
            // (batch A11: and the attack-move state, whose machine has an attack state)
            // (batch A12b: and the attack-follow-waypoint states)
            matches!(sm.get_current_state_id(), 9..=12 | 15 | 17 | 28 | 30..=32)
                || matches!(module.ai().machine.temporary_state, 9..=12 | 15 | 17 | 28 | 30..=32)
        });
        // batch A10b: without a controlling player the team does not attack a common target (the idle state would attack it, and
        // the chase states read the player unchecked)
        if host.env.player_type < 0 {
            host.env.team_attack_common = 0;
        }
        // batch A15: the rappel state uses the physics unchecked (kept while it is in the machine)
        if module.machine.as_ref().is_some_and(|sm| sm.get_current_state_id() == 35 || module.ai().machine.temporary_state == 35) {
            host.env.loco.has_physics = 1;
        }
        // (and so does the idle state's mood target: `getNextMoodTarget` reads the player unchecked, and the C++ stand-in's
        // `getPlayerType` tolerated a missing one; kept while the idle state is in the machine)
        let idle = module.machine.as_ref().is_some_and(|sm| sm.get_current_state_id() == 0 || module.ai().machine.temporary_state == 0);
        if (a9_states(&module).1 || attacking || idle) && host.env.player_type < 0 {
            host.env.player_type = 0;
        }
        let moving_away = (module.ai().move_out_of_way1, module.ai().move_out_of_way2);
        let (mut op, mut fa, mut ia) = gen_op(&mut g, &host.env, waypoint_heavy, moving_away, module.machine.is_some(), has_saved);
        a9_guard(&module, &host.env, &mut op, &mut ia);
        // batch A10b: an attack-position command a third of the time at the object (the approach's exact goal on exit)
        if op == 72 && ia[0] == 14 && g.below(3) == 0 {
            let p = *host.thing.position();
            fa[0] = p.x + g.uniform(-0.3, 0.3);
            fa[1] = p.y + g.uniform(-0.3, 0.3);
            fa[2] = p.z;
        }
        // batch A10a: in an attack state, half the operations are the AI's update (which runs the attack machine)
        // (batch A12: and in the hunt and attack-area states, whose machines the update runs)
        if module.machine.as_ref().is_some_and(|sm| matches!(sm.get_current_state_id(), 9..=12 | 17 | 28)) && g.below(2) == 0 {
            op = 65;
            ia.iter_mut().for_each(|x| *x = 0); // (the update takes no arguments; another op's would be read for coordinates)
        }
        if std::env::var("AI_A2_TRACE").is_ok() {
            let cur = module.machine.as_ref().map(|sm| sm.get_current_state_id());
            println!("TRACE step {step} op {op} ia {:?} state {cur:?} temp {} goal_waypoint {:?}", &ia[..6], module.ai().machine.temporary_state, module.ai().machine.goal_waypoint);
        }
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
        host.others_height_asked = 0;
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
            if code == L_ATTACK && n > 0 {
                ck.attack_logs[host_log_copy[k + 2] as usize] += 1;
            }
            if (70..=103).contains(&code) {
                ck.a4_logs[(code - 70) as usize] += 1;
            }
            k += 2 + n;
        }
        if let Some(sm) = &module.machine {
            if let Some(AiState::Hunt { machine: Some(m), .. }) = sm.state(sm.get_current_state_id()) {
                ck.hunt_inner[match m.get_current_state_id() { 10 => 0, 38 => 1, _ => 2 }] += 1;
            }
            if let Some(AiState::FollowWaypointPath { machine: Some(m), .. }) = sm.state(sm.get_current_state_id()) {
                ck.follow_inner[match m.get_current_state_id() { 0 => 0, 38 => 1, _ => 2 }] += 1;
            }
            if let Some(AiState::AttackMove { machine: Some(m), .. }) = sm.state(sm.get_current_state_id()) {
                ck.attack_move_inner[match m.get_current_state_id() { 0 => 0, 38 => 1, _ => 2 }] += 1;
            }
            if let Some(AiState::Attack { machine: Some(m), .. }) = sm.state(sm.get_current_state_id()) {
                ck.attack_inner[(m.get_current_state_id() as usize).min(3)] += 1;
            }
            ck.a4_states[match sm.get_current_state_id() {
                0 => 0,
                13 => 1,
                8 => 2,
                41 => 3,
                1 => 5,
                23 => 6,
                24 => 7,
                39 => 8,
                6 | 7 => 9,
                25 | 26 => 10,
                27 => 11,
                2 | 3 => 12,
                4 | 5 => 13,
                18 => 14,
                40 => 15,
                19 => 16,
                9..=12 => 17,
                30 => 18,
                38 => 19,
                17 | 28 => 20,
                31 | 32 => 21,
                33 | 34 => 22,
                37 | 42 => 23,
                15 => 24,
                35 => 25,
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
        // batch A9: a waypoint state with no locomotor would dereference it (upstream undefined): the case ends here
        if a9_states(&module).0 && module.ai().cur_locomotor.is_none() {
            ck.a9_undefined += 1;
            return true;
        }
    }
    true
}

/// Batch A9: whether the machine (its state or its temporary state) is in a state that dereferences the locomotor unchecked (the
/// waypoint-path, exact, wander and panic states), and whether in the as-team waypoint-path state (the controlling player too).
fn a9_states(module: &AiUpdateModule<Host>) -> (bool, bool) {
    let Some(sm) = &module.machine else { return (false, false) };
    let ids = [sm.get_current_state_id(), module.ai().machine.temporary_state];
    // (batch A12b: and the attack-follow-waypoint states, the as-team one as the as-team waypoint state)
    (ids.iter().any(|id| matches!(id, 2..=5 | 18 | 19 | 31 | 32)), ids.contains(&2) || ids.contains(&32))
}

/// Batch A9: what would enter those states where upstream dereferences what is not there (no locomotor; for the as-team state no
/// goal waypoint or no controlling player; the state already the machine's state or its temporary state, whose one instance the
/// other's end would leave without a current waypoint, or, for the attack states (batch A10a), without its attack machine) is the
/// idle state (`setState`, `setTemporaryState`) or command 0 instead;
/// in them, `chooseLocomotorSetExplicit` (which leaves no locomotor) is the ultra-accurate setter instead.
fn a9_guard(module: &AiUpdateModule<Host>, e: &Env, op: &mut i32, ia: &mut [i32]) {
    let no_loco = module.ai().cur_locomotor.is_none();
    let in_machine = |state: i32| {
        let cur = module.machine.as_ref().map(|sm| sm.get_current_state_id());
        cur == Some(state as u32) || module.ai().machine.temporary_state == state as u32
    };
    let unsafe_entry = |state: i32, way: bool| {
        (matches!(state, 2..=5 | 18 | 19 | 31 | 32) && (no_loco || in_machine(state) || (matches!(state, 2 | 32) && (!way || e.player_type < 0))))
            // batch A12b: the attack-follow-waypoint states without a controlling player (their machines' attack reads it)
            || (matches!(state, 31 | 32) && e.player_type < 0)
            // batch A14: the enter state without a controlling player (it may attack the goal)
            || (state == 15 && e.player_type < 0)
            // batch A15: the rappel state without physics (used unchecked)
            || (state == 35 && e.loco.has_physics == 0)
            // batch A10a: an attack state already the machine's state or its temporary state (the other's exit deletes the one
            // attack machine, which the next update dereferences); batch A10b: or without a controlling player (the chase states
            // read it unchecked)
            || (matches!(state, 9..=12) && (in_machine(state) || e.player_type < 0))
            // batch A11: the attack-move state without a controlling player (its machine's attack reads it)
            || (state == 30 && e.player_type < 0)
            // batch A12: the hunt and attack-area states already the machine's state or its temporary state (the other's exit
            // deletes the one machine), or without a controlling player (their machines' attack reads it)
            || (matches!(state, 17 | 28) && (in_machine(state) || e.player_type < 0))
    };
    match *op {
        59 | 60 if unsafe_entry(ia[0], module.ai().machine.goal_waypoint.is_some()) => ia[0] = 0,
        72 => {
            let state = match ia[0] {
                6 => 3,
                7 => 2,
                46 => 5,
                47 => 4,
                42 => 18,
                44 => 19,
                11 => 10,
                12 => 11,
                14 => 9,
                15 => 30,
                18 => 17,
                33 => 28,
                16 => 31,
                17 => 32,
                23 => 15,
                37 => 35,
                _ => -1,
            };
            if unsafe_entry(state, ia[4] >= 0) {
                ia[0] = 0;
            }
        }
        2 if a9_states(module).0 => *op = 86,
        _ => {}
    }
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

/// # Results (2026-10-28, batch A15; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - The rappel state (with `findEnemyInContainer` and `killEnemiesInContainer`): 30,000 constructions and 1,799,389 operations,
///   **0 mismatches** in the results, the call logs and the dumps (which add its rate, destination height and building flag).
///   After an operation the machine was rappelling 34,958 times. The contain lists the existing objects without an AI of the
///   scene's mask (A5's passengers' `aiIdle` reaches none); the kills, the FX, the layer, the physics' reset and fall clamp are
///   logged (`LOG_RAPPEL`). Not generated: the rappel state without physics (used unchecked; kept while it is in the machine).
/// - The first runs: no rappel state persisted until the stand-in's `isKindOf` answered `KINDOF_CAN_RAPPEL` (a harness gap); then
///   the rate differed by 1 ULP in 3,338 dumps: `fabs` is C's, so `|gravity| * 30 * 2.5f` is a double product stored as a Real, not
///   a float product (fixed in the translation before it was committed).
/// - Planted bugs: plants 3370-3373 at 30,000 cases, each caught (1 to 4,539 mismatches): the rate's factor, the ground height not
///   followed, one kill fewer, the speed not restored on exit.
///
/// # Results (2026-10-28, batch A14; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - The enter state: 30,000 constructions and 1,798,842 operations, **0 mismatches** in the results, the call logs and the dumps
///   (which add the enter state's move members and entry to clear). After an operation the machine was entering 10,896 times.
///   Another object's `isAboveTerrain` is the hash's; the action manager's `getCanAttackObject` is two of its answers (possible,
///   else possible after moving); the other objects' geometry (minor radius, box or sphere) is the scene's, the minor radius large
///   a third of the time so that the arrival test against a goal at the origin is reached. Not generated: the enter state without
///   a controlling player (it may attack its goal, and the attack states read the player unchecked).
/// - Planted bugs: plants 3360-3364 at 30,000 cases, each caught (1 to 1,613 mismatches): the locomotor allowed off the map, the
///   entry not cleared on exit, the attack after moving, the held success, the arrival radius (caught only after the minor radius
///   was made large at times; 0 mismatches before).
///
/// # Results (2026-10-28, batch A13; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - The exit, exit-instantly and face states (both face ids): 30,000 constructions and 1,799,025 operations, **0 mismatches** in the
///   results, the call logs and the dumps (which add the exit states' entries to clear and the face states' turn-in-place flags).
///   After an operation the machine was facing 67,473 times and in an exit state 65,158 times. The contain's exit (whether there is
///   one, busy, the door it reserves) is the scene's; another object's AI answers `getAiFreeToExit` from the hash (upstream: the
///   base class's `FREE_TO_EXIT`, a subclass's others). `exitObjectViaDoor` is logged only, so the exit states' "the machine left
///   this state" branch is not reached.
/// - Planted bugs: plants 3350-3356 at 30,000 cases, each caught (7 to 9,755 mismatches): the instant exit's door, the wait for
///   the goal's AI, the wait for a busy exit, the instant exit's success, the entry not cleared on exit, the turn-in-place test,
///   the facing threshold.
///
/// # Results (2026-10-28, batch A12b; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - The attack-follow-waypoint states (both ids: a waypoint path with an attack-move machine): 30,000 constructions and
///   1,799,304 operations, **0 mismatches** in the results, the call logs and the dumps (which add both states' waypoint-path
///   members and machines). After an operation the machine was in one of them 32,104 times (its machine idle 32,015 times,
///   attacking 89; its crate state never: the crate is never found).
/// - Not generated: either state without a locomotor, already the machine's state or its temporary state, or without a controlling
///   player; the as-team one without a goal waypoint (as the as-team waypoint state).
/// - Planted bugs: plants 3340-3346 at 30,000 cases: caught 3340 (the machine not cleared on entry, 12 mismatches), 3341 (no forced
///   retarget, 3), 3342 (no repath, 2), 3343 (the machine not idled on exit, 20), 3344 (the waypoint update after a retargeting,
///   63). **Not caught:** 3345 (the state's `isAttack` false): the AI's machine's attack state is asked while the machine attacks in
///   89 operations, none of which asks it; 3346 (the machines' default states entered in the other order): it only swaps the random
///   sleep offsets of two idle states that are never updated while idle and are not dumped, so it cannot be seen here.
///
/// # Results (2026-10-28, batch A12; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - The hunt and attack-area states (each with its attack-then-idle machine of attack, crate and idle states): 30,000
///   constructions and 1,799,396 operations, **0 mismatches** in the results, the call logs and the dumps (which add both states'
///   scan times and machines: state, goal, sleep, goal position, crate state, the attack state's machine). After an operation the
///   machine was hunting or attacking an area 142,100 times (its machine attacking 1,076 times, idle 141,024; its crate state never:
///   the crate is never found); the attack priorities were asked 118 times. New operation 87 sets the attack priority info.
/// - Not generated: either state entered while it is already the machine's state or its temporary state (the other's exit deletes
///   the one machine), or without a controlling player (their machines' attack reads it).
/// - Planted bugs: plants 3330-3337 at 30,000 cases, each caught but one (2 to 10,956 mismatches): the first scan's random range,
///   the hunt's weapon-lock release, the attack from idle, the scan rate, the all-hunt retry without priorities, the team target's
///   priority tie, the area's success without a victim. **3337 (the AI's machine not locked around the machine's update) is not
///   caught, and cannot be here**: nothing inside the machine sets the AI's machine's state, and `friend_setGoalObject` unlocks it on
///   purpose. Earlier plants re-run: all caught but 3313 and 3315 (the chase states' edge branches: see the porting graph).
/// - The first runs found two port defects, both fixed: the idle state's first update asked whether its own machine was idle where
///   upstream asks the AI's (`ai->isIdle()`: in a sub-machine, the AI's machine is hunting), and the view of the AI's machine lent to a
///   sub-machine was taken after the sub-machine was taken out of its state, so the AI's machine was not in an attack state while its
///   hunt (or attack-move) machine attacked.
///
/// # Results (2026-10-28, batch A11; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - The attack-move state (with its machine of idle, crate and attack states) and the crate state: 30,000 constructions and
///   1,799,110 operations, **0 mismatches** in the results, the call logs and the dumps (which add both states, the attack-move
///   machine's state, goal, crate and attack states). After an operation the machine was attack-moving 69,222 times (its machine
///   attacking 330 times; its crate state never: the crate is never found) and in the crate state 1,982 times.
/// - Planted bugs: plants 3320-3327 at 30,000 cases, each caught (10 to 344 mismatches): the crate's delay, the auto-acquired
///   command source, the repath after a retargeting, the retry's sleep, the retry count, the machine not cleared on entry, the
///   close-enough distance's goal, the AI's goal set on the inner machine. Earlier plants re-run: all caught but 3316 (see the
///   porting graph: at the edge of the scene's reach, as 3315).
/// - The first run found one port defect, fixed: the attack state's exit cleared the goal of its own machine instead of the AI's
///   (`ai->friend_setGoalObject`), which differs once the attack state is inside the attack-move machine.
///
/// # Results (2026-10-28, batch A10b; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases, the default from now on)
///
/// - The attack machine's pursue and approach states (no placeholders left in it): 30,000 constructions and 1,799,000 operations,
///   **0 mismatches** in the results, the call logs and the dumps (which add the chase states' members). After an operation the
///   machine was in an attack state 15,008 times; its attack machine chasing 6,277 times, approaching 2,269, aiming 5,276, firing
///   1,186.
/// - Not generated: an attack state entered without a controlling player (the chase states read it unchecked); a player is kept
///   while the idle state is in the machine (its mood target reads it unchecked: before this batch the C++ stand-in tolerated a
///   missing one), and without a player the team attacks no common target.
/// - Planted bugs: plants 3310-3317 at 30,000 cases, each caught but one: the recompute timestamp's wrap (15), the slow victim
///   (13), the direction away (45), the follow continue (2), the snap to the goal (62), the contact weapon's extra distance (1),
///   the initial approach's targets (62). **3315 (the pursuit's 0.95 speed match) is not caught**: the in-range pursuit branch is
///   reached about once in a thousand cases (it was caught by 3 mismatches before the last generator change, by none after).
///   All earlier AI plants re-run at 30,000: caught.
///
/// # Results (2026-10-28, batch A10a; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 10,000 cases)
///
/// - The attack states (ids 9-12, now `AIAttackState` with its attack machine: aim, fire, the chase placeholders) entered by
///   `setState`, the commands and the idle state's mood; while one is current, half the operations are the AI's update. 10,000
///   constructions and 599,643 operations, **0 mismatches** in the results, the call logs and the dumps (which add the attack
///   states' machines, their goals and aim states). After an operation the machine was in an attack state 6,989 times; its attack
///   machine was chasing 3,658 times, approaching 492, aiming 1,872, firing 496; the weapon was fired at objects 20 times, at
///   positions 34, through linked turrets 14; the turrets were given targets 7,146 times.
/// - Not generated (upstream dereferences a deleted machine): an attack state entered while it is already the machine's state or
///   its temporary state.
/// - Planted bugs: in the Rust source plants 3300-3309, each caught at 10,000 cases (1 to 556 mismatches): the team-change
///   relationship check, the aim state's locomotor goal on exit, the stealth flag, the locked weapon, the aim threshold's floor,
///   the new victim's goal in the AI's machine (1), the do-not-crush kind, the forced conditions' order, turning in place, the
///   attack possible after moving (1). 3300 survived until the other objects had teams of their own. All earlier AI plants
///   (3240-3297) re-run: caught.
/// - Harness defects found and fixed: the AI's own object could share an id with another object; the C++ data's linked-turret flag
///   was never set; the C++ AI data lacked the crush flag; the other objects' heights were asked anew by the Rust side within an
///   operation (the C++ side caches them per operation).
///
/// # Results (2026-10-28, batch A9; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 10,000 cases)
///
/// - The waypoint-path (as team, as individuals), exact (both), wander, wander-in-place and panic states (ids 2-5, 18, 19, 40;
///   `setState` and the commands now pick them often): 10,000 constructions and 599,696 operations, **0 mismatches** in the results,
///   the call logs and the dumps (which add those states' members, the team's current waypoint and the completed waypoint). After
///   an operation the machine was on a waypoint path 8,358 times, exact 11,732, wandering 5,787, wandering in place 27,112,
///   panicking 5,309; the team's waypoint was set 684 times, the team asked as a group 94 times, the skirmish test 180 times.
/// - Not generated, as upstream dereferences what is not there: entering those states without a current locomotor, the as-team
///   state without a goal waypoint or a controlling player, or a state already the machine's state or its temporary state (one
///   instance: the other's end leaves it without a current waypoint) — such an entry is the idle state or command 0 instead;
///   `chooseLocomotorSetExplicit` while in them is the ultra-accurate setter instead; the controlling player is kept while the
///   as-team state is in the machine. 11 cases end early where a locomotor would still be missing (after `chooseLocomotorSet`).
/// - Planted bugs: in the Rust source plants 3290-3297, each caught at 10,000 cases (1 to 1,247 mismatches): the goal's east clamp,
///   the off-map goal appended every update, the panic state moving on after a failure, the random link not drawn, the wander
///   offset's minimum, the exact state's completed waypoint (caught once the dump had it), the skirmish distance with its height
///   (1 case, after the team's centre was put near the object), the wall's height. All earlier plants (3240-3282) re-run: caught.
/// - Harness defect found: the C++ side's save file was shared by planted runs in parallel, which made every plant look caught;
///   it is now one file per process.
///
/// # Results (2026-10-28, batch A8; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 10,000 cases)
///
/// - The follow-path, move-and-evacuate and move-and-delete states (ids 6, 7, 25, 26, 27; `setState` now picks them often):
///   10,000 constructions and 600,000 operations, **0 mismatches** in the results, the call logs and the dumps (which add those
///   states' members). After an operation the machine was following a path 30,422 times, evacuating 37,979, deleting 20,244; the
///   evacuations activated the team 169 times and the delete state destroyed the object 103 times.
/// - Planted bugs: in the Rust source plants 3280-3282, each caught ~~at 3,000 cases (262 to 310 result mismatches)~~
///   **CORRECTED 2026-10-28 (batch A9)**: those counts came from parallel runs sharing one save file (each loaded the others'
///   saves); with a file per process, at 10,000 cases: 15, 3 and 91 mismatches (call logs, call logs, dumps): the follow path's
///   retries skipped, its failed final adjustment ignored, the delete state's goal appended again every update.
///
/// # Results (2026-10-28, batch A7; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 10,000 cases)
///
/// - The move states (the machine's `AI_MOVE_TO`, `AI_MOVE_OUT_OF_THE_WAY`, `AI_MOVE_AND_TIGHTEN`, `AI_MOVE_AWAY_FROM_REPULSORS`,
///   entered by `setState`, the commands and the idle state's repulsors, run by the AI's update): 10,000 constructions and
///   600,000 operations, **0 mismatches** in the results, the call logs and the dumps (which add every move state's members).
///   After an operation the machine was in a move state 66,493 times (46,155 move-to, 3,190 out of the way, 10,072 tighten, 7,076
///   away from repulsors); 9,901 move sounds started and 19,211 removed.
/// - Planted bugs: in the Rust source plants 3270-3272, each caught: the repulsors' path recomputed without limit, the early finish
///   not continued, the ignored goal object's destination still adjusted.
///
/// # Results (2026-10-27, batch A6; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 10,000 cases)
///
/// - 10,000 constructions and 600,000 operations, a sixth of them A6's (12,867 `requestPath`, 19,344 `doPathfind`, 12,788
///   `computePath`, 13,000 `computeAttackPath`, the other requests, `setPathFromWaypoint`, the quick paths, the goal object through
///   the lock, the ultra-accurate flag): **0 mismatches** in the results, the call logs and the dumps. The stand-in searches were
///   asked 13,806 times, the pathfinder's other answers 7,961 times, the weapon's range 11,527 times.
/// - Planted bugs: in the Rust source plants 3260-3264, each caught: no spin wait for a quick re-request, the prior repulsor not
///   kept, the ultra-accurate destination not jammed in, the short attack path not unstacked, the path wait not kept after a
///   pathfind. 3262 survived the first run (no locomotor was ultra-accurate; op 86 sets it).
///
/// # Results (2026-10-27, batch A5; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 10,000 cases)
///
/// - 10,000 constructions and 600,000 operations, a quarter of them commands (128,461 `aiDoCommand`, every command type, and
///   9,013 `doQuickExit`): **0 mismatches** in the results, the call logs and the dumps. The commands reached 8,846 action-manager
///   questions, 1,566 move-away paths and 730 allies moved, 8,700 shot counts, 934 partition searches, 333 positions found, 540
///   container calls, 336 special modules.
/// - Planted bugs: in the Rust source plants 3250-3256, each caught: the player's forbidden commands allowed, the goal's north edge
///   not clipped, the retaliation not cleared before a guard, an AI-sourced move of a busy AI not temporary, the follow-append's
///   goal position dropped, a subdued container exited, the guard target type always in the first slot. 3250 survived the first run
///   (no data forbade player commands; the data now does one time in five). A4's plants re-run: all caught.
/// - Two harness defects, no port bug: the module lookups' name keys are function-local statics that outlive the name-generator
///   reset between cases (the three names are now registered first on both sides); a goal waypoint had no name to save.
///
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
    let cases: u64 = std::env::var("AI_A2_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(30_000);
    let mut ck = Checks::new();
    // (AI_A2_FROM: the first case, to re-run one)
    let from: u64 = std::env::var("AI_A2_FROM").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    for case in from..from + cases {
        run_case(0xA2_0000 + case, &mut ck);
    }
    println!(
        "COVERAGE: {} cases ({} ended at a mismatch); ops {:?}; doLocomotor sleeps none/longer {:?}; moved by goal none/path/explicit/angle {:?}; \
         path frames {}; final-position moves {}; locomotor changes {}; mood targets {}, victims {}, enter targets {}; \
         A4-A10 log records 70..=103 {:?}; current state idle/dead/wait/busy/other/moveTo/outOfWay/tighten/repulsors/followPath/evacuate/\
         delete/waypointPath/exact/wander/wanderInPlace/panic/attack/attackMove/crate/hunt/attackFollow/face/exit/enter/rappel {:?}; A9 cases ended undefined {}; attack machine \
         chase/approach/aim/fire {:?}; LOG_ATTACK kinds {:?}; attack-move machine idle/crate/attack {:?}; hunt machines attack/crate/idle {:?}; attack-follow machines idle/crate/attack {:?}; temporary states {}; loads {}",
        ck.cases, ck.ended_early, ck.ops, ck.sleeps, ck.goals_moved, ck.paths_followed, ck.final_pops, ck.loco_changes, ck.mood_targets, ck.victims,
        ck.enter_targets, ck.a4_logs, ck.a4_states, ck.a9_undefined, ck.attack_inner, ck.attack_logs, ck.attack_move_inner, ck.hunt_inner, ck.follow_inner, ck.temporary_states, ck.loads
    );
    ck.construction.finish();
    ck.results.finish();
    ck.log.finish();
    ck.dump.finish();
    ck.results_bug.finish_planted();
    ck.dump_bug.finish_planted();
}
