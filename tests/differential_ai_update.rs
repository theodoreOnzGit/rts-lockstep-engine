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
    KindOfMaskType, KindOfType, KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT, KINDOF_CASH_GENERATOR, KINDOF_COMMANDCENTER, KINDOF_DOZER, KINDOF_IGNORED_IN_GUI, KINDOF_IMMOBILE, KINDOF_INFANTRY,
    KINDOF_PRODUCED_AT_HELIPAD, KINDOF_PROJECTILE, KINDOF_CAN_BE_REPULSED, KINDOF_BRIDGE, KINDOF_PORTABLE_STRUCTURE, KINDOF_CAN_ATTACK,
    KINDOF_MINE, KINDOF_SPAWNS_ARE_THE_WEAPONS, KINDOF_STRUCTURE, KINDOF_AIRCRAFT, KINDOF_DONT_AUTO_CRUSH_INFANTRY,
    KINDOF_CAN_RAPPEL, KINDOF_DRONE,
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
use rts_lockstep_engine::common::xfer::{Xfer, XferMode, XferResult, XferVersion};
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::differential::AiEnv as Env;
use rts_lockstep_engine::logic::ai::ai_commands::AiCommandParms;
use rts_lockstep_engine::logic::ai::ai_paths;
use rts_lockstep_engine::logic::ai::ai_states::{self as states, AiState, AiStateHost, AiUpdateModule, PassengerCommand};
use rts_lockstep_engine::logic::ai::ai_waypoint_states::WaypointPathData;
use rts_lockstep_engine::logic::ai::ai_update::{AiHost, AiSubclass, AiUpdate, LocoGoalType};
use rts_lockstep_engine::logic::ai::ai_update_types::{AiStateType, CMD_FROM_AI, CMD_FROM_PLAYER};
use rts_lockstep_engine::logic::ai::assault_transport_ai_update::{AssaultTransport, AssaultTransportData, MemberCommand};
use rts_lockstep_engine::logic::ai::deploy_style_ai_update::{DeployStyle, DeployStyleData};
use rts_lockstep_engine::logic::ai::railed_transport_ai_update::{RailedTransport, RailedTransportData};
use rts_lockstep_engine::logic::ai::hack_internet_ai_update::{HackInternet, HackInternetData};
use rts_lockstep_engine::common::state_machine::{SmResult, StateMachine};
use rts_lockstep_engine::logic::ai::missile_ai_update::{self as missile, Missile, MissileData};
use rts_lockstep_engine::logic::ai::ai_states::DistanceKind;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::ai::supply_truck_ai_update::{self as supply, SupplyDockKind, SupplyTruck, SupplyTruckData};
use rts_lockstep_engine::common::game_common::VeterancyLevel;
use rts_lockstep_engine::common::damage_fx::FxListName;
use rts_lockstep_engine::logic::ai::deliver_payload_ai_update::{self as deliver, DeliverPayload, DeliverPayloadModuleData, DeliverState, ItemModule};
use rts_lockstep_engine::logic::modules::deliver_payload_data::DeliverPayloadData;
use rts_lockstep_engine::logic::radius_decal::RadiusDecalTemplate;
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
/// Batch A17: the dock's calls (`LOG_DOCK`).
const L_DOCK: u32 = 105;
/// Batch A20: the tunnel network's calls (`LOG_TUNNEL`).
const L_TUNNEL: u32 = 106;
/// Batch A21: the AI's snapshot's calls (`LOG_AI_SNAPSHOT`).
const L_AI_SNAPSHOT: u32 = 107;
/// Batch A30: a transport's passenger's command (`LOG_PASSENGER`).
const L_PASSENGER: u32 = 108;
const L_ANIM_FRAME: u32 = 109;
const L_CLEAR_MODEL_FLAGS: u32 = 110;
const L_MEMBER_AI: u32 = 111;
const L_RAILED: u32 = 112;
const L_HACK: u32 = 113;
const L_SUPPLY: u32 = 114;
const L_MISSILE: u32 = 115;
const L_DELIVER: u32 = 116;

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
    /// Batch A35: the AI's machine, lent while the supply truck's machine runs.
    ai_machines: Vec<StateMachine<AiState<Host>>>,
    /// Batch A37: the delivering AI's machine, lent while its AI runs; the visible payloads' next id (from 40 in each case, as the
    /// wrapper's) and the last one's position.
    deliver_machines: Vec<Option<StateMachine<DeliverState<Host>>>>,
    next_payload: u32,
    payload_pos: Coord3D,
    /// Batch A6: the pathfinder's ignored obstacle (`m_ignoreObstacleID`).
    pf_ignore: ObjectId,
    /// Batch A7: TheAudio's next handle (from 1 in each case).
    next_audio: u32,
    /// Batch A36: the next particle system's id (from 1 in each case, as the wrapper's).
    next_particle: u32,
    /// Batch A9: the team's current waypoint.
    team_waypoint: Option<u32>,
    /// Batch A10a: the attack frames lent while attack machines run; the current weapon's max shot count.
    attack_frames: Vec<AttackFrame>,
    max_shots: i32,
    /// The other objects whose height above the terrain this operation has asked (wrap_ai.cpp makes them anew every operation;
    /// a second question is answered from the object's cache, unlogged).
    others_height_asked: u32,
    /// Batch A21: the AI's last save (op 88), and its machine's state and temporary state then.
    ai_saved: Vec<u8>,
    ai_saved_states: Option<(u32, u32)>,
    /// Batch A18: whether this operation has cleared the body's clearable last attacker (wrap_ai.cpp's scene is copied anew every
    /// operation, so the clearing lasts the operation; the scene itself must not change before the C++ side runs it).
    attacker_cleared: bool,
    /// Batch A33: the last op reached what upstream leaves undefined (`StateMachineError::Undefined`): the case ends before the C++.
    undefined: bool,
}

impl Host {
    fn l(&mut self, code: u32, w: &[u32]) {
        self.log.push(code);
        self.log.push(w.len() as u32);
        self.log.extend_from_slice(w);
    }

    /// The object's drawable (the wrapper's `getDrawable`): a hacker's, a deployer's or (batch A37) a delivering AI's when the scene
    /// says so.
    fn drawable(&self) -> bool {
        self.env.subclass == 6 || (self.env.deploy_flags >> 4) & 1 != 0 || (self.env.subclass == 9 && self.env.deliver_flags & 1 != 0)
    }

