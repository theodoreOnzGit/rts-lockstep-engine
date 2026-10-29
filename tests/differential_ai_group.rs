// SPDX-License-Identifier: GPL-3.0-only
//! Differential test of the AI group (`logic::ai::ai_group`, Phase 1 layer 10, `p1w13-sonnet`, batch A24) against AIGroup.cpp's
//! group, members, centre, speed and commands (all but the moves), cut verbatim and compiled over a world of the wrapper's own
//! (`differential/wrap_ai_group.cpp`).
//!
//! Each case is one scene ([`GrpScene`]: twelve objects with their AI, held, dead, contain, spawn, production, drawable and other
//! flags, kinds, status bits, positions, players, formations, damage states, speeds, passengers, stealth delays and command sets)
//! and a run of operations ([`GrpOp`]) on one new group: members added (only non-members) and removed, the queries, and every
//! command. The answers the world gives that the scene does not hold are one hash of (kind, object, argument) both sides compute
//! (`grpAnswer`). Compared: each operation's answer words and the group's members, count, speed, dirty flag and id at the end;
//! and the log of every call the group makes into the world (each AI command, the contain, spawn, special power, button,
//! weapon, upgrade, emoticon and formation calls, `enterGroup`, `leaveGroup`, `TheAI->destroyGroup`, the random values).
//!
//! Not generated: a member added twice (upstream keeps it twice; the callers check first), a null member, and the destructor
//! (`TheAI->destroyGroup`'s, AI.cpp).
//!
//! # Results (2026-10-29, batch A24; upstream commit 0a05454d, seeds `0xA24_0000 + case`, 1,000,000 cases)
//!
//! - 1,000,000 cases of 4 to 39 operations (21,479,973 in all: 7,821,564 adds, 1,934,485 removes, about 202,000 of each other kind), **0 mismatches**
//!   in the answers and the final group, and 0 in the call log (about 134 million words).
//! - The first run found one harness defect, fixed: the `SimpleObjectIterator` stand-in appended where upstream's `insert`
//!   prepends, so members at one distance from the victim were attacked in the other order (778 log mismatches in 100,000 cases).
//!   No port defect found.
//! - Planted bugs at 100,000 cases: 3530 (the empty group not destroyed), 3531 (the centre without AI members), 3532 (one member a
//!   formation), 3533 (the speed's damage test), 3534 (the garrison's stop), 3535 (the member's own attack), 3536 (attack-move for
//!   the unable), 3537 (the aircraft's landing), 3538 (the formation id kept), 3539 (the special power at a location), 3540 (the
//!   weapon set's template test), 3541 (the full production queue) each caught (428 to 16,742 mismatches).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D, Region3D};
use rts_lockstep_engine::common::game_common::PlayerType;
use rts_lockstep_engine::common::game_common::AbleToAttackType;
use rts_lockstep_engine::common::game_type::{ObjectId, INVALID_ID};
use rts_lockstep_engine::common::kind_of::{
    KindOfType, KINDOF_AIRCRAFT, KINDOF_ALWAYS_SELECTABLE, KINDOF_CLIFF_JUMPER, KINDOF_IMMOBILE, KINDOF_INFANTRY, KINDOF_MOB_NEXUS, KINDOF_PRODUCED_AT_HELIPAD,
    KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusTypes, OBJECT_STATUS_CAN_STEALTH, OBJECT_STATUS_DETECTED, OBJECT_STATUS_STEALTHED};
use rts_lockstep_engine::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferState};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::differential::{GrpObj, GrpOp, GrpScene, GRP_OBJECTS};
use rts_lockstep_engine::logic::ai::ai_group::{AiGroup, AiGroupHost, GroupAiCommand, GroupButtonCall};
use rts_lockstep_engine::logic::ai::ai_group_moves::{get_helicopter_offset, AiGroupMoveHost};
use rts_lockstep_engine::logic::ai::ai_update_types::{AttitudeType, CanAttackResultValue, CommandSourceType};
use rts_lockstep_engine::logic::module::body_module::BodyDamageType;
use std::os::raw::c_int;
use support::{Check, Gen};

const CASES: u64 = 1_000_000;

// the scene's flags (wrap_ai_group.cpp's GRP_F_*)
const F_AI: i32 = 1;
const F_HELD: i32 = 2;
const F_DEAD: i32 = 4;
const F_ABLE: i32 = 8;
const F_AIRBORNE: i32 = 16;
const F_CONTAIN: i32 = 32;
const F_FIRE: i32 = 64;
const F_SPAWN: i32 = 128;
const F_FREEDOM: i32 = 256;
const F_ACQUIRE: i32 = 512;
const F_ACQUIRE_STEALTHED: i32 = 1024;
const F_IDLE: i32 = 2048;
const F_BUSY: i32 = 4096;
const F_DRAWABLE: i32 = 8192;
const F_PRODUCTION: i32 = 16384;
const F_GROUND: i32 = 32768;

// the log's records (GRP_LOG_*) and the AI commands (GC_*)
const L_ENTER: u32 = 1;
const L_LEAVE: u32 = 2;
const L_DESTROY: u32 = 3;
const L_AI: u32 = 4;
const L_MOOD: u32 = 5;
const L_SLAVES: u32 = 6;
const L_EXIT_ALL: u32 = 7;
const L_SP: u32 = 8;
const L_DEFECTOR: u32 = 9;
const L_CHEER: u32 = 10;
const L_SELL: u32 = 11;
const L_OVERCHARGE: u32 = 12;
const L_BUTTON: u32 = 13;
const L_ATTITUDE: u32 = 14;
const L_WEAPONSET: u32 = 15;
const L_LOCK: u32 = 16;
const L_UPGRADE: u32 = 17;
const L_EMOTICON: u32 = 18;
const L_OVERRIDE: u32 = 19;
const L_FORMATION: u32 = 20;
const L_RANDOM: u32 = 21;
// batch A25: the pathfinder
const L_REMOVE_GOAL: u32 = 22;
const L_ADJUST: u32 = 23;
const L_UPDATE_GOAL: u32 = 24;
const L_UPDATE_POS: u32 = 25;
const L_LINE: u32 = 26;
const L_GROUND_PATH: u32 = 27;
/// `LAYER_GROUND` as the wrapper logs it.
const LAYER_GROUND: u32 = 1;

