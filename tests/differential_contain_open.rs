#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/OpenContain.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/TransportContain.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `OpenContain` (`logic::modules::open_contain`, batch R) and `TransportContain` (`logic::modules::transport_contain`, batch R2) against the unchanged C++,
//! `p1w13-sonnet`.
//!
//! # Method
//!
//! The C++ side is `OpenContain.cpp`, compiled UNCHANGED into `wrap_module.cpp`'s unit (`differential/l9_open_contain_r.inc`) on the module build's stand-ins, with
//! the REAL `DieMuxData`, `INI`, `RandomValue`, `Matrix3D`, `Xfer`, `Object::kill` and `Object::getRelationship`. A scene of up to four objects is built, each with a
//! team and a player, optionally a drawable (selected, with model conditions), an AI, a physics block, a stealth update and a body, and an `OpenContain` on those that
//! have one (the first reads the INI, the others take their data from the scene). Every call the container makes on the rest of the world is a line of the effect log,
//! in the order it is made.
//!
//! The Rust side is the real thing: a `World` with the same scene (`World::new_object` with the container from `modules::new_module` and `ProbeModule`s for the
//! AI, physics, stealth and body), the cut points of `contain_cuts` answered from the same tables, `cuts.contain.stub_object_calls` set (the Object-level calls that
//! cascade are records only; see `contain_cuts`), and the ops run as `Object` runs them: `World::module_call`. The scene is one list of words (`cfg`), written by
//! `Obj::write` and read by `Obj::read` (the C++ driver reads the same order).
//!
//! Each case reads a `D` block (the fifteen fields; mostly well formed) for the first container and runs 1 to about 60 ops: adding and removing objects, killing
//! and harming them, collisions, deaths, updates, the exits through the door, the orders to the passengers, the queries, the environment (the frame, the selection,
//! the model conditions, the statuses, the places, the booby traps, the AI) and the three xfers with the load post process.
//!
//! Batch R3 adds the `I` (`InternetHackContain`) and `R` (`RailedTransportContain`) blocks (the transport's fields), class codes 2 and 3 for the scene's
//! containers, and per object a dock behaviour after the contain (open or closed; `setDockOpen` is a line of the log).
//!
//! Batch R6 adds the `O` block (`OverlordContain`: the transport's fields, `PayloadTemplateName`, `ExperienceSinkForRider`) and class code 6. A ring of
//! redirecting Overlords is undefined upstream and excluded (the C++ reference would loop until the driver's alarm).
//!
//! Batch R5 adds the `C` block (`RiderChangeContain`: `Rider1`..`Rider8`, `ScuttleDelay`, `ScuttleStatus`), class code 5 with the scene's rider sets,
//! the AI's `isMoving`, the local player (player 0), and each object's command set override and veterancy level. The scene's templates are the factory's
//! (the rider match is `isEquivalentTo` of the factory's templates).
//!
//! Batch R4 adds the `H` block (`HelixContain`: the transport's fields, `PayloadTemplateName`, `ShouldDrawPips`), class code 4, the PORTABLE_STRUCTURE kind,
//! ops 56 and 57, and each object's weapon bonus word. Both sides register the containers' update modules as `GameLogic::registerObject` does, so a save's
//! scheduling word is compared, not masked (it used to be masked at a fixed offset that was right only for `OpenContain`).
//!
//! Batch R2 adds `TransportContain`: one data block in two is a `T` block (the base's fields, then `Slots`, `ExitBone`, `ExitPitchRate`, `InitialPayload`,
//! `HealthRegen%PerSec`, `ExitDelay` and the eight flags), and each later container is a transport one time in two (its data from the scene). The scene adds, per
//! object, what a transport reads of its riders and itself: the template's raw transport slot count, an airborne locomotor, the AI's free-to-exit answer and
//! locomotor, the movement terrain, the physics block's centre of mass and velocity, the three weapon slots (contact, damage), `DISABLED_UNMANNED`, the kinds
//! of the Jarmen Kell hack (`CLIFF_JUMPER`, `HERO`, `SALVAGER`) and the art's exit bone. Ops 49 to 55 are the transport's own: `reserveDoorForExit`,
//! `isExitBusy`, `getContainMax`, `getExtraSlotsInUse`, `onCapture`, `killRidersWhoAreNotFreeToExit`, `isSpecificRiderFreeToExit`. The payload's factory
//! finds the scene's templates by name and makes nothing (`newobject <template> <team>` is a line of the log; a missing template throws `ERROR_BAD_ARG` out of
//! `update`, compared as the op's status), so the payload's riders are never added: that half of `createPayload` (plant 1424) is left to a test with a real
//! factory.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, a result word, the effects in order, the xfer status, bytes and CRC, and the state of
//! every object (the container's lists, counters, fire points, rally point and flags; the place, the container, the statuses).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` flips one branch of `open_contain.rs` or `transport_contain.rs`; each must make the test fail at 9,000 cases (all 37 of batch R below were caught, as mismatches or a crash):
//!
//! | id | planted bug |
//! |---|---|
//! | 1371 | `allow_allies_inside` test inverted |
//! | 1372 | neutral objects judged by `allow_enemies_inside` |
//! | 1373 | `forbid_inside_kind_of` ignored |
//! | 1374 | `player_entered_mask` ORed instead of set |
//! | 1376 | the already-contained refusal of `addToContain` dropped |
//! | 1377 | `STEALTH_GARRISON` count not kept |
//! | 1378 | the `containedBy != me` refusal of `removeFromContain` dropped |
//! | 1379 | `exposeStealthUnits` inverted |
//! | 1380 | `setLayer` on removal skipped |
//! | 1381 | `kill` of a contained object skipped |
//! | 1382 | harm: damage before removal instead of after |
//! | 1383 | load sound frame guard `||` |
//! | 1384 | `lastUnloadSoundFrame` not stored |
//! | 1385 | closest exit picks the farthest |
//! | 1386 | scatter radius 2x instead of 1.5x |
//! | 1387 | no-AI exit lands at the container |
//! | 1388 | controlling-player comparison inverted |
//! | 1389 | `isValidContainerFor` check dropped on collide |
//! | 1390 | burned/normal death type swapped |
//! | 1391 | percent-damage kill threshold 0.5 |
//! | 1392 | `onObjectWantsToEnterOrExit` always takes the container path |
//! | 1393 | drawable not hidden |
//! | 1394 | `removeAllContained(true)` for the wrong flag |
//! | 1395 | contained mass is the maximum, not the sum |
//! | 1396 | door closes one frame early |
//! | 1397 | `playerEnteredMask` not cleared on update |
//! | 1398 | effectively dead riders kept |
//! | 1399 | condition-state change ignored |
//! | 1400 | order-all-exit iterates forward |
//! | 1401 | fire point number padding at 9 |
//! | 1402 | fire point wraps one early |
//! | 1403 | exit bone number padding at 9 |
//! | 1404 | exit path counter from 0 |
//! | 1405 | idle test accepts any vehicle |
//! | 1406 | exit path has one point |
//! | 1407 | `instantly` inverted |
//! | 1408 | money hacker test always true |
//!
//! Batch R2, `transport_contain.rs` (at 9,000 cases, 20 of the 22 caught; the generator was widened for 1413, 1426 and 1430, which survived the first run, and the
//! 37 above were run again on the widened generator and are still all caught):
//!
//! | id | planted bug |
//! |---|---|
//! | 1411 | a zero-slot rider not replaced by its first passenger (SURVIVES: the scene has no zero-slot container; `ParachuteContain`'s test will see it) |
//! | 1412 | controlling-player test inverted |
//! | 1413 | `m_extraSlotsInUse` left out of the capacity check |
//! | 1414 | the damage-weapon test inverted |
//! | 1415 | the weapon set flag not cleared when no rider is armed |
//! | 1416 | the rider not `DISABLED_HELD` |
//! | 1417 | the Jarmen Kell transfer on the way in skipped |
//! | 1418 | `orientLikeContainerOnExit` inverted |
//! | 1419 | the motive force's z not scaled by the mass |
//! | 1420 | the above-terrain test inverted |
//! | 1421 | the scatter-when-dead test inverted |
//! | 1422 | the exit delay ignored |
//! | 1423 | the Jarmen Kell transfer on the way out reversed |
//! | 1424 | the payload never added (SURVIVES: the test's factory makes nothing; see "Method") |
//! | 1425 | no healing |
//! | 1426 | riders at full health healed |
//! | 1427 | `destroyRidersWhoAreNotFreeToExit` inverted |
//! | 1428 | the free-to-exit test inverted |
//! | 1429 | the movement terrain test dropped |
//! | 1430 | the infantry test of `isPassengerAllowedToFire` inverted |
//! | 1431 | `isExitBusy` on the frame itself |
//! | 1432 | the `DISABLED_UNMANNED` test of `onCapture` inverted |
//!
//! Batch R3, `internet_hack_contain.rs` and `railed_transport_contain.rs` (9,000 cases: all 6 caught; the 57 caught plants above were run again on the
//! generator with the four classes and are all still caught):
//!
//! | id | planted bug |
//! |---|---|
//! | 1441 | InternetHack: the rider not told to hack |
//! | 1442 | InternetHack: the hack ordered `CMD_FROM_PLAYER` |
//! | 1443 | RailedTransport: the dock not reopened when the last rider is out |
//! | 1444 | RailedTransport: the dock closed instead of opened |
//! | 1445 | RailedTransport: the closed-dock test of `isSpecificRiderFreeToExit` inverted |
//! | 1446 | RailedTransport: `exitObjectViaDoor` exits through the base (as if the static helper were called) |
//!
//! Batch R4, `helix_contain.rs` (9,000 cases: 11 of 12 caught):
//!
//! | id | planted bug |
//! |---|---|
//! | 1451 | a second portable structure taken while one is held |
//! | 1452 | the payload never added (SURVIVES: the stub factory makes no object, and a Helix dereferences it: undefined, excluded) |
//! | 1453 | `onObjectCreated` does not create the payload |
//! | 1454 | the portable structure does not follow the Helix |
//! | 1455 | `redeployOccupants` at the Helix's height, not 8 above |
//! | 1456 | the rubble test of `onBodyDamageStateChange` inverted |
//! | 1457 | the portable structure's team not changed on capture |
//! | 1458 | the portable structure not forgotten by `removeFromContain` |
//! | 1459 | xfer version 1 (no portable structure saved) |
//! | 1460 | no garrisoned weapon bonus for a rider |
//! | 1461 | the stealth grant's test inverted |
//! | 1462 | the garrisoned weapon bonus not cleared |
//!
//! Batch R5, `rider_change_contain.rs` (9,000 cases: all 9 caught):
//!
//! | id | planted bug |
//! |---|---|
//! | 1471 | `isExitBusy` TRUE |
//! | 1472 | the base asked with the capacity check |
//! | 1473 | the rider's status not set on the bike |
//! | 1474 | the rider's veterancy not given to the bike |
//! | 1475 | the dead-bike test of `onRemoving` inverted |
//! | 1476 | no scuttle when the rider leaves |
//! | 1477 | the moving test of the scuttle inverted |
//! | 1478 | the scuttle delay ignored |
//! | 1479 | `m_payloadCreated` not saved a second time |
//!
//! Batch R6, `overlord_contain.rs` (9,000 cases: 8 of 9 caught; the 83 caught plants above were run again and are still all caught):
//!
//! | id | planted bug |
//! |---|---|
//! | 1491 | the payload never added (SURVIVES: the stub factory makes no object, and `isValidContainerFor(NULL)` is `FALSE`) |
//! | 1492 | no damage state for the rider |
//! | 1493 | the rider's team not changed on capture |
//! | 1494 | `addToContain` not redirected |
//! | 1495 | redirection never activated |
//! | 1496 | `killAllContained` kills the Overlord's own riders |
//! | 1497 | `getContainedItemsList` not redirected |
//! | 1498 | the nested-containment test of `isPassengerAllowedToFire` inverted (caught as a crash: an endless recursion) |
//! | 1499 | `m_redirectionActivated` not saved |
//!
//! Not planted, unobservable here: the `wasSelected` argument of `onContaining` (1375, removed). `OpenContain::onContaining` ignores it; ~~`TransportContain` and `GarrisonContain` use it, and
//! their tests will catch it~~ **CORRECTED 2026-10-22 (batch R2)**: `TransportContain::onContaining` only hands it to the base, so it is unobservable there too;
//! of the contain family only `RiderChangeContain` reads it (it reselects the container), so its test is the one that can catch it.
//!
//! # Deviations the test cannot see
//!
//! The Object-level calls that cascade are stubbed on the Rust side (`onContainedBy`, `onRemovedFrom`, `kill`, `attemptDamage`, `setPosition`, `setLayer`,
//! `setOrientation`, `setTransformMatrix`, `leaveGroup`, `checkAndDetonateBoobyTrap`, `destroyObject`): the C++ side stands in for them with a log line, and each has
//! its own differential test. A container that kills or damages a contained object takes `getBodyModule()->getMaxHealth()` from a body that logs. The partition
//! manager, the pathfinder, the terrain layers and the art's bones are stand-ins on both sides. Upstream destroys the objects of a contain list that a load finds
//! where the list was not empty in the middle of the xfer; the translation (which has no world there) hands them to the world (`DrainXferDestroys`), which the test
//! asks for straight after the xfer: the same lines, in the same op.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::{ObjectId, INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::model_state::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, MODULEINTERFACE_BODY};
use rts_lockstep_engine::common::object_status_types::*;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate, ThingTemplateAudioType};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::*;
use rts_lockstep_engine::logic::modules::open_contain::{OpenContain, OpenContainModuleData};
use rts_lockstep_engine::logic::modules::transport_contain::{InitialPayload, TransportContainModuleData};
use rts_lockstep_engine::logic::world::object_cuts::WeaponSlotFacts;
use std::collections::BTreeMap;
use rts_lockstep_engine::logic::world::body_cuts::info_line;
use rts_lockstep_engine::logic::world::contain_cuts::ContainCall;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9ct_r.bin");
const XFER_FILE: &str = "l9ct_r.bin";