    /// Batch A37: a `LOG_DELIVER` record of `w` then a name (its length, its bytes).
    fn l_named(&mut self, mut w: Vec<u32>, name: &[u8]) {
        w.push(name.len() as u32);
        w.extend(name.iter().map(|&b| u32::from(b)));
        self.l(L_DELIVER, &w);
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
            || (kind == KINDOF_DRONE && m & 131072 != 0)
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
    /// Batch A17: the dock's answers (`dockAnswer`).
    fn dock_answer(&self, kind: u32, words: &[u32]) -> bool {
        pick(p1_hash(self.env.seed, 80 + kind, words), self.env.dock_rate)
    }
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
    // batch A30: the transport's own contain (the stand-in's one answer), another object's AI, a passenger's command (logged)
    fn contain_passenger_allowed_to_fire(&mut self) -> bool {
        self.env.container_allows_fire != 0
    }
    fn object_has_ai(&self, id: ObjectId) -> bool {
        id == self.env.object_id || (1..32).contains(&id) && (self.env.ai_mask >> id) & 1 != 0
    }
    fn passenger_ai_command(&mut self, passenger: ObjectId, cmd: PassengerCommand) {
        let (kind, victim, pos, shots, src) = match cmd {
            PassengerCommand::AttackObject { victim, max_shots, src } => (0, victim, None, max_shots, src),
            PassengerCommand::ForceAttackObject { victim, max_shots, src } => (1, victim, None, max_shots, src),
            PassengerCommand::AttackPosition { pos, max_shots, src } => (2, 0, Some(pos), max_shots, src),
        };
        let v = if self.object_exists(victim) { victim as u32 } else { 0 };
        let b = pos.map_or([0; 3], |p: Coord3D| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
        self.l(L_PASSENGER, &[kind, passenger as u32, v, b[0], b[1], b[2], shots as u32, src as u32]);
    }
    // batch A31: DeployStyleAIUpdate's reach (the wrapper's stand-ins: the cleared conditions in increasing order; "Deploy" is
    // "s4", "Undeploy" "s5", when the scene's sound bits say so, else the empty sound; the drawable when the scene gives one)
    fn clear_model_condition_flags(&mut self, clear: &[ModelConditionFlagType]) {
        let mut w: Vec<u32> = clear.iter().map(|&c| c as u32).collect();
        w.sort_unstable();
        w.dedup();
        self.l(L_CLEAR_MODEL_FLAGS, &w);
    }
    fn play_per_unit_sound(&mut self, name: &[u8]) {
        let k = match name {
            b"Deploy" | b"UnitUnpack" => 4,
            b"Undeploy" | b"UnitPack" => 5,
            b"UnitCashPing" => 6,
            b"StartDive" => 8,
            _ => -1,
        };
        let named = k >= 0 && (self.env.sounds >> k) & 1 != 0;
        self.audio_add(if named { k as u32 } else { u32::MAX });
    }
    fn set_drawable_animation_frame(&mut self, frame: i32) {
        if self.drawable() {
            self.l(L_ANIM_FRAME, &[frame as u32]);
        }
    }
    // batch A32: the assault members (the wrapper's stand-ins: their bodies' health by hash over 100, contained when the assault
    // transport's passengers, their AIs logged cut points answering by hash and the scene)
    fn object_health(&mut self, id: ObjectId) -> Option<(f32, f32)> {
        let k = if (1..32).contains(&id) { id as u32 } else { 0 };
        let h = [100.0f32, 100.0, 55.0, 15.0][((p1_hash(self.env.seed, 132, &[k, self.env.loco.frame >> 5]) >> 8) & 3) as usize];
        Some((h, 100.0))
    }
    fn object_is_contained(&mut self, id: ObjectId) -> bool {
        self.env.subclass == 4 && (1..32).contains(&id) && (self.env.contained_mask >> id) & 1 != 0
    }
    fn member_ai_last_command_source(&mut self, id: ObjectId) -> CommandSourceType {
        let ai_src = pick(p1_hash(self.env.seed, 130, &[id as u32, self.env.loco.frame]), self.env.object_rate);
        let s = if ai_src { CMD_FROM_AI } else { CMD_FROM_PLAYER };
        self.l(L_MEMBER_AI, &[0, id as u32, s as u32]);
        s
    }
    fn member_ai_set_allowed_to_chase(&mut self, id: ObjectId, allowed: bool) {
        self.l(L_MEMBER_AI, &[1, id as u32, u32::from(allowed)]);
    }
    fn member_ai_state_type(&mut self, id: ObjectId) -> AiStateType {
        let s = self.env.other_ai_state;
        self.l(L_MEMBER_AI, &[4, id as u32, s as u32]);
        s as AiStateType
    }
    fn member_ai_is_moving(&mut self, id: ObjectId) -> bool {
        let r = pick(p1_hash(self.env.seed, 131, &[id as u32, self.env.loco.frame]), 50);
        self.l(L_MEMBER_AI, &[5, id as u32, u32::from(r)]);
        r
    }
    fn member_ai_goal_object(&mut self, id: ObjectId) -> ObjectId {
        let g = self.env.other_ai_goal;
        let g = if self.object_exists(g) { g } else { 0 };
        self.l(L_MEMBER_AI, &[6, id as u32, g as u32]);
        g
    }
    // batch A33: the railed transport's waypoints (the wrapper's names: "wpN", and "railStartNN"/"railEndNN" at waypoints
    // (3 NN) % 8 and (3 NN + 1) % 8 when the scene's masks say so; the id is the index) and docks (logged)
    fn waypoint_by_name(&mut self, name: &[u8]) -> Option<u32> {
        let name = std::str::from_utf8(name).ok()?;
        let num = |rest: &str| -> Option<u32> {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse().ok()
        };
        if let Some(i) = name.strip_prefix("wp").and_then(num) {
            return (i < 8).then_some(i);
        }
        if let Some(i) = name.strip_prefix("railStart").and_then(num) {
            return ((1..=32).contains(&i) && (self.env.rail_start_mask >> (i - 1)) & 1 != 0).then_some((3 * i) % 8);
        }
        if let Some(i) = name.strip_prefix("railEnd").and_then(num) {
            return ((1..=32).contains(&i) && (self.env.rail_end_mask >> (i - 1)) & 1 != 0).then_some((3 * i + 1) % 8);
        }
        None
    }
    fn waypoint_id(&mut self, way: u32) -> u32 {
        way
    }
    // batch A34: the hacker's world (the wrapper's stand-ins: the scene's one team "team0", buttons "btnN", the money, tracker and
    // locally-controlled bits, LOG_HACK)
    fn team_name(&mut self, _team: u32) -> Vec<u8> {
        b"team0".to_vec()
    }
    fn team_by_name(&mut self, name: &[u8]) -> Option<u32> {
        (name == b"team0").then_some(0)
    }
    fn command_button_name(&mut self, button: u32) -> Vec<u8> {
        format!("btn{button}").into_bytes()
    }
    fn command_button_by_name(&mut self, name: &[u8]) -> Option<u32> {
        (0..4).find(|i| format!("btn{i}").as_bytes() == name)
    }
    fn set_drawable_animation_loop_duration(&mut self, frames: u32) {
        self.l(L_HACK, &[5, frames]);
    }
    fn player_has_money(&mut self) -> bool {
        self.env.hack_flags & 1 != 0
    }
    fn veterancy_level(&mut self) -> Option<VeterancyLevel> {
        (self.env.hack_flags & 2 != 0).then(|| {
            [VeterancyLevel::Regular, VeterancyLevel::Veteran, VeterancyLevel::Elite, VeterancyLevel::Heroic][self.env.hack_level as usize]
        })
    }
    fn money_deposit(&mut self, amount: u32) {
        self.l(L_HACK, &[0, amount]);
    }
    fn score_add_money_earned(&mut self, amount: u32) {
        self.l(L_HACK, &[1, amount]);
    }
    fn add_experience_points(&mut self, xp: u32) {
        self.l(L_HACK, &[2, xp, 1]);
    }
    fn is_locally_controlled(&mut self, id: ObjectId) -> bool {
        let bit = if id == self.env.object_id { 4 } else { 8 };
        let r = self.env.hack_flags & bit != 0;
        self.l(L_HACK, &[3, id as u32, u32::from(r)]);
        r
    }
    fn hack_cash_floating_text(&mut self, amount: u32) {
        self.l(L_HACK, &[4, amount]);
    }
    // batch A35: the supply truck's world (the wrapper's stand-ins: the scene's objects; the 2-D distance of the centres; LOG_SUPPLY)
    fn ai_machine_stack(&mut self) -> &mut Vec<StateMachine<AiState<Host>>> {
        &mut self.ai_machines
    }
    // batch A37: the delivery's world (the wrapper's stand-ins, LOG_DELIVER)
    fn deliver_machine_stack(&mut self) -> &mut Vec<Option<StateMachine<DeliverState<Host>>>> {
        &mut self.deliver_machines
    }
    fn delivery_decal_create(&mut self, t: &RadiusDecalTemplate, pos: &Coord3D, radius: f32, has_player: bool) {
        let w = vec![
            0,
            pos.x.to_bits(),
            pos.y.to_bits(),
            pos.z.to_bits(),
            radius.to_bits(),
            u32::from(has_player),
            t.shadow_type,
            t.min_opacity.to_bits(),
            t.max_opacity.to_bits(),
            t.opacity_throb_time,
            t.color as u32,
            u32::from(t.only_visible_to_owning_player),
        ];
        self.l_named(w, &t.name.clone());
    }
    fn delivery_decal_update(&mut self) {
        self.l(L_DELIVER, &[1]);
    }
    fn delivery_decal_clear(&mut self) {
        self.l(L_DELIVER, &[2]);
    }
    fn play_per_unit_sound_at(&mut self, name: &[u8], pos: &Coord3D) {
        let k = if name == b"StartDive" { 8 } else { -1 };
        if k >= 0 && (self.env.sounds >> k) & 1 != 0 {
            self.l(L_DELIVER, &[3, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
            let h = self.next_audio;
            self.next_audio += 1;
            self.l(L_AUDIO_ADD, &[k as u32, 0, h]);
        }
    }
    fn set_weapon_lock(&mut self, slot: i32, lock: i32) {
        self.l(L_DELIVER, &[4, slot as u32, lock as u32]);
    }
    fn fx_do_pos(&mut self, fx: &FxListName, pos: &Coord3D) {
        if let Some(name) = fx {
            let k = u32::from(name.as_slice() == b"fx1");
            self.l(L_DELIVER, &[5, k, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
        }
    }
    fn has_drawable(&mut self) -> bool {
        self.drawable()
    }
    fn drawable_show_sub_object(&mut self, name: &[u8], show: bool) {
        self.l_named(vec![6, u32::from(show)], name);
    }
    fn drawable_update_sub_objects(&mut self) {
        self.l(L_DELIVER, &[7]);
    }
    fn drawable_pristine_bone_position(&mut self, prefix: &[u8], start_index: i32) -> Option<Coord3D> {
        let n = p1_hash(self.env.seed, 140, &[start_index as u32, self.env.loco.frame]) & 1;
        self.l_named(vec![8, start_index as u32, 1, n], prefix);
        (n != 0).then_some(Coord3D { x: 2.0 * start_index as f32, y: 3.0, z: 1.0 })
    }
    fn drawable_convert_bone_pos_to_world_pos(&mut self, pos: &Coord3D) -> Coord3D {
        let o = *self.thing.position();
        let w = Coord3D { x: pos.x + o.x, y: pos.y + o.y, z: pos.z + o.z };
        self.l(L_DELIVER, &[9, w.x.to_bits(), w.y.to_bits(), w.z.to_bits()]);
        w
    }
    fn object_destroy(&mut self, id: ObjectId) {
        self.l(L_DESTROY_OBJECT, &[id as u32]);
    }
    fn object_set_position(&mut self, id: ObjectId, pos: &Coord3D) {
        self.l(L_DELIVER, &[10, id as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
    }
    fn object_contain_set_override_destination(&mut self, _id: ObjectId, pos: &Coord3D) {
        self.l(L_DELIVER, &[11, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
    }
    fn object_aim_update_module(&mut self, id: ObjectId, module: ItemModule, pos: &Coord3D) {
        let i = match module {
            ItemModule::GenerateMinefield => 0,
            ItemModule::SmartBombTargetHoming => 1,
        };
        if (1..32).contains(&id) && (self.env.deliver_module_masks[i] >> id) & 1 != 0 {
            self.l(L_DELIVER, &[12 + i as u32, id as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
        }
    }
    fn object_apply_force(&mut self, _id: ObjectId, force: &Coord3D) {
        self.l(L_DELIVER, &[14, force.x.to_bits(), force.y.to_bits(), force.z.to_bits()]);
    }
    fn thing_template_by_name(&mut self, name: &[u8]) -> Option<u32> {
        match name {
            b"pt0" => Some(0),
            b"pt1" => Some(1),
            _ => None,
        }
    }
    fn payload_new(&mut self, name: &[u8]) -> ObjectId {
        let t = self.thing_template_by_name(name).expect("a payload template");
        let id = self.next_payload;
        self.next_payload += 1;
        self.payload_pos = Coord3D::default();
        self.l(L_DELIVER, &[15, t, id]);
        id as ObjectId
    }
    fn payload_set_producer(&mut self, id: ObjectId) {
        let me = self.env.object_id as u32;
        self.l(L_DELIVER, &[16, id as u32, me]);
    }
    fn payload_set_position(&mut self, id: ObjectId, pos: &Coord3D) {
        self.payload_pos = *pos;
        self.l(L_DELIVER, &[17, id as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
    }
    fn payload_position(&mut self, _id: ObjectId) -> Coord3D {
        self.payload_pos
    }
    fn payload_set_orientation(&mut self, id: ObjectId, angle: f32) {
        self.l(L_DELIVER, &[18, id as u32, angle.to_bits()]);
    }
    fn payload_mass(&mut self, _id: ObjectId) -> f32 {
        self.env.deliver_world[1]
    }
    fn payload_apply_motive_force(&mut self, id: ObjectId, f: &Coord3D) {
        self.l(L_DELIVER, &[20, id as u32, f.x.to_bits(), f.y.to_bits(), f.z.to_bits()]);
    }
    fn payload_is_projectile(&mut self, _id: ObjectId) -> bool {
        self.env.deliver_flags & 2 != 0
    }
    fn payload_projectile_fire(&mut self, id: ObjectId, pos: &Coord3D, weapon: u32, exhaust: Option<u32>) {
        self.l(L_DELIVER, &[21, id as u32, 0, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), weapon, exhaust.unwrap_or(u32::MAX)]);
    }
    fn payload_center_of_mass_offset(&mut self, _id: ObjectId) -> f32 {
        self.env.deliver_world[2]
    }
    fn payload_set_pitch_rate(&mut self, id: ObjectId, rate: f32) {
        self.l(L_DELIVER, &[23, id as u32, rate.to_bits()]);
    }
    fn payload_has_ai(&mut self, _id: ObjectId) -> bool {
        self.env.deliver_flags & 4 != 0
    }
    fn payload_ai_move_to_position(&mut self, id: ObjectId, pos: &Coord3D) {
        self.l(L_DELIVER, &[24, id as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), CMD_FROM_AI as u32]);
    }
    fn weapon_template_projectile_exhaust(&mut self, template: u32, level: VeterancyLevel) -> Option<u32> {
        let k = (template + level as u32) % 4;
        (k != 3).then_some(k)
    }
    fn terrain_extent_including_border(&mut self) -> (Coord3D, Coord3D) {
        let b = self.env.deliver_world[0];
        (Coord3D { x: -b, y: -b, z: 0.0 }, Coord3D { x: self.env.extent_hi[0] + b, y: self.env.extent_hi[1] + b, z: 0.0 })
    }
    fn partition_register(&mut self) {
        let me = self.env.object_id as u32;
        self.l(L_DELIVER, &[25, me]);
    }
    fn partition_unregister(&mut self) {
        let me = self.env.object_id as u32;
        self.l(L_DELIVER, &[26, me]);
    }
    fn physics_turning(&mut self) -> i32 {
        self.env.deliver_turning
    }
    fn unit_direction_vector_3d(&mut self) -> Coord3D {
        self.thing.unit_direction_vector_3d()
    }
    fn supply_find_best_center(&mut self) -> ObjectId {
        let o = self.env.supply_best[0];
        let o = if self.object_exists(o) { o } else { 0 };
        self.l(L_SUPPLY, &[0, o as u32]);
        o
    }
    fn supply_find_best_warehouse(&mut self) -> ObjectId {
        let o = self.env.supply_best[1];
        let o = if self.object_exists(o) { o } else { 0 };
        self.l(L_SUPPLY, &[1, o as u32]);
        o
    }
    fn player_find_closest_by_kind_of(&mut self, kind: KindOfType) -> ObjectId {
        let i = match kind {
            KINDOF_CASH_GENERATOR => 0,
            KINDOF_COMMANDCENTER => 1,
            _ => 2,
        };
        let o = self.env.supply_closest[i];
        let o = if self.object_exists(o) { o } else { 0 };
        self.l(L_SUPPLY, &[2, kind as u32, o as u32]);
        o
    }
    fn distance_squared_to_object_2d(&mut self, other: ObjectId) -> f32 {
        let a = *self.thing.position();
        let b = self.object_position(other);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let d = dx * dx + dy * dy;
        self.l(L_SUPPLY, &[3, other as u32, d.to_bits()]);
        d
    }
    fn dock_supply_kind(&mut self, dock: ObjectId) -> SupplyDockKind {
        let other = (1..32).contains(&dock);
        if other && (self.env.supply_dock_masks[0] >> dock) & 1 != 0 {
            SupplyDockKind::Warehouse
        } else if other && (self.env.supply_dock_masks[1] >> dock) & 1 != 0 {
            SupplyDockKind::Center
        } else {
            SupplyDockKind::None
        }
    }
    fn drawable_update_supply_status(&mut self, max_boxes: i32, boxes: i32) {
        if self.drawable() {
            self.l(L_SUPPLY, &[4, max_boxes as u32, boxes as u32]);
        }
    }
    fn play_supplies_depleted_voice(&mut self) {
        self.audio_add(7);
    }
    // batch A36: the missile's world (the wrapper's stand-ins: three weapon templates "wtN" and exhaust templates "psN", the
    // scene's speeds, bits and rates; LOG_MISSILE)
    fn object_weapon_bonus_condition(&mut self, id: ObjectId) -> u32 {
        p1_hash(self.env.seed, 135, &[id as u32]) & 0xff
    }
    fn position_projectile_for_launch(&mut self, launcher: ObjectId, wslot: i32, barrel: i32) {
        self.l(L_MISSILE, &[2, launcher as u32, wslot as u32, barrel as u32]);
    }
    fn weapon_template_speed(&mut self, template: u32) -> f32 {
        self.env.missile_weapon_speeds[template as usize]
    }
    fn weapon_template_should_projectile_collide_with(&mut self, template: u32, launcher: ObjectId, other: ObjectId, victim: ObjectId) -> bool {
        let w = [template, launcher as u32, other as u32, victim as u32];
        let r = pick(p1_hash(self.env.seed, 134, &w), self.env.missile_collide_rate);
        self.l(L_MISSILE, &[3, w[0], w[1], w[2], w[3], u32::from(r)]);
        r
    }
    fn weapon_template_die_on_detonate(&mut self, template: u32) -> bool {
        (self.env.missile_die_mask >> template) & 1 != 0
    }
    fn weapon_store_handle_projectile_detonation(&mut self, template: u32, bonus: u32, inflict_damage: bool) {
        self.l(L_MISSILE, &[4, template, bonus, u32::from(inflict_damage)]);
    }
    fn weapon_template_name(&mut self, template: u32) -> Vec<u8> {
        format!("wt{template}").into_bytes()
    }
    fn weapon_template_by_name(&mut self, name: &[u8]) -> Option<u32> {
        (0..3).find(|i| format!("wt{i}").as_bytes() == name)
    }
    fn particle_system_create_attached(&mut self, template: u32) -> u32 {
        let id = self.next_particle;
        self.next_particle += 1;
        self.l(L_MISSILE, &[5, template, id]);
        id
    }
    fn particle_system_destroy(&mut self, id: u32) {
        self.l(L_MISSILE, &[6, id]);
    }
    fn particle_template_name(&mut self, template: u32) -> Vec<u8> {
        format!("ps{template}").into_bytes()
    }
    fn particle_template_by_name(&mut self, name: &[u8]) -> Option<u32> {
        (0..3).find(|i| format!("ps{i}").as_bytes() == name)
    }
    fn object_garrison_hit_killable(&mut self, _id: ObjectId) -> bool {
        // (the other objects' one contain stand-in)
        self.env.contain_count > 0 && self.env.garrisonable != 0 && self.env.missile_garrison_immune == 0
    }
    fn object_is_kind_of_multi(&mut self, id: ObjectId, must_be_set: &KindOfMaskType, must_be_clear: &KindOfMaskType) -> bool {
        for k in 0..31 {
            if must_be_set.test(k) && !self.object_is_kind_of(id, k) {
                return false;
            }
            if must_be_clear.test(k) && self.object_is_kind_of(id, k) {
                return false;
            }
        }
        true
    }
    fn object_score_the_kill(&mut self, _killer: ObjectId, victim: ObjectId) {
        // (the stand-in's scoreTheKill: whoever scores)
        self.l(L_RAPPEL, &[3, victim as u32]);
    }
    fn fx_do_obj(&mut self, _fx: Option<u32>, primary: ObjectId) {
        self.l(L_RAPPEL, &[5, primary as u32]);
    }
    fn body_max_health(&mut self) -> f32 {
        100.0
    }
    fn attempt_damage(&mut self, d: &mut DamageInfo) {
        let i = &d.input;
        self.l(L_MISSILE, &[0, i.damage_type as u32, i.death_type as u32, i.source_id as u32, i.amount.to_bits()]);
    }
    fn set_drawable_hidden(&mut self, hidden: bool) {
        if self.drawable() {
            self.l(L_MISSILE, &[1, u32::from(hidden)]);
        }
    }
    fn distance_squared_to_object(&mut self, other: ObjectId, kind: DistanceKind) -> f32 {
        let a = *self.thing.position();
        let b = self.object_position(other);
        let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
        let (k, d) = match kind {
            DistanceKind::Center2d => (0, dx * dx + dy * dy),
            DistanceKind::BoundingSphere3d => (1, dx * dx + dy * dy + dz * dz),
        };
        self.l(L_MISSILE, &[7, k, other as u32, d.to_bits()]);
        d
    }
    fn distance_squared_to_position_2d(&mut self, pos: &Coord3D) -> f32 {
        let a = *self.thing.position();
        let (dx, dy) = (pos.x - a.x, pos.y - a.y);
        let d = dx * dx + dy * dy;
        self.l(L_MISSILE, &[7, 0, 0, d.to_bits()]);
        d
    }
    fn calculate_countermeasure_to_divert_to(&mut self, victim: ObjectId) -> ObjectId {
        let o = self.env.missile_countermeasure;
        let o = if self.object_exists(o) { o } else { 0 };
        self.l(L_MISSILE, &[8, victim as u32, o as u32]);
        o
    }
    fn railed_set_dock_open(&mut self, open: bool) {
        if self.env.rail_flags & 1 != 0 {
            self.l(L_RAILED, &[0, u32::from(open)]);
        }
    }
    fn railed_dock_present(&mut self) -> bool {
        self.env.rail_flags & 2 != 0
    }
    fn railed_dock_is_loading_or_unloading(&mut self) -> bool {
        let r = pick(p1_hash(self.env.seed, 133, &[self.env.loco.frame]), 30);
        self.l(L_RAILED, &[1, u32::from(r)]);
        r
    }
    fn railed_dock_unload_all(&mut self) {
        self.l(L_RAILED, &[2]);
    }
    fn member_ai_command(&mut self, id: ObjectId, cmd: MemberCommand) {
        let w = match cmd {
            MemberCommand::Exit { transport, src } => vec![2, id as u32, transport as u32, src as u32],
            MemberCommand::Enter { transport, src } => vec![3, id as u32, transport as u32, src as u32],
            MemberCommand::AttackObject { victim, max_shots, src } => vec![7, id as u32, victim as u32, max_shots as u32, src as u32],
            MemberCommand::AttackMoveToPosition { pos, max_shots, src } => {
                vec![8, id as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), max_shots as u32, src as u32]
            }
            MemberCommand::MoveToPosition { pos, src } => vec![9, id as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), src as u32],
        };
        self.l(L_MEMBER_AI, &w);
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
        // (batch A21: not the goal's z, which upstream's updateGoal does not read)
        self.l(L_PF_UPDATE_GOAL, &[pos.x.to_bits(), pos.y.to_bits(), layer as u32]);
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
    fn contain_idle_passengers(&mut self, src: CommandSourceType) {
        // (the stand-in container lists passengers with an AI only for a transport: batch A30's cut point, logged; no contain, no
        // loop)
        if self.env.has_contain == 0 {
            return;
        }
        let me = self.env.object_id;
        for p in self.contain_items(me) {
            if self.object_has_ai(p) {
                self.l(L_PASSENGER, &[3, p as u32, 0, 0, 0, 0, 0, src as u32]);
            }
        }
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
    fn go_prone(&mut self) {
        if self.env.modules & 2 != 0 {
            self.l(L_MODULE, &[1]);
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
        // (batch A30: the transport's passengers with an AI too; batch A32: the assault transport's with an AI only; batch A37: the
        // delivering AI's with an AI too)
        let transport = self.env.subclass == 2 || self.env.subclass == 9;
        let assault = self.env.subclass == 4;
        (1..32)
            .filter(|&i| {
                let has_ai = (self.env.ai_mask >> i) & 1 != 0;
                (self.env.contained_mask >> i) & 1 != 0 && (if assault { has_ai } else { transport || !has_ai }) && self.object_exists(i)
            })
            .collect()
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
    // ---- batch A16
    fn player_difficulty(&mut self) -> i32 {
        self.env.difficulty
    }
    fn choose_victim_always_uses_normal(&mut self) -> bool {
        self.env.choose_normal != 0
    }
    fn object_is_selectable(&mut self, _id: ObjectId) -> bool {
        true // (the stand-in's)
    }
    fn closest_object_on_squad(&mut self, squad: &[ObjectId]) -> ObjectId {
        let mut best: Option<(u32, ObjectId)> = None;
        for &id in squad {
            if !self.object_exists(id) {
                continue;
            }
            let h = p1_hash(self.env.seed, 75, &[id as u32]);
            if best.is_none_or(|(b, _)| h < b) {
                best = Some((h, id));
            }
        }
        let ans = best.map_or(0, |(_, id)| id);
        let me = self.env.object_id as u32;
        self.l(L_CLOSEST_OBJECT, &[u32::MAX, me, 1_000_000.0f32.to_bits(), 0, ans as u32]); // (FROM_CENTER_2D)
        ans
    }
    // ---- batch A17
    fn object_has_dock(&mut self, id: ObjectId) -> bool {
        (1..32).contains(&id) && (self.env.dock_mask >> id) & 1 != 0
    }
    fn dock_is_open(&mut self, _dock: ObjectId) -> bool {
        self.env.dock_flags & 1 != 0
    }
    fn dock_cancel(&mut self, _dock: ObjectId) {
        self.l(L_DOCK, &[0]);
    }
    fn dock_reserve_approach(&mut self, _dock: ObjectId, advance: bool) -> Option<(Coord3D, i32)> {
        let kind = if advance { 2 } else { 1 };
        let r = self.dock_answer(kind, &[self.env.loco.frame]);
        self.l(L_DOCK, &[kind, u32::from(r)]);
        let p = self.env.dock_pos;
        r.then_some((Coord3D { x: p[0], y: p[1], z: p[2] }, self.env.dock_index))
    }
    fn dock_is_clear_to_enter(&mut self, _dock: ObjectId) -> bool {
        let r = self.dock_answer(3, &[self.env.loco.frame]);
        self.l(L_DOCK, &[3, u32::from(r)]);
        r
    }
    fn dock_is_clear_to_advance(&mut self, _dock: ObjectId, index: i32) -> bool {
        let r = self.dock_answer(4, &[self.env.loco.frame, index as u32]);
        self.l(L_DOCK, &[4, index as u32, u32::from(r)]);
        r
    }
    fn dock_is_allow_passthrough(&mut self, _dock: ObjectId) -> bool {
        self.env.dock_flags & 2 != 0
    }
    fn dock_is_rally_point_after_dock(&mut self, _dock: ObjectId) -> bool {
        self.env.dock_flags & 4 != 0
    }
    fn dock_position(&mut self, _dock: ObjectId, which: u32) -> Coord3D {
        let kind = 5 + which;
        self.l(L_DOCK, &[kind]);
        let p = self.env.dock_pos;
        Coord3D { x: p[0] + 0.1 * (kind - 4) as f32, y: p[1], z: p[2] }
    }
    fn dock_reached(&mut self, _dock: ObjectId, which: u32) {
        self.l(L_DOCK, &[8 + which]);
    }
    fn dock_action(&mut self, _dock: ObjectId, drone: ObjectId) -> bool {
        let r = self.dock_answer(12, &[self.env.loco.frame, drone as u32]);
        self.l(L_DOCK, &[12, drone as u32, u32::from(r)]);
        r
    }
    fn object_rally_point(&mut self, _id: ObjectId) -> Option<Coord3D> {
        let (f, p) = (self.env.dock_flags, self.env.dock_pos);
        (f & 8 != 0 && f & 16 != 0).then_some(Coord3D { x: p[0] - 15.0, y: p[1] + 5.0, z: p[2] })
    }
    fn supply_truck_action_delay(&mut self, _dock: ObjectId) -> Option<u32> {
        None // (no supply truck here)
    }
    fn player_object_ids(&mut self) -> Vec<ObjectId> {
        (1..32).filter(|&i| self.object_exists(i)).collect()
    }
    fn object_producer_id(&mut self, id: ObjectId) -> ObjectId {
        if (1..32).contains(&id) && (self.env.drone_mask >> id) & 1 != 0 { self.env.object_id } else { 0 }
    }
    // ---- batch A18
    fn clearable_last_attacker(&mut self) -> ObjectId {
        if self.attacker_cleared { 0 } else { self.env.clearable_attacker }
    }
    fn clear_last_attacker(&mut self) {
        self.l(L_ATTACK, &[17]);
        self.attacker_cleared = true;
    }
    fn template_is_enter_guard(&mut self) -> bool {
        self.env.guard_flags & 1 != 0
    }
    fn template_is_hijack_guard(&mut self) -> bool {
        self.env.guard_flags & 2 != 0
    }
    fn frame_objects_changed_trigger_areas(&mut self) -> u32 {
        self.env.objects_changed_frame
    }
    fn ai_data_guard_enemy_scan_rate(&mut self) -> u32 {
        self.env.guard_scan_rate
    }
    fn ai_data_guard_enemy_return_scan_rate(&mut self) -> u32 {
        self.env.guard_return_scan_rate
    }
    fn ai_data_guard_chase_unit_frames(&mut self) -> u32 {
        self.env.guard_chase_frames
    }
    fn polygon_radius(&mut self, polygon: i32) -> f32 {
        30.0 + 5.0 * polygon as f32
    }
    fn closest_guard_target(&mut self, pos: &Coord3D, range: f32, filters: u32, area: Option<i32>) -> ObjectId {
        let poly = if filters & 8 != 0 { area.map_or(0, |a| a as u32 + 1) } else { 0 };
        self.l(L_CLOSEST_OBJECT, &[0xffff_fffe, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), range.to_bits(), 0, filters, poly]);
        let c = self.env.closest_attack;
        if self.object_exists(c) { c } else { 0 }
    }
    fn has_team(&mut self) -> bool {
        true // (the stand-in's team)
    }
    // ---- batch A20
    fn tunnel_system(&mut self) -> bool {
        self.env.player_type >= 0 // (the player's, always made)
    }
    fn tunnel_container_list(&mut self) -> Vec<ObjectId> {
        let mut list = Vec::new();
        if self.env.container_id != 0 {
            list.push(self.env.container_id as ObjectId);
        }
        list.extend(self.env.tunnel_ids.iter().filter(|&&t| t != 0).map(|&t| t as ObjectId));
        list
    }
    fn tunnel_cur_nemesis(&mut self) -> ObjectId {
        let id = self.env.tunnel_nemesis;
        if self.object_exists(id) { id } else { 0 }
    }
    fn tunnel_update_nemesis(&mut self, target: ObjectId) {
        self.l(L_TUNNEL, &[0, target as u32]);
    }
    fn object_ai_goal_object(&mut self, id: ObjectId) -> Option<ObjectId> {
        // (`getAI()`: the scene's AI bit, as wrap_ai.cpp's; batch A23: the team's members are iterated whether or not they exist)
        if !(1..32).contains(&id) || (self.env.ai_mask >> id) & 1 == 0 {
            return None;
        }
        let g = self.env.other_ai_goal;
        Some(if self.object_exists(g) { g } else { 0 })
    }
    fn object_last_damage(&mut self, id: ObjectId) -> Option<(bool, ObjectId, u32)> {
        let k = id as u32;
        let source = (p1_hash(self.env.seed, 73, &[k]) % 32) as ObjectId;
        let stamp = self.env.tunnel_stamp.wrapping_add(p1_hash(self.env.seed, 74, &[k]) % 64);
        Some(((self.env.tunnel_no_effect >> k) & 1 != 0, source, stamp))
    }
    fn exit_object_in_a_hurry(&mut self, _container: ObjectId) {
        let id = self.env.object_id as u32;
        self.l(L_CONTAIN, &[8, id]);
    }
    // ---- batch A21
    fn update_module_xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut v: XferVersion = 1;
        xfer.xfer_version(&mut v, 1)?;
        xfer.xfer_unsigned_int(&mut self.next_call_frame)
    }
    fn update_module_crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    fn update_module_load_post_process(&mut self) {}
    fn polygon_trigger_name(&mut self, polygon: i32) -> Vec<u8> {
        format!("poly{polygon}").into_bytes()
    }
    fn polygon_trigger_by_name(&mut self, name: &[u8]) -> Option<i32> {
        (0..8).find(|i| format!("poly{i}").as_bytes() == name)
    }
    fn attack_info_name(&mut self, info: usize) -> Vec<u8> {
        format!("ap{info}").into_bytes()
    }
    fn attack_info_by_name(&mut self, name: &[u8]) -> Option<usize> {
        (0..2).find(|i| format!("ap{i}").as_bytes() == name)
    }
    fn waypoint_by_id(&mut self, id: u32) -> Option<u32> {
        (id < 8).then_some(id)
    }
    fn locomotor_set_xfer(&mut self, set: &mut LocomotorSet, cur: &mut Option<usize>, xfer: &mut dyn Xfer) -> Result<(), LocomotorSetError> {
        let frame = self.env.loco.frame;
        set.xfer_self_and_cur_loco_ptr(xfer, &self.store, &mut self.names, &mut self.rng, frame, cur)
    }
    fn turret_ai_snapshot(&mut self, which: usize, xfer: &mut dyn Xfer) -> XferResult<()> {
        if xfer.get_xfer_mode() == XferMode::Crc {
            self.l(L_AI_SNAPSHOT, &[1, which as u32]);
            return Ok(());
        }
        self.l(L_AI_SNAPSHOT, &[2, which as u32]);
        let mut v: XferVersion = 1;
        xfer.xfer_version(&mut v, 1)?;
        let mut w = which as i32;
        xfer.xfer_int(&mut w)
    }
    fn pf_update_pos(&mut self, pos: &Coord3D) {
        self.l(L_AI_SNAPSHOT, &[0, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
    }
    // ---- batch A23
    fn own_team_member_ids(&mut self) -> Vec<ObjectId> {
        (1..32).filter(|i| (self.env.team_mask >> i) & 1 != 0).collect()
    }
    fn object_ai_is_idle(&mut self, _id: ObjectId) -> bool {
        // (the other AIs' machine: AI_IDLE is idle; its other states (move-to, busy) are not)
        self.env.other_ai_state == 0
    }
    fn object_ai_goal_position(&mut self, _id: ObjectId) -> Coord3D {
        let p = self.env.other_ai_goal_pos;
        Coord3D { x: p[0], y: p[1], z: p[2] }
    }
    fn turret_target_object(&mut self, which: usize, clear_dead_targets: bool) -> Option<ObjectId> {
        let h = p1_hash(self.env.seed, 76, &[which as u32, u32::from(clear_dead_targets)]);
        self.l(L_AI_SNAPSHOT, &[5, which as u32, u32::from(clear_dead_targets)]);
        let o = ((h >> 4) % 32) as ObjectId;
        let o = if self.object_exists(o) { o } else { 0 };
        // (TARGET_OBJECT: 1)
        (h % 3 == 1).then_some(o).filter(|&o| o != 0)
    }
    fn turret_set_enabled(&mut self, which: usize, enabled: bool) {
        self.l(L_AI_SNAPSHOT, &[3, which as u32, u32::from(enabled)]);
    }
    fn turret_recenter(&mut self, which: usize) {
        self.l(L_AI_SNAPSHOT, &[4, which as u32]);
    }
    fn turret_is_enabled(&mut self, which: usize) -> bool {
        p1_hash(self.env.seed, 77, &[which as u32]) & 1 != 0
    }
    fn turret_is_in_natural_position(&mut self, which: usize) -> bool {
        p1_hash(self.env.seed, 78, &[which as u32]) & 1 != 0
    }
    fn turret_is_weapon_slot_on(&mut self, which: usize, wslot: WeaponSlotType) -> bool {
        p1_hash(self.env.seed, 79, &[which as u32, wslot as u32]) & 1 != 0
    }
    fn turret_is_trying_to_aim_at(&mut self, which: usize, victim: ObjectId) -> bool {
        let v = if self.object_exists(victim) { victim as u32 } else { 0 };
        p1_hash(self.env.seed, 80, &[which as u32, v]) & 1 != 0
    }
    fn turret_angle(&mut self, which: usize) -> f32 {
        (p1_hash(self.env.seed, 81, &[which as u32]) % 628) as f32 / 100.0 - 3.0
    }
    fn turret_pitch(&mut self, which: usize) -> f32 {
        (p1_hash(self.env.seed, 82, &[which as u32]) % 100) as f32 / 100.0
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
        // batch A18: the guard state (its machine: state, goal, sleep, members; its states' members)
        match sm.state(16) {
            Some(AiState::Guard { machine, data, .. }) => {
                v.push(u32::from(machine.is_some()));
                if let Some(m) = machine {
                    v.extend([m.get_current_state_id(), m.goal_object_id() as u32, m.peek_sleep_till(), data.target_to_guard as u32]);
                    v.push(data.area_to_guard.map_or(u32::MAX, |a| a as u32));
                    put_pos(&mut v, &data.position_to_guard);
                    v.extend([data.nemesis_to_attack as u32, data.guard_mode as u32]);
                    for (gid, kinds) in [(5000u32, 3u32), (5002, 1), (5005, 1)] {
                        match m.state(gid) {
                            Some(AiState::GuardAttack { exit, embedded, .. }) => {
                                v.extend([exit.conditions_to_consider as u32]);
                                put_pos(&mut v, &exit.center);
                                v.extend([exit.radius_sqr.to_bits(), exit.attack_give_up_frame]);
                                let e = match embedded.first() {
                                    Some(AiState::Attack { .. }) => 1,
                                    Some(_) if kinds == 3 => 2,
                                    _ => 0,
                                };
                                v.push(e);
                                // (the embedded attack state's follow flag and its machine's state)
                                if let Some(AiState::Attack { follow, machine, .. }) = embedded.first() {
                                    v.extend([u32::from(*follow), machine.as_ref().map_or(u32::MAX, |m| m.get_current_state_id())]);
                                }
                            }
                            _ => panic!("a guard attack state"),
                        }
                    }
                    match m.state(5003) {
                        Some(AiState::GuardReturn { mv, next_return_scan_time }) => {
                            put_move(&mut v, mv);
                            v.push(*next_return_scan_time);
                        }
                        _ => panic!("the guard return state"),
                    }
                    match m.state(5001) {
                        Some(AiState::GuardIdle { next_enemy_scan_time, guardee_pos, .. }) => {
                            v.push(*next_enemy_scan_time);
                            put_pos(&mut v, guardee_pos);
                        }
                        _ => panic!("the guard idle state"),
                    }
                    match m.state(5004) {
                        Some(AiState::GuardCrate { mv, delay_counter }) => {
                            put_move(&mut v, mv);
                            v.push(*delay_counter as u32);
                        }
                        _ => panic!("the guard crate state"),
                    }
                }
            }
            _ => panic!("the guard state"),
        }
        // batch A19: the guard-retaliate state (its machine: state, goal, sleep, members; its states' members)
        match sm.state(43) {
            Some(AiState::Guard { machine, data, .. }) => {
                v.push(u32::from(machine.is_some()));
                if let Some(m) = machine {
                    v.extend([m.get_current_state_id(), m.goal_object_id() as u32, m.peek_sleep_till()]);
                    put_pos(&mut v, &data.position_to_guard);
                    v.push(data.nemesis_to_attack as u32);
                    for (gid, kinds) in [(5000u32, 3u32), (5002, 1), (5005, 1)] {
                        match m.state(gid) {
                            Some(AiState::GuardAttack { exit, embedded, .. }) => {
                                v.push(exit.conditions_to_consider as u32);
                                put_pos(&mut v, &exit.center);
                                v.extend([exit.radius_sqr.to_bits(), exit.attack_give_up_frame]);
                                v.push(match embedded.first() {
                                    Some(AiState::Attack { .. }) => 1,
                                    Some(_) if kinds == 3 => 2,
                                    _ => 0,
                                });
                                if let Some(AiState::Attack { follow, machine, .. }) = embedded.first() {
                                    v.extend([u32::from(*follow), machine.as_ref().map_or(u32::MAX, |m| m.get_current_state_id())]);
                                }
                            }
                            _ => panic!("a guard-retaliate attack state"),
                        }
                    }
                    match m.state(5003) {
                        Some(AiState::GuardReturn { mv, next_return_scan_time }) => {
                            put_move(&mut v, mv);
                            v.push(*next_return_scan_time);
                        }
                        _ => panic!("the guard-retaliate return state"),
                    }
                    match m.state(5001) {
                        Some(AiState::GuardIdle { next_enemy_scan_time, .. }) => v.push(*next_enemy_scan_time),
                        _ => panic!("the guard-retaliate idle state"),
                    }
                    match m.state(5004) {
                        Some(AiState::GuardCrate { mv, delay_counter }) => {
                            put_move(&mut v, mv);
                            v.push(*delay_counter as u32);
                        }
                        _ => panic!("the guard-retaliate crate state"),
                    }
                }
            }
            _ => panic!("the guard-retaliate state"),
        }
        // batch A20: the tunnel-network guard state (its machine: state, goal, sleep, members; its states' members)
        match sm.state(21) {
            Some(AiState::Guard { machine, data, .. }) => {
                v.push(u32::from(machine.is_some()));
                if let Some(m) = machine {
                    v.extend([m.get_current_state_id(), m.goal_object_id() as u32, m.peek_sleep_till()]);
                    put_pos(&mut v, &data.position_to_guard);
                    v.extend([data.nemesis_to_attack as u32, data.guard_mode as u32]);
                    for (gid, inner) in [(5000u32, true), (5002, false), (5005, false)] {
                        match m.state(gid) {
                            Some(AiState::GuardAttack { exit, embedded, .. }) => {
                                v.push(exit.attack_give_up_frame);
                                if inner {
                                    v.push(0); // (m_scanForEnemy: never set)
                                }
                                match embedded.first() {
                                    Some(AiState::Attack { follow, machine, .. }) => {
                                        v.extend([1, u32::from(*follow), machine.as_ref().map_or(u32::MAX, |m| m.get_current_state_id())])
                                    }
                                    _ => v.push(0),
                                }
                            }
                            _ => panic!("a tunnel-network guard attack state"),
                        }
                    }
                    match m.state(5003) {
                        Some(AiState::TnGuardReturn { mv, entry_to_clear, next_return_scan_time }) => {
                            put_move(&mut v, mv);
                            v.extend([*entry_to_clear as u32, *next_return_scan_time]);
                        }
                        _ => panic!("the tunnel-network guard return state"),
                    }
                    match m.state(5001) {
                        Some(AiState::GuardIdle { next_enemy_scan_time, .. }) => v.push(*next_enemy_scan_time),
                        _ => panic!("the tunnel-network guard idle state"),
                    }
                    match m.state(5004) {
                        Some(AiState::GuardCrate { mv, delay_counter }) => {
                            put_move(&mut v, mv);
                            v.push(*delay_counter as u32);
                        }
                        _ => panic!("the tunnel-network guard crate state"),
                    }
                }
            }
            _ => panic!("the tunnel-network guard state"),
        }
        // batch A17: the dock state (its machine: state, goal, sleep, lock, approach position; its states' members)
        match sm.state(14) {
            Some(AiState::Dock { machine, approach_position, using_precision_movement }) => {
                v.extend([u32::from(machine.is_some()), u32::from(*using_precision_movement)]);
                if let Some(m) = machine {
                    v.extend([m.get_current_state_id(), m.goal_object_id() as u32, m.peek_sleep_till(), u32::from(m.is_locked())]);
                    v.push(*approach_position as u32);
                    for id in [0u32, 2, 3, 4, 6, 7] {
                        match m.state(id) {
                            Some(AiState::DockMove { mv, .. }) => put_move(&mut v, mv),
                            _ => panic!("a dock move state"),
                        }
                    }
                    match m.state(1) {
                        Some(AiState::DockWait { enter_frame }) => v.push(*enter_frame),
                        _ => panic!("the dock wait state"),
                    }
                    match m.state(5) {
                        Some(AiState::DockProcess { next_action_frame, drone_id }) => v.extend([*next_action_frame, *drone_id as u32]),
                        _ => panic!("the dock process state"),
                    }
                }
            }
            _ => panic!("the dock state"),
        }
        // batch A16: the attack-squad state's machine (state, goal, its attack state's machine)
        match sm.state(20) {
            Some(AiState::AttackSquad { machine, .. }) => {
                v.push(u32::from(machine.is_some()));
                if let Some(m) = machine {
                    v.extend([m.get_current_state_id(), m.goal_object_id() as u32, m.peek_sleep_till()]);
                    match m.state(10) {
                        Some(AiState::Attack { machine, .. }) => {
                            v.push(u32::from(machine.is_some()));
                            if let Some(am) = machine {
                                v.push(am.get_current_state_id());
                            }
                        }
                        _ => panic!("the attack-squad machine's attack state"),
                    }
                }
            }
            _ => panic!("the attack-squad state"),
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
    // batch A31: a DeployStyleAIUpdate's members
    if let AiSubclass::DeployStyle(d) = &module.ai().subclass {
        v.extend([d.state, d.frame_to_wait_for_deploy]);
    }
    // batch A37: a DeliverPayloadAIUpdate's members, data and machine
    if let AiSubclass::DeliverPayload(d) = &module.ai().subclass {
        put_pos(&mut v, &d.target_pos);
        put_pos(&mut v, &d.move_to_pos);
        v.extend([d.visible_items_delivered as u32, u32::from(d.delivery_decal.empty), d.previous_distance_sqr.to_bits()]);
        v.extend([u32::from(d.free_to_exit), u32::from(d.accepting_commands), d.dive_state as u32]);
        let a = &d.data;
        v.extend([a.visible_drop_bone_name.len() as u32, a.visible_sub_object_name.len() as u32, a.visible_payload_template_name.len() as u32]);
        v.extend([a.dist_to_target.to_bits(), a.pre_open_distance.to_bits(), a.max_attempts as u32]);
        put_pos(&mut v, &a.drop_offset);
        put_pos(&mut v, &a.drop_variance);
        v.extend([a.drop_delay, u32::from(a.fire_weapon), u32::from(a.self_destruct_object), a.visible_num_bones as u32]);
        v.extend([a.dive_start_distance.to_bits(), a.dive_end_distance.to_bits(), a.strafing_weapon_slot as u32]);
        v.extend([a.visible_items_dropped_per_interval as u32, u32::from(a.inherit_transport_velocity), u32::from(a.is_parachute_directly)]);
        v.extend([a.exit_pitch_rate.to_bits(), a.strafe_fx.as_ref().map_or(u32::MAX, |n| u32::from(n.as_slice() == b"fx1"))]);
        v.extend([a.strafe_length.to_bits(), a.visible_payload_weapon_template.unwrap_or(u32::MAX)]);
        let t = &a.delivery_decal_template;
        v.extend([t.name.len() as u32, t.shadow_type, t.min_opacity.to_bits(), t.max_opacity.to_bits(), t.opacity_throb_time]);
        v.extend([t.color as u32, u32::from(t.only_visible_to_owning_player), a.delivery_decal_radius.to_bits()]);
        v.push(u32::from(module.deliver_machine.is_some()));
        if let Some(m) = module.deliver_machine.as_ref() {
            v.extend([m.get_current_state_id(), m.peek_sleep_till(), u32::from(m.is_default_state_inited())]);
            match m.state(deliver::DELIVERING) {
                Some(DeliverState::Delivering { drop_delay_left, did_open }) => v.extend([*drop_delay_left, u32::from(*did_open)]),
                _ => panic!("the delivering state"),
            }
            match m.state(deliver::CONSIDER_NEW_APPROACH) {
                Some(DeliverState::ConsiderNewApproach { number_entries_to_state }) => v.push(*number_entries_to_state as u32),
                _ => panic!("the considering state"),
            }
            match m.state(deliver::RECOVER_FROM_OFF_MAP) {
                Some(DeliverState::RecoverFromOffMap { re_entry_frame }) => v.push(*re_entry_frame),
                _ => panic!("the recovering state"),
            }
            match m.state(deliver::HEAD_OFF_MAP) {
                Some(DeliverState::HeadOffMap { facing_direction_upon_delivery }) => put_pos(&mut v, facing_direction_upon_delivery),
                _ => panic!("the heading-off state"),
            }
        }
    }
    // batch A36: a MissileAIUpdate's members
    if let AiSubclass::Missile(m) = &module.ai().subclass {
        v.extend([m.state, m.state_timestamp, m.next_target_track_time, m.launcher_id as u32, m.victim_id as u32, m.fuel_expiration_date]);
        v.extend([m.no_turn_dist_left.to_bits(), m.max_accel.to_bits()]);
        put_pos(&mut v, &m.original_target_pos);
        put_pos(&mut v, &m.prev_pos);
        v.extend([m.extra_bonus_flags, m.detonation_weapon.unwrap_or(u32::MAX), m.exhaust_sys.unwrap_or(u32::MAX), m.exhaust_id]);
        v.extend([m.frames_till_decoyed, u32::from(m.is_tracking_target), u32::from(m.is_armed), u32::from(m.no_damage), u32::from(m.is_jammed)]);
    }
    // batch A35: a SupplyTruckAIUpdate's members and its machine's
    if let AiSubclass::SupplyTruck(t) = &module.ai().subclass {
        v.extend([t.preferred_dock as u32, t.number_boxes as u32, u32::from(t.force_pending), u32::from(t.forced_busy_pending)]);
        let m = module.supply_machine.as_ref().expect("the supply truck's machine");
        v.extend([m.get_current_state_id(), m.peek_sleep_till(), u32::from(m.is_default_state_inited())]);
    }
    // batch A34: a HackInternetAIUpdate's members (the waiting command's storage) and its machine's three states' frames
    if let AiSubclass::HackInternet(k) = &module.ai().subclass {
        let c = &k.pending_command;
        v.extend([u32::from(k.has_pending_command), c.cmd as u32, c.cmd_source as u32]);
        put_pos(&mut v, &c.pos);
        v.extend([c.obj as u32, c.other_obj as u32, c.team_name.len() as u32, c.coords.len() as u32]);
        for p in &c.coords {
            put_pos(&mut v, p);
        }
        v.extend([c.waypoint.unwrap_or(u32::MAX), c.polygon.map_or(u32::MAX, |p| p as u32), c.int_value as u32]);
        v.extend([c.command_button.unwrap_or(u32::MAX), u32::from(c.path.is_some())]);
        if let Some(sm) = module.machine.as_ref() {
            for id in [1000u32, 1001, 1002] {
                match sm.state(id) {
                    Some(AiState::HackUnpacking { frames_remaining })
                    | Some(AiState::HackInternet { frames_remaining })
                    | Some(AiState::HackPacking { frames_remaining }) => v.push(*frames_remaining),
                    _ => panic!("a hacking state"),
                }
            }
        }
    }
    // batch A33: a RailedTransportAIUpdate's members
    if let AiSubclass::RailedTransport(t) = &module.ai().subclass {
        v.push(u32::from(t.in_transit));
        for p in &t.path {
            v.extend([p.0, p.1]);
        }
        v.extend([t.num_paths as u32, t.current_path as u32, u32::from(t.waypoint_data_loaded)]);
    }
    // batch A32: an AssaultTransportAIUpdate's members
    if let AiSubclass::AssaultTransport(a) = &module.ai().subclass {
        v.push(a.current_members as u32);
        for i in 0..10 {
            v.extend([a.member_ids[i] as u32, u32::from(a.member_healing[i]), u32::from(a.new_member[i])]);
        }
        put_pos(&mut v, &a.attack_move_goal_pos);
        v.extend([a.designated_target as u32, a.state as u32, a.frames_remaining]);
        v.extend([u32::from(a.is_attack_move), u32::from(a.is_attack_object), u32::from(a.new_occupants_are_new_members)]);
    }
    v
}

/// Batch A37: the scene's delivering AI's module data (the wrapper's `setScene`).
fn scene_deliver_module_data(e: &Env) -> DeliverPayloadModuleData {
    let r = e.deliver_reals;
    DeliverPayloadModuleData {
        door_delay: e.deliver_times[0],
        drop_delay: e.deliver_times[1],
        max_distance_to_target: r[0],
        drop_offset: Coord3D { x: r[1], y: r[2], z: r[3] },
        drop_variance: Coord3D { x: r[4], y: r[5], z: r[6] },
        delivery_decal_radius: r[7],
        max_number_attempts: e.deliver_ints[0],
        put_in_container_name: deliver_template_name(e.deliver_ints[1]),
        delivery_decal_template: scene_decal_template(e),
    }
}

/// Batch A37: a template's name (0 none, 1 "pt0", 2 "pt1", 3 "ptx").
fn deliver_template_name(k: i32) -> Vec<u8> {
    [&b""[..], b"pt0", b"pt1", b"ptx"][(k & 3) as usize].to_vec()
}

/// Batch A37: the scene's decals' template (the wrapper's `sceneDecalTemplate`).
fn scene_decal_template(e: &Env) -> RadiusDecalTemplate {
    RadiusDecalTemplate {
        name: if e.deliver_decal[0] != 0 { b"decal0".to_vec() } else { Vec::new() },
        shadow_type: e.deliver_decal[1] as u32,
        opacity_throb_time: e.deliver_decal[2] as u32,
        color: e.deliver_decal[3],
        only_visible_to_owning_player: e.deliver_decal[4] != 0,
        min_opacity: e.deliver_opacity[0],
        max_opacity: e.deliver_opacity[1],
    }
}

/// Batch A37: the data op 109 delivers (the wrapper's `sceneDeliverData`).
fn scene_deliver_data(e: &Env) -> DeliverPayloadData {
    let (r, k, f) = (e.deliver_data_reals, e.deliver_data_ints, e.deliver_flags);
    DeliverPayloadData {
        dist_to_target: r[0],
        pre_open_distance: r[1],
        drop_offset: Coord3D { x: r[2], y: r[3], z: r[4] },
        drop_variance: Coord3D { x: r[5], y: r[6], z: r[7] },
        dive_start_distance: r[8],
        dive_end_distance: r[9],
        exit_pitch_rate: r[10],
        strafe_length: r[11],
        delivery_decal_radius: r[12],
        max_attempts: k[0],
        drop_delay: k[1] as u32,
        visible_num_bones: k[2],
        strafing_weapon_slot: k[3],
        visible_items_dropped_per_interval: k[4],
        visible_payload_weapon_template: (k[5] >= 0).then_some(k[5] as u32),
        strafe_fx: (k[6] >= 0).then(|| format!("fx{}", k[6]).into_bytes()),
        visible_drop_bone_name: if k[7] != 0 { b"bone".to_vec() } else { Vec::new() },
        visible_sub_object_name: if k[8] != 0 { b"sub".to_vec() } else { Vec::new() },
        visible_payload_template_name: deliver_template_name(k[9]),
        fire_weapon: (f >> 3) & 1 != 0,
        self_destruct_object: (f >> 4) & 1 != 0,
        inherit_transport_velocity: (f >> 5) & 1 != 0,
        is_parachute_directly: (f >> 6) & 1 != 0,
        delivery_decal_template: scene_decal_template(e),
    }
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
    // batch A17: the other objects with a dock (fixed for the case: the move-to-dock state asks its goal's dock unchecked)
    e.dock_mask = g.u32();
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
    // (batch A31: bits 4 and 5 the per-unit deploy and undeploy sounds; batch A34: 6 the cash ping)
    e.sounds = g.below(128) as i32;
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
    // batch A30: the AI's class, for the whole case (a third of the AIs AIUpdateInterface itself; batch A31: a third deployers)
    e.subclass = [0, 0, 1, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9][g.below(18) as usize];
    // batch A31: a deployer's data and drawable, its times now and then zero
    e.deploy_flags = g.below(32) as i32;
    e.deploy_times = [if g.below(4) == 0 { 0 } else { g.below(40) }, if g.below(4) == 0 { 0 } else { g.below(40) }];
    // batch A32: an assault transport's healing ratio
    e.assault_ratio = [0.0, 0.3, 0.6, 1.0][g.below(4) as usize];
    // batch A33: the railed transport's named waypoints (mostly among the first eight numbers, now and then none) and docks
    e.rail_start_mask = if g.below(6) == 0 { 0 } else { g.below(256) | if g.below(4) == 0 { g.u32() } else { 0 } };
    e.rail_end_mask = if g.below(3) == 0 { e.rail_start_mask } else { g.below(256) | e.rail_start_mask & g.u32() };
    e.rail_flags = g.below(4) as i32;
    // batch A34: a hacker's data (short times, often zero; cash amounts often zero, to fall through), world and level; its
    // controlling player always (the hacking state reads it unchecked)
    let short = |g: &mut Gen| if g.below(3) == 0 { 0 } else { g.below(12) };
    e.hack_times = [short(g), short(g), short(g), short(g)];
    e.hack_cash = [short(g), short(g), short(g), short(g)];
    e.hack_xp = g.below(5);
    e.hack_variation = [0.0, 0.1, 0.5][g.below(3) as usize];
    e.hack_flags = g.below(16) as i32;
    e.hack_level = g.below(4) as i32;
    // batch A35: a supply truck's data and world (the scene's objects, now and then none)
    e.supply_max_boxes = g.below(5) as i32;
    e.supply_delays = [g.below(20), g.below(20)];
    e.supply_scan = [100.0, 40.0, 400.0][g.below(3) as usize];
    e.supply_flags = g.below(2) as i32;
    let some = |g: &mut Gen| if g.below(4) == 0 { 0 } else { 1 + g.below(31) as i32 };
    e.supply_best = [some(g), some(g)];
    e.supply_closest = [some(g), some(g), some(g)];
    e.supply_dock_masks = [g.u32(), g.u32()];
    // batch A36: a missile's data and world (short times, distances around the scene's, a few kinds)
    e.missile_flags = g.below(64) as i32;
    e.missile_times = [if g.below(3) == 0 { 0 } else { g.below(40) }, g.below(6)];
    e.missile_reals = [g.uniform(0.0, 30.0), [0.0, 20.0, 60.0][g.below(3) as usize], g.uniform(0.0, 80.0), [0.0, 30.0, 75.0][g.below(3) as usize], [0.0, 40.0, 75.0, 200.0][g.below(4) as usize]];
    e.missile_garrison = [g.below(4) as i32, if g.below(3) != 0 { 0 } else { (g.below(4) as i32) << 1 }, if g.below(3) != 0 { 0 } else { 1 << 3 }];
    e.missile_kill_self_delay = [0, 3, 10][g.below(3) as usize];
    e.missile_weapon_speeds = [g.uniform(5.0, 40.0), g.uniform(5.0, 40.0), g.uniform(5.0, 40.0)];
    e.missile_die_mask = g.below(8) as i32;
    e.missile_collide_rate = 40 + g.below(61);
    e.missile_countermeasure = if g.below(3) == 0 { 0 } else { 1 + g.below(31) as i32 };
    e.missile_garrison_immune = i32::from(g.below(4) == 0);
    // batch A37: a delivering AI's data (short delays; distances around the scene's), the data op 109 delivers, the decals' template,
    // its flags (the drawable mostly), its world; the "StartDive" sound now and then
    let short4 = |g: &mut Gen| if g.below(3) == 0 { 0 } else { g.below(4) };
    e.deliver_times = [short4(g), short4(g)];
    e.deliver_reals = [
        g.uniform(20.0, 300.0),
        g.uniform(-5.0, 5.0),
        g.uniform(-5.0, 5.0),
        g.uniform(-10.0, 0.0),
        if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, 8.0) },
        if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, 8.0) },
        if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, 8.0) },
        [0.0, 50.0][g.below(2) as usize],
    ];
    e.deliver_ints = [g.below(4) as i32, [0, 1, 1, 2, 3][g.below(5) as usize]];
    e.deliver_decal = [g.below(2) as i32, [0x20, 0x40][g.below(2) as usize], [30, 60][g.below(2) as usize], g.below(3) as i32, g.below(2) as i32];
    e.deliver_opacity = [g.uniform(0.0, 0.5), g.uniform(0.5, 1.0)];
    e.deliver_data_reals = [
        g.uniform(20.0, 300.0),
        if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, 100.0) },
        g.uniform(-5.0, 5.0),
        g.uniform(-5.0, 5.0),
        g.uniform(-10.0, 0.0),
        if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, 8.0) },
        if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, 8.0) },
        if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, 8.0) },
        if g.below(3) == 0 { 0.0 } else { g.uniform(30.0, 400.0) },
        g.uniform(0.0, 100.0),
        if g.below(2) == 0 { 0.0 } else { g.uniform(-2.0, 2.0) },
        g.uniform(0.0, 50.0),
        [0.0, 50.0][g.below(2) as usize],
    ];
    e.deliver_data_ints = [
        g.below(4) as i32,
        g.below(4) as i32,
        g.below(5) as i32,
        g.below(4) as i32 - 1,
        g.below(4) as i32 - 1,
        g.below(4) as i32 - 1,
        g.below(3) as i32 - 1,
        g.below(2) as i32,
        g.below(2) as i32,
        [0, 1, 1, 2, 2, 3][g.below(6) as usize],
    ];
    e.deliver_flags = (g.below(128) as i32) | i32::from(g.below(4) != 0);
    e.deliver_world = [[0.0, 20.0, 60.0][g.below(3) as usize], g.uniform(1.0, 10.0), g.uniform(-2.0, 2.0)];
    e.deliver_turning = g.below(3) as i32 - 1;
    e.deliver_module_masks = [g.u32(), g.u32()];
    if e.subclass == 9 && g.below(2) == 0 {
        e.sounds |= 1 << 8;
    }
    if e.subclass == 2 && g.below(2) == 0 {
        // (the scene's kinds and disabled types are every object's: portable structures, now and then disabled, as passengers)
        e.loco.kind_mask |= 512;
        if g.below(2) == 0 {
            e.disabled_mask = [1 << DISABLED_PARALYZED, 1 << DISABLED_EMP, 1 << DISABLED_SUBDUED, 1 << DISABLED_HACKED][g.below(4) as usize];
        }
    }
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
    // (batch A36: a missile's targets mostly garrisoned buildings, holding some)
    if e.subclass == 8 && g.below(3) != 0 {
        e.garrisonable = 1;
        e.contain_count = 1 + g.below(2) as i32;
        e.contained_mask |= g.u32() & !e.ai_mask;
    }
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
    e.difficulty = g.below(3) as i32;
    e.choose_normal = i32::from(g.below(4) == 0);
    e.dock_flags = (g.below(32) as i32) | i32::from(g.below(4) != 0); // (open three times in four)
    e.dock_pos = [g.uniform(20.0, 200.0), g.uniform(20.0, 120.0), g.uniform(0.0, 10.0)];
    e.dock_index = g.below(4) as i32 - 1;
    e.drone_mask = if g.below(2) == 0 { 0 } else { g.u32() };
    e.dock_rate = [10, 50, 90][g.below(3) as usize];
    e.loco.kind_mask |= i32::from(g.below(4) == 0) << 17; // (KINDOF_DRONE)
    e.clearable_attacker = if g.below(2) == 0 { 0 } else { g.below(32) as i32 };
    e.guard_flags = if g.below(4) == 0 { g.below(4) as i32 } else { 0 };
    e.objects_changed_frame = e.loco.frame.wrapping_sub(g.below(60));
    e.guard_scan_rate = [0, 1, 5, 30][g.below(4) as usize];
    e.guard_return_scan_rate = [0, 1, 5, 30][g.below(4) as usize];
    e.guard_chase_frames = [0, 10, 100, 1000][g.below(4) as usize];
    // batch A20: the tunnel network (tunnels, current nemesis, the tunnels' damage times and no-effect bits), others' AIs' goal
    for t in e.tunnel_ids.iter_mut() {
        *t = if g.below(3) == 0 { 0 } else { g.below(32) as i32 };
    }
    e.tunnel_nemesis = if g.below(3) == 0 { g.below(32) as i32 } else { 0 };
    e.tunnel_stamp = e.loco.frame.wrapping_sub(g.below(80));
    e.tunnel_no_effect = if g.below(2) == 0 { 0 } else { g.u32() };
    e.other_ai_goal = g.below(32) as i32;
    // batch A23: the other AIs' machine's state (idle, move-to, busy) and goal position
    e.other_ai_state = [0, 0, 1, 41][g.below(4) as usize];
    e.other_ai_goal_pos = [g.uniform(0.0, 200.0), g.uniform(0.0, 200.0), 0.0];
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
    } else if e.subclass == 9 && g.below(5) == 0 {
        // batch A37: a delivering AI's members (the deliveries most)
        [109, 109, 109, 110, 110, 111, 112, 113, 114, 115, 116, 117][g.below(12) as usize]
    } else if e.subclass == 8 && g.below(5) == 0 {
        // batch A36: the weapon's calls to a missile
        [104, 104, 105, 106, 106, 106, 106, 107, 108][g.below(9) as usize]
    } else if e.subclass == 7 && g.below(6) == 0 {
        // batch A35: the docks' calls to a supply truck
        [101, 101, 102, 103][g.below(4) as usize]
    } else if e.subclass == 4 && g.below(10) == 0 {
        // batch A32: the weapon's beginAssault, at an object (or none)
        100
    } else if g.below(40) == 0 {
        // batch A12: setAttackInfo
        87
    } else if g.below(40) == 0 {
        // batch A21: the AI saved, loaded (its last save; else saved), its CRC
        [88, 88, 89, 89, 90][g.below(5) as usize]
    } else if g.below(40) == 0 {
        // batch A23: joinTeam, the turrets' members
        [91, 91, 92, 93, 94, 95, 96, 97, 98, 99][g.below(10) as usize]
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
    let needs_machine = matches!(op, 7 | 36 | 38 | 41 | 43 | 44 | 48 | 50..=52 | 54 | 55 | 59..=65 | 67..=86 | 88..=99 | 104..=117);
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
            ia[0] = if g.below(2) == 0 { [0, 0, 13, 8, 41, 15, 21, 1, 1, 23, 24, 39, 6, 7, 25, 26, 27, 2, 3, 4, 5, 18, 19, 40, 3, 18, 9, 10, 11, 12, 10, 10, 2, 2, 30, 30, 38, 17, 17, 28, 28, 31, 31, 32, 32, 33, 34, 37, 42, 35, 35, 20, 20, 14, 14, 16, 16, 16, 43, 43, 43, 21, 21, 21][g.below(64) as usize] } else { g.below(46) as i32 };
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
        // batch A23: a turret (a weapon slot for 97 and 99), a flag or an object
        92..=99 => {
            ia[0] = g.below(if matches!(op, 97 | 99) { 3 } else { 2 }) as i32;
            ia[1] = g.below(32) as i32;
        }
        79 => {
            ia[0] = g.below(9) as i32 - 1;
            fa[3] = g.uniform(-10.0, 10.0);
            fa[4] = g.uniform(-10.0, 10.0);
        }
        // batch A5: a command's parms (see ref_ai_a2_op): any command (55, 8 and 32 are unhandled), its source, an object, a team, a
        // waypoint, a polygon, an int value, a button and coords; what upstream dereferences unchecked is always given
        72 | 73 => {
            let cmd = if g.below(5) == 0 { g.below(56) as i32 } else { [0, 1, 5, 11, 14, 29, 30, 48, 49, 9, 54, 41, 40, 23, 21, 25, 6, 7, 46, 47, 42, 43, 44, 15, 15, 18, 18, 33, 33, 16, 16, 17, 17, 25, 25, 53, 53, 35, 35, 36, 36, 37, 37, 13, 13, 24, 24, 24, 54, 54, 51, 51, 51][g.below(53) as usize] };
            // (batch A33: a railed transport's two commands, often)
            let cmd = if e.subclass == 5 && g.below(3) == 0 { [26, 27, 27][g.below(3) as usize] } else { cmd };
            // (batch A34: a hacker's command, often)
            let cmd = if e.subclass == 6 && g.below(3) == 0 { 34 } else { cmd };
            // (batch A37: a delivering AI's scripted reinforcement, the move-and-evacuate-and-exit, often)
            let cmd = if e.subclass == 9 && g.below(4) == 0 { 4 } else { cmd };
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
        100 => ia[0] = g.below(32) as i32,
        101 => ia[0] = [0, 0, 1, 5][g.below(4) as usize],
        104..=108 => {
            ia[0] = if op == 107 { g.below(20) as i32 } else if g.below(5) == 0 { -1 } else { g.below(32) as i32 };
            ia[1] = g.below(32) as i32;
            ia[2] = g.below(3) as i32;
            ia[3] = g.below(3) as i32;
            // (the detonation weapon now and then none: a collision with an object then is undefined, the case ends)
            ia[4] = if g.below(10) == 0 { -1 } else { g.below(3) as i32 };
            ia[5] = g.below(4) as i32 - 1;
        }
        103 => ia[0] = g.below(2) as i32,
        // batch A37: the exiter; the target near the move position, now and then far
        109..=117 => {
            ia[0] = g.below(32) as i32;
            let far = g.below(4) == 0;
            for k in 0..3 {
                fa[3 + k] = fa[k] + if far { g.uniform(-300.0, 300.0) } else { g.uniform(-20.0, 20.0) };
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
        // batch A23
        91 => {
            module.run(h, rts_lockstep_engine::logic::ai::ai_commands::join_team).expect("joinTeam");
            vec![]
        }
        92..=99 => {
            use rts_lockstep_engine::logic::ai::ai_attack_states as at;
            let (a, b) = (ia[0], ia[1]);
            let slot = |s: i32| [WeaponSlotType::Primary, WeaponSlotType::Secondary, WeaponSlotType::Tertiary][s as usize];
            module.run(h, |_, h| match op {
                92 => vec![at::get_turret_target_object(h, a, b != 0).map_or(0, |o| o as u32)],
                93 => {
                    at::set_turret_enabled(h, a, b != 0);
                    vec![]
                }
                94 => {
                    at::recenter_turret(h, a);
                    vec![]
                }
                95 => vec![u32::from(at::is_turret_enabled(h, a))],
                96 => vec![u32::from(at::is_turret_in_natural_position(h, a))],
                97 => vec![u32::from(at::is_weapon_slot_on_turret_and_aiming_at_target(h, slot(a), b))],
                98 => {
                    let r = at::get_turret_rot_and_pitch(h, a);
                    let (an, pi) = r.unwrap_or((-9.0, -9.0));
                    vec![u32::from(r.is_some()), an.to_bits(), pi.to_bits()]
                }
                _ => {
                    let (t, r) = at::get_which_turret_for_weapon_slot(h, slot(a));
                    let (an, pi) = r.unwrap_or((-9.0, -9.0));
                    vec![t as u32, an.to_bits(), pi.to_bits()]
                }
            })
        }
        // batch A21: the AI's snapshot
        88 => {
            let mut x = XferSave::new();
            x.open(b"ai_a21.bin").expect("open");
            let st = module.xfer(h, &mut x, waypoint_name, waypoint_by_name).is_err();
            let _ = x.close();
            h.ai_saved = x.bytes().to_vec();
            let mut r = vec![u32::from(st), h.ai_saved.len() as u32];
            r.extend(h.ai_saved.iter().map(|&c| u32::from(c)));
            r
        }
        89 => {
            let mut x = XferLoad::new(h.ai_saved.clone());
            x.open(b"ai_a21.bin").expect("open");
            let st = module.xfer(h, &mut x, waypoint_name, waypoint_by_name).is_err();
            let _ = x.close();
            module.load_post_process(h);
            vec![u32::from(st)]
        }
        90 => {
            let mut x = XferCrc::new();
            x.open(b"ai_a21_crc").expect("open");
            let _ = module.crc(h, &mut x, waypoint_name, waypoint_by_name);
            let _ = x.close();
            vec![x.get_crc()]
        }
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
        65 => match module.update(h) {
            Ok(s) => vec![s.0],
            // (batch A33: a railed transport's path followed without a locomotor)
            Err(e) if e.is_undefined_upstream() => {
                h.undefined = true;
                vec![]
            }
            Err(e) => panic!("update: {e}"),
        },
        // batch A36: the weapon's calls to a missile (undefined upstream: the case ends)
        104..=108 => {
            if !matches!(module.ai().subclass, AiSubclass::Missile(_)) {
                return vec![];
            }
            let p = Coord3D { x: fa[0], y: fa[1], z: fa[2] };
            let det = (ia[4] >= 0).then_some(ia[4] as u32);
            let exhaust = (ia[5] >= 0).then_some(ia[5] as u32);
            let r = module.run(h, |sm, h| -> SmResult<Vec<u32>> {
                match op {
                    104 => missile::projectile_launch_at_object_or_position(sm, h, ia[0], &p, ia[1], ia[2], ia[3], det, exhaust).map(|_| vec![]),
                    105 => missile::projectile_fire_at_object_or_position(sm, h, ia[0], &p, det, exhaust).map(|_| vec![]),
                    106 => missile::projectile_handle_collision(sm, h, ia[0]).map(|b| vec![u32::from(b)]),
                    107 => {
                        missile::set_frames_till_countermeasure_diversion_occurs(h, ia[0] as u32);
                        Ok(vec![])
                    }
                    _ => missile::projectile_now_jammed(sm, h).map(|_| vec![]),
                }
            });
            match r {
                Ok(v) => v,
                Err(e) if e.is_undefined_upstream() => {
                    h.undefined = true;
                    vec![]
                }
                Err(e) => panic!("a missile's call: {e}"),
            }
        }
        // batch A37: a delivering AI's members (undefined upstream: the case ends)
        109..=117 => {
            if !matches!(module.ai().subclass, AiSubclass::DeliverPayload(_)) {
                return vec![];
            }
            let p = Coord3D { x: fa[0], y: fa[1], z: fa[2] };
            let target = Coord3D { x: fa[3], y: fa[4], z: fa[5] };
            let data = scene_deliver_data(&h.env);
            let r = module.run(h, |sm, h| -> SmResult<Vec<u32>> {
                let d = |h: &mut Host| match &states::ai(h).subclass {
                    AiSubclass::DeliverPayload(d) => d.clone(),
                    _ => unreachable!(),
                };
                match op {
                    109 => deliver::deliver_payload(sm, h, &p, &target, &data).map(|_| vec![]),
                    110 => deliver::deliver_payload_via_module_data(sm, h, &p).map(|_| vec![]),
                    111 => {
                        let dead = h.is_effectively_dead();
                        // (FREE_TO_EXIT 0, NOT_FREE_TO_EXIT 1)
                        Ok(vec![u32::from(!d(h).ai_free_to_exit(dead))])
                    }
                    112 => Ok(vec![u32::from(deliver::is_close_enough_to_target(h))]),
                    113 => deliver::calc_min_turn_radius(h).map(|(r, t)| vec![r.to_bits(), t.to_bits()]),
                    114 => {
                        deliver::kill_delivery_decal(h);
                        Ok(vec![])
                    }
                    115 => {
                        let dd = d(h);
                        Ok(vec![deliver::put_in_container_template(h, &dd).unwrap_or(u32::MAX)])
                    }
                    116 => Ok(vec![u32::from(deliver::is_off_map(h))]),
                    _ => Ok(vec![u32::from(h.deliver_machine_stack().last().expect("the lent deliver machine").is_some())]),
                }
            });
            match r {
                Ok(v) => v,
                Err(e) if e.is_undefined_upstream() => {
                    h.undefined = true;
                    vec![]
                }
                Err(e) => panic!("a delivery's call: {e}"),
            }
        }
        // batch A35: the docks' calls to a supply truck
        101..=103 => {
            let mut r = vec![];
            let mut s = match &module.ai().subclass {
                AiSubclass::SupplyTruck(s) => *s,
                _ => return vec![],
            };
            match op {
                101 => r.push(u32::from(supply::gain_one_box(h, &mut s, ia[0]))),
                102 => r.push(u32::from(supply::lose_one_box(h, &mut s))),
                _ => s.force_pending = ia[0] != 0,
            }
            module.ai_mut().subclass = AiSubclass::SupplyTruck(s);
            r
        }
        // batch A32: the weapon's beginAssault
        100 => {
            let t = ia[0];
            module.begin_assault(h.object_exists(t).then_some(t));
            vec![]
        }
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
            // (batch A37: a delivery remade by the command may reach what upstream leaves undefined: the case ends)
            match module.ai_do_command(h, &parms) {
                Ok(()) => {}
                Err(e) if e.is_undefined_upstream() => h.undefined = true,
                Err(e) => panic!("aiDoCommand: {e}"),
            }
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
    ops: [u64; 118],
    /// Batch A37: the delivering AI's machine's state after an operation (approach .. clean up, none).
    deliver_states: [u64; 7],
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
    a4_states: [u64; 30],
    temporary_states: u64,
    /// Batch A9: cases ended before an operation upstream would make undefined (a waypoint state without a locomotor).
    a9_undefined: u64,
    /// Batch A10a: the attack machine's state after an operation (chase, approach, aim, fire), and LOG_ATTACK's kinds.
    attack_inner: [u64; 4],
    attack_logs: [u64; 18],
    /// Batch A11: the attack-move machine's state after an operation in the attack-move state (idle, crate, attack).
    attack_move_inner: [u64; 3],
    /// Batch A12: the hunt and attack-area machines' state after an operation in those states (attack, crate, idle).
    hunt_inner: [u64; 3],
    /// Batch A17: the docking machine's state after an operation in the dock state (approach .. rally).
    dock_inner: [u64; 8],
    /// Batch A18: the guard machine's state after an operation in the guard state (inner, idle, outer, return, crate, aggressor).
    guard_inner: [u64; 6],
    /// Batch A20: the tunnel-network guard machine's likewise; the network's nemesis offered, exits in a hurry, cases ended where upstream is undefined.
    tn_inner: [u64; 6],
    tn_logs: [u64; 3],
    /// Batch A21: the AI's saves and loads that succeeded; cases ended at a failed load, and at one state both current and
    /// temporary.
    ai_snapshot: [u64; 4],
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
            ops: [0; 118],
            deliver_states: [0; 7],
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
            a4_states: [0; 30],
            temporary_states: 0,
            a9_undefined: 0,
            attack_inner: [0; 4],
            attack_logs: [0; 18],
            attack_move_inner: [0; 3],
            hunt_inner: [0; 3],
            dock_inner: [0; 8],
            guard_inner: [0; 6],
            tn_inner: [0; 6],
            tn_logs: [0; 3],
            ai_snapshot: [0; 4],
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
        ai_machines: Vec::new(),
        deliver_machines: Vec::new(),
        next_payload: 40,
        payload_pos: Coord3D::default(),
        pf_ignore: 0,
        next_audio: 1,
        next_particle: 1,
        team_waypoint: None,
        attack_frames: Vec::new(),
        max_shots: 0,
        others_height_asked: 0,
        attacker_cleared: false,
        undefined: false,
        ai_saved: Vec::new(),
        ai_saved_states: None,
    };
    host.max_shots = host.env.max_shots_init;
    host.team_waypoint = (host.env.team_waypoint >= 0).then_some(host.env.team_waypoint as u32);
    host.names.reset();
    // batch A5: the modules the commands look up, first (as ref_ai_a2_reset)
    for name in ["DeliverPayloadAIUpdate", "ProneUpdate", "HackInternetAIUpdate", "GenerateMinefieldBehavior", "SmartBombTargetHomingUpdate"] {
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
        // (batch A33: a railed transport has every set, each with a locomotor: its update follows its path after the base's update,
        // whose states may change the set, and a waypoint state without a locomotor is undefined upstream)
        // (batch A37: a delivering AI likewise: its states use the locomotor unchecked)
        let railed = host.env.subclass == 5 || host.env.subclass == 9;
        for set in [0, 1, 2, 3, 4, 5, 6, 7] {
            let wanted = match set {
                0 => g.below(8) != 0,
                1 => g.below(3) == 0,
                _ => g.below(5) == 0,
            };
            if !wanted && !railed {
                continue;
            }
            let n = if railed { 1 + g.below(3) as usize } else { g.below(4) as usize };
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
    let subclass = match host.env.subclass {
        1 => AiSubclass::Wander,
        2 => AiSubclass::Transport,
        8 => AiSubclass::Missile(Missile::new(
            {
                let (f, r, gk) = (host.env.missile_flags, host.env.missile_reals, host.env.missile_garrison);
                let mut d = MissileData {
                    try_to_follow_target: f & 1 != 0,
                    use_weapon_speed: f & 2 != 0,
                    detonate_on_no_fuel: f & 4 != 0,
                    detonate_calls_kill: f & 8 != 0,
                    ignition_fx: (f & 16 != 0).then_some(0),
                    garrison_hit_kill_fx: (f & 32 != 0).then_some(1),
                    fuel_lifetime: host.env.missile_times[0],
                    ignition_delay: host.env.missile_times[1],
                    initial_vel: r[0],
                    initial_dist: r[1],
                    dive_distance: r[2],
                    distance_scatter_when_jammed: r[3],
                    lock_distance: r[4],
                    garrison_hit_kill_count: gk[0],
                    kill_self_delay: host.env.missile_kill_self_delay,
                    ..Default::default()
                };
                for k in 0..31 {
                    if (gk[1] >> k) & 1 != 0 {
                        d.garrison_hit_kill_kindof.set(k, 1);
                    }
                    if (gk[2] >> k) & 1 != 0 {
                        d.garrison_hit_kill_kindof_not.set(k, 1);
                    }
                }
                d
            },
            host.env.loco.frame,
            start_pos,
        )),
        7 => AiSubclass::SupplyTruck(SupplyTruck::new(SupplyTruckData {
            max_boxes: host.env.supply_max_boxes,
            center_delay: host.env.supply_delays[0],
            warehouse_delay: host.env.supply_delays[1],
            warehouse_scan_distance: host.env.supply_scan,
            has_depleted_voice: host.env.supply_flags & 1 != 0,
        })),
        6 => AiSubclass::HackInternet(HackInternet::new(HackInternetData {
            unpack_time: host.env.hack_times[0],
            pack_time: host.env.hack_times[1],
            cash_update_delay: host.env.hack_times[2],
            cash_update_delay_fast: host.env.hack_times[3],
            regular_cash_amount: host.env.hack_cash[0],
            veteran_cash_amount: host.env.hack_cash[1],
            elite_cash_amount: host.env.hack_cash[2],
            heroic_cash_amount: host.env.hack_cash[3],
            xp_per_cash_update: host.env.hack_xp,
            pack_unpack_variation_factor: host.env.hack_variation,
        })),
        5 => AiSubclass::RailedTransport(RailedTransport::new(RailedTransportData { path_prefix_name: "rail".to_string() })),
        4 => AiSubclass::AssaultTransport(AssaultTransport::new(AssaultTransportData {
            members_get_healed_at_life_ratio: host.env.assault_ratio,
            ..Default::default()
        })),
        3 => AiSubclass::DeployStyle(DeployStyle::new(DeployStyleData {
            unpack_time: host.env.deploy_times[0],
            pack_time: host.env.deploy_times[1],
            reset_turret_before_packing: host.env.deploy_flags & 1 != 0,
            turrets_function_only_when_deployed: host.env.deploy_flags & 2 != 0,
            turrets_must_center_before_packing: host.env.deploy_flags & 4 != 0,
            manual_deploy_animations: host.env.deploy_flags & 8 != 0,
        })),
        9 => AiSubclass::DeliverPayload(DeliverPayload::new(scene_deliver_module_data(&host.env))),
        _ => AiSubclass::Plain,
    };
    let mut module = AiUpdateModule::new_subclass(&mut host, data, subclass).expect("the AI's construction");
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
            // (batch A20: and the tunnel-network guard state, whose inner state offers to the network unchecked)
            matches!(sm.get_current_state_id(), 9..=12 | 15..=17 | 20 | 21 | 28 | 30..=32 | 43)
                || matches!(module.ai().machine.temporary_state, 9..=12 | 15..=17 | 20 | 21 | 28 | 30..=32 | 43)
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
        // (batch A34: and so does the hacking state: a hacker keeps its player)
        if (a9_states(&module).1 || attacking || idle || host.env.subclass == 6) && host.env.player_type < 0 {
            host.env.player_type = 0;
        }
        let moving_away = (module.ai().move_out_of_way1, module.ai().move_out_of_way2);
        let (mut op, mut fa, mut ia) = gen_op(&mut g, &host.env, waypoint_heavy, moving_away, module.machine.is_some(), has_saved);
        a9_guard(&module, &host.env, &mut op, &mut ia);
        // batch A19: no enter guard while the guard-retaliate state is entered or runs (it does not lock the AI's machine, so its
        // enter state's attack command would switch the AI's state, deleting the machine it runs in: undefined upstream)
        let retaliating = module.machine.as_ref().is_some_and(|sm| sm.get_current_state_id() == 43 || module.ai().machine.temporary_state == 43);
        if retaliating || (op == 72 && ia[0] == 54) || (matches!(op, 59 | 60) && ia[0] == 43) {
            host.env.guard_flags &= !1;
        }
        // batch A10b: an attack-position command a third of the time at the object (the approach's exact goal on exit)
        if op == 72 && ia[0] == 14 && g.below(3) == 0 {
            let p = *host.thing.position();
            fa[0] = p.x + g.uniform(-0.3, 0.3);
            fa[1] = p.y + g.uniform(-0.3, 0.3);
            fa[2] = p.z;
        }
        // batch A10a: in an attack state, half the operations are the AI's update (which runs the attack machine)
        // (batch A12: and in the hunt and attack-area states, whose machines the update runs)
        // (batch A17: and the dock state, whose docking machine the update runs; its waits jump frames at times)
        // (batch A20: in the tunnel-network guard state, at times its nemesis is gone, and half those times the team has no target:
        // the inner state then takes the network's nemesis)
        if let Some(AiState::Guard { machine: Some(_), data, .. }) = module.machine.as_ref().and_then(|sm| sm.state(21)) {
            let n = data.nemesis_to_attack;
            if g.below(3) == 0 {
                if (1..32).contains(&n) {
                    host.env.exist_mask &= !(1u32 << n);
                }
                if g.below(2) == 0 {
                    host.env.team_target = 0;
                }
            }
        }
        let docking = module.machine.as_ref().is_some_and(|sm| sm.get_current_state_id() == 14);
        if docking && g.below(4) == 0 {
            host.env.loco.frame += g.below(400);
        }
        // (and half the time the dock is where the owner is, so that its moves arrive: the approach position is reserved when
        // the dock state is entered)
        if g.below(2) == 0 {
            let p = *host.thing.position();
            host.env.dock_pos = [p.x, p.y, p.z];
        }
        if module.machine.as_ref().is_some_and(|sm| matches!(sm.get_current_state_id(), 9..=12 | 14 | 16 | 17 | 20 | 28 | 43)) && g.below(2) == 0 {
            op = 65;
            ia.iter_mut().for_each(|x| *x = 0); // (the update takes no arguments; another op's would be read for coordinates)
            // (batch A33: the guard again, for a railed transport's update)
            a9_guard(&module, &host.env, &mut op, &mut ia);
        }
        if std::env::var("AI_A2_TRACE").is_ok() {
            let cur = module.machine.as_ref().map(|sm| sm.get_current_state_id());
            println!("TRACE step {step} op {op} ia {:?} state {cur:?} temp {} goal_waypoint {:?} loco {:?}", &ia[..6], module.ai().machine.temporary_state, module.ai().machine.goal_waypoint, module.ai().cur_locomotor);
        }
        has_saved |= op == 70;
        // (batch A21: a load of the AI needs its save, made with the machine in the same state and temporary state: upstream loads
        // only into a new object, whose machine the snapshot's shape matches; another state's reads the stream misaligned)
        let states_now = module.machine.as_ref().map(|sm| (sm.get_current_state_id(), module.ai().machine.temporary_state));
        if op == 89 && (host.ai_saved.is_empty() || host.ai_saved_states != states_now) {
            op = 88;
        }
        if op == 88 {
            host.ai_saved_states = states_now;
        }
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
        host.attacker_cleared = false;
        let r = rust_op(&mut module, &mut host, op, &fa, &ia, 0, &mut saved);
        if host.undefined {
            // (batch A33: undefined upstream: the case ends before the C++ runs the op)
            ck.a9_undefined += 1;
            return true;
        }
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
        if let AiSubclass::DeliverPayload(_) = module.ai().subclass {
            let s = module.deliver_machine.as_ref().map_or(6, |m| (m.get_current_state_id() as usize).min(6));
            ck.deliver_states[s] += 1;
        }
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
            if code == L_TUNNEL {
                ck.tn_logs[0] += 1;
            }
            if code == L_CONTAIN && n > 0 && host_log_copy[k + 2] == 8 {
                ck.tn_logs[1] += 1;
            }
            if (70..=103).contains(&code) {
                ck.a4_logs[(code - 70) as usize] += 1;
            }
            k += 2 + n;
        }
        if let Some(sm) = &module.machine {
            if let Some(AiState::Guard { machine: Some(m), data, .. }) = sm.state(sm.get_current_state_id()) {
                ck.guard_inner[(m.get_current_state_id().saturating_sub(5000) as usize).min(5)] += 1;
                if data.tunnel {
                    ck.tn_inner[(m.get_current_state_id().saturating_sub(5000) as usize).min(5)] += 1;
                }
            }
            if let Some(AiState::Dock { machine: Some(m), .. }) = sm.state(sm.get_current_state_id()) {
                ck.dock_inner[(m.get_current_state_id() as usize).min(7)] += 1;
            }
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
                20 => 26,
                14 => 27,
                16 => 28,
                43 => 29,
                _ => 4,
            }] += 1;
        }
        match op {
            88 if r[0] == 0 => ck.ai_snapshot[0] += 1,
            89 if r[0] == 0 => ck.ai_snapshot[1] += 1,
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
        // batch A20: the tunnel-network guard machine left (by the transition-depth limit, 20) in its inner, outer or attack-aggressor
        // state without an attack state: its next update dereferences it (upstream undefined; AIGuard's states check, AITNGuard's
        // do not): the case ends here
        if tn_attack_state_missing(&module) {
            ck.tn_logs[2] += 1;
            return true;
        }
        // batch A21: a load of the AI that failed (the stream read past what was saved: a save of another state, or another
        // machine) leaves it half loaded, its later members garbage (a waypoint count out of range, say), which upstream never runs
        // on (a failed load ends the game's): the case ends here
        if op == 89 && r.first() == Some(&1) {
            ck.ai_snapshot[2] += 1;
            return true;
        }
        // (and a load of the machine (op 71) that made one state both its state and its temporary state: the one instance's two
        // roles, which the other's end leaves without its members (a sub-machine, a path), undefined as in a9_guard: the case ends)
        if let Some(sm) = &module.machine {
            let cur = sm.get_current_state_id();
            // (the states a9_guard keeps from both roles: with a sub-machine, a path or waypoint members of their own)
            if cur == module.ai().machine.temporary_state && matches!(cur, 2..=5 | 9..=12 | 14 | 16..=21 | 28 | 30..=32 | 43) {
                ck.ai_snapshot[3] += 1;
                return true;
            }
        }
    }
    true
}

/// Batch A20: whether the tunnel-network guard state's machine is in an attack state (inner, outer, aggressor) with none embedded.
fn tn_attack_state_missing(module: &AiUpdateModule<Host>) -> bool {
    let Some(sm) = &module.machine else { return false };
    match sm.state(21) {
        Some(AiState::Guard { machine: Some(m), .. }) => {
            matches!(m.state(m.get_current_state_id()), Some(AiState::GuardAttack { embedded, .. }) if embedded.is_empty())
        }
        _ => false,
    }
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
            // batch A16: the attack-squad state likewise (its exit deletes the one machine; it reads the player unchecked)
            || (state == 20 && (in_machine(state) || e.player_type < 0))
            // batch A17: the dock state already the machine's state or its temporary state (the other's exit deletes the one
            // machine, which the update dereferences)
            || (state == 14 && in_machine(state))
            // batch A18: the guard state likewise, or without a controlling player (its machine's attack states read it)
            || (state == 16 && (in_machine(state) || e.player_type < 0))
            // batch A19: the guard-retaliate state likewise
            || (state == 43 && (in_machine(state) || e.player_type < 0))
            // batch A20: the tunnel-network guard state likewise (its machine reads the player's tunnels unchecked)
            || (state == 21 && (in_machine(state) || e.player_type < 0))
    };
    match *op {
        59 | 60 if unsafe_entry(ia[0], module.ai().machine.goal_waypoint.is_some()) => ia[0] = 0,
        // batch A23: joinTeam chooses the normal locomotor set (which may leave none) and then clears the machine (the waypoint states'
        // exit reads the locomotor): not in those states (the doLocomotor op instead)
        91 if a9_states(module).0 => *op = 0,
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
                13 => 20,
                24 => 14,
                29..=31 => 16,
                54 => 43,
                51 => 21,
                _ => -1,
            };
            if unsafe_entry(state, ia[4] >= 0) || (ia[0] == 27 && e.subclass == 5 && unsafe_entry(3, true)) {
                ia[0] = 0;
            }
        }
        // batch A33: a railed transport's update (which may follow its path: the follow-waypoint-path state, 3) where that entry
        // is undefined: op 66 instead (its transit command, 27, is command 0 instead above)
        65 if e.subclass == 5 && unsafe_entry(3, true) => *op = 66,
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
    if std::env::var("AI_A2_FULL").is_ok() {
        println!("  rust full {r:?}");
        println!("  cpp  full {c:?}");
    }
    println!("  rust {:?}", &r[lo..(i + 8).min(r.len())]);
    println!("  cpp  {:?}", &c[lo..(i + 8).min(c.len())]);
}

/// A boolean result flipped (the planted bug of the results).
fn rust_op_bug_view(r: &[u32]) -> Vec<u32> {
    r.iter().map(|x| x ^ 1).collect()
}

/// # Results (2026-10-30, batch A37; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - `DeliverPayloadAIUpdate` (DeliverPayloadAIUpdate.cpp cut verbatim with its header's machine, states and `DeliverPayloadData`,
///   RadiusDecal.cpp's template constructor and xfers; the decal, the bones, the subobjects, the items' positions, contains,
///   modules and forces, the visible payload (a new object), the partition and the physics' turning the scene's stand-ins,
///   `LOG_DELIVER`; the AI's calls as ops 109-117, which need the AI's machine; the move-and-evacuate-and-exit command often): a
///   ninth of the cases: 30,000 constructions and 1,762,739 operations, **0 mismatches** in the results, the call logs and the
///   dumps (its members, data and machine added). The delivery machine after an operation: approach 116,617, delivering 20,218,
///   considering 3,646, recovering 2,597, heading off 6,442, cleaning up 1,287, none 33,130.
/// - Not compared (undefined upstream; refused, `StateMachineError::Undefined`, the case ends before the C++): a locomotor call
///   without a current locomotor, a physics call without physics, the first item of an empty contain list, a visible payload of
///   an unknown template (`newObject` throws) or with no controlling player; 1,211 cases ended undefined in all (with the earlier
///   batches'). The stand-in templates have no build variations (upstream's `newObject` would draw one at random).
/// - Planted bugs (5,000 cases each): 3680 (off the map), 3681 (the pre-open distance), 3682 (the turn radius), 3683 (no dive),
///   3684 (the dive's sound), 3685 (the strafe), 3686 (the approach's move again), 3687 (the drop delay), 3688 (the weapon
///   delivery), 3689 (the attempts), 3690 (the re-entry frame), 3691 (the self-destruction), 3692 (the turned-away kill), 3693 (the
///   decal's update), 3694 (commands refused heading off), 3695 (the scripted delivery's attempts), 3696 (the machine's snapshot)
///   caught. Five first placed where they skipped a refusal (the dive, the approach, the delivery's end, the machine's update, the
///   scripted delivery) let the C++ reach a null locomotor and crash; they were moved.
///
/// # Results (2026-10-30, batch A36; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - `MissileAIUpdate` (MissileAIUpdate.cpp cut verbatim with its header's module data; the weapon templates, the store, the
///   particle systems, the launcher's bonus, the countermeasure, the garrison's immunity, the 2-D and 3-D distances, the damage and
///   the drawable the scene's stand-ins, `LOG_MISSILE`; the weapon's calls as ops 104-108, which need the AI's machine): an eighth
///   of the cases: 30,000 constructions and 1,777,193 operations, **0 mismatches** in the results, the call logs and the dumps
///   (its members added).
/// - Not compared (undefined upstream; the Rust side refuses them, `StateMachineError::Undefined`, and the case ends before the
///   C++): a collision with an object or the weapon's speed without a detonation weapon, a countermeasure that is not there, the
///   garrison kill loop reaching its list's end (its `*it != NULL` test reads the end); 752 cases ended undefined in all (with
///   A9's and A33's). The first plant of the garrison loop, which skipped the loop and so the refusal, let the C++ loop for ever on
///   its circular list.
/// - Planted bugs (5,000 cases each): 3670 (the launcher's kill score; 4 mismatches), 3671 (the missile's own death; 6), 3672 (the
///   lock; 10), 3673 (the countermeasure's diversion; 6), 3674 (the bridge's detonation; 24), 3675 (the snapshot's jammed flag; 338)
///   caught.
///
/// # Results (2026-10-30, batch A35; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - `SupplyTruckAIUpdate` (SupplyTruckAIUpdate.cpp cut verbatim with its header's machine, states and interface, and
///   WorkerAIUpdate.h's interface; the resource manager's best center and warehouse, the player's closest by kind, the 2-D distance
///   to an object and the drawable's supply status the scene's stand-ins, `LOG_SUPPLY`; the dock's warehouse and center modules by
///   the scene's masks; the depleted voice "s7"; the docks' calls `gainOneBox`, `loseOneBox`, `setForceWantingState` as ops
///   101-103): a seventh of the cases: 30,000 constructions and 1,797,876 operations, **0 mismatches** in the results, the call
///   logs and the dumps (its members and its machine's state, sleep and default-state flag added).
/// - The first runs found two mistakes of the translation, fixed: the transitions checked on entering the default state in the AI's
///   constructor reached the AI (upstream: none yet, so every condition answers false), and the CRC took the machine's `xfer`
///   (upstream's `xferSnapshot` in CRC mode is its `crc`, which writes nothing). `getSupplyTruckAIInterface` is dispatched
///   explicitly in the harness, as `isIdle` (another object's AI has no vtable); the stand-in interface answering the dock's
///   delay 0 is gone.
/// - Planted bugs (5,000 cases each): 3660 (the wanting state's dock; 57 mismatches), 3661 (regrouping near the base; 1), 3662
///   (forced busy; 2), 3663 (a full truck; 421), 3664 (the player's idle; 9), 3665 (the preferred dock; 18), 3666 (the snapshot's
///   forced-wanting flag; 445), 3667 (the snapshot's machine; 383) caught.
///
/// # Results (2026-10-29, batch A34; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - `HackInternetAIUpdate` (HackInternetAIUpdate.cpp cut verbatim with its header's states and machine, but for the floating cash
///   text, the client's: `hackCashText`, `LOG_HACK`; AI.h's `AICommandParmsStorage` and AIStates.cpp's members verbatim, the stored
///   damage upstream's default; Damage.h's classes and Damage.cpp's xfer verbatim, renamed `Stored*`, for it; the money, tracker,
///   locally-controlled bits and level the scene's; its command, 34, often; the controlling player kept): a sixth of the cases:
///   30,000 constructions and 1,798,178 operations, **0 mismatches** in the results, the call logs and the dumps (the waiting
///   command's storage and the three states' frames added).
/// - `isIdle` is virtual upstream and the hacker overrides it: making it virtual in the harness crashed on another object's AI (not
///   constructed: no vtable), so the harness dispatches it explicitly (`baseIsIdle`, `hackIsIdle`). The old stand-in (a non-hacker's
///   `HackInternetAIUpdate` module answering `hackInternet`, `modules` bit 2) is gone: only a hacker has that module.
/// - Planted bugs (5,000 cases each): 3650 (the storage's second word; 14 mismatches), 3651 (the fast delay; 16), 3652 (the
///   veterancy's cash; 2), 3653 (the floating text; 3), 3654 (packing on a command; 41), 3655 (the storage saved; 14), 3656
///   (`isIdle` with a command waiting; 2) caught.
///
/// # Results (2026-10-29, batch A33; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - `RailedTransportAIUpdate` (RailedTransportAIUpdate.cpp cut verbatim, its `INVALID_PATH` too, but for the object's own dock,
///   the scene's: `railedOwnDock`; the prefix "rail"; the waypoints "railStartNN"/"railEndNN" present by the scene's masks; a
///   railed transport dock on the second behavior module when the scene says so, `isLoadingOrUnloading` by hash, `LOG_RAILED`; its
///   two commands, 26 and 27, drawn often; every locomotor set present): a fifth of the cases: 30,000 constructions and 1,798,121
///   operations, **0 mismatches** in the results, the call logs and the dumps (its members added).
/// - Not compared (undefined upstream): following its path with no locomotor (the waypoint state's `onEnter` dereferences it).
///   Before the op, `a9_guard` makes such an update op 66 and such a transit command command 0; when the base's update leaves no
///   locomotor before the railed part follows its path, the Rust side refuses it (`StateMachineError::Undefined`), and the case
///   ends before the C++ runs the op (37 cases ended undefined in all, A9's and these).
/// - Planted bugs (5,000 cases each): 3640 (the player's commands filter; 339 mismatches), 3641 (a path's slot; 133), 3642 (the
///   closest path's tie; 172), 3643 (ultra-accurate; 78), 3644 (no unloading in transit; 82), 3645 (the snapshot's loaded flag; 682)
///   caught.
///
/// # Results (2026-10-29, batch A32; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - `AssaultTransportAIUpdate` (AssaultTransportAIUpdate.cpp cut verbatim but for its members' AIs, other objects' and not
///   constructed in this build: each an `AssaultMemberAi` whose calls are logged cut points, `LOG_MEMBER_AI`; its header's
///   `AssaultStateTypes` and `MAX_TRANSPORT_SLOTS` verbatim; the healing ratio of 0, 0.3, 0.6 or 1 per case; the members' health by
///   hash over 100; the passengers with an AI only listed, and contained; the weapon's `beginAssault` as op 100, 38,550 times): a
///   quarter of the cases (a quarter `AIUpdateInterface`, an eighth each wanderer and transport, a quarter deployers): 30,000
///   constructions and 1,798,463 operations, **0 mismatches** in the results, the call logs and the dumps (its members added).
/// - Planted bugs (5,000 cases each): 3630 (an attack order's flag; 144 mismatches), 3631 (a member's removal; 119), 3632 (a new
///   occupant's mark; 27), 3633 (a wounded member called back; 20), 3634 (the snapshot's attack-object flag; 850) caught.
///
/// # Results (2026-10-29, batch A31; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - `DeployStyleAIUpdate` (DeployStyleAIUpdate.cpp cut verbatim, its header's `DeployStateTypes` too; the module data the
///   scene's: times of 0 to 39 frames, a quarter of each zero, the four flags and the drawable drawn per case; the per-unit sounds
///   "s4"/"s5" or empty; `setAnimationFrame` and `clearModelConditionFlags` logged): a third of the cases (now a third
///   `AIUpdateInterface`, a sixth each wanderer and transport): 30,000 constructions and 1,798,455 operations, **0 mismatches** in
///   the results, the call logs and the dumps (the deployer's state and wait frame added to the dump; its version-4 snapshot).
/// - The first 3,000-case run found a **port defect from batch A30**: `ai_attack_object_on_locked_machine` (an enter state's
///   attack inside a guard machine) inlines `privateAttackObject` and so skipped `TransportAIUpdate`'s override (the passengers'
///   attack, 4 `LOG_PASSENGER` records missing); fixed.
/// - Planted bugs (5,000 cases each): 3620 (the wait's end; 39 mismatches), 3621 (guarding idle deploys; 7), 3622 (centre the
///   turrets first; 8), 3623 (the deploy's animation frame; 13), 3624 (a reversed undeploy's unpack time; 7), 3625 (the deployed
///   status; 27), 3626 (the snapshot's wait frame; 1,201) caught.
///
/// # Results (2026-10-29, batch A30; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - The AI's class per case (half `AIUpdateInterface`, a quarter each `WanderAIUpdate` and `TransportAIUpdate`, cut verbatim
///   from WanderAIUpdate.cpp and TransportAIUpdate.cpp; the passengers' commands and `privateIdle`'s passengers' `aiIdle` are
///   logged cut points, `LOG_PASSENGER`): 30,000 constructions and 1,798,557 operations, **0 mismatches** in the results, the
///   call logs and the dumps (with the subclasses' version byte before the base's snapshot).
/// - The first runs found harness defects, fixed: the passenger list for a transport (with AI passengers, never NULL) reached
///   `privateIdle`'s `aiIdle` of another object's unconstructed AI (a SIGSEGV), and the class was drawn again with some ops'
///   scenes (a transport whose list then came back NULL); the host's idle-passengers call ignored the contain's absence.
/// - Planted bugs: 3610 (the wanderer's move; 5 mismatches at 5,000 cases), 3611 (the command's source; 7), 3612 (a disabled
///   portable passenger; 1), 3613 (the subclass's version byte; 1,758) caught.
///
/// # Results (2026-10-29, batch A23; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - `joinTeam` (op 91) and the AI's turret members (ops 92-99): 30,000 constructions and 1,798,300 operations, **0 mismatches** in
///   the results, the call logs and the dumps; 2,346 joins and 3,554 to 7,490 of each turret op. Not generated: `joinTeam` in the
///   waypoint states (undefined: their exit reads the locomotor its normal set may leave unset).
/// - The first runs found two harness defects, fixed: the other AIs' machine was made at the first case's first operation, after
///   the random values were seeded (AIIdleState's constructor draws one); the host's other-object `getAI()` also required the
///   object to exist, which the team's members need not.
/// - Planted bugs at 30,000 cases: 3520 (the other's goal object taken), 3521 (the turret's aim), 3522 (the turret for a weapon
///   slot) caught (409 to 1,720 mismatches).
///
/// # Results (2026-10-29, batch A21; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - The AI's snapshot (`AIUpdateInterface::crc`, `xfer`, `loadPostProcess`; ops 88-90: save, load then post-process, CRC):
///   30,000 constructions and 1,798,811 operations, **0 mismatches** in the results (the saved bytes, the statuses, the CRCs), the
///   call logs and the dumps. 27,669 saves and 2,334 loads succeeded. A load is generated only into a machine in the save's state
///   and temporary state; 6 cases ended at a failed load (the object half loaded) and 21 at a load of the machine (op 71) that
///   made one state both current and temporary (undefined, as `a9_guard` keeps from the setters).
/// - The first runs found one port defect, fixed: `LocomotorSet::xferSelfAndCurLocoPtr` goes through `xferSnapshot`, so a CRC
///   visits the set's (empty) `crc`; the port transferred it.
/// - Planted bugs at 30,000 cases: 3501 (the path made anew on load), 3502 (the pathfinder told the position after a load) and 3503
///   (the set's CRC) caught (150 to 6,671 mismatches); 3500 (the guard target types' overrun into `m_locationToGuard`) unobservable,
///   the location's own transfer overwriting it at once.
///
/// # Results (2026-10-29, batch A20; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - The tunnel-network guard state and its machine (`AITNGuard.cpp`): 30,000 constructions and 1,799,163 operations, **0
///   mismatches** in the results, the call logs and the dumps (which add the machine's state, goal, sleep and members, its attack
///   states' give-up frames and embedded attacks, the return state's enter members and scan time, the idle and crate states'). After
///   an operation the machine was in the inner state 2,078 times, idle 7,908, outer 8, returning 6,630, attacking an aggressor 10,
///   picking up a crate never; the network's nemesis was offered 50 times and the owner left a tunnel in a hurry 63 times. The
///   tunnels are the owner's container then the scene's; their damage is a hash of their id (source, time) and the scene's
///   no-effect bits; other objects' AIs share the scene's goal. Not generated: the state already the machine's or the temporary
///   state, or without a controlling player (the inner state offers to the network unchecked). One case ended where upstream is
///   undefined: the transition-depth limit (20) left an attack state without its attack, which the next update dereferences.
/// - The first full run found one port defect, fixed in both guard machines: after the idle state's `setState` to the crate state,
///   the returned sleep reads `m_nextEnemyScanTime` anew (the transitions may have entered the idle state again).
/// - Planted bugs at 30,000 cases: 3420, 3422-3430 caught (1 to 3,830 mismatches); 3421 (the nearest tunnel) unobservable, every
///   other object standing at the one stand-in position; 3409 (the guard's re-read) not reached, its twin 3430 caught.
///
/// # Results (2026-10-29, batch A19; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - The guard-retaliate state and its machine (`AIGuardRetaliate.cpp`): 30,000 constructions and 1,799,536 operations, **0
///   mismatches** in the results, the call logs and the dumps (which add the retaliate machine as the guard machine's). After an
///   operation the machine was retaliating 84,053 times. Not generated: the state already the machine's or the temporary state, or
///   without a controlling player; an enter guard while it is entered or runs (it does not lock the AI's machine, so its enter
///   state's attack command would switch the AI's state and delete the machine it runs in: undefined upstream).
/// - The first runs found one port defect, fixed: the retaliate machine's states do not override `isAttack` (nor `isGuardIdle`), so
///   they answer false while their embedded attack runs; the swapped-in attack state answered true, and the approach state's
///   `getNextMoodTarget` then took the AI as attacking.
/// - Planted bugs: plants 3410-3414 at 30,000 cases, each caught (16 to 3,296 mismatches): the owner's guard range in the exit
///   conditions, the buildings rejected, the inner radius's factor, the aggressor made the nemesis, the idle state's failure.
///
/// # Results (2026-10-29, batch A18; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - The guard state and its guard machine (`AIGuard.cpp`: inner, idle, outer, return, crate and attack-aggressor states, the
///   aggressor condition, `ExitConditions`, `lookForInnerTarget`), with the inner, outer and aggressor states' embedded attack (or
///   enter) states: 30,000 constructions and 1,799,323 operations, **0 mismatches** in the results, the call logs and the dumps
///   (which add the guard machine's state, goal, sleep and members, the exit conditions, the embedded state's kind, follow flag and
///   machine state, and the return, idle and crate states' members). After an operation the machine was guarding 68,983 times; its
///   guard machine in the inner state 2,087 times, idle 1,368, outer 162, returning 65,265, attacking an aggressor 101, picking up a
///   crate never (the crate is never found).
/// - The guard's search is the closest-object stand-in, logged with what its filters are (`LOG_CLOSEST_OBJECT` 0xfffffffe); the
///   template's enter and hijack guard flags, the trigger-area frame and the AI data's guard rates are the scene's; the body's
///   clearable last attacker is the scene's, cleared for the rest of the operation. Not generated: the guard state already the
///   machine's or the temporary state, or without a controlling player.
/// - The first runs found, all fixed: a harness gap (the polygon filter's stand-in read its trigger, which the guard passes
///   empty: a crash), a harness defect (the Rust host cleared the attacker in the scene that the C++ side then ran), and three port
///   defects in shared code: the enter state's `ai->friend_setGoalObject` and its attack command must reach the AI's machine from
///   inside the guard machine (the latter with the AI's machine locked: only the command's other effects remain), and the goal
///   sets lent from a sub-machine must be applied in order (an object, then none, leaves the goal position at the object's).
/// - Planted bugs: plants 3400-3408 at 30,000 cases: caught but two (2 to 5,613 mismatches): the exit's Z, the attacker cleared,
///   flying units only, the aggressor's follow (caught only once the dump showed the embedded attack's follow flag), the team
///   target cleared, the return's adjustment, the attack's exit check. **Not caught, unreachable here:** 3406 (a guarded object
///   that moved in y: the other objects are always at the origin) and 3407 (the guard machine's default state failing: the inner
///   state never fails on entry).
///
/// # Results (2026-10-28, batch A17; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - The dock state and its docking machine (`AIDock.cpp`: approach, wait for clearance with its advance condition, advance, move
///   to the entry, into the dock, process, move to the exit, to the rally point): 30,000 constructions and 1,799,333 operations,
///   **0 mismatches** in the results, the call logs and the dumps (which add the dock state, its machine's state, goal, sleep, lock
///   and approach position, and its states' members). After an operation the machine was docking 10,635 times; its docking machine
///   approaching 9,940, waiting 136, advancing 306, moving to the entry 141, into the dock 33, processing 31, moving to the exit 48,
///   to the rally point never (with no rally point its entry succeeds at once; with one, the move rarely survives an operation).
/// - The dock (`DockUpdateInterface`) is a logged stand-in (`LOG_DOCK`): its answers (reserve, advance, clear to enter, clear to
///   advance, action) from the hash at the scene's rate, its open, passthrough and rally flags the scene's, its positions 0.1 apart
///   from the scene's dock position, which is the owner's half the time (so that the moves arrive). The other objects with a dock
///   are fixed for a case (the move-to-dock state asks its goal's dock unchecked). In the dock state the update is forced half the
///   time and the frame jumps up to 400 a quarter of the time (the 30-second wait).
/// - Planted bugs: plants 3390-3395 at 30,000 cases, each caught (1 to 779 mismatches): the approach position cleared at the entry,
///   the machine locked into the dock, the cancel on interruption, the wait's limit, the drone, paths through units. (3391, 3393 and
///   3394 were not caught until the dock position followed the owner and the frame jumped.)
///
/// # Results (2026-10-28, batch A16; upstream commit 0a05454d, seeds `0xA2_0000 + case`, 30,000 cases)
///
/// - The attack-squad state (`chooseVictim` with the AI's machine's goal squad): 30,000 constructions and 1,799,455 operations,
///   **0 mismatches** in the results, the call logs and the dumps (which add its machine's state, goal, sleep and attack machine).
///   After an operation the machine was attacking a squad 38,798 times. The player's difficulty and the script engine's "always
///   normal" are the scene's; the closest squad member (normal) is the existing one with the smallest hash. Not generated: the
///   state already the machine's or the temporary state (the other's exit deletes the one machine), or without a controlling
///   player (read unchecked).
/// - Planted bugs: plants 3380-3383 at 30,000 cases, each caught (1 to 1,453 mismatches): the passive mood's attacker, a player's
///   command as hard, the hard choice's member, the attack after a new victim.
///
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
        if std::env::var("AI_A2_CASEPRINT").is_ok() {
            eprintln!("CASE {case}");
        }
        run_case(0xA2_0000 + case, &mut ck);
    }
    println!(
        "COVERAGE: {} cases ({} ended at a mismatch); ops {:?}; doLocomotor sleeps none/longer {:?}; moved by goal none/path/explicit/angle {:?}; \
         path frames {}; final-position moves {}; locomotor changes {}; mood targets {}, victims {}, enter targets {}; \
         A4-A10 log records 70..=103 {:?}; current state idle/dead/wait/busy/other/moveTo/outOfWay/tighten/repulsors/followPath/evacuate/\
         delete/waypointPath/exact/wander/wanderInPlace/panic/attack/attackMove/crate/hunt/attackFollow/face/exit/enter/rappel/squad/dock/guard/retaliate {:?}; A9 cases ended undefined {}; attack machine \
         chase/approach/aim/fire {:?}; LOG_ATTACK kinds {:?}; attack-move machine idle/crate/attack {:?}; hunt machines attack/crate/idle {:?}; docking machine approach..rally {:?}; guard machine inner/idle/outer/return/crate/aggressor {:?}; tunnel-network guard machine likewise {:?}, network nemesis offers/hurried exits/cases ended undefined {:?}; AI saves/loads that succeeded, cases ended at a failed load, at a state both current and temporary {:?}; attack-follow machines idle/crate/attack {:?}; temporary states {}; loads {}; delivery machine approach..clean up/none {:?}",
        ck.cases, ck.ended_early, ck.ops, ck.sleeps, ck.goals_moved, ck.paths_followed, ck.final_pops, ck.loco_changes, ck.mood_targets, ck.victims,
        ck.enter_targets, ck.a4_logs, ck.a4_states, ck.a9_undefined, ck.attack_inner, ck.attack_logs, ck.attack_move_inner, ck.hunt_inner, ck.dock_inner, ck.guard_inner, ck.tn_inner, ck.tn_logs, ck.ai_snapshot, ck.follow_inner, ck.temporary_states, ck.loads,
        ck.deliver_states
    );
    ck.construction.finish();
    ck.results.finish();
    ck.log.finish();
    ck.dump.finish();
    ck.results_bug.finish_planted();
    ck.dump_bug.finish_planted();
}