/// `grpAnswer`: the hash both sides answer the world's questions with.
fn grp_answer(seed: u32, kind: u32, obj: u32, arg: u32) -> u32 {
    let mut x = seed ^ kind.wrapping_mul(0x85eb_ca6b);
    x ^= obj;
    x = x.wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x ^= arg;
    x = x.wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x
}

struct Host {
    s: GrpScene,
    log: Vec<u32>,
    rand: u32,
    next_group: u32,
    next_formation: u32,
    /// the AIs' `m_tmpInt` (batch A25)
    tmp: [i32; GRP_OBJECTS],
    /// (coverage) column and formation paths given, their points, ground paths found, tightens, move commands
    seen: [u64; 5],
}

fn b(f: f32) -> u32 {
    f.to_bits()
}

impl Host {
    fn new(s: &GrpScene) -> Self {
        Host { s: *s, log: Vec::new(), rand: s.seed, next_group: s.next_group_id as u32, next_formation: s.next_formation_id as u32, tmp: s.o.map(|o| o.tmp), seen: [0; 5] }
    }
    fn idx(&self, id: ObjectId) -> usize {
        self.s.o.iter().position(|o| o.id == id && id != 0).expect("an object of the scene")
    }
    fn o(&self, id: ObjectId) -> &GrpObj {
        self.s.o.iter().find(|o| o.id == id && id != 0).expect("an object of the scene")
    }
    /// An id of the scene's (else `INVALID_ID`: `findObj`'s NULL).
    fn find(&self, id: i32) -> ObjectId {
        if id != 0 && self.s.o.iter().any(|o| o.id == id) {
            id
        } else {
            INVALID_ID
        }
    }
    fn f(&self, id: ObjectId, flag: i32) -> bool {
        self.o(id).flags & flag != 0
    }
    fn ans(&self, kind: u32, obj: u32, arg: u32) -> bool {
        grp_answer(self.s.seed, kind, obj, arg) & 3 != 0
    }
    fn l(&mut self, w: &[u32]) {
        self.log.extend_from_slice(w);
    }
}