/// The kinds, statuses and model conditions the scene's masks name (local bit i is `KINDS[i]`, ...), as the driver's `kCtKinds`, `kCtStatus` and `kCtConds`.
const KINDS: [i32; 8] =
    [KINDOF_STEALTH_GARRISON, KINDOF_STRUCTURE, KINDOF_VEHICLE, KINDOF_MONEY_HACKER, KINDOF_INFANTRY, KINDOF_SELECTABLE, KINDOF_PROJECTILE, KINDOF_IMMOBILE];
const KIND_NAMES: [&str; 8] = ["STEALTH_GARRISON", "STRUCTURE", "VEHICLE", "MONEY_HACKER", "INFANTRY", "SELECTABLE", "PROJECTILE", "IMMOBILE"];
const STATUSES: [i32; 3] = [OBJECT_STATUS_IS_ATTACKING, OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_STEALTHED];
const STATUS_NAMES: [&str; 3] = ["IS_ATTACKING", "AIRBORNE_TARGET", "STEALTHED"];
const CONDS: [i32; 6] = [MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DAMAGED, MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_NIGHT, MODELCONDITION_SNOW];
/// The exit bones the scene can name: the plain pair, the numbered pairs 1 to 3, then those at the width boundary of the number (`01`..`09` are padded, `10` and
/// on are not; `9` and `09` are told apart, which is what plant 1403 breaks).
const BONES: [&str; NB] = ["ExitStart", "ExitEnd", "ExitStart01", "ExitEnd01", "ExitStart02", "ExitEnd02", "ExitStart03", "ExitEnd03", "ExitStart09", "ExitEnd09", "ExitStart9", "ExitEnd9", "ExitStart10", "ExitEnd10"];
const NB: usize = 14;
/// Fire points, and turret bones `FIREPOINT01`..: twelve, so that the next fire point reaches 9 and 10 (the width of the number in the turret bone name).
const FP: usize = 12;

// ---- the scene, as words -------------------------------------------------------------------------------------------------------------------------------------------

fn f(x: u32) -> f32 {
    f32::from_bits(x)
}

struct W(Vec<u32>);
impl W {
    fn u(&mut self, x: u32) {
        self.0.push(x);
    }
    fn s(&mut self, x: i32) {
        self.0.push(x as u32);
    }
    fn f(&mut self, x: f32) {
        self.0.push(x.to_bits());
    }
}

struct R<'a> {
    w: &'a [u32],
    i: usize,
}
impl R<'_> {
    fn u(&mut self) -> u32 {
        let v = self.w[self.i];
        self.i += 1;
        v
    }
    fn s(&mut self) -> i32 {
        self.u() as i32
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.u())
    }
}

/// One object of the scene: the words are written by [`Obj::write`] and read by [`Obj::read`] in the order the C++ driver (`l9ctRunInner`) reads them.
#[derive(Clone, Debug, Default)]
struct Obj {
    snd: [u32; 3],
    kinds: u32,
    status: u32,
    player: i32,
    pos: [f32; 3],
    orient: f32,
    layer: u32,
    radius: f32,
    drawable: bool,
    selected: bool,
    cond: u32,
    has_ai: bool,
    ai_idle: bool,
    enter_target: u32,
    has_phys: bool,
    mass: f32,
    can_fall: bool,
    has_stealth: bool,
    has_body: bool,
    max_health: f32,
    booby: bool,
    eff_dead: bool,
    weapon_bonus: u32,
    contained_by: u32,
    has_contain: bool,
    contain_max: i32,
    enter_snd: u32,
    exit_snd: u32,
    allow_fire: bool,
    in_turret: bool,
    exit_paths: i32,
    damage_pct: f32,
    burned: bool,
    door_open: u32,
    allow_mode: u32,
    allow_mask: u32,
    forbid_mask: u32,
    weapon_pass: bool,
    allow_rel: u32,
    death_mask: u32,
    vet_mask: u32,
    exempt_mask: u32,
    required_mask: u32,
    adj_has: bool,
    adj: [f32; 3],
    fp_count: u32,
    fp: [[f32; 3]; FP],
    bone_has: [bool; NB],
    bone: [[f32; 3]; NB],
    turret_has: [bool; FP],
    turret: [[f32; 3]; FP],
    // batch R2 (TransportContain): the words after the turret bones, in the driver's order
    /// 1: the container is a `TransportContain` (the first object's class is the data block's)
    cls: u32,
    raw_slots: i32,
    airborne: bool,
    free_to_exit: i32,
    has_loco: bool,
    valid_terrain: bool,
    com: f32,
    vel: [f32; 3],
    /// per weapon slot s: bit 3s has a weapon, 3s+1 contact, 3s+2 damage
    weapons: u32,
    unmanned: bool,
    /// bit 0 CLIFF_JUMPER, 1 HERO, 2 SALVAGER
    extra_kinds: u32,
    exit_bone_has: bool,
    exit_bone: [f32; 3],
    slot_cap: i32,
    regen: f32,
    exit_delay: u32,
    /// bit 1 orient, 2 keep velocity, 4 aggressive, 8 armed riders upgrade, 16 reset mood, 32 destroy riders, 64 delay exit in air
    tflags: u32,
    pitch: f32,
    payload_count: i32,
    /// index into [`PAYLOADS`]
    payload_name: u32,
    /// index into [`EXIT_BONES`]
    exit_bone_name: u32,
    /// the body's health (a transport heals its riders up to `max_health`)
    health: f32,
    /// batch R3: a dock behaviour (after the contain), and whether it is open
    has_dock: bool,
    dock_open: bool,
    /// batch R4: a scene-built Helix's payload names, an index into [`HELIX_PAYLOADS`]
    helix_payload: u32,
    /// batch R5: a scene-built combat bike's riders (an index into [`RIDER_SETS`]), its scuttle delay and state (an index into [`CONDS`]); the AI's
    /// `isMoving()`
    rider_set: u32,
    scuttle_frames: u32,
    scuttle_cond: u32,
    ai_moving: bool,
    /// batch R6: a scene-built Overlord's `ExperienceSinkForRider` (its payload list is `helix_payload`'s)
    ov_sink: bool,
}

/// The riders a scene-built combat bike can name (the driver's `kCtRiderSets`): per set up to three of (template, index into [`RIDER_TEMPLATES`], 0
/// none; model condition, index into [`CONDS`]; weapon set type; status, index into [`STATUSES`]; command set, 0 `CmdA` 1 `CmdB`; locomotor set).
const RIDER_SETS: [[[u32; 6]; 3]; 4] = [
    [[0; 6], [0; 6], [0; 6]],
    [[1, 2, 9, 0, 0, 3], [0; 6], [0; 6]],
    [[2, 3, 10, 1, 1, 1], [5, 4, 11, 2, 0, 2], [0; 6]],
    [[3, 0, 12, 0, 1, 4], [2, 5, 13, 2, 0, 0], [4, 1, 14, 1, 1, 6]],
];
const RIDER_TEMPLATES: [&str; 6] = ["", "T1", "T2", "T3", "T4", "Nope"];

/// The payload lists a scene-built Helix can name (the driver's `kHelixPayload`).
const HELIX_PAYLOADS: [&[&str]; 4] = [&[], &["Nope"], &["T1"], &["Nope", "T2"]];

/// The payload template names and exit bone names a scene-built transport can name (the driver's `kPayload` and `kExitBone`).
const PAYLOADS: [&str; 4] = ["", "T1", "T2", "Nope"];
const EXIT_BONES: [&str; 3] = ["", "ExitBoneT", "Other"];

impl Obj {
    fn write(&self, w: &mut W) {
        for s in self.snd {
            w.u(s);
        }
        w.u(self.kinds);
        w.u(self.status);
        w.s(self.player);
        for p in self.pos {
            w.f(p);
        }
        w.f(self.orient);
        w.u(self.layer);
        w.f(self.radius);
        w.u(u32::from(self.drawable));
        w.u(u32::from(self.selected));
        w.u(self.cond);
        w.u(u32::from(self.has_ai));
        w.u(u32::from(self.ai_idle));
        w.u(self.enter_target);
        w.u(u32::from(self.has_phys));
        w.f(self.mass);
        w.u(u32::from(self.can_fall));
        w.u(u32::from(self.has_stealth));
        w.u(u32::from(self.has_body));
        w.f(self.max_health);
        w.u(u32::from(self.booby));
        w.u(u32::from(self.eff_dead));
        w.u(self.weapon_bonus);
        w.u(self.contained_by);
        w.u(u32::from(self.has_contain));
        w.s(self.contain_max);
        w.u(self.enter_snd);
        w.u(self.exit_snd);
        w.u(u32::from(self.allow_fire));
        w.u(u32::from(self.in_turret));
        w.s(self.exit_paths);
        w.f(self.damage_pct);
        w.u(u32::from(self.burned));
        w.u(self.door_open);
        w.u(self.allow_mode);
        w.u(self.allow_mask);
        w.u(self.forbid_mask);
        w.u(u32::from(self.weapon_pass));
        w.u(self.allow_rel);
        w.u(self.death_mask);
        w.u(self.vet_mask);
        w.u(self.exempt_mask);
        w.u(self.required_mask);
        w.u(u32::from(self.adj_has));
        for p in self.adj {
            w.f(p);
        }
        w.u(self.fp_count);
        for j in 0..FP {
            for p in self.fp[j] {
                w.f(p);
            }
        }
        for j in 0..NB {
            w.u(u32::from(self.bone_has[j]));
            for p in self.bone[j] {
                w.f(p);
            }
        }
        for j in 0..FP {
            w.u(u32::from(self.turret_has[j]));
            for p in self.turret[j] {
                w.f(p);
            }
        }
        w.u(self.cls);
        w.s(self.raw_slots);
        w.u(u32::from(self.airborne));
        w.s(self.free_to_exit);
        w.u(u32::from(self.has_loco));
        w.u(u32::from(self.valid_terrain));
        w.f(self.com);
        for p in self.vel {
            w.f(p);
        }
        w.u(self.weapons);
        w.u(u32::from(self.unmanned));
        w.u(self.extra_kinds);
        w.u(u32::from(self.exit_bone_has));
        for p in self.exit_bone {
            w.f(p);
        }
        w.s(self.slot_cap);
        w.f(self.regen);
        w.u(self.exit_delay);
        w.u(self.tflags);
        w.f(self.pitch);
        w.s(self.payload_count);
        w.u(self.payload_name);
        w.u(self.exit_bone_name);
        w.f(self.health);
        w.u(u32::from(self.has_dock));
        w.u(u32::from(self.dock_open));
        w.u(self.helix_payload);
        w.u(self.rider_set);
        w.u(self.scuttle_frames);
        w.u(self.scuttle_cond);
        w.u(u32::from(self.ai_moving));
        w.u(u32::from(self.ov_sink));
    }

    fn read(r: &mut R<'_>) -> Obj {
        let mut o = Obj::default();
        for s in o.snd.iter_mut() {
            *s = r.u();
        }
        o.kinds = r.u();
        o.status = r.u();
        o.player = r.s();
        for p in o.pos.iter_mut() {
            *p = r.f();
        }
        o.orient = r.f();
        o.layer = r.u();
        o.radius = r.f();
        o.drawable = r.u() != 0;
        o.selected = r.u() != 0;
        o.cond = r.u();
        o.has_ai = r.u() != 0;
        o.ai_idle = r.u() != 0;
        o.enter_target = r.u();
        o.has_phys = r.u() != 0;
        o.mass = r.f();
        o.can_fall = r.u() != 0;
        o.has_stealth = r.u() != 0;
        o.has_body = r.u() != 0;
        o.max_health = r.f();
        o.booby = r.u() != 0;
        o.eff_dead = r.u() != 0;
        o.weapon_bonus = r.u();
        o.contained_by = r.u();
        o.has_contain = r.u() != 0;
        o.contain_max = r.s();
        o.enter_snd = r.u();
        o.exit_snd = r.u();
        o.allow_fire = r.u() != 0;
        o.in_turret = r.u() != 0;
        o.exit_paths = r.s();
        o.damage_pct = r.f();
        o.burned = r.u() != 0;
        o.door_open = r.u();
        o.allow_mode = r.u();
        o.allow_mask = r.u();
        o.forbid_mask = r.u();
        o.weapon_pass = r.u() != 0;
        o.allow_rel = r.u();
        o.death_mask = r.u();
        o.vet_mask = r.u();
        o.exempt_mask = r.u();
        o.required_mask = r.u();
        o.adj_has = r.u() != 0;
        for p in o.adj.iter_mut() {
            *p = r.f();
        }
        o.fp_count = r.u();
        for j in 0..FP {
            for p in o.fp[j].iter_mut() {
                *p = r.f();
            }
        }
        for j in 0..NB {
            o.bone_has[j] = r.u() != 0;
            for p in o.bone[j].iter_mut() {
                *p = r.f();
            }
        }
        for j in 0..FP {
            o.turret_has[j] = r.u() != 0;
            for p in o.turret[j].iter_mut() {
                *p = r.f();
            }
        }
        o.cls = r.u();
        o.raw_slots = r.s();
        o.airborne = r.u() != 0;
        o.free_to_exit = r.s();
        o.has_loco = r.u() != 0;
        o.valid_terrain = r.u() != 0;
        o.com = r.f();
        for p in o.vel.iter_mut() {
            *p = r.f();
        }
        o.weapons = r.u();
        o.unmanned = r.u() != 0;
        o.extra_kinds = r.u();
        o.exit_bone_has = r.u() != 0;
        for p in o.exit_bone.iter_mut() {
            *p = r.f();
        }
        o.slot_cap = r.s();
        o.regen = r.f();
        o.exit_delay = r.u();
        o.tflags = r.u();
        o.pitch = r.f();
        o.payload_count = r.s();
        o.payload_name = r.u();
        o.exit_bone_name = r.u();
        o.health = r.f();
        o.has_dock = r.u() != 0;
        o.dock_open = r.u() != 0;
        o.helix_payload = r.u();
        o.rider_set = r.u();
        o.scuttle_frames = r.u();
        o.scuttle_cond = r.u();
        o.ai_moving = r.u() != 0;
        o.ov_sink = r.u() != 0;
        o
    }
}

struct Scene {
    frame: u32,
    logic_seed: u32,
    client_seed: u32,
    objs: Vec<Obj>,
    rel: Vec<Vec<i32>>,
    layer: [i32; 8],
    layer_height: [f32; 8],
}

fn parse_scene(cfg: &[u32]) -> Scene {
    let mut r = R { w: cfg, i: 0 };
    let frame = r.u();
    let (logic_seed, client_seed) = (r.u(), r.u());
    let n = r.u() as usize;
    let objs: Vec<Obj> = (0..n).map(|_| Obj::read(&mut r)).collect();
    let rel = (0..n).map(|_| (0..n).map(|_| r.s()).collect()).collect();
    let mut layer = [0; 8];
    for l in layer.iter_mut() {
        *l = r.s();
    }
    let mut layer_height = [0.0; 8];
    for l in layer_height.iter_mut() {
        *l = r.f();
    }
    Scene { frame, logic_seed, client_seed, objs, rel, layer, layer_height }
}

// ---- the generators ------------------------------------------------------------------------------------------------------------------------------------------------

fn health_str(g: &mut Gen, clean: bool) -> String {
    match g.below(8) {
        0..=5 => format!("{}", *pick(g, &[&1i32, &2, &3, &5, &10, &-1, &0])),
        6 => "0".to_string(),
        _ if clean => "4".to_string(),
        _ => gen_int(g, clean),
    }
}

/// A list of names from `names` (a mask over the local list): at least one, `NONE` sometimes.
fn name_list(g: &mut Gen, names: &[&str], clean: bool) -> String {
    let n = 1 + g.below(3);
    let mut v: Vec<String> = (0..n).map(|_| names[g.below(names.len() as u32) as usize].to_string()).collect();
    if g.below(8) == 0 {
        v.insert(0, "NONE".to_string());
    }
    if !clean && g.below(12) == 0 {
        v.push("NOT_A_NAME".to_string());
    }
    v.join(" ")
}

/// The value of `DeathTypes` or `VeterancyLevels` (`INI::parseDeathTypeFlags`): the flags start as ALL, and each token is `ALL`, `NONE`, `+NAME` (set) or `-NAME`
/// (clear); a bare name is `INI_UNKNOWN_TOKEN`. (`NORMAL` and `REGULAR` are left out: their flag is the shift by -1, undefined upstream; see `death_flags`.)
fn signed_list(g: &mut Gen, names: &[&str], clean: bool) -> String {
    let mut v: Vec<String> = Vec::new();
    match g.below(8) {
        0 => v.push("NONE".to_string()),
        1 => v.push("ALL".to_string()),
        _ => {}
    }
    for _ in 0..1 + g.below(3) {
        let sign = if g.below(2) == 0 { '+' } else { '-' };
        v.push(format!("{sign}{}", names[g.below(names.len() as u32) as usize]));
    }
    if !clean && g.below(12) == 0 {
        v.push(["NOT_A_NAME".to_string(), "+NOT_A_NAME".to_string(), names[0].to_string()][g.below(3) as usize].clone());
    }
    v.join(" ")
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    macro_rules! field {
        ($p:expr, $name:expr, $value:expr) => {
            if g.below(100) < $p {
                let value: String = $value;
                // a field line is `name = value`: upstream reads the `=` as the first token, so a line without it (or with two) throws; only the unclean files have those
                let sep = if clean { *pick(g, &[&" = ", &"=", &" =", &"= ", &" = "]) } else { gen_sep(g) };
                lines.push(format!("{}{}{}", $name, sep, value));
            }
        };
    }
    field!(70, "ContainMax", health_str(g, clean));
    field!(50, "EnterSound", format!("snd{}", 1 + g.below(5)));
    field!(50, "ExitSound", format!("snd{}", 1 + g.below(5)));
    field!(40, "DamagePercentToUnits", format!("{}%", *pick(g, &[&0i32, &25, &50, &100, &150])));
    field!(40, "BurnedDeathToUnits", gen_bool(g, clean));
    field!(50, "AllowInsideKindOf", name_list(g, &KIND_NAMES, clean));
    field!(50, "ForbidInsideKindOf", name_list(g, &KIND_NAMES, clean));
    field!(50, "PassengersAllowedToFire", gen_bool(g, clean));
    field!(40, "PassengersInTurret", gen_bool(g, clean));
    field!(50, "NumberOfExitPaths", format!("{}", *pick(g, &[&0i32, &1, &2, &3, &4, &-1])));
    field!(50, "DoorOpenTime", gen_int(g, clean));
    field!(40, "WeaponBonusPassedToPassengers", gen_bool(g, clean));
    field!(40, "AllowAlliesInside", gen_bool(g, clean));
    field!(40, "AllowEnemiesInside", gen_bool(g, clean));
    field!(40, "AllowNeutralInside", gen_bool(g, clean));
    field!(30, "DeathTypes", signed_list(g, &["EXPLODED", "BURNED", "CRUSHED", "POISONED"], clean));
    field!(25, "VeterancyLevels", signed_list(g, &["VETERAN", "ELITE", "HEROIC"], clean));
    field!(25, "ExemptStatus", name_list(g, &STATUS_NAMES, clean));
    field!(25, "RequiredStatus", name_list(g, &STATUS_NAMES, clean));
    // batch R2: one block in two is a `TransportContain` (`T`), with its own fields after the base's
    // batch R3: a transport block is `T`, `I` (InternetHackContain) or `R` (RailedTransportContain); the fields are the same
    let transport = g.below(2) == 0;
    let token = if transport { ["T\n", "I\n", "R\n", "H\n", "C\n", "O\n"][g.below(6) as usize] } else { "D\n" };
    if transport {
        field!(60, "Slots", health_str(g, clean));
        field!(30, "ScatterNearbyOnExit", gen_bool(g, clean));
        field!(40, "OrientLikeContainerOnExit", gen_bool(g, clean));
        field!(40, "KeepContainerVelocityOnExit", gen_bool(g, clean));
        field!(40, "GoAggressiveOnExit", gen_bool(g, clean));
        field!(30, "ResetMoodCheckTimeOnExit", gen_bool(g, clean));
        field!(40, "DestroyRidersWhoAreNotFreeToExit", gen_bool(g, clean));
        field!(40, "ExitBone", EXIT_BONES[1 + g.below(2) as usize].to_string());
        field!(40, "ExitPitchRate", format!("{}", *pick(g, &[&0i32, &30, &90, &-45])));
        field!(40, "InitialPayload", format!("{} {}", PAYLOADS[1 + g.below(3) as usize], *pick(g, &[&0i32, &1, &2, &3])));
        field!(40, "HealthRegen%PerSec", format!("{}", *pick(g, &[&0i32, &10, &50, &100])));
        field!(40, "ExitDelay", format!("{}", *pick(g, &[&0i32, &33, &100, &1000])));
        field!(40, "ArmedRidersUpgradeMyWeaponSet", gen_bool(g, clean));
        field!(40, "DelayExitInAir", gen_bool(g, clean));
    }
    // batch R4: a Helix's own fields (the payload names are rare: see `helix_payload`)
    if token == "H\n" {
        if g.below(4) == 0 {
            for _ in 0..1 + g.below(2) {
                field!(100, "PayloadTemplateName", PAYLOADS[1 + g.below(3) as usize].to_string());
            }
        }
        field!(30, "ShouldDrawPips", gen_bool(g, clean));
    }
    // batch R6: an Overlord's payload names (rare, as the Helix's) and experience sink
    if token == "O\n" {
        if g.below(4) == 0 {
            for _ in 0..1 + g.below(2) {
                field!(100, "PayloadTemplateName", PAYLOADS[1 + g.below(3) as usize].to_string());
            }
        }
        field!(40, "ExperienceSinkForRider", gen_bool(g, clean));
    }
    // batch R5: a combat bike's riders (`Rider1` .. `Rider8`: template, model condition, weapon set, status, command set, locomotor set) and scuttle
    if token == "C\n" {
        use rts_lockstep_engine::common::model_state::MODEL_CONDITION_NAMES;
        use rts_lockstep_engine::logic::weapon_set_flags::WEAPON_SET_NAMES;
        for i in 0..8 {
            if g.below(3) == 0 {
                let name = ["T1", "T2", "T3", "T4", "Nope"][g.below(5) as usize];
                let cond = MODEL_CONDITION_NAMES[CONDS[g.below(6) as usize] as usize];
                let ws = WEAPON_SET_NAMES[9 + g.below(8) as usize];
                let st = STATUS_NAMES[g.below(3) as usize];
                let loco = rts_lockstep_engine::logic::modules::rider_change_contain::LOCOMOTOR_SET_NAMES[g.below(8) as usize];
                let bad = if !clean && g.below(20) == 0 { "NOT_A_NAME" } else { st };
                field!(100, &format!("Rider{}", i + 1), format!("{name} {cond} {ws} {bad} Cmd{} {loco}", g.below(3)));
            }
        }
        field!(40, "ScuttleDelay", format!("{}", *pick(g, &[&0i32, &33, &100, &1000])));
        field!(40, "ScuttleStatus", MODEL_CONDITION_NAMES[CONDS[g.below(6) as usize] as usize].to_string());
    }
    if !clean && g.below(12) == 0 {
        lines.push(format!("Bogus{}1", gen_sep(g)));
    }
    let mut s = String::from(token);
    for l in &lines {
        s.push_str(gen_indent(g));
        s.push_str(l);
        s.push('\n');
    }
    if clean || g.below(12) != 0 {
        s.push_str("End\n");
    }
    s.into_bytes()
}

fn coord(g: &mut Gen) -> f32 {
    *pick(g, &[&0.0f32, &10.0, &50.5, &-20.0, &100.0, &3.5])
}