impl AiGroupHost for Host {
    fn ai_next_group_id(&mut self) -> u32 {
        let id = self.next_group;
        self.l(&[L_RANDOM, 0, id]);
        self.next_group = self.next_group.wrapping_add(1);
        id
    }
    fn ai_next_formation_id(&mut self) -> u32 {
        let id = self.next_formation;
        self.next_formation = self.next_formation.wrapping_add(1);
        id
    }
    fn ai_destroy_group(&mut self, group: u32) {
        self.l(&[L_DESTROY, group]);
    }
    fn frame(&mut self) -> u32 {
        self.s.frame
    }
    fn logic_random_value(&mut self, lo: i32, hi: i32) -> i32 {
        self.rand = self.rand.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let r = lo + ((self.rand >> 8) % ((hi - lo + 1) as u32)) as i32;
        self.l(&[L_RANDOM, 1, lo as u32, hi as u32, r as u32]);
        r
    }
    fn movement_penalty_damage_state(&mut self) -> BodyDamageType {
        self.s.penalty
    }
    fn has_ai(&mut self, o: ObjectId) -> bool {
        self.f(o, F_AI)
    }
    fn is_kind_of(&mut self, o: ObjectId, k: KindOfType) -> bool {
        let bit = if k == KINDOF_STRUCTURE {
            0
        } else if k == KINDOF_ALWAYS_SELECTABLE {
            1
        } else if k == KINDOF_IMMOBILE {
            2
        } else if k == KINDOF_AIRCRAFT {
            3
        } else if k == KINDOF_INFANTRY {
            4
        } else if k == KINDOF_VEHICLE {
            5
        } else if k == KINDOF_MOB_NEXUS {
            6
        } else if k == KINDOF_PRODUCED_AT_HELIPAD {
            7
        } else if k == KINDOF_CLIFF_JUMPER {
            8
        } else {
            panic!("kind {k:?} not in the scene")
        };
        (self.o(o).kinds >> bit) & 1 != 0
    }
    fn status_test(&mut self, o: ObjectId, s: ObjectStatusTypes) -> bool {
        let bit = if s == OBJECT_STATUS_CAN_STEALTH {
            0
        } else if s == OBJECT_STATUS_STEALTHED {
            1
        } else if s == OBJECT_STATUS_DETECTED {
            2
        } else {
            panic!("status {s:?} not in the scene")
        };
        (self.o(o).status >> bit) & 1 != 0
    }
    fn is_held(&mut self, o: ObjectId) -> bool {
        self.f(o, F_HELD)
    }
    fn is_effectively_dead(&mut self, o: ObjectId) -> bool {
        self.f(o, F_DEAD)
    }
    fn is_able_to_attack(&mut self, o: ObjectId) -> bool {
        self.f(o, F_ABLE)
    }
    fn is_airborne_target(&mut self, o: ObjectId) -> bool {
        self.f(o, F_AIRBORNE)
    }
    fn position(&mut self, o: ObjectId) -> Coord3D {
        let p = self.o(o).pos;
        Coord3D { x: p[0], y: p[1], z: p[2] }
    }
    fn controlling_player(&mut self, o: ObjectId) -> Option<u32> {
        Some((self.o(o).player & 3) as u32)
    }
    fn enter_group(&mut self, o: ObjectId, group: u32) {
        self.l(&[L_ENTER, o as u32, group]);
    }
    fn leave_group(&mut self, o: ObjectId) {
        self.l(&[L_LEAVE, o as u32]);
    }
    fn formation_id(&mut self, o: ObjectId) -> u32 {
        self.o(o).formation as u32
    }
    fn set_formation_id(&mut self, o: ObjectId, id: u32) {
        self.l(&[L_FORMATION, o as u32, 0, id]);
    }
    fn set_formation_offset(&mut self, o: ObjectId, offset: Coord2D) {
        self.l(&[L_FORMATION, o as u32, 1, b(offset.x), b(offset.y)]);
    }
    fn damage_state(&mut self, o: ObjectId) -> BodyDamageType {
        self.o(o).damage
    }
    fn cur_locomotor_speed(&mut self, o: ObjectId) -> f32 {
        self.o(o).loco_speed
    }
    fn able_to_attack_specific_object(&mut self, o: ObjectId, t: AbleToAttackType, victim: ObjectId, _src: CommandSourceType) -> CanAttackResultValue {
        (grp_answer(self.s.seed, 60, o as u32, (victim as u32).wrapping_mul(4).wrapping_add(t as u32)) % 4) as CanAttackResultValue
    }
    fn able_to_use_weapon_against_position(&mut self, o: ObjectId, _t: AbleToAttackType, pos: &Coord3D, _src: CommandSourceType) -> CanAttackResultValue {
        (grp_answer(self.s.seed, 61, o as u32, b(pos.x) ^ b(pos.y)) % 4) as CanAttackResultValue
    }
    fn ai_command(&mut self, o: ObjectId, cmd: GroupAiCommand, src: CommandSourceType) {
        use GroupAiCommand as C;
        let p3 = |p: &Coord3D| [b(p.x), b(p.y), b(p.z)];
        let mut w: Vec<u32> = vec![L_AI, o as u32];
        match cmd {
            C::Idle => w.push(0),
            C::AttackObject { victim, max_shots, forced } => w.extend([if forced { 2 } else { 1 }, victim as u32, max_shots as u32]),
            C::AttackTeam { team, max_shots } => w.extend([3, team, max_shots as u32]),
            C::AttackPosition { pos, max_shots } => {
                w.push(4);
                w.extend(p3(&pos));
                w.push(max_shots as u32);
            }
            C::AttackMoveToPosition { pos, max_shots } => {
                w.push(5);
                w.extend(p3(&pos));
                w.push(max_shots as u32);
            }
            C::MoveToPosition { pos } => {
                self.seen[4] += 1;
                w.push(6);
                w.extend(p3(&pos));
            }
            C::Hunt => w.push(7),
            C::Repair { obj } => w.extend([8, obj as u32]),
            C::ResumeConstruction { obj } => w.extend([9, obj as u32]),
            C::GetHealed { depot } => w.extend([10, depot as u32]),
            C::GetRepaired { depot } => w.extend([11, depot as u32]),
            C::Enter { obj } => w.extend([12, obj as u32]),
            C::Dock { obj } => w.extend([13, obj as u32]),
            C::Exit { obj } => w.extend([14, obj as u32]),
            C::MoveToAndEvacuate { pos } => {
                w.push(15);
                w.extend(p3(&pos));
            }
            C::Evacuate { expose_stealth_units } => w.extend([16, u32::from(expose_stealth_units)]),
            C::ExecuteRailedTransport => w.push(17),
            C::GoProne { damage } => w.extend([18, damage]),
            C::GuardPosition { pos, mode } => {
                w.push(19);
                w.extend(p3(&pos));
                w.push(mode as u32);
            }
            C::GuardObject { obj, mode } => w.extend([20, obj as u32, mode as u32]),
            C::GuardArea { area, mode } => w.extend([21, area as u32, mode as u32]),
            C::AttackArea { area } => w.extend([22, area as u32]),
            C::HackInternet => w.push(23),
            C::CombatDrop { target, pos } => {
                w.extend([24, target as u32]);
                w.extend(p3(&pos));
            }
            C::FollowPathAppend { pos } => {
                w.push(26);
                w.extend(p3(&pos));
            }
            C::TightenToPosition { pos } => {
                self.seen[3] += 1;
                w.push(27);
                w.extend(p3(&pos));
            }
            C::FollowWaypointPath { way, exact, as_team } => w.extend([28, way, u32::from(exact), u32::from(as_team)]),
            C::MoveToAndEvacuateAndExit { pos } => {
                w.push(29);
                w.extend(p3(&pos));
            }
        }
        w.push(src as u32);
        self.l(&w);
    }
    fn ai_set_attitude(&mut self, o: ObjectId, a: AttitudeType) {
        self.l(&[L_ATTITUDE, o as u32, a as u32]);
    }
    fn ai_is_idle(&mut self, o: ObjectId) -> bool {
        self.f(o, F_IDLE)
    }
    fn ai_is_busy(&mut self, o: ObjectId) -> bool {
        self.f(o, F_BUSY)
    }
    fn ai_can_auto_acquire(&mut self, o: ObjectId) -> bool {
        self.f(o, F_ACQUIRE)
    }
    fn ai_can_auto_acquire_while_stealthed(&mut self, o: ObjectId) -> bool {
        self.f(o, F_ACQUIRE_STEALTHED)
    }
    fn ai_set_next_mood_check_time(&mut self, o: ObjectId, t: u32) {
        self.l(&[L_MOOD, o as u32, t]);
    }
    fn stealth_delay(&mut self, o: ObjectId) -> Option<u32> {
        let d = self.o(o).stealth_delay;
        (d >= 0).then_some(d as u32)
    }
    fn has_contain(&mut self, o: ObjectId) -> bool {
        self.f(o, F_CONTAIN)
    }
    fn contained(&mut self, o: ObjectId) -> Vec<ObjectId> {
        let c = self.o(o).contained;
        c.iter().map(|&i| self.find(i)).filter(|&i| i != INVALID_ID).collect()
    }
    fn contain_passengers_allowed_to_fire(&mut self, o: ObjectId) -> bool {
        self.f(o, F_FIRE)
    }
    fn contain_order_all_passengers_to_exit(&mut self, o: ObjectId, src: CommandSourceType) {
        self.l(&[L_EXIT_ALL, o as u32, src as u32]);
    }
    fn has_spawn(&mut self, o: ObjectId) -> bool {
        self.f(o, F_SPAWN)
    }
    fn spawn_slaves_have_freedom(&mut self, o: ObjectId) -> bool {
        self.f(o, F_FREEDOM)
    }
    fn spawn_order_slaves_idle(&mut self, o: ObjectId, src: CommandSourceType) {
        self.l(&[L_SLAVES, o as u32, 0, src as u32]);
    }
    fn spawn_order_slaves_attack_target(&mut self, o: ObjectId, victim: ObjectId, max_shots: i32, src: CommandSourceType) {
        self.l(&[L_SLAVES, o as u32, 1, victim as u32, max_shots as u32, src as u32]);
    }
    fn spawn_order_slaves_attack_position(&mut self, o: ObjectId, pos: &Coord3D, max_shots: i32, src: CommandSourceType) {
        self.l(&[L_SLAVES, o as u32, 2, b(pos.x), b(pos.y), b(pos.z), max_shots as u32, src as u32]);
    }
    fn highest_layer_for_destination(&mut self, pos: &Coord3D) -> i32 {
        (grp_answer(self.s.seed, 50, b(pos.x), b(pos.y)) % 3) as i32
    }
    fn layer_height(&mut self, _x: f32, _y: f32, layer: i32) -> f32 {
        10.0 * layer as f32 + 0.5
    }
    fn special_power_required_science(&mut self, sp: u32) -> Option<i32> {
        if sp < 8 && self.ans(41, 0, sp) {
            Some(if self.ans(40, 0, sp) { -1 } else { (sp % 3) as i32 })
        } else {
            None
        }
    }
    fn player_has_science(&mut self, o: ObjectId, science: i32) -> bool {
        let p = (self.o(o).player & 3) as u32;
        self.ans(20, p, science as u32)
    }
    fn has_special_power_module(&mut self, o: ObjectId, sp: u32) -> bool {
        self.ans(42, o as u32, sp)
    }
    fn can_do_special_power(&mut self, o: ObjectId, sp: u32, options: u32) -> bool {
        self.ans(45, o as u32, sp.wrapping_mul(16).wrapping_add(options))
    }
    fn can_do_special_power_at_location(&mut self, o: ObjectId, sp: u32, _pos: &Coord3D, object_in_way: ObjectId, options: u32) -> bool {
        self.ans(46, o as u32, sp.wrapping_mul(16).wrapping_add(options).wrapping_add((object_in_way as u32).wrapping_mul(256)))
    }
    fn can_do_special_power_at_object(&mut self, o: ObjectId, sp: u32, target: ObjectId, options: u32) -> bool {
        self.ans(47, o as u32, sp.wrapping_mul(16).wrapping_add(options).wrapping_add((target as u32).wrapping_mul(256)))
    }
    fn do_special_power(&mut self, o: ObjectId, sp: u32, options: u32) {
        self.l(&[L_SP, o as u32, 0, sp, options]);
    }
    fn do_special_power_at_location(&mut self, o: ObjectId, sp: u32, pos: &Coord3D, angle: f32, options: u32) {
        self.l(&[L_SP, o as u32, 1, sp, b(pos.x), b(pos.y), b(pos.z), b(angle), options]);
    }
    fn do_special_power_at_object(&mut self, o: ObjectId, sp: u32, target: ObjectId, options: u32) {
        self.l(&[L_SP, o as u32, 2, sp, target as u32, options]);
    }
    fn clear_undetected_defector(&mut self, o: ObjectId) {
        self.l(&[L_DEFECTOR, o as u32]);
    }
    fn override_special_power_destination(&mut self, o: ObjectId, sp_type: i32, loc: &Coord3D) {
        if self.ans(44, o as u32, sp_type as u32) {
            self.l(&[L_OVERRIDE, o as u32, b(loc.x), b(loc.y), b(loc.z)]);
        }
    }
    fn set_cheering(&mut self, o: ObjectId, frames: i32) {
        self.l(&[L_CHEER, o as u32, frames as u32]);
    }
    fn sell(&mut self, o: ObjectId) {
        self.l(&[L_SELL, o as u32]);
    }
    fn toggle_overcharge(&mut self, o: ObjectId) {
        // (the wrapper's behaviour modules: an overcharge interface on the first and, for odd ids, the third)
        self.l(&[L_OVERCHARGE, o as u32]);
        if o & 1 != 0 {
            self.l(&[L_OVERCHARGE, o as u32]);
        }
    }
    fn do_command_button(&mut self, o: ObjectId, button: u32, call: GroupButtonCall, src: CommandSourceType) {
        match call {
            GroupButtonCall::Plain => self.l(&[L_BUTTON, o as u32, 0, button, src as u32]),
            GroupButtonCall::AtPosition(p) => self.l(&[L_BUTTON, o as u32, 1, button, b(p.x), b(p.y), b(p.z), src as u32]),
            GroupButtonCall::UsingWaypoints(w) => self.l(&[L_BUTTON, o as u32, 2, button, w, src as u32]),
            GroupButtonCall::AtObject(t) => self.l(&[L_BUTTON, o as u32, 3, button, t as u32, src as u32]),
        }
    }
    fn set_weapon_set_flag(&mut self, o: ObjectId, wst: i32) {
        self.l(&[L_WEAPONSET, o as u32, 1, wst as u32]);
    }
    fn clear_weapon_set_flag(&mut self, o: ObjectId, wst: i32) {
        self.l(&[L_WEAPONSET, o as u32, 0, wst as u32]);
    }
    fn template_has_weapon_set(&mut self, o: ObjectId, wst: i32) -> bool {
        self.ans(48, o as u32, wst as u32)
    }
    fn set_weapon_lock(&mut self, o: ObjectId, slot: i32, lock: i32) -> bool {
        self.l(&[L_LOCK, o as u32, 1, slot as u32, lock as u32]);
        self.ans(43, o as u32, (slot * 8 + lock) as u32)
    }
    fn release_weapon_lock(&mut self, o: ObjectId, lock: i32) {
        self.l(&[L_LOCK, o as u32, 0, lock as u32]);
    }
    fn can_afford_upgrade(&mut self, o: ObjectId, upgrade: u32) -> bool {
        let p = (self.o(o).player & 3) as u32 + 1;
        self.ans(35, p, upgrade)
    }
    fn upgrade_is_object_type(&mut self, upgrade: u32) -> bool {
        self.ans(30, 0, upgrade)
    }
    fn has_upgrade(&mut self, o: ObjectId, upgrade: u32) -> bool {
        self.ans(31, o as u32, upgrade)
    }
    fn affected_by_upgrade(&mut self, o: ObjectId, upgrade: u32) -> bool {
        self.ans(32, o as u32, upgrade)
    }
    fn can_produce_upgrade(&mut self, o: ObjectId, upgrade: u32) -> bool {
        self.ans(33, o as u32, upgrade)
    }
    fn production_queue_full_for_upgrade(&mut self, o: ObjectId, upgrade: u32) -> Option<bool> {
        self.f(o, F_PRODUCTION).then(|| !self.ans(34, o as u32, upgrade))
    }
    fn production_queue_upgrade(&mut self, o: ObjectId, upgrade: u32) {
        self.l(&[L_UPGRADE, o as u32, upgrade]);
    }
    fn command_set_has_button_type(&mut self, o: ObjectId, command_type: i32) -> Option<bool> {
        let cs = self.o(o).command_set;
        (cs >= 0).then_some((0..8).contains(&command_type) && (cs >> command_type) & 1 != 0)
    }
    fn set_emoticon(&mut self, o: ObjectId, name: &[u8], duration: i32) {
        if self.f(o, F_DRAWABLE) {
            self.l(&[L_EMOTICON, o as u32, name.len() as u32, duration as u32]);
        }
    }
}