fn gen_obj(g: &mut Gen, k: usize, n: usize) -> Obj {
    let mut o = Obj::default();
    for s in o.snd.iter_mut() {
        *s = if g.below(3) == 0 { 0 } else { 1 + g.below(6) };
    }
    o.kinds = g.below(256) & g.below(256) | if g.below(3) == 0 { 1 } else { 0 };
    o.status = g.below(8) & g.below(8);
    // one object in two belongs to player 0: a transport only takes its own player's units, so the riders it holds are often enough multi-slot ones
    o.player = if g.below(40) == 0 { -1 } else if g.below(2) == 0 { 0 } else { g.below(4) as i32 };
    for p in o.pos.iter_mut() {
        *p = coord(g);
    }
    o.orient = *pick(g, &[&0.0f32, &1.5, &3.0, &-1.0]);
    o.layer = g.below(3);
    o.radius = *pick(g, &[&0.0f32, &5.0, &20.0, &10.0]);
    o.drawable = g.below(4) != 0;
    o.selected = g.below(3) == 0;
    o.cond = g.below(64);
    o.has_ai = g.below(10) < 7;
    o.ai_idle = g.below(2) == 0;
    o.enter_target = if g.below(3) == 0 { 0 } else { 1 + g.below(n as u32) };
    o.has_phys = g.below(3) == 0;
    o.mass = *pick(g, &[&1.0f32, &25.0, &100.0, &0.5]);
    o.can_fall = g.below(2) == 0;
    o.has_stealth = g.below(3) == 0;
    o.has_body = g.below(10) < 8;
    o.max_health = *pick(g, &[&100.0f32, &200.0, &50.0, &0.0, &1000.0]);
    o.booby = g.below(12) == 0;
    o.eff_dead = g.below(15) == 0;
    o.weapon_bonus = g.below(8);
    // an object is contained only by an EARLIER one that has a container (so the scene starts without a ring)
    o.contained_by = if k > 0 && g.below(5) == 0 { 1 + g.below(k as u32) } else { 0 };
    o.has_contain = k == 0 || g.below(10) < 6;
    o.contain_max = *pick(g, &[&-1i32, &-1, &1, &2, &3, &0]);
    o.enter_snd = if g.below(3) == 0 { 0 } else { 1 + g.below(5) };
    o.exit_snd = if g.below(3) == 0 { 0 } else { 1 + g.below(5) };
    o.allow_fire = g.below(2) == 0;
    o.in_turret = g.below(3) == 0;
    o.exit_paths = *pick(g, &[&0i32, &1, &1, &2, &3, &12, &12]);
    o.damage_pct = *pick(g, &[&0.0f32, &0.0, &0.25, &0.5, &1.0]);
    o.burned = g.below(2) == 0;
    o.door_open = *pick(g, &[&0u32, &1, &1, &3]);
    o.allow_mode = g.below(2);
    o.allow_mask = g.below(256);
    o.forbid_mask = g.below(256) & g.below(256);
    o.weapon_pass = g.below(2) == 0;
    o.allow_rel = if g.below(3) == 0 { g.below(8) } else { 7 };
    o.death_mask = if g.below(2) == 0 { 0xffff_ffff } else { g.below(32) };
    o.vet_mask = if g.below(2) == 0 { 0xffff_ffff } else { g.below(16) };
    o.exempt_mask = if g.below(4) == 0 { g.below(8) } else { 0 };
    o.required_mask = if g.below(4) == 0 { g.below(8) } else { 0 };
    o.adj_has = g.below(2) == 0;
    for p in o.adj.iter_mut() {
        *p = coord(g);
    }
    o.fp_count = if g.below(3) == 0 { FP as u32 } else { g.below(FP as u32 + 1) };
    for j in 0..FP {
        for p in o.fp[j].iter_mut() {
            *p = coord(g);
        }
    }
    for j in 0..NB {
        o.bone_has[j] = g.below(4) != 0;
        for p in o.bone[j].iter_mut() {
            *p = coord(g);
        }
    }
    for j in 0..FP {
        o.turret_has[j] = g.below(8) != 0;
        for p in o.turret[j].iter_mut() {
            *p = coord(g);
        }
    }
    // 0 OpenContain, 1 TransportContain, 2 InternetHackContain, 3 RailedTransportContain
    o.cls = if o.has_contain && k > 0 { g.below(7) } else { 0 };
    o.raw_slots = *pick(g, &[&0i32, &1, &1, &1, &2, &3]);
    // upstream's airborne locomotor is the AI's
    o.airborne = o.has_ai && g.below(4) == 0;
    o.free_to_exit = *pick(g, &[&0i32, &0, &1, &2]);
    o.has_loco = g.below(4) != 0;
    o.valid_terrain = g.below(4) != 0;
    o.com = *pick(g, &[&0.0f32, &0.5, &-1.0, &2.0]);
    for p in o.vel.iter_mut() {
        *p = coord(g);
    }
    o.weapons = g.below(512);
    o.unmanned = g.below(5) == 0;
    o.extra_kinds = if g.below(3) == 0 { 7 } else { g.below(8) & g.below(8) };
    o.exit_bone_has = g.below(2) == 0;
    for p in o.exit_bone.iter_mut() {
        *p = coord(g);
    }
    o.slot_cap = *pick(g, &[&0i32, &1, &2, &3, &5]);
    o.regen = *pick(g, &[&0.0f32, &0.0, &10.0, &50.0]);
    o.exit_delay = *pick(g, &[&0u32, &0, &1, &5, &100]);
    o.tflags = g.below(128);
    o.pitch = *pick(g, &[&0.0f32, &0.5, &-1.0]);
    o.payload_count = *pick(g, &[&0i32, &0, &0, &1, &2]);
    o.payload_name = g.below(4);
    o.exit_bone_name = g.below(3);
    o.health = if g.below(3) == 0 { o.max_health } else { *pick(g, &[&0.0f32, &20.0, &99.5, &150.0]) };
    o.has_dock = g.below(2) == 0;
    o.dock_open = g.below(2) == 0;
    // mostly no payload: a payload template that exists makes the stub factory's NULL object, which the Helix dereferences (undefined, excluded)
    o.helix_payload = if g.below(4) == 0 { g.below(4) } else { 0 };
    o.rider_set = g.below(4);
    o.scuttle_frames = *pick(g, &[&0u32, &1, &3, &30]);
    o.scuttle_cond = g.below(6);
    o.ai_moving = g.below(2) == 0;
    o.ov_sink = g.below(3) != 0;
    if g.below(4) == 0 {
        o.extra_kinds |= 8;
    }
    o
}

fn gen_scene(g: &mut Gen, _data: &Option<ModuleDataKind>) -> Vec<u32> {
    let mut w = W(Vec::new());
    let n = 2 + g.below(3) as usize;
    w.u(g.below(300));
    w.u(g.u32());
    w.u(g.u32());
    w.u(n as u32);
    for k in 0..n {
        let mut o = gen_obj(g, k, n);
        if k == 0 {
            o.cls = match _data {
                Some(ModuleDataKind::TransportContain(_)) => 1,
                Some(ModuleDataKind::InternetHackContain(_)) => 2,
                Some(ModuleDataKind::RailedTransportContain(_)) => 3,
                Some(ModuleDataKind::HelixContain(_)) => 4,
                Some(ModuleDataKind::RiderChangeContain(_)) => 5,
                Some(ModuleDataKind::OverlordContain(_)) => 6,
                _ => 0,
            };
        }
        o.write(&mut w);
    }
    for i in 0..n {
        for j in 0..n {
            w.s(if i == j { 2 } else { *pick(g, &[&0i32, &1, &2, &2]) });
        }
    }
    for _ in 0..8 {
        w.s(g.below(3) as i32);
    }
    for _ in 0..8 {
        w.f(*pick(g, &[&0.0f32, &5.0, &12.5, &-3.0]));
    }
    w.0
}

/// One op of five words: `[op, container, a, b, c]`.
fn gen_ops(g: &mut Gen, n: u32) -> Vec<[u32; 5]> {
    let count = 1 + g.below(60);
    let mut ops = Vec::new();
    for _ in 0..count {
        let c = g.below(n);
        let a = g.below(n);
        // one op in twelve is a burst: the exit path number and the next fire point only reach the width boundary of their bone names after many calls
        let burst = g.below(24);
        let op = if burst < 2 {
            [47 + burst, c, a, g.below(16), g.below(2)]
        } else if burst < 6 {
            // batch R2: the transport's own (49 to 55)
            [49 + g.below(9), c, a, g.below(10), g.below(10)]
        } else {
            gen_op(g, c, a)
        };
        ops.push(op);
    }
    ops
}

fn gen_op(g: &mut Gen, c: u32, a: u32) -> [u32; 5] {
    {
        match g.below(100) {
            0..=17 => [0, c, a, 0, 0],
            18..=24 => [1, c, a, g.below(2), 0],
            25..=27 => [2, c, g.below(2), 0, 0],
            28..=30 => [3, c, 0, 0, 0],
            31..=33 => [4, c, g.below(8), g.below(5), g.below(6)],
            34..=38 => [5, c, a, 0, 0],
            39..=41 => [6, c, g.below(8), g.below(5), g.below(6)],
            42..=46 => [7, c, 0, 0, 0],
            47..=49 => [8, c, a, g.below(2), 0],
            50..=55 => [9, c, a, 0, 0],
            56..=58 => [10, c, a, 0, 0],
            59..=61 => [11, c, g.below(2), 0, 0],
            62 => [12, c, 0, 0, 0],
            63 => [13, c, 0, 0, 0],
            64 => [14, c, 0, 0, 0],
            65..=67 => [15, c, (*pick(g, &[&0.0f32, &0.25, &0.5, &1.0, &1.5])).to_bits(), 0, 0],
            68..=70 => [16, c, a, g.below(3), 0],
            71 => [17, c, coord(g).to_bits(), coord(g).to_bits(), coord(g).to_bits()],
            72 => [18, c, 0, 0, 0],
            73 => [19, c, g.below(2), 0, 0],
            74..=75 => [20, c, coord(g).to_bits(), coord(g).to_bits(), coord(g).to_bits()],
            76 => [21, c, 0, 0, 0],
            77 => [22, c, 0, 0, 0],
            78 => [23, c, 0, 0, 0],
            79 => [24, c, g.below(2), 0, 0],
            80 => [25, c, a, g.below(2), 0],
            81..=82 => [26, c, a, 0, 0],
            83..=84 => [27, c, a, 0, 0],
            85 => [28, c, g.below(2), 0, 0],
            86 => [29, c, 0, 0, 0],
            87 => [30, c, 0, 0, 0],
            88 => [31, c, 0, 0, 0],
            89 => [32, c, 0, 0, 0],
            90 => [33, c, 0, 0, 0],
            91 => [34, 0, g.below(300), 0, 0],
            92 => [35, c, g.below(2), 0, 0],
            93 => [36, c, g.below(64), 0, 0],
            94 => [37, c, g.below(8), 0, 0],
            95 => [38, c, coord(g).to_bits(), coord(g).to_bits(), coord(g).to_bits()],
            96 => [39, c, g.below(2), 0, 0],
            97 => [40, c, g.below(2), 0, 0],
            98 => [41, c, g.below(4), 0, 0],
            _ => match g.below(4) {
                0 => [42, c, a, g.below(2), 0],
                1 => [43, c, a, 0, 0],
                2 => [44, c, 0, 0, 0],
                _ => match g.below(3) {
                    0 => [46, c, a, g.below(2), 0],
                    _ => [45, c, a, 0, 0],
                },
            },
        }
    }
}

fn gen_case(g: &mut Gen, data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let cfg = gen_scene(g, data);
    let n = cfg[3];
    (cfg, gen_ops(g, n))
}

// ---- the Rust run ---------------------------------------------------------------------------------------------------------------------------------------------------

fn fold(h: u32, x: u32) -> u32 {
    h.wrapping_mul(31).wrapping_add(x)
}

/// The words of a container's state, as the driver's `ctPutObject` writes them: the `state_words` of the module, with the fire points folded.
fn put_object(v: &mut Vec<u8>, w: &mut World, id: ObjectId, has_contain: bool) {
    if has_contain {
        let o = w.objects.find(id).unwrap();
        let idx = o.contain.expect("the container");
        let u = o.modules[idx as usize].update_index.expect("an update module");
        match o.behaviors[u as usize].body.as_ref() {
            Some(UpdateBody::OpenContain(m)) => {
                put_u32(v, 1);
                put_words_state(v, m);
            }
            // a transport: 2, then its state words (the base's, then the three of its own)
            Some(UpdateBody::TransportContain(m)) => {
                put_u32(v, 2);
                for x in m.state_words() {
                    put_u32(v, x);
                }
            }
            // batch R3: the derived transports have no state of their own
            Some(UpdateBody::InternetHackContain(m)) => {
                put_u32(v, 2);
                for x in m.state_words() {
                    put_u32(v, x);
                }
            }
            Some(UpdateBody::RailedTransportContain(m)) => {
                put_u32(v, 2);
                for x in m.state_words() {
                    put_u32(v, x);
                }
            }
            // batch R4: a Helix, 3 (the transport's words and its portable structure)
            // batch R6: an Overlord, 5 (the transport's words and the redirection flag)
            Some(UpdateBody::OverlordContain(m)) => {
                put_u32(v, 5);
                for x in m.state_words() {
                    put_u32(v, x);
                }
            }
            // batch R5: a combat bike, 4 (the transport's words, its own two counters and the scuttle frame)
            Some(UpdateBody::RiderChangeContain(m)) => {
                put_u32(v, 4);
                for x in m.state_words() {
                    put_u32(v, x);
                }
            }
            Some(UpdateBody::HelixContain(m)) => {
                put_u32(v, 3);
                for x in m.state_words() {
                    put_u32(v, x);
                }
            }
            _ => panic!("the container is in its slot"),
        }
    } else {
        put_u32(v, 0);
    }
    let o = w.objects.find(id).unwrap();
    put_u32(v, o.contained_by as u32);
    let pos = o.get_position();
    put_u32(v, pos.x.to_bits());
    put_u32(v, pos.y.to_bits());
    put_u32(v, pos.z.to_bits());
    let (mut w0, mut w1) = (0u32, 0u32);
    for i in 0..OBJECT_STATUS_COUNT.min(64) {
        if o.status.test(i) {
            if i < 32 {
                w0 |= 1 << i;
            } else {
                w1 |= 1 << (i - 32);
            }
        }
    }
    put_u32(v, w0);
    put_u32(v, w1);
    put_u32(v, u32::from(o.is_effectively_dead()));
    let dead = o.ai.and_then(|a| match o.modules[a as usize].body.as_ref() {
        Some(UpdateBody::ModuleProbe(p)) => Some(p.answers.ai_dead),
        _ => None,
    });
    put_u32(v, u32::from(dead.unwrap_or(false)));
    // the physics block's `getAllowToFall()` (2: no physics)
    let fall = o.physics.and_then(|p| match o.modules[p as usize].body.as_ref() {
        Some(UpdateBody::ModuleProbe(p)) => Some(u32::from(p.answers.can_fall)),
        _ => None,
    });
    put_u32(v, fall.unwrap_or(2));
    // the geometry's bounding circle radius, the orientation and the layer the container reads
    put_u32(v, o.geometry_info.get_bounding_circle_radius().to_bits());
    put_u32(v, o.thing.orientation().to_bits());
    put_u32(v, o.layer as u32);
    // the model conditions the client reads give (four words) and whether the object is selected
    let flags = w.reads.drawable_model_condition_flags(id);
    for word in 0..4 {
        let mut m = 0u32;
        for b in 0..32 {
            let i = word * 32 + b;
            if i < MODELCONDITION_COUNT && flags.test(i) {
                m |= 1 << b;
            }
        }
        put_u32(v, m);
    }
    put_u32(v, u32::from(w.reads.local_selection.contains(&id)));
    // batch R4: the weapon bonus conditions (HelixContain sets and clears GARRISONED)
    put_u32(v, w.objects.find(id).unwrap().weapon_bonus_condition);
    // batch R5: the command set override and the veterancy level (RiderChangeContain sets both)
    put_str(v, &w.objects.find(id).unwrap().command_set_string_override);
    put_u32(v, w.objects.find(id).unwrap().experience.level as u32);
}

/// The state words of the module with the 32 fire point matrices folded into one word (the driver folds the same way).
fn put_words_state(v: &mut Vec<u8>, m: &OpenContain) {
    for x in m.state_words() {
        put_u32(v, x);
    }
}

fn run_xfer(w: &mut World, me: ObjectId, idx: usize, how: u32, saved: &[u8]) -> (i32, Vec<u8>, u32) {
    fn status<T>(r: Result<T, rts_lockstep_engine::common::xfer::XferStatus>) -> i32 {
        match r {
            Ok(_) => 0,
            Err(e) => e as i32,
        }
    }
    match how {
        0 => {
            let mut x = XferSave::new();
            let st = match x.open(XFER_FILE.as_bytes()) {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, x.bytes().to_vec(), 0)
        }
        1 => {
            let mut b = saved.to_vec();
            b.truncate(8192);
            let mut x = XferLoad::new(b);
            let st = match x.open(XFER_FILE.as_bytes()) {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), 0)
        }
        _ => {
            let mut x = XferCrc::new_without_nan_guard();
            let st = match x.open(b"crc") {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), x.get_crc())
        }
    }
}

/// The factory's template id of the scene's `T{k+1}` (an object's template is the factory's, as `isEquivalentTo` needs; batch R5).
fn factory_id(names: &BTreeMap<u16, String>, k: usize) -> u16 {
    let want = format!("T{}", k + 1);
    names.iter().find(|(_, n)| **n == want).map(|(id, _)| *id).expect("the factory has the scene's templates")
}

fn hex3(p: &Coord3D) -> String {
    format!("{:08x} {:08x} {:08x}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits())
}

/// The lines of the effect log the trace and the records of one op make, in order.
fn effect_lines(trace: Vec<String>, cuts: Vec<CutCall>, events: Vec<ObjectClientEvent>, tally: &RefCell<Tally>, template_names: &BTreeMap<u16, String>) -> Vec<String> {
    let mut log = Vec::new();
    let (mut cut_iter, mut ev_iter) = (cuts.into_iter(), events.into_iter());
    for t in trace {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::Contain(c)) => {
                    tally.borrow_mut().cuts += 1;
                    match c {
                        ContainCall::PartitionRegisterObject { object } => log.push(format!("pregister {object}")),
                        ContainCall::PartitionUnregisterObject { object } => log.push(format!("punregister {object}")),
                        ContainCall::PartitionGetDistanceSquared { object, position } => log.push(format!("cdist {object} {}", hex3(&position))),
                        ContainCall::DrawableSetHidden { object, hidden } => log.push(format!("hidden {object} {}", i32::from(hidden))),
                        ContainCall::PathfindAddObject { object } => log.push(format!("pathadd {object}")),
                        ContainCall::PathfindRemoveObject { object } => log.push(format!("pathremove {object}")),
                        ContainCall::PathfindRemoveUnit { object } => log.push(format!("pathremoveunit {object}")),
                        ContainCall::PathfindUpdatePos { object, position } => log.push(format!("pathupdatepos {object} {}", hex3(&position))),
                        ContainCall::PathfindUpdateGoal { object, position, layer } => log.push(format!("pathupdategoal {object} {} {layer}", hex3(&position))),
                        ContainCall::PathfindAdjustToPossibleDestination { object, before } => log.push(format!("pathadjust {object} {}", hex3(&before))),
                        ContainCall::TerrainGetLayerForDestination { position } => log.push(format!("destlayer {}", hex3(&position))),
                        ContainCall::LeaveGroup { object } => log.push(format!("leavegroup {object}")),
                        ContainCall::CheckAndDetonateBoobyTrap { container, rider, result } => log.push(format!("booby {container} {rider} {}", i32::from(result))),
                        ContainCall::OnContainedBy { rider, container } => log.push(format!("oncontainedby {rider} {container}")),
                        ContainCall::OnRemovedFrom { rider, container } => log.push(format!("onremovedfrom {rider} {container}")),
                        ContainCall::Kill { object, amount } => {
                            // `Object::kill`: unresistable damage of the body's maximum health, `kill` true, to the body (a line of its own)
                            let mut info = DamageInfo::default();
                            info.input.damage_type = DAMAGE_UNRESISTABLE;
                            info.input.death_type = DEATH_NORMAL;
                            info.input.source_id = INVALID_ID;
                            info.input.amount = amount;
                            info.input.kill = true;
                            log.push(format!("bodydamage {object} {}", info_line(&info)));
                        }
                        ContainCall::AttemptDamage { object, info, has_body } => {
                            if has_body {
                                log.push(format!("bodydamage {object} {}", info_line(&info)));
                            }
                        }
                        ContainCall::SetPosition { .. } => {}
                        ContainCall::SetOrientation { object, angle } => log.push(format!("setorient {object} {:08x}", angle.to_bits())),
                        ContainCall::SetLayer { object, layer } => log.push(format!("setlayer {object} {layer}")),
                        ContainCall::SetTransformMatrix { object, matrix } => {
                            log.push(format!("settransform {object}"));
                            let mut s = format!("xform {object}");
                            for row in matrix.row.iter() {
                                s.push_str(&format!(" {:08x} {:08x} {:08x} {:08x}", row.x.to_bits(), row.y.to_bits(), row.z.to_bits(), row.w.to_bits()));
                            }
                            log.push(s);
                        }
                        ContainCall::DestroyObject { object } => log.push(format!("destroy {object}")),
                        ContainCall::GetSingleLogicalBonePosition { object, name } => log.push(format!("bone1 {object} {}", String::from_utf8_lossy(&name))),
                        ContainCall::GetMultiLogicalBonePosition { object, prefix, max } => log.push(format!("bonen {object} {} {max}", String::from_utf8_lossy(&prefix))),
                        ContainCall::GetSingleLogicalBonePositionOnTurret { object, name } => log.push(format!("boneturret {object} {}", String::from_utf8_lossy(&name))),
                        ContainCall::SetDisabled { object, disabled } => log.push(format!("setdisabled {object} {disabled}")),
                        ContainCall::ClearDisabled { object, disabled } => log.push(format!("cleardisabled {object} {disabled}")),
                        ContainCall::ExperienceSetSink { object, sink } => log.push(format!("xpsink {object} {sink}")),
                        ContainCall::ExperienceSetVeterancyLevel { object, level } => log.push(format!("xplevel {object} {level}")),
                        ContainCall::ExperienceSetExperienceAndLevel { object, experience } => log.push(format!("xpset {object} {experience}")),
                        ContainCall::KillWith { object, amount, damage_type, death_type } => {
                            let mut info = DamageInfo::default();
                            info.input.damage_type = damage_type;
                            info.input.death_type = death_type;
                            info.input.source_id = INVALID_ID;
                            info.input.amount = amount;
                            info.input.kill = true;
                            log.push(format!("bodydamage {object} {}", info_line(&info)));
                        }
                        ContainCall::SetTeam { object, team } => log.push(format!("setteam {object} {team}")),
                        ContainCall::SetDamageState { object, state } => log.push(format!("setdamagestate {object} {state}")),
                        ContainCall::StealthReceiveGrant { object, active, frames } => log.push(format!("stealthgrant {object} {} {frames}", i32::from(active))),
                        ContainCall::NewObject { template, team } => log.push(format!("newobject {} {team}", template_names.get(&template).cloned().unwrap_or_default())),
                    }
                }
                Some(CutCall::WeaponSetUpdate { .. }) => log.push("wset".to_string()),
                Some(CutCall::WeaponTransferNextShotStats { to, from }) => log.push(format!("shotxfer {to} {from}")),
                Some(CutCall::TerrainGetLayerHeight { x, y, layer }) => log.push(format!("layerheight {:08x} {:08x} {layer}", x.to_bits(), y.to_bits())),
                _ => {}
            }
        } else if t.starts_with("evt ") {
            match ev_iter.next() {
                Some(ObjectClientEvent::AudioAddEvent { name, object_id, position, player_index }) => {
                    let pos = match position {
                        Some(p) => format!("{},{},{}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits()),
                        None => "-".to_string(),
                    };
                    log.push(format!("audio {} {pos} {} {object_id}", String::from_utf8_lossy(&name), player_index.unwrap_or(-1)));
                }
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::ClearAndSet { clear, set }, .. }) => log.push(format!("clearset {} {}", clear[0], set[0])),
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Set(a), .. }) => log.push(format!("mc {}", a[0])),
                Some(ObjectClientEvent::DrawableFlashAsSelected { .. }) => log.push("flash".to_string()),
                Some(ObjectClientEvent::DrawableModelCondition { op: ModelConditionOp::Clear(a), .. }) => {
                    // every flag of the mask, in index order (as the driver's `clearModelConditionFlags`)
                    let mut a = a.clone();
                    a.sort();
                    a.dedup();
                    for x in a {
                        log.push(format!("mcclear {x}"));
                    }
                }
                Some(ObjectClientEvent::MessageCreateSelectedGroup { create_new, object_id }) => {
                    log.push("msg create".to_string());
                    log.push(format!("msgbool {}", i32::from(create_new)));
                    log.push(format!("msgobj {object_id}"));
                }
                Some(ObjectClientEvent::MessageRemoveFromSelectedGroup { object_id }) => {
                    log.push("msg remove".to_string());
                    log.push(format!("msgobj {object_id}"));
                }
                Some(ObjectClientEvent::UiSelectDrawable { object_id }) => log.push(format!("uiselect {object_id}")),
                Some(ObjectClientEvent::UiDeselectDrawable { object_id }) => log.push(format!("uideselect {object_id}")),
                Some(ObjectClientEvent::UiSetDisplayedMaxWarning { on }) => log.push(format!("maxwarning {}", i32::from(on))),
                _ => {}
            }
        } else if t.starts_with("heal ") {
            // a transport's heal of a rider (`probe_effect_trace`); the driver's body logs the same line
            log.push(t);
        } else if let Some(line) = t.strip_prefix("bt ") {
            // a probe's line; the idle order carries its command source, which the C++ stand-in does not print
            let tok: Vec<&str> = line.split(' ').collect();
            if tok[0] == "aiidlefrom" {
                log.push(format!("aiidle {}", tok[1]));
            } else if tok[0] == "pfall" {
                // the physics block's flag is part of the object's state: no line of its own
            } else {
                log.push(line.to_string());
            }
        }
    }
    log
}

#[derive(Default)]
struct Tally {
    cuts: u64,
    adds: u64,
    removes: u64,
    exits: u64,
    kills: u64,
    deaths: u64,
    xfers: u64,
    updates: u64,
    collisions: u64,
    effects: u64,
    /// reads of `list::end()` by `processDamageToContained`, compared as NULL
    end_reads: u64,
}