impl AiGroupMoveHost for Host {
    fn debug_ai_terrain(&mut self) -> bool {
        self.s.debug_terrain != 0
    }
    fn group_move_click_to_gather_factor(&mut self) -> f32 {
        self.s.gather_factor
    }
    fn ai_distance_requires_group(&mut self) -> f32 {
        self.s.distance_requires_group
    }
    fn ai_min_distance_for_group(&mut self) -> f32 {
        self.s.min_distance_for_group
    }
    fn ai_min_infantry_for_group(&mut self) -> i32 {
        self.s.min_infantry_for_group
    }
    fn ai_min_vehicles_for_group(&mut self) -> i32 {
        self.s.min_vehicles_for_group
    }
    fn formation_offset(&mut self, o: ObjectId) -> Coord2D {
        let f = self.o(o).formation_offset;
        Coord2D { x: f[0], y: f[1] }
    }
    fn bounding_circle_radius(&mut self, o: ObjectId) -> f32 {
        self.o(o).bounding_radius
    }
    fn major_radius(&mut self, o: ObjectId) -> f32 {
        self.o(o).major_radius
    }
    fn player_type(&mut self, o: ObjectId) -> Option<PlayerType> {
        let p = self.o(o).player & 3;
        Some(if (self.s.player_types >> p) & 1 != 0 { PlayerType::Computer } else { PlayerType::Human })
    }
    fn ai_is_doing_ground_movement(&mut self, o: ObjectId) -> bool {
        self.f(o, F_GROUND)
    }
    fn ai_tmp_value(&mut self, o: ObjectId) -> i32 {
        self.tmp[self.idx(o)]
    }
    fn ai_set_tmp_value(&mut self, o: ObjectId, v: i32) {
        let i = self.idx(o);
        self.tmp[i] = v;
    }
    fn ai_cur_locomotor_priority(&mut self, o: ObjectId) -> Option<i32> {
        let p = self.o(o).loco_priority;
        (p >= 0).then_some(p)
    }
    fn ai_follow_path(&mut self, o: ObjectId, path: &[Coord3D], src: CommandSourceType) {
        self.seen[0] += 1;
        self.seen[1] += path.len() as u64;
        let mut w = vec![L_AI, o as u32, 25, path.len() as u32];
        for p in path {
            w.extend([b(p.x), b(p.y), b(p.z)]);
        }
        w.extend([0, src as u32]);
        self.l(&w);
    }
    fn pf_remove_goal(&mut self, o: ObjectId) {
        self.l(&[L_REMOVE_GOAL, o as u32]);
    }
    fn pf_adjust_destination(&mut self, o: ObjectId, dest: &mut Coord3D, group_dest: Option<&Coord3D>) {
        let mut w = vec![L_ADJUST, o as u32, b(dest.x), b(dest.y), b(dest.z), u32::from(group_dest.is_some())];
        if let Some(g) = group_dest {
            w.extend([b(g.x), b(g.y), b(g.z)]);
        }
        self.l(&w);
        if self.ans(57, o as u32, b(dest.x)) {
            dest.x += 1.0;
            dest.y -= 0.5;
        }
    }
    fn pf_update_goal(&mut self, o: ObjectId, dest: &Coord3D) {
        self.l(&[L_UPDATE_GOAL, o as u32, b(dest.x), b(dest.y), b(dest.z), LAYER_GROUND]);
    }
    fn pf_update_pos(&mut self, o: ObjectId, pos: &Coord3D) {
        self.l(&[L_UPDATE_POS, o as u32, b(pos.x), b(pos.y), b(pos.z)]);
    }
    fn pf_is_line_passable(&mut self, o: ObjectId, from: &Coord3D, to: &Coord3D) -> bool {
        self.l(&[L_LINE, o as u32, b(from.x), b(from.y), b(to.x), b(to.y)]);
        self.ans(53, o as u32, b(to.x))
    }
    fn pf_find_ground_path(&mut self, from: &Coord3D, to: &Coord3D) -> Option<Vec<Coord3D>> {
        // (the wrapper's: 2 to 8 nodes from `from` toward `to`, each off the line by the hash's jitter; none for an eighth)
        self.l(&[L_GROUND_PATH, b(from.x), b(from.y), b(from.z), b(to.x), b(to.y), b(to.z), 6]);
        let h = grp_answer(self.s.seed, 54, b(from.x) ^ b(to.y), b(to.x));
        if h & 7 == 0 {
            return None;
        }
        self.seen[2] += 1;
        let n = 2 + ((h >> 3) % 7) as usize;
        Some(
            (0..n)
                .map(|k| {
                    let t = k as f32 / (n - 1) as f32;
                    let jx = (grp_answer(self.s.seed, 55, k as u32, h) % 401) as f32 * 0.1 - 20.0;
                    let jy = (grp_answer(self.s.seed, 56, k as u32, h) % 401) as f32 * 0.1 - 20.0;
                    Coord3D { x: from.x + (to.x - from.x) * t + jx, y: from.y + (to.y - from.y) * t + jy, z: from.z }
                })
                .collect(),
        )
    }
    fn layer_for_destination(&mut self, pos: &Coord3D) -> i32 {
        (grp_answer(self.s.seed, 51, b(pos.x), b(pos.y)) % 3) as i32
    }
    fn ground_height(&mut self, x: f32, y: f32) -> f32 {
        x * 0.01 + y * 0.02
    }
    fn map_extent(&mut self) -> Region3D {
        region(self.s.extent)
    }
    fn max_pathfind_extent(&mut self) -> Region3D {
        region(self.s.path_extent)
    }
}