fn veterancy_of(k: usize) -> rts_lockstep_engine::common::game_common::VeterancyLevel {
    use rts_lockstep_engine::common::game_common::VeterancyLevel as V;
    // never REGULAR: `getVeterancyLevelFlag` shifts by -1 for it (undefined upstream, see `death_flags`)
    [V::Veteran, V::Elite, V::Heroic][k % 3]
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let mut v = Vec::new();
    {
        let d: &OpenContainModuleData = match &data {
            ModuleDataKind::OpenContain(d) => d,
            ModuleDataKind::TransportContain(t) | ModuleDataKind::InternetHackContain(t) | ModuleDataKind::RailedTransportContain(t) => &t.open,
            ModuleDataKind::HelixContain(h) => &h.transport.open,
            ModuleDataKind::RiderChangeContain(r) => &r.transport.open,
            ModuleDataKind::OverlordContain(r) => &r.transport.open,
            _ => unreachable!("not a contain"),
        };
        put_u32(&mut v, d.contain_max as u32);
        put_str(&mut v, &d.enter_sound.event_name);
        put_str(&mut v, &d.exit_sound.event_name);
        put_u32(&mut v, u32::from(d.passengers_allowed_to_fire));
        put_u32(&mut v, u32::from(d.passengers_in_turret));
        put_u32(&mut v, d.number_of_exit_paths as u32);
        put_u32(&mut v, d.damage_percentage_to_units.to_bits());
        put_u32(&mut v, u32::from(d.is_burned_death_to_units));
        put_u32(&mut v, d.door_open_time);
        let (mut am, mut fm) = (0u32, 0u32);
        for (i, k) in KINDS.iter().enumerate() {
            if d.allow_inside_kind_of.test(*k) {
                am |= 1 << i;
            }
            if d.forbid_inside_kind_of.test(*k) {
                fm |= 1 << i;
            }
        }
        put_u32(&mut v, am);
        put_u32(&mut v, fm);
        put_u32(&mut v, u32::from(d.weapon_bonus_passed_to_passengers));
        put_u32(&mut v, u32::from(d.allow_allies_inside) | u32::from(d.allow_enemies_inside) << 1 | u32::from(d.allow_neutral_inside) << 2);
        put_u32(&mut v, d.die_mux_data.death_types);
        put_u32(&mut v, d.die_mux_data.veterancy_levels);
        let (mut ex, mut rq) = (0u32, 0u32);
        for (i, st) in STATUSES.iter().enumerate() {
            if d.die_mux_data.exempt_status.test(*st) {
                ex |= 1 << i;
            }
            if d.die_mux_data.required_status.test(*st) {
                rq |= 1 << i;
            }
        }
        put_u32(&mut v, ex);
        put_u32(&mut v, rq);
        // batch R2: 1 and the transport's fields, or 0
        let block_cls = match &data {
            ModuleDataKind::TransportContain(t) => Some((1, &**t)),
            ModuleDataKind::InternetHackContain(t) => Some((2, &**t)),
            ModuleDataKind::RailedTransportContain(t) => Some((3, &**t)),
            ModuleDataKind::HelixContain(h) => Some((4, &h.transport)),
            ModuleDataKind::RiderChangeContain(r) => Some((5, &r.transport)),
            ModuleDataKind::OverlordContain(r) => Some((6, &r.transport)),
            _ => None,
        };
        if let Some((cls, t)) = block_cls {
            put_u32(&mut v, cls);
            put_u32(&mut v, t.slot_capacity as u32);
            put_u32(&mut v, t.exit_pitch_rate.to_bits());
            put_str(&mut v, &t.exit_bone);
            put_str(&mut v, &t.initial_payload.name);
            put_u32(&mut v, t.initial_payload.count as u32);
            put_u32(&mut v, t.health_regen.to_bits());
            put_u32(&mut v, t.exit_delay);
            let b = |x: bool, m: u32| if x { m } else { 0 };
            put_u32(
                &mut v,
                b(t.scatter_nearby_on_exit, 1) | b(t.orient_like_container_on_exit, 2) | b(t.keep_container_velocity_on_exit, 4) | b(t.go_aggressive_on_exit, 8)
                    | b(t.armed_riders_upgrade_weapon_set, 16) | b(t.reset_mood_check_time_on_exit, 32) | b(t.destroy_riders_who_are_not_free_to_exit, 64)
                    | b(t.is_delay_exit_in_air, 128),
            );
            if let ModuleDataKind::HelixContain(h) = &data {
                put_u32(&mut v, h.payload_template_name_data.len() as u32);
                for n in &h.payload_template_name_data {
                    put_str(&mut v, n);
                }
                put_u32(&mut v, u32::from(h.draw_pips));
            }
            if let ModuleDataKind::RiderChangeContain(rd) = &data {
                for r in rd.riders.iter() {
                    put_str(&mut v, &r.template_name);
                    if r.template_name.is_empty() {
                        continue;
                    }
                    put_u32(&mut v, r.model_condition_flag_type as u32);
                    put_u32(&mut v, r.weapon_set_flag as u32);
                    put_u32(&mut v, r.object_status_type as u32);
                    put_str(&mut v, &r.command_set);
                    put_u32(&mut v, r.locomotor_set_type as u32);
                }
                put_u32(&mut v, rd.scuttle_frames);
                put_u32(&mut v, rd.scuttle_state as u32);
            }
            if let ModuleDataKind::OverlordContain(od) = &data {
                put_u32(&mut v, od.payload_template_name_data.len() as u32);
                for n in &od.payload_template_name_data {
                    put_str(&mut v, n);
                }
                put_u32(&mut v, u32::from(od.experience_sink_for_rider));
            }
        } else {
            put_u32(&mut v, 0);
        }
    }
    let sc = parse_scene(cfg);
    let n = sc.objs.len();
    {
        // L9CT_SHOW=<k> prints the scene and the ops of the k-th case that reaches the Rust run (a debugging aid)
        static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let k = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let want = std::env::var("L9CT_SHOW").ok();
        if want.as_deref() == Some("all") || want.and_then(|s| s.parse::<usize>().ok()) == Some(k) {
            eprintln!("SHOW run {k}");
            for (i, o) in sc.objs.iter().enumerate() {
                eprintln!("SHOW obj {i}: {o:?}");
            }
            eprintln!("SHOW ops {ops:?}");
        }
    }
    put_u32(&mut v, n as u32);

    let mut w = World::new();
    w.names = ctx.names;
    // the payload's templates: the factory finds the scene's by name (the test's factory makes nothing; `newobject` is a line of the log)
    let mut factory = rts_lockstep_engine::common::thing_factory::ThingFactory::new();
    let mut template_names = BTreeMap::new();
    for k in 0..n {
        let name = format!("T{}", k + 1);
        factory.new_template(name.as_bytes(), 0).expect("a template");
        let id = factory.find_template(name.as_bytes()).expect("the new template").data.get_template_id();
        template_names.insert(id, name);
    }
    let mut templates = Vec::new();
    for (k, o) in sc.objs.iter().enumerate() {
        let mut kind_of: Vec<i32> = (0..8).filter(|&i| o.kinds >> i & 1 != 0).map(|i| KINDS[i]).collect();
        for (i, kk) in [KINDOF_CLIFF_JUMPER, KINDOF_HERO, KINDOF_SALVAGER, KINDOF_PORTABLE_STRUCTURE].iter().enumerate() {
            if o.extra_kinds >> i & 1 != 0 {
                kind_of.push(*kk);
            }
        }
        let slots = [ThingTemplateAudioType::SoundEnter, ThingTemplateAudioType::SoundExit, ThingTemplateAudioType::SoundFalling];
        let audio = (0..3).filter(|&i| o.snd[i] > 0).map(|i| (slots[i], format!("snd{}", o.snd[i]).into_bytes())).collect();
        let geometry = Some(GeometryInfo::new(GeometryType::Sphere, false, 1.0, o.radius, o.radius));
        templates.push(Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues { name: format!("T{}", k + 1).into_bytes(), kind_of, audio, geometry, transport_slot_count: o.raw_slots as i8, template_id: factory_id(&template_names, k), ..Default::default() })));
    }
    w.statics = Arc::new(rts_lockstep_engine::logic::world::statics::GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, thing_factory: factory, ..Default::default() });
    // the cut points
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.probe_trace = true;
    w.cuts.probe_effect_trace = true;
    w.cuts.contain.stub_object_calls = true;
    // batch R5: the local player is player 0 (as the driver's)
    w.players.set_local_player(Some(0));
    for i in 0..8 {
        w.cuts.terrain_layer[i] = sc.layer[i];
        w.cuts.terrain_layer_height[i] = sc.layer_height[i];
    }
    for i in 0..n {
        for j in 0..n {
            let rel = match sc.rel[i][j] {
                0 => Relationship::Enemies,
                1 => Relationship::Neutral,
                _ => Relationship::Allies,
            };
            w.cuts.team_relationships.insert((i as u32 + 1, j as u32 + 1), rel);
        }
    }
    for (k, o) in sc.objs.iter().enumerate() {
        if o.player >= 0 {
            w.cuts.team_players.insert(k as u32 + 1, o.player);
        }
    }
    w.set_frame(sc.frame);
    // the scene's tables
    let apply_tables = |w: &mut World, k: usize, o: &Obj| {
        let id = k as i32 + 1;
        w.cuts.contain.booby_trapped.remove(&id);
        if o.booby {
            w.cuts.contain.booby_trapped.insert(id);
        }
        if o.adj_has {
            w.cuts.contain.adjusted_destination.insert(id, Coord3D { x: o.adj[0], y: o.adj[1], z: o.adj[2] });
        }
        let mut fps = Vec::new();
        for j in 0..(o.fp_count as usize).min(FP) {
            let mut m = Matrix3D::identity();
            m.set_translation(&Vector3::new(o.fp[j][0], o.fp[j][1], o.fp[j][2]));
            fps.push(m);
        }
        if !fps.is_empty() {
            w.cuts.contain.fire_points.insert(id, fps);
        }
        for (j, bone) in BONES.iter().enumerate() {
            if o.bone_has[j] {
                w.cuts.contain.single_bones.insert((id, bone.as_bytes().to_vec()), Coord3D { x: o.bone[j][0], y: o.bone[j][1], z: o.bone[j][2] });
            }
        }
        for j in 0..FP {
            if o.turret_has[j] {
                let mut m = Matrix3D::identity();
                m.set_translation(&Vector3::new(o.turret[j][0], o.turret[j][1], o.turret[j][2]));
                w.cuts.contain.turret_bones.insert((id, format!("FIREPOINT{:02}", j + 1).into_bytes()), m);
            }
        }
    };
    // the objects
    for (k, o) in sc.objs.iter().enumerate() {
        let mut modules = Vec::new();
        if o.has_body || o.has_ai || o.has_stealth {
            let mut a = ProbeAnswers { idle: o.ai_idle, enter_target: o.enter_target as i32, ..Default::default() };
            a.can_fall = o.can_fall;
            a.free_to_exit = o.free_to_exit;
            a.moving = o.ai_moving;
            a.has_locomotor = o.has_loco;
            let mut pm = NewModule::plain(b"CtProbe", b"ModuleTag_Probe");
            pm.interfaces = if o.has_body { MODULEINTERFACE_BODY } else { 0 };
            pm.is_ai = o.has_ai;
            pm.is_stealth = o.has_stealth;
            let mut probe = ProbeModule::new(a, if o.has_body { o.max_health } else { 0.0 }, Default::default());
            probe.health = if o.has_body { o.health } else { 0.0 };
            pm.body = Some(UpdateBody::ModuleProbe(probe));
            modules.push(pm);
        }
        if o.has_phys {
            let a = ProbeAnswers { mass: o.mass, can_fall: o.can_fall, center_of_mass_offset: o.com, velocity: Coord3D { x: o.vel[0], y: o.vel[1], z: o.vel[2] }, ..Default::default() };
            let mut pm = NewModule::plain(b"PhysicsBehavior", b"ModuleTag_Physics");
            pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(a, 0.0, Default::default())));
            modules.push(pm);
        }
        if o.has_contain {
            let cd = if k == 0 {
                data.clone()
            } else {
                // a transport's base starts from `TransportContainModuleData()` (infantry allowed inside)
                let base = if o.cls != 0 { TransportContainModuleData::default().open } else { OpenContainModuleData::default() };
                let mut d = OpenContainModuleData { contain_max: o.contain_max, passengers_allowed_to_fire: o.allow_fire, passengers_in_turret: o.in_turret, number_of_exit_paths: o.exit_paths, damage_percentage_to_units: o.damage_pct, is_burned_death_to_units: o.burned, door_open_time: o.door_open, weapon_bonus_passed_to_passengers: o.weapon_pass, allow_allies_inside: o.allow_rel & 1 != 0, allow_enemies_inside: o.allow_rel & 2 != 0, allow_neutral_inside: o.allow_rel & 4 != 0, ..base };
                if o.enter_snd > 0 {
                    d.enter_sound.event_name = format!("snd{}", o.enter_snd).into_bytes();
                }
                if o.exit_snd > 0 {
                    d.exit_sound.event_name = format!("snd{}", o.exit_snd).into_bytes();
                }
                if o.allow_mode != 0 {
                    d.allow_inside_kind_of = KindOfMaskType::new();
                    for (i, &k) in KINDS.iter().enumerate() {
                        if o.allow_mask >> i & 1 != 0 {
                            d.allow_inside_kind_of.set(k, 1);
                        }
                    }
                }
                for (i, &k) in KINDS.iter().enumerate() {
                    if o.forbid_mask >> i & 1 != 0 {
                        d.forbid_inside_kind_of.set(k, 1);
                    }
                }
                d.die_mux_data.death_types = o.death_mask;
                d.die_mux_data.veterancy_levels = o.vet_mask;
                for (i, &st) in STATUSES.iter().enumerate() {
                    if o.exempt_mask >> i & 1 != 0 {
                        d.die_mux_data.exempt_status.set(st, 1);
                    }
                    if o.required_mask >> i & 1 != 0 {
                        d.die_mux_data.required_status.set(STATUSES[i], 1);
                    }
                }
                if o.cls != 0 {
                    let t = TransportContainModuleData {
                        open: d,
                        slot_capacity: o.slot_cap,
                        health_regen: o.regen,
                        exit_delay: o.exit_delay,
                        orient_like_container_on_exit: o.tflags & 1 != 0,
                        keep_container_velocity_on_exit: o.tflags & 2 != 0,
                        go_aggressive_on_exit: o.tflags & 4 != 0,
                        armed_riders_upgrade_weapon_set: o.tflags & 8 != 0,
                        reset_mood_check_time_on_exit: o.tflags & 16 != 0,
                        destroy_riders_who_are_not_free_to_exit: o.tflags & 32 != 0,
                        is_delay_exit_in_air: o.tflags & 64 != 0,
                        exit_pitch_rate: o.pitch,
                        initial_payload: InitialPayload { name: PAYLOADS[o.payload_name as usize % 4].as_bytes().to_vec(), count: o.payload_count },
                        exit_bone: EXIT_BONES[o.exit_bone_name as usize % 3].as_bytes().to_vec(),
                        ..Default::default()
                    };
                    match o.cls {
                        2 => ModuleDataKind::InternetHackContain(Arc::new(t)),
                        3 => ModuleDataKind::RailedTransportContain(Arc::new(t)),
                        6 => ModuleDataKind::OverlordContain(Arc::new(rts_lockstep_engine::logic::modules::overlord_contain::OverlordContainModuleData {
                            transport: t,
                            payload_template_name_data: HELIX_PAYLOADS[o.helix_payload as usize % 4].iter().map(|n| n.as_bytes().to_vec()).collect(),
                            experience_sink_for_rider: o.ov_sink,
                        })),
                        5 => {
                            let mut rd = rts_lockstep_engine::logic::modules::rider_change_contain::RiderChangeContainModuleData { transport: t, ..Default::default() };
                            for (j, e) in RIDER_SETS[o.rider_set as usize % 4].iter().enumerate() {
                                if e[0] == 0 {
                                    continue;
                                }
                                let r = &mut rd.riders[j];
                                r.template_name = RIDER_TEMPLATES[e[0] as usize].as_bytes().to_vec();
                                r.model_condition_flag_type = CONDS[e[1] as usize];
                                r.weapon_set_flag = e[2] as i32;
                                r.object_status_type = STATUSES[e[3] as usize];
                                r.command_set = if e[4] != 0 { b"CmdB".to_vec() } else { b"CmdA".to_vec() };
                                r.locomotor_set_type = e[5] as i32;
                            }
                            rd.scuttle_frames = o.scuttle_frames;
                            rd.scuttle_state = CONDS[o.scuttle_cond as usize % 6];
                            ModuleDataKind::RiderChangeContain(Arc::new(rd))
                        }
                        4 => ModuleDataKind::HelixContain(Arc::new(rts_lockstep_engine::logic::modules::helix_contain::HelixContainModuleData {
                            transport: t,
                            payload_template_name_data: HELIX_PAYLOADS[o.helix_payload as usize % 4].iter().map(|n| n.as_bytes().to_vec()).collect(),
                            draw_pips: true,
                        })),
                        _ => ModuleDataKind::TransportContain(Arc::new(t)),
                    }
                } else {
                    ModuleDataKind::OpenContain(Arc::new(d))
                }
            };
            let class: &[u8] = match cd {
                ModuleDataKind::TransportContain(_) => b"TransportContain",
                ModuleDataKind::InternetHackContain(_) => b"InternetHackContain",
                ModuleDataKind::RailedTransportContain(_) => b"RailedTransportContain",
                ModuleDataKind::HelixContain(_) => b"HelixContain",
                ModuleDataKind::RiderChangeContain(_) => b"RiderChangeContain",
                ModuleDataKind::OverlordContain(_) => b"OverlordContain",
                _ => b"OpenContain",
            };
            let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), class, b"ModuleTag_Contain", &cd).unwrap().unwrap();
            modules.push(m);
        }
        // batch R3: the dock, a probe after the contain (as the driver's `CtDock`)
        if o.has_dock {
            let a = ProbeAnswers { has: 1 << 2, dock_open: o.dock_open, ..Default::default() };
            let mut pm = NewModule::plain(b"CtDock", b"ModuleTag_Dock");
            pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(a, 0.0, Default::default())));
            modules.push(pm);
        }
        let mut status = Default::default();
        for (b, st) in STATUSES.iter().enumerate() {
            if o.status >> b & 1 != 0 {
                rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType::set(&mut status, *st, 1);
            }
        }
        let id = w.new_object(NewObject { template: templates[k].clone(), status, team: Some(k as u32 + 1), modules }).unwrap();
        assert_eq!(id, k as i32 + 1);
        w.contain_set_position(id, &Coord3D { x: o.pos[0], y: o.pos[1], z: o.pos[2] });
        w.set_orientation(id, o.orient);
        w.friend_bind_to_drawable(id, if o.drawable { 7 + k as i32 } else { INVALID_DRAWABLE_ID });
        let ob = w.objects.find_mut(id).unwrap();
        ob.experience.level = veterancy_of(k);
        ob.layer = o.layer as i32;
        ob.weapon_bonus_condition = o.weapon_bonus;
        ob.contained_by = INVALID_ID;
        if o.unmanned {
            ob.disabled.set(rts_lockstep_engine::common::disabled_types::DISABLED_UNMANNED, 1);
        }
        apply_tables(&mut w, k, o);
        // batch R2: the transport's tables
        if o.airborne {
            w.cuts.ai_airborne.insert(id);
        }
        if o.valid_terrain && o.has_loco {
            w.cuts.contain.valid_movement_terrain.insert(id);
        }
        if o.exit_bone_has {
            w.cuts.contain.exit_bones.insert((id, b"ExitBoneT".to_vec()), Coord3D { x: o.exit_bone[0], y: o.exit_bone[1], z: o.exit_bone[2] });
        }
        for s in 0..3 {
            if o.weapons >> (3 * s) & 1 != 0 {
                w.cuts.weapon_slot_facts.insert((id, s), WeaponSlotFacts { is_contact_weapon: o.weapons >> (3 * s + 1) & 1 != 0, is_damage_weapon: o.weapons >> (3 * s + 2) & 1 != 0 });
            }
        }
        if o.selected && o.drawable {
            w.reads.local_selection.push(id);
        }
        let mut flags = ModelConditionFlags::new();
        for (i, &c) in CONDS.iter().enumerate() {
            if o.cond >> i & 1 != 0 {
                flags.set(c, 1);
            }
        }
        w.reads.drawable_model_condition.insert(id, flags);
    }
    // the objects that start contained: their container lists them
    // the random generators start after the objects are made (the object creation may draw)
    w.random.init_random(sc.client_seed);
    w.random.init_game_logic_random(sc.logic_seed);
    let ids: Vec<ObjectId> = (1..=n as i32).collect();
    for (k, o) in sc.objs.iter().enumerate() {
        w.objects.find_mut(k as i32 + 1).unwrap().contained_by = o.contained_by as i32;
        w.set_effectively_dead(k as i32 + 1, o.eff_dead);
        if o.contained_by != 0 {
            let cont = o.contained_by as i32;
            if let Some(c) = w.get_contain(cont) {
                w.module_call(cont, c, ModuleCall::AddToContainList(k as i32 + 1));
            }
        }
    }
    let _ = &ids;
    // batch R4: undefined in the setup (a Helix's payload at `onObjectCreated`: the stub factory's NULL object): never sent to the C++
    if w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined() && !e.is_end_iterator_dereferenced()) {
        return RunOut { dump: v, keys: Vec::new(), excluded: true };
    }
    let mut saved: Vec<u8> = Vec::new();
    let mut excluded = false;
    let mut end_reads = 0u64;
    for phase in 0..=ops.len() {
        let mut events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let errors_before = w.cuts.module_errors.len();
        let (mut xst, mut xbytes, mut crc, mut result) = (-2i32, Vec::new(), 0u32, 0u32);
        let mut load_status = 0;
        if phase > 0 {
            let o = &ops[phase - 1];
            let id = (o[1] % n as u32) as i32 + 1;
            let rider = (o[2] % n as u32) as i32 + 1;
            let m = w.get_contain(id);
            let mut info = DamageInfo::default();
            if o[0] == 4 || o[0] == 6 {
                info.input.source_id = (o[3] % 5) as i32;
                info.input.damage_type = (o[2] % 8) as i32;
                info.input.death_type = (o[4] % 5 + 1) as i32;
                info.input.amount = 10.0;
            }
            let fold3 = |p: &Coord3D| p.x.to_bits().wrapping_mul(31 * 31).wrapping_add(p.y.to_bits().wrapping_mul(31)).wrapping_add(p.z.to_bits());
            if let Some(m) = m.filter(|_| o[0] < 34 || (42..=45).contains(&o[0]) || (47..=57).contains(&o[0])) {
                match o[0] {
                    0 => {
                        w.module_call(id, m, ModuleCall::AddToContain(rider));
                        tally.borrow_mut().adds += 1;
                    }
                    1 => {
                        w.module_call(id, m, ModuleCall::RemoveFromContainExposing { obj: rider, expose_stealth_units: o[3] != 0 });
                        tally.borrow_mut().removes += 1;
                    }
                    2 => {
                        w.module_call(id, m, ModuleCall::RemoveAllContained { expose_stealth_units: o[2] != 0 });
                    }
                    3 => {
                        w.module_call(id, m, ModuleCall::KillAllContained);
                        tally.borrow_mut().kills += 1;
                    }
                    4 => {
                        if let ModuleReply::Damage(d) = w.module_call(id, m, ModuleCall::HarmAndForceExitAllContained(info)) {
                            result = d.input.amount.to_bits();
                        }
                    }
                    5 => {
                        w.module_call(id, m, ModuleCall::OnCollide { other: rider, loc: None, normal: None });
                        tally.borrow_mut().collisions += 1;
                    }
                    6 => {
                        // whether the die mux of the container's data takes this death (the module's own check, asked again)
                        {
                            let ob = w.objects.find(id).unwrap();
                            let u = ob.modules[m as usize].update_index.expect("an update module");
                            let mux = match ob.behaviors[u as usize].body.as_ref() {
                                Some(UpdateBody::OpenContain(c)) => Some(c.module_data().die_mux_data),
                                Some(UpdateBody::TransportContain(c)) => Some(c.module_data().open.die_mux_data),
                                Some(UpdateBody::InternetHackContain(c)) => Some(c.module_data().open.die_mux_data),
                                Some(UpdateBody::RailedTransportContain(c)) => Some(c.module_data().open.die_mux_data),
                                Some(UpdateBody::HelixContain(c)) => Some(c.module_data().transport.open.die_mux_data),
                                Some(UpdateBody::RiderChangeContain(c)) => Some(c.module_data().transport.open.die_mux_data),
                                Some(UpdateBody::OverlordContain(c)) => Some(c.module_data().transport.open.die_mux_data),
                                _ => None,
                            };
                            if let Some(mux) = mux {
                                result = u32::from(mux.is_die_applicable(info.input.death_type, ob.get_veterancy_level() as i32, &ob.status));
                            }
                        }
                        w.module_call(id, m, ModuleCall::OnDie(info));
                        tally.borrow_mut().deaths += 1;
                    }
                    7 => {
                        let u = w.update_handle_of_module(ModuleHandle { object: id, index: m }).expect("an update module");
                        result = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                        tally.borrow_mut().updates += 1;
                    }
                    8 => result = u32::from(w.module_call(id, m, ModuleCall::IsValidContainerFor { obj: rider, check_capacity: o[3] != 0 }).as_bool()),
                    9 => {
                        w.module_call(id, m, ModuleCall::ExitObjectViaDoor { obj: rider, door: 0 });
                        tally.borrow_mut().exits += 1;
                    }
                    10 => {
                        w.module_call(id, m, ModuleCall::ExitObjectInAHurry(rider));
                        tally.borrow_mut().exits += 1;
                    }
                    11 => {
                        w.module_call(id, m, ModuleCall::OrderAllPassengersToExit { command_source: 2, instantly: o[2] != 0 });
                    }
                    12 => {
                        w.module_call(id, m, ModuleCall::OrderAllPassengersToIdleFrom { command_source: 2 });
                    }
                    13 => {
                        w.module_call(id, m, ModuleCall::OrderAllPassengersToHackInternetFrom { command_source: 2 });
                    }
                    14 => {
                        w.module_call(id, m, ModuleCall::MarkAllPassengersDetected);
                    }
                    15 => {
                        w.module_call(id, m, ModuleCall::ProcessDamageToContained(f(o[2])));
                    }
                    16 => {
                        w.module_call(id, m, ModuleCall::OnObjectWantsToEnterOrExit { obj: rider, wants: (o[3] % 3) as i32 });
                    }
                    17 => {
                        w.module_call(id, m, ModuleCall::SetRallyPoint(Coord3D { x: f(o[2]), y: f(o[3]), z: f(o[4]) }));
                    }
                    18 => {
                        result = match w.module_call(id, m, ModuleCall::GetRallyPoint) {
                            ModuleReply::Coord(p) => fold3(&p),
                            _ => 0xdead_0000,
                        };
                    }
                    19 => {
                        if let ModuleReply::Coord(p) = w.module_call(id, m, ModuleCall::GetNaturalRallyPoint) {
                            result = fold3(&p);
                        }
                    }
                    20 => result = w.module_call(id, m, ModuleCall::GetClosestRider(Coord3D { x: f(o[2]), y: f(o[3]), z: f(o[4]) })).as_id() as u32,
                    21 => result = u32::from(w.module_call(id, m, ModuleCall::IsAnyRiderAttacking).as_bool()),
                    22 => {
                        w.module_call(id, m, ModuleCall::ContainReactToTransformChange);
                    }
                    23 => {
                        w.module_call(id, m, ModuleCall::OnSelling);
                    }
                    24 => {
                        w.module_call(id, m, ModuleCall::SetPassengerAllowedToFire(o[2] != 0));
                    }
                    25 => result = u32::from(w.module_call(id, m, ModuleCall::IsPassengerAllowedToFire(if o[3] & 1 != 0 { rider } else { INVALID_ID })).as_bool()),
                    26 => {
                        let int = |r: ModuleReply| if let ModuleReply::Int(i) = r { i as u32 } else { 0xdead_0001 };
                        let boo = |r: ModuleReply| u32::from(r.as_bool());
                        let mut h = w.module_call(id, m, ModuleCall::GetContainedItemsMass).as_real(f32::NAN).to_bits();
                        h = fold(h, int(w.module_call(id, m, ModuleCall::GetStealthUnitsContained)));
                        h = fold(h, int(w.module_call(id, m, ModuleCall::GetPlayerWhoEntered)));
                        h = fold(h, int(w.module_call(id, m, ModuleCall::GetContainCount)));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::IsWeaponBonusPassedToPassengers)));
                        h = fold(h, int(w.module_call(id, m, ModuleCall::GetWeaponBonusPassedToPassengers)));
                        h = fold(h, int(w.module_call(id, m, ModuleCall::GetContainMax)));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::IsEnclosingContainerFor(rider))));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::IsDisplayedOnControlBar)));
                        h = fold(h, int(w.module_call(id, m, ModuleCall::GetExtraSlotsInUse)));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::IsKickOutOnCapture)));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::HasObjectsWantingToEnterOrExit)));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::IsExitBusy)));
                        h = fold(h, boo(w.module_call(id, m, ModuleCall::IsImmuneToClearBuildingAttacks)));
                        result = h;
                    }
                    27 => {
                        w.module_call(id, m, ModuleCall::ScatterToNearbyPosition(rider));
                    }
                    28 => {
                        w.module_call(id, m, ModuleCall::EnableLoadSounds(o[2] != 0));
                    }
                    29 => {
                        w.module_call(id, m, ModuleCall::OnDelete);
                    }
                    30 => {
                        let (st, bytes, _) = run_xfer(&mut w, id, m as usize, 0, &[]);
                        xst = st;
                        saved = bytes.clone();
                        xbytes = bytes;
                        tally.borrow_mut().xfers += 1;
                    }
                    31 => {
                        let (st, bytes, _) = run_xfer(&mut w, id, m as usize, 1, &saved);
                        xst = st;
                        xbytes = bytes;
                        tally.borrow_mut().xfers += 1;
                        // the objects a load found in a list that was not empty are destroyed in the middle of the xfer upstream
                        w.module_call(id, m, ModuleCall::DrainXferDestroys);
                    }
                    32 => {
                        let (st, bytes, c) = run_xfer(&mut w, id, m as usize, 2, &[]);
                        xst = st;
                        xbytes = bytes;
                        crc = c;
                        tally.borrow_mut().xfers += 1;
                    }
                    33 => {
                        let before = w.cuts.module_errors.len();
                        w.module_call(id, m, ModuleCall::LoadPostProcess);
                        // `throw SC_INVALID_DATA` is not upstream-undefined
                        let thrown = w.cuts.module_errors[before..].iter().any(|(_, e)| !e.is_undefined());
                        if thrown {
                            load_status = rts_lockstep_engine::common::xfer::XferStatus::ListNotEmpty as i32;
                            w.cuts.module_errors.truncate(before);
                        }
                        xst = load_status;
                    }
                    42 => {
                        w.module_call(id, m, ModuleCall::OnContaining { obj: rider, was_selected: o[3] != 0 });
                    }
                    43 => {
                        w.module_call(id, m, ModuleCall::OnRemoving(rider));
                    }
                    44 => {
                        w.module_call(id, m, ModuleCall::RedeployOccupants);
                    }
                    45 => {
                        w.module_call(id, m, ModuleCall::PutObjAtNextFirePoint(rider));
                    }
                    // a burst of exits through the door (of the quick kind when the fifth word is odd): the exit path number runs up to the width boundary of its bone names
                    47 => {
                        for _ in 0..1 + o[3] % 12 {
                            w.module_call(id, m, if o[4] & 1 != 0 { ModuleCall::ExitObjectInAHurry(rider) } else { ModuleCall::ExitObjectViaDoor { obj: rider, door: 0 } });
                            tally.borrow_mut().exits += 1;
                        }
                    }
                    // a burst of fire point placements: the next fire point runs up to the width boundary of the turret bone names
                    48 => {
                        for _ in 0..1 + o[3] % 12 {
                            w.module_call(id, m, ModuleCall::PutObjAtNextFirePoint(rider));
                        }
                    }
                    // batch R2: the transport's own (the specific rider is NULL when the fourth word is odd)
                    49 => result = w.module_call(id, m, ModuleCall::ReserveDoorForExit { specific: if o[3] & 1 != 0 { INVALID_ID } else { rider } }).as_int(-1) as u32,
                    50 => result = u32::from(w.module_call(id, m, ModuleCall::IsExitBusy).as_bool()),
                    51 => result = w.module_call(id, m, ModuleCall::GetContainMax).as_int(-1) as u32,
                    52 => result = w.module_call(id, m, ModuleCall::GetExtraSlotsInUse).as_int(-1) as u32,
                    53 => {
                        let who = |x: u32| if x % 5 < 4 { (x % 5) as i32 } else { -1 };
                        w.module_call(id, m, ModuleCall::OnCapture { old_owner: who(o[3]), new_owner: who(o[4]) });
                    }
                    54 => {
                        w.module_call(id, m, ModuleCall::KillRidersWhoAreNotFreeToExit);
                    }
                    55 => {
                        if sc.objs[id as usize - 1].cls != 0 {
                            result = u32::from(w.module_call(id, m, ModuleCall::IsSpecificRiderFreeToExit { obj: if o[3] & 1 != 0 { INVALID_ID } else { rider } }).as_bool());
                        }
                    }
                    // batch R4
                    56 => {
                        w.module_call(id, m, ModuleCall::OnBodyDamageStateChange { info: DamageInfo::default(), old_state: (o[3] % 4) as i32, new_state: (o[4] % 4) as i32 });
                    }
                    57 => {
                        w.module_call(id, m, ModuleCall::ClientVisibleContainedFlashAsSelected);
                    }
                    _ => {}
                }
            }
            // the environment
            match o[0] {
                34 => w.set_frame(o[2]),
                35 => {
                    let ob = w.objects.find(id).unwrap();
                    let has_draw = ob.drawable != INVALID_DRAWABLE_ID;
                    w.reads.local_selection.retain(|&x| x != id);
                    if o[2] != 0 && has_draw {
                        w.reads.local_selection.push(id);
                    }
                }
                36 => {
                    let mut flags = ModelConditionFlags::new();
                    for (i, &c) in CONDS.iter().enumerate() {
                        if o[2] >> i & 1 != 0 {
                            flags.set(c, 1);
                        }
                    }
                    w.reads.drawable_model_condition.insert(id, flags);
                }
                37 => {
                    let ob = w.objects.find_mut(id).unwrap();
                    ob.status = Default::default();
                    for (b, st) in STATUSES.iter().enumerate() {
                        if o[2] >> b & 1 != 0 {
                            ob.status.set(*st, 1);
                        }
                    }
                }
                38 => w.contain_set_position(id, &Coord3D { x: f(o[2]), y: f(o[3]), z: f(o[4]) }),
                39 => w.set_effectively_dead(id, o[2] != 0),
                40 => {
                    if o[2] != 0 {
                        w.cuts.contain.booby_trapped.insert(id);
                    } else {
                        w.cuts.contain.booby_trapped.remove(&id);
                    }
                }
                // the container of an object set apart from the lists (a rider that is listed but says it is elsewhere, or says it is here and is not listed)
                46 => {
                    w.objects.find_mut(id).unwrap().contained_by = if o[3] & 1 != 0 || rider == id { INVALID_ID } else { rider };
                }
                41 => {
                    let ob = w.objects.find_mut(id).unwrap();
                    if let Some(ai) = ob.ai {
                        if let Some(UpdateBody::ModuleProbe(p)) = ob.modules[ai as usize].body.as_mut() {
                            p.answers.idle = o[2] & 1 != 0;
                            p.answers.enter_target = if o[2] & 2 != 0 { rider } else { 0 };
                        }
                    }
                }
                _ => {}
            }
        }
        let new_errors = w.cuts.module_errors[errors_before..].to_vec();
        // the end-iterator read of `processDamageToContained` is compared as the shipped game reads it (NULL: the loop ends), the C++ edited to say so
        end_reads += new_errors.iter().filter(|(_, e)| e.is_end_iterator_dereferenced()).count() as u64;
        if new_errors.iter().any(|(_, e)| e.is_undefined() && !e.is_end_iterator_dereferenced()) {
            excluded = true;
            break;
        }
        // an `ErrorCode` thrown out of the op (`TransportContain::createPayload`'s `newObject(NULL)`): CATCH_ALL's status 3, the result never assigned
        let mut op_status = 0;
        if new_errors.iter().any(|(_, e)| matches!(e.thrown(), Some(rts_lockstep_engine::common::ini::Thrown::ErrorCode(_)))) {
            op_status = 3;
            result = 0;
        }
        // the effects of the op, in order
        let trace = w.cuts.trace.take().unwrap_or_default();
        let cuts = w.cuts.log.take().unwrap_or_default();
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        events_before = w.events.events().len();
        let _ = events_before;
        let log = effect_lines(trace, cuts, events, tally, &template_names);
        tally.borrow_mut().effects += log.len() as u64;
        put_i32(&mut v, op_status);
        put_u32(&mut v, result);
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for b in &xbytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
        for k in 0..n {
            put_object(&mut v, &mut w, k as i32 + 1, sc.objs[k].has_contain);
        }
    }
    tally.borrow_mut().end_reads += end_reads;
    RunOut { dump: v, keys: Vec::new(), excluded }
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let max = r.u32() as i32;
    let (en, ex) = (r.str(), r.str());
    let data: Vec<u32> = (0..14).map(|_| r.u32()).collect();
    out.push(format!("data {max} {en} {ex} {data:08x?}"));
    let cls = r.u32();
    if cls != 0 {
        let (cap, pitch) = (r.u32(), r.u32());
        let (bone, payload) = (r.str(), r.str());
        let rest: Vec<u32> = (0..4).map(|_| r.u32()).collect();
        out.push(format!("transport data {cap} {pitch:08x} {bone} {payload} {rest:08x?}"));
        if cls == 4 {
            let names: Vec<String> = (0..r.u32()).map(|_| r.str()).collect();
            out.push(format!("helix data {names:?} pips {}", r.u32()));
        }
        if cls == 6 {
            let names: Vec<String> = (0..r.u32()).map(|_| r.str()).collect();
            out.push(format!("overlord data {names:?} sink {}", r.u32()));
        }
        if cls == 5 {
            let mut riders = Vec::new();
            for _ in 0..8 {
                let name = r.str();
                if name.is_empty() {
                    continue;
                }
                let (c, w, st) = (r.u32(), r.u32(), r.u32());
                let cmd = r.str();
                riders.push(format!("{name} {c} {w} {st} {cmd} {}", r.u32()));
            }
            out.push(format!("rider data {riders:?} scuttle {} {}", r.u32(), r.u32()));
        }
    }
    let n_obj = r.u32();
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let st = r.u32() as i32;
        let result = r.u32();
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        // the scheduling half's frame-and-phase word is compared too: both sides register the containers as `GameLogic::registerObject` does
        let xfer = {
            let xst = r.u32() as i32;
            let blen = r.u32();
            let bytes: Vec<u32> = (0..blen).map(|_| r.u32()).collect();
            let crc = r.u32();
            format!("xfer {xst} bytes {bytes:?} crc {crc:#x}")
        };
        out.push(format!("op {i}: status {st} result {result:08x} log {log:?} {xfer}"));
        for k in 0..n_obj {
            let has = r.u32();
            let mut state: Vec<u32> = Vec::new();
            if has != 0 {
                let size = r.u32();
                let len = r.u32();
                state.push(size);
                state.push(len);
                for _ in 0..len {
                    state.push(r.u32());
                }
                // the entered mask, the two sound frames, the stealth count, the door countdown, the four condition words, the fire point hash, the three indexes,
                // the art flag, the rally point (3 + exists), then the enter-exit map, the exit path and two flags
                for _ in 0..(5 + 4 + 1 + 3 + 1 + 4) {
                    state.push(r.u32());
                }
                let map = r.u32();
                state.push(map);
                for _ in 0..map * 2 {
                    state.push(r.u32());
                }
                for _ in 0..match has { 2 => 6, 3 | 5 => 7, 4 => 9, _ => 3 } {
                    state.push(r.u32());
                }
            }
            let rest: Vec<u32> = (0..18).map(|_| r.u32()).collect();
            let (cmdset, level) = (r.str(), r.u32());
            out.push(format!("op {i} object {k}: contain {state:08x?} container {} at {:08x} {:08x} {:08x} status {:08x} {:08x} dead {} aidead {} fall {} radius {:08x} orient {:08x} layer {} conds {:08x?} selected {} bonus {:x} cmdset {cmdset} level {level}", rest[0], rest[1], rest[2], rest[3], rest[4], rest[5], rest[6], rest[7], rest[8], rest[9], rest[10], rest[11], &rest[12..16], rest[16], rest[17]));
        }
    }
    out
}