fn region(r: [f32; 4]) -> Region3D {
    Region3D { lo: Coord3D { x: r[0], y: r[1], z: 0.0 }, hi: Coord3D { x: r[2], y: r[3], z: 0.0 } }
}

/// A member not held without an AI: the formation move commands it through its null AI (undefined upstream), so the moves that may
/// reach it (ops 60 and 72) are not run on such a group.
fn member_without_ai(h: &mut Host, g: &AiGroup) -> bool {
    g.members.iter().any(|&m| !h.is_held(m) && !h.has_ai(m))
}

/// The group's ground path: its node count and their positions (0: none).
fn path_words(g: &AiGroup, r: &mut Vec<u32>) {
    match &g.ground_path {
        None => r.push(0),
        Some(p) => {
            r.push(p.len() as u32);
            for n in p {
                r.extend([b(n.x), b(n.y), b(n.z)]);
            }
        }
    }
}

/// The save (or CRC) into bytes.
struct MemXfer {
    state: XferState,
    bytes: Vec<u8>,
}

impl Xfer for MemXfer {
    fn state(&self) -> &XferState {
        &self.state
    }
    fn state_mut(&mut self) -> &mut XferState {
        &mut self.state
    }
    fn open(&mut self, _identifier: &[u8]) -> XferResult<()> {
        Ok(())
    }
    fn close(&mut self) -> XferResult<()> {
        Ok(())
    }
    fn begin_block(&mut self) -> XferResult<i32> {
        Ok(0)
    }
    fn end_block(&mut self) -> XferResult<()> {
        Ok(())
    }
    fn skip(&mut self, _data_size: i32) -> XferResult<()> {
        Ok(())
    }
    fn xfer_snapshot(&mut self, _snapshot: &mut dyn Snapshot) -> XferResult<()> {
        Ok(())
    }
    fn xfer_implementation(&mut self, data: &mut [u8]) -> XferResult<()> {
        self.bytes.extend_from_slice(data);
        Ok(())
    }
}