#[test]
fn open_contain_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "contain module R1",
        seed: 0x00C0_0A17,
        cases_var: "L9CT_CASES",
        ini_stem: "Ct",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9ct_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "contain module R1: {} cases, {} data blocks threw, {} excluded (undefined upstream), {} ops, {} adds, {} removes, {} exits, {} kill-alls, {} deaths, {} collisions, {} updates, {} xfers, {} effect lines, {} cut points, {} end-iterator reads",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.adds, t.removes, t.exits, t.kills, t.deaths, t.collisions, t.updates, t.xfers, t.effects, t.cuts, t.end_reads
    );
    assert!(c.cases > 0);
}

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::OpenContain(Arc::new(OpenContainModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn read_transport_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::TransportContain(Arc::new(TransportContainModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn read_internet_hack_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(rts_lockstep_engine::logic::modules::internet_hack_contain::new_module_data(Some(ini))?);
    Ok(())
}

fn read_helix_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(rts_lockstep_engine::logic::modules::helix_contain::new_module_data(Some(ini))?);
    Ok(())
}

fn read_rider_change_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(rts_lockstep_engine::logic::modules::rider_change_contain::new_module_data(Some(ini))?);
    Ok(())
}

fn read_overlord_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(rts_lockstep_engine::logic::modules::overlord_contain::new_module_data(Some(ini))?);
    Ok(())
}

fn read_railed_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(rts_lockstep_engine::logic::modules::railed_transport_contain::new_module_data(Some(ini))?);
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }, BlockParse { token: "T", parse: read_transport_block }, BlockParse { token: "I", parse: read_internet_hack_block }, BlockParse { token: "R", parse: read_railed_block }, BlockParse { token: "H", parse: read_helix_block }, BlockParse { token: "C", parse: read_rider_change_block }, BlockParse { token: "O", parse: read_overlord_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}