/// The Rust group over the scene: (the answer words, the log).
fn rust_run(s: &GrpScene, ops: &[GrpOp], seen: &mut [u64; 5]) -> (Vec<u32>, Vec<u32>) {
    let mut h = Host::new(s);
    let mut r: Vec<u32> = Vec::new();
    let mut g = AiGroup::new(&mut h);
    for op in ops {
        let p = Coord3D { x: op.x, y: op.y, z: op.z };
        let src = op.c & 3;
        let oa = h.find(op.a);
        let ob = h.find(op.b);
        let player = (op.a >= 0).then_some((op.a & 3) as u32);
        let name: &[u8] = if op.a & 1 != 0 { b"Emoticon" } else { b"E" };
        match op.kind {
            0 => {
                if oa != INVALID_ID && !g.is_member(oa) {
                    g.add(&mut h, oa)
                }
            }
            1 => r.push(if oa != INVALID_ID { u32::from(g.remove(&mut h, oa)) } else { 2 }),
            2 => r.push(u32::from(g.is_member(oa))),
            3 => r.push(b(g.get_speed(&mut h))),
            4 => {
                let (c, ok) = g.get_center(&mut h);
                r.extend([u32::from(ok), b(c.x), b(c.y), b(c.z)]);
            }
            5 => {
                let (mn, mx, c, ok) = g.get_min_max_and_center(&mut h);
                r.extend([u32::from(ok), b(mn.x), b(mn.y), b(mx.x), b(mx.y), b(c.x), b(c.y), b(c.z)]);
            }
            6 => g.recompute(&mut h),
            7 => r.push(u32::from(g.contains_any_objects_not_owned_by_player(&mut h, player))),
            8 => r.push(u32::from(g.remove_any_objects_not_owned_by_player(&mut h, player))),
            9 => g.group_idle(&mut h, src),
            10 => g.group_attack_object_private(&mut h, op.b != 0, oa, op.c >> 2, src),
            11 => g.group_attack_team(&mut h, (op.a >= 0).then_some(op.a as u32), op.b, src),
            12 => g.group_attack_position(&mut h, (op.a != 0).then_some(&p), op.b, src),
            13 => g.group_attack_move_to_position(&mut h, &p, op.b, src),
            14 => g.group_command(&mut h, GroupAiCommand::Hunt, src),
            15 => g.group_command(&mut h, GroupAiCommand::Repair { obj: oa }, src),
            16 => g.group_command(&mut h, GroupAiCommand::ResumeConstruction { obj: oa }, src),
            17 => g.group_command(&mut h, GroupAiCommand::GetHealed { depot: oa }, src),
            18 => g.group_command(&mut h, GroupAiCommand::GetRepaired { depot: oa }, src),
            19 => g.group_command(&mut h, GroupAiCommand::Enter { obj: oa }, src),
            20 => g.group_command(&mut h, GroupAiCommand::Dock { obj: oa }, src),
            21 => g.group_command(&mut h, GroupAiCommand::Exit { obj: oa }, src),
            22 => g.group_evacuate(&mut h, src),
            23 => g.group_command(&mut h, GroupAiCommand::ExecuteRailedTransport, src),
            24 => g.group_command(&mut h, GroupAiCommand::GoProne { damage: op.a as u32 }, src),
            25 => g.group_guard(&mut h, (op.b != 0).then_some(GroupAiCommand::GuardPosition { pos: p, mode: op.a & 3 }), src),
            26 => g.group_guard(&mut h, (oa != INVALID_ID).then_some(GroupAiCommand::GuardObject { obj: oa, mode: op.b & 3 }), src),
            27 => g.group_guard(&mut h, (op.b != 0).then_some(GroupAiCommand::GuardArea { area: op.a, mode: op.b & 3 }), src),
            28 => g.group_guard(&mut h, (op.b != 0).then_some(GroupAiCommand::AttackArea { area: op.a }), src),
            29 => g.group_command(&mut h, GroupAiCommand::HackInternet, src),
            30 => g.group_create_formation(&mut h),
            31 => g.group_do_special_power(&mut h, op.a as u32, op.b as u32),
            32 => g.group_do_special_power_at_location(&mut h, op.a as u32, &p, op.w, ob, (op.c >> 2) as u32),
            33 => g.group_do_special_power_at_object(&mut h, op.a as u32, ob, (op.c >> 2) as u32),
            34 => g.group_cheer(&mut h),
            35 => g.group_sell(&mut h),
            36 => g.group_toggle_overcharge(&mut h),
            37 => g.group_command(&mut h, GroupAiCommand::CombatDrop { target: ob, pos: p }, src),
            38 => g.group_do_command_button(&mut h, op.a as u32, GroupButtonCall::Plain, src),
            39 => g.group_do_command_button(&mut h, op.a as u32, GroupButtonCall::AtPosition(p), src),
            40 => g.group_do_command_button(&mut h, op.a as u32, GroupButtonCall::UsingWaypoints(op.b as u32), src),
            41 => g.group_do_command_button(&mut h, op.a as u32, GroupButtonCall::AtObject(ob), src),
            42 => g.set_attitude(&mut h, op.a % 5 - 2),
            43 => r.push(g.get_attitude() as u32),
            44 => g.set_mine_clearing_detail(&mut h, op.a != 0, 0),
            45 => r.push(u32::from(g.set_weapon_lock_for_group(&mut h, op.a & 3, op.b & 3))),
            46 => g.release_weapon_lock_for_group(&mut h, op.a & 3),
            47 => g.set_weapon_set_flag(&mut h, op.a & 7),
            48 => g.queue_upgrade(&mut h, (op.b != 0).then_some(op.a as u32)),
            49 => r.push(u32::from(g.is_idle(&mut h))),
            50 => r.push(u32::from(g.is_busy(&mut h))),
            51 => r.push(u32::from(g.is_group_ai_dead(&mut h))),
            52 => r.push(g.get_special_power_source_object(&mut h, op.a as u32).unwrap_or(INVALID_ID) as u32),
            53 => r.push(g.get_command_button_source_object(&mut h, op.a & 7).unwrap_or(INVALID_ID) as u32),
            54 => g.group_set_emoticon(&mut h, name, op.b),
            55 => g.group_override_special_power_destination(&mut h, op.a, &p),
            56 | 57 => {
                let mut x = MemXfer { state: XferState::new(if op.kind == 56 { XferMode::Crc } else { XferMode::Save }), bytes: Vec::new() };
                if op.kind == 56 { g.crc(&mut x) } else { g.xfer(&mut x) }.expect("xfer");
                r.push(x.bytes.len() as u32);
                r.extend(x.bytes.iter().map(|&c| u32::from(c)));
            }
            58 => r.extend([g.get_count() as u32, u32::from(g.is_empty()), g.get_id()]),
            59 => {
                let ids = g.get_all_ids();
                r.push(ids.len() as u32);
                r.extend(ids.iter().map(|&i| i as u32));
            }
            // batch A25
            60 => {
                if !member_without_ai(&mut h, &g) {
                    g.group_move_to_position(&mut h, &p, op.b != 0, src)
                }
            }
            61 => g.group_scatter(&mut h, src),
            62 => g.group_tighten_to_position(&mut h, &p, op.b != 0, src),
            63 => g.group_follow_waypoint_path(&mut h, op.b as u32, false, false, src),
            64 => g.group_follow_waypoint_path(&mut h, op.b as u32, true, false, src),
            65 => g.group_command(&mut h, GroupAiCommand::MoveToAndEvacuate { pos: p }, src),
            66 => g.group_command(&mut h, GroupAiCommand::MoveToAndEvacuateAndExit { pos: p }, src),
            67 => g.group_follow_waypoint_path(&mut h, op.b as u32, false, true, src),
            68 => g.group_follow_waypoint_path(&mut h, op.b as u32, true, true, src),
            69 => {
                let ok = g.friend_compute_ground_path(&mut h, &p);
                r.push(u32::from(ok));
                path_words(&g, &mut r);
            }
            70 => {
                r.push(u32::from(g.friend_move_infantry_to_pos(&mut h, &p, src)));
                path_words(&g, &mut r);
            }
            71 => {
                r.push(u32::from(g.friend_move_vehicle_to_pos(&mut h, &p, src)));
                path_words(&g, &mut r);
            }
            72 => {
                if !member_without_ai(&mut h, &g) {
                    g.friend_move_formation_to_pos(&mut h, &p, src);
                    path_words(&g, &mut r);
                }
            }
            73 => {
                let c = Coord3D { x: op.w, y: op.w * 0.5, z: 0.0 };
                if oa != INVALID_ID && h.has_ai(oa) {
                    let d = AiGroup::compute_individual_destination(&mut h, &p, oa, &c, op.b != 0);
                    r.extend([b(d.x), b(d.y), b(d.z)]);
                }
            }
            74 => {
                let mut ho = p;
                get_helicopter_offset(&mut ho, op.a);
                r.extend([b(ho.x), b(ho.y)]);
            }
            _ => unreachable!("op {}", op.kind),
        }
    }
    r.push(g.members.len() as u32);
    r.extend(g.members.iter().map(|&m| m as u32));
    path_words(&g, &mut r);
    r.extend(h.tmp.iter().map(|&t| t as u32));
    r.extend([g.member_list_size, b(g.speed), u32::from(g.dirty), g.id]);
    for (t, v) in seen.iter_mut().zip(h.seen) {
        *t += v;
    }
    (r, h.log)
}

fn gen_scene(g: &mut Gen) -> GrpScene {
    let mut s = GrpScene::default();
    // (members in a cluster, some on one spot; a few far)
    let base = [g.uniform(0.0, 500.0), g.uniform(0.0, 500.0)];
    for i in 0..GRP_OBJECTS {
        let prev_pos = if i > 0 { s.o[i - 1].pos } else { [0.0; 3] };
        let o = &mut s.o[i];
        o.id = i as i32 + 1;
        let mut flags = 0;
        for (bit, chance) in [
            (F_AI, 4),
            (F_HELD, 1),
            (F_DEAD, 1),
            (F_ABLE, 3),
            (F_AIRBORNE, 1),
            (F_CONTAIN, 1),
            (F_FIRE, 2),
            (F_SPAWN, 1),
            (F_FREEDOM, 2),
            (F_ACQUIRE, 2),
            (F_ACQUIRE_STEALTHED, 1),
            (F_IDLE, 2),
            (F_BUSY, 2),
            (F_DRAWABLE, 3),
            (F_PRODUCTION, 2),
            (F_GROUND, 3),
        ] {
            if g.below(5) < chance {
                flags |= bit;
            }
        }
        o.flags = flags;
        // (structure, always selectable, immobile, aircraft; infantry, vehicle (more often); mob nexus, helipad, cliff jumper)
        o.kinds = (0..9).filter(|&bit| g.below(if bit == 4 || bit == 5 { 2 } else { 6 }) == 0).fold(0, |k, bit| k | (1 << bit));
        o.status = (0..3).filter(|_| g.below(2) == 0).fold(0, |k, bit| k | (1 << bit));
        o.pos = if g.below(8) == 0 && i > 0 {
            prev_pos
        } else if g.below(10) == 0 {
            [g.uniform(0.0, 3000.0), g.uniform(0.0, 3000.0), g.uniform(0.0, 50.0)]
        } else {
            [base[0] + g.uniform(-60.0, 60.0), base[1] + g.uniform(-60.0, 60.0), g.uniform(0.0, 20.0)]
        };
        o.player = g.below(4) as i32;
        o.formation = [0, 0, 1, 2][g.below(4) as usize];
        o.damage = g.below(4) as i32;
        o.loco_speed = if g.below(10) == 0 { 0.0 } else { g.uniform(1.0, 60.0) };
        for c in &mut o.contained {
            *c = if g.below(2) == 0 { 0 } else { g.below(14) as i32 };
        }
        o.stealth_delay = if g.below(3) == 0 { -1 } else { g.below(120) as i32 };
        o.command_set = if g.below(4) == 0 { -1 } else { g.below(256) as i32 };
        // batch A25
        o.formation_offset = [g.uniform(-40.0, 40.0), g.uniform(-40.0, 40.0)];
        o.bounding_radius = g.uniform(2.0, 20.0);
        o.major_radius = g.uniform(2.0, 30.0);
        o.tmp = g.below(7) as i32 - 3;
        o.loco_priority = g.below(4) as i32 - 1;
    }
    s.frame = 1000 + g.below(5000);
    s.seed = g.u32();
    s.penalty = g.below(4) as i32;
    s.next_group_id = g.below(100) as i32;
    s.next_formation_id = 1 + g.below(50) as i32;
    // batch A25: the map (sometimes smaller than the group's spread), the AI's group-move data, the players' types
    for e in [&mut s.extent, &mut s.path_extent] {
        let lo = [g.uniform(-50.0, 100.0), g.uniform(-50.0, 100.0)];
        *e = [lo[0], lo[1], lo[0] + g.uniform(150.0, 1500.0), lo[1] + g.uniform(150.0, 1500.0)];
    }
    s.gather_factor = if g.below(3) == 0 { 0.0 } else { g.uniform(0.5, 3.0) };
    s.debug_terrain = i32::from(g.below(20) == 0);
    s.distance_requires_group = g.uniform(50.0, 400.0);
    s.min_distance_for_group = g.uniform(0.0, 200.0);
    s.min_infantry_for_group = 1 + g.below(5) as i32;
    s.min_vehicles_for_group = 1 + g.below(4) as i32;
    s.player_types = g.below(16) as i32;
    s
}

fn gen_op(g: &mut Gen, i: usize) -> GrpOp {
    let kind = if i < 3 || g.below(4) == 0 {
        0
    } else if g.below(8) == 0 {
        1
    } else if g.below(2) == 0 {
        // batch A25: the moves
        60 + g.below(15) as i32
    } else {
        g.below(60) as i32
    };
    let a = match kind {
        7 | 8 | 11 if g.below(4) == 0 => -1,
        12 | 44 | 48 if g.below(4) == 0 => 0,
        _ => g.below(14) as i32,
    };
    let b = match g.below(4) {
        0 => 0,
        _ => g.below(14) as i32,
    };
    GrpOp {
        kind,
        a,
        b,
        c: g.below(64) as i32,
        x: g.uniform(-100.0, 700.0),
        y: g.uniform(-100.0, 700.0),
        z: g.uniform(0.0, 30.0),
        w: g.uniform(-3.0, 3.0),
    }
}

#[test]
fn ai_group_matches_upstream() {
    let cases: u64 = std::env::var("AI_GRP_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(CASES);
    let mut results = Check::new("AI group results (each operation's answer; members, count, speed, dirty, id)");
    let mut logs = Check::new("AI group calls (commands, members' calls, enter/leave, destroy, random)");
    let mut by_kind = [0u64; 75];
    let mut log_words = 0u64;
    let mut seen = [0u64; 5];
    for case in 0..cases {
        let mut g = Gen::new(0xA24_0000 + case);
        let s = gen_scene(&mut g);
        assert_eq!(grp_answer(s.seed, 7, 3, 11), unsafe { cpp::grpAnswer(s.seed, 7, 3, 11) }, "the hash");
        let n = 4 + g.below(36) as usize;
        let ops: Vec<GrpOp> = (0..n).map(|i| gen_op(&mut g, i)).collect();
        for op in &ops {
            by_kind[op.kind as usize] += 1;
        }
        let (r, rl) = rust_run(&s, &ops, &mut seen);
        let mut out = vec![0u32; 4096];
        let mut log = vec![0u32; 16384];
        let mut log_len: c_int = 0;
        let k = unsafe { cpp::ref_ai_group_run(&s, ops.as_ptr(), ops.len() as c_int, out.as_mut_ptr(), out.len() as c_int, log.as_mut_ptr(), log.len() as c_int, &mut log_len) };
        assert!(k as usize <= out.len() && log_len as usize <= log.len(), "output too long");
        out.truncate(k as usize);
        log.truncate(log_len as usize);
        log_words += log.len() as u64;
        if std::env::var("AI_GRP_DEBUG").is_ok() && (r != out || rl != log) {
            println!("case {case}: ops {:?}\n rust {r:?}\n cpp  {out:?}\n rust log {rl:?}\n cpp log  {log:?}", ops.iter().map(|o| o.kind).collect::<Vec<_>>());
        }
        let tag = || format!("case {case} ops {ops:?}");
        results.eq(r, out, tag);
        logs.eq(rl, log, tag);
    }
    println!("COVERAGE: {cases} cases; operations per kind {by_kind:?}; {log_words} log words");
    println!("COVERAGE (moves): column and formation paths given, their points, ground paths found, tightens, move commands {seen:?}");
    results.finish();
    logs.finish();
}
