#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/TurretAI.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (outOfWeaponRangeObject, AIAttackFireWeaponState)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `TurretAI` and the fire state (`logic::ai::turret`, `logic::ai::fire_state`) against the unchanged C++, `p1w13-sonnet`, layer 10
//! batch T1.
//!
//! # Method
//!
//! The C++ side is `differential/wrap_turret.cpp` (`ref_turret_run`): TurretAI.cpp (two verbatim cuts but for one implicit `int`),
//! AIStates.cpp's `outOfWeaponRangeObject` and `AIAttackFireWeaponState` (verbatim), the real TurretAI.h and the AIStateMachine.h
//! declarations, and StateMachine.cpp unchanged, on stand-ins (`differential/shim_turret`): objects with teams, players, kinds,
//! statuses, an AI and three weapons each, whose facts are the scene's, and the partition, terrain, pathfinder, random values and
//! audio answered from a hash of their arguments (the same function here). The Rust side is the real `TurretAi` on [`Host`], which
//! answers alike. Every call with an effect is a line of a log on both sides.
//!
//! Each case is a scene (2 to 6 objects, the turret on object 0) and the turret's data, then 1 to 40 operations: updates, frames,
//! object and position targets (forced or not, none), recentering, enabling, the queries, a new victim, changes to the objects (team,
//! kinds, statuses, flags, mood, goal, victim, weapons, their statuses), a save and a load of the turret; the destructor last.
//! Compared, word for word, after construction and after every operation: the results, the log, and the turret's state (angle,
//! pitch, sweep and fire frames, the victim's initial team, the target, the flags, the sound, and its machine's state, sleep, goal
//! and lock).
//!
//! # Results (2026-10-27, `TU_T1_CASES=10000`, the default)
//!
//! 10,000 cases, 318,789 operations (121,360 updates), 319,022 steps compared: **0 mismatches**. In the logs: 1,815 pre-fires, 252
//! shots at objects and 156 at positions, 59 linked-turret volleys, 13,389 rotate-condition changes, 9,410 turret reactions, 8,795
//! mood targets asked, 4,134 rotation sounds started and stopped; steps ending in the idle, idle scan, aim, fire, recenter and hold
//! states: 77,119, 1,427, 130,716, 12,968, 30,836 and 46,256. The first runs found one port bug, in `isTryingToAimAtTarget`
//! (upstream asks the target, which clears a dead one, only while aiming: its `&&` stops first), and four in the harness (the
//! save's buffer, the owner's AI and team, which upstream reads unchecked, and the Rust save's `open`). The fire state's
//! continue-range search is not reached: a turret's original victim position is always `NULL` (the attack state's batch reaches it).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 10,000 cases: 10 of 10 caught. (A first 3174, the ground-unit pitch's adjustment
//! skipped for a ground target, cannot fire: the enemy is nulled before it, so the adjustment is always made. It was moved.)
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3170 | a blocked view does not put the weapon out of range | 835 |
//! | 3171 | the goal does not follow an attack transferred to another object | 1,028 |
//! | 3172 | leaving the fire state does not cancel a pre-attack | 49 |
//! | 3173 | clearing the target does not remove the turret as its targeter | 9,138 |
//! | 3174 | the aim's pitch is not raised to the minimum pitch | 18,566 |
//! | 3175 | recentering turns at full speed, not half | 16,884 |
//! | 3176 | the continuous-fire window is 2 frames, not 3 | 7,078 |
//! | 3177 | `isTryingToAimAtTarget` asks the target (clearing a dead one) when not aiming | 187 |
//! | 3178 | a turn exactly the turn rate away snaps | 617 |
//! | 3179 | the hold lasts a frame longer | 6,964 |

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;
use std::sync::Arc;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::{CommandSourceType, WhichTurretType};
use rts_lockstep_engine::common::game_type::{ObjectId, WeaponSlotType};
use rts_lockstep_engine::common::kind_of::{KindOfType, KINDOF_BRIDGE, KINDOF_IMMOBILE, KINDOF_MINE, KINDOF_SPAWNS_ARE_THE_WEAPONS, KINDOF_STRUCTURE, KINDOF_VEHICLE};
use rts_lockstep_engine::common::model_state::ModelConditionFlagType;
use rts_lockstep_engine::common::object_status_types::{ObjectStatusTypes, OBJECT_STATUS_MASKED, OBJECT_STATUS_NO_ATTACK, OBJECT_STATUS_UNDER_CONSTRUCTION};
use rts_lockstep_engine::common::state_machine::{ObjectRef, StateContext};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::ai::fire_state::FireHost;
use rts_lockstep_engine::logic::ai::turret::{TurretAi, TurretAiData, TurretData, TurretHost};
use rts_lockstep_engine::logic::weapon_host::CanAttackResult;
use rts_lockstep_engine::logic::weapon_types::WeaponStatus;
use rts_lockstep_engine::logic::world::ids::{PlayerIndex, TeamId};
use support::{Check, Gen};

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const NONE: u32 = u32::MAX;
const OBJ_WORDS: usize = 26;
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/turret_t1.bin");

const F_DEAD: u32 = 1;
const F_DESTROYED: u32 = 2;
const F_AIRBORNE: u32 = 4;
const F_ABOVE: u32 = 8;
const F_AI: u32 = 16;
const F_GROUND: u32 = 32;
const F_PREVENTING: u32 = 64;
const F_LINKED: u32 = 128;

fn hash(seed: u32, tag: u32, words: &[u32]) -> u32 {
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

/// An object of the scene (the cfg's 26 words).
#[derive(Clone, Debug, Default)]
struct TObj {
    id: ObjectId,
    pos: Coord3D,
    team: u32,
    kinds: u32,
    status_lo: u64,
    status_hi: u64,
    flags: u32,
    contained_by: i32,
    player: i32,
    mood_idle: u32,
    mood_attack: u32,
    next_mood: u32,
    goal: i32,
    last_cmd: u32,
    victim: i32,
    cur_slot: i32,
    weapons: u32,
    wstatus: u32,
    range: [f32; 3],
    cont_range: f32,
    max_height: f32,
    turret_sync: i32,
    team_common: bool,
}

#[derive(Clone, Debug)]
struct Scene {
    seed: u32,
    rates: [u32; 3],
    frame: u32,
    objs: Vec<TObj>,
    data: TurretAiData,
    which: WhichTurretType,
}

fn status_word(o: &TObj) -> u32 {
    let b = |bit: i32| u32::from(o.status_lo >> bit & 1 != 0);
    b(OBJECT_STATUS_UNDER_CONSTRUCTION) | (b(OBJECT_STATUS_MASKED) << 1) | (b(OBJECT_STATUS_NO_ATTACK) << 2)
}

impl Scene {
    fn cfg(&self) -> Vec<u32> {
        let mut c = vec![self.seed, self.rates[0], self.rates[1], self.rates[2], self.frame, self.objs.len() as u32];
        for o in &self.objs {
            c.extend([o.id as u32, o.pos.x.to_bits(), o.pos.y.to_bits(), o.pos.z.to_bits(), o.team, o.kinds, status_word(o), o.flags]);
            c.extend([o.contained_by as u32, o.player as u32, o.mood_idle, o.mood_attack, o.next_mood, o.goal as u32, o.last_cmd, o.victim as u32]);
            c.extend([o.cur_slot as u32, o.weapons, o.wstatus]);
            c.extend([o.range[0].to_bits(), o.range[1].to_bits(), o.range[2].to_bits(), o.cont_range.to_bits(), o.max_height.to_bits()]);
            c.extend([o.turret_sync as u32, u32::from(o.team_common)]);
        }
        assert_eq!(c.len(), 6 + OBJ_WORDS * self.objs.len());
        c
    }
    fn data_words(&self) -> Vec<u32> {
        let d = &self.data;
        let mut w = vec![d.turn_rate.to_bits(), d.pitch_rate.to_bits(), d.natural_turret_angle.to_bits(), d.natural_turret_pitch.to_bits()];
        w.extend(d.turret_fire_angle_sweep.map(f32::to_bits));
        w.extend(d.turret_sweep_speed_modifier.map(f32::to_bits));
        w.extend([d.fire_pitch.to_bits(), d.min_pitch.to_bits(), d.ground_unit_pitch.to_bits(), d.turret_weapon_slots]);
        w.extend([d.min_idle_scan_angle.to_bits(), d.max_idle_scan_angle.to_bits(), d.min_idle_scan_interval, d.max_idle_scan_interval, d.recenter_time]);
        w.push(u32::from(d.initially_disabled) | (u32::from(d.fires_while_turning) << 1) | (u32::from(d.is_allows_pitch) << 2));
        w.push(self.which as u32);
        w
    }
}

/// The Rust side's world: the scene, the frame, the log, the teams' targets, the audio, the lent turrets.
struct Host {
    s: Scene,
    frame: u32,
    counter: u32,
    log: Vec<u32>,
    team_targets: BTreeMap<u32, ObjectId>,
    playing: BTreeSet<u32>,
    next_handle: u32,
    turrets: Vec<TurretData>,
    /// Each team's attack-common-target flag: its last object's in the scene (as the C++ reads them), false for a team made later.
    team_common: BTreeMap<u32, bool>,
}

impl Host {
    fn new(s: &Scene) -> Self {
        let mut team_common = BTreeMap::new();
        for o in s.objs.iter().filter(|o| o.team != 0) {
            team_common.insert(o.team, o.team_common);
        }
        Host { s: s.clone(), frame: s.frame, counter: 0, log: Vec::new(), team_targets: BTreeMap::new(), playing: BTreeSet::new(), next_handle: 0, turrets: Vec::new(), team_common }
    }
    fn line(&mut self, code: u32, words: &[u32]) {
        self.log.push(words.len() as u32 + 1);
        self.log.push(code);
        self.log.extend_from_slice(words);
    }
    fn k(&self, id: ObjectId) -> usize {
        self.s.objs.iter().position(|o| o.id == id).expect("an object")
    }
    fn o(&self, id: ObjectId) -> &TObj {
        &self.s.objs[self.k(id)]
    }
    fn by_index_plus1(&self, k: i32) -> Option<ObjectId> {
        (k > 0).then(|| self.s.objs[((k - 1) as usize) % self.s.objs.len()].id)
    }
}

impl StateContext for Host {
    fn frame(&self) -> u32 {
        self.frame
    }
    fn find_object(&self, id: ObjectId) -> Option<ObjectRef> {
        self.s.objs.iter().find(|o| o.id == id).map(|o| ObjectRef { id: o.id, position: o.pos })
    }
}

impl FireHost for Host {
    fn owner(&self) -> ObjectId {
        self.s.objs[0].id
    }
    fn ai_mood_action_adjustment(&self, obj: ObjectId, action: i32) -> u32 {
        if action == 0 { self.o(obj).mood_idle } else { self.o(obj).mood_attack }
    }
    fn ai_are_turrets_linked(&self, obj: ObjectId) -> bool {
        self.o(obj).flags & F_LINKED != 0
    }
    fn ai_current_victim(&self, obj: ObjectId) -> Option<ObjectId> {
        self.by_index_plus1(self.o(obj).victim)
    }
    fn ai_last_command_source(&self, obj: ObjectId) -> Option<CommandSourceType> {
        let o = self.o(obj);
        (o.flags & F_AI != 0).then_some(match o.last_cmd {
            0 => CommandSourceType::FromPlayer,
            1 => CommandSourceType::FromScript,
            2 => CommandSourceType::FromAi,
            3 => CommandSourceType::FromDozer,
            _ => CommandSourceType::DefaultSwitchWeapon,
        })
    }
    fn ai_is_doing_ground_movement(&self, obj: ObjectId) -> Option<bool> {
        let o = self.o(obj);
        (o.flags & F_AI != 0).then_some(o.flags & F_GROUND != 0)
    }
    fn is_kind_of(&self, obj: ObjectId, kind: KindOfType) -> bool {
        let kinds = [KINDOF_BRIDGE, KINDOF_IMMOBILE, KINDOF_SPAWNS_ARE_THE_WEAPONS, KINDOF_STRUCTURE, KINDOF_MINE, KINDOF_VEHICLE];
        kinds.iter().position(|&k| k == kind).is_some_and(|b| self.o(obj).kinds >> b & 1 != 0)
    }
    fn test_status(&self, obj: ObjectId, bit: ObjectStatusTypes) -> bool {
        let o = self.o(obj);
        if bit < 64 { o.status_lo >> bit & 1 != 0 } else { o.status_hi >> (bit - 64) & 1 != 0 }
    }
    fn set_status(&mut self, obj: ObjectId, bits: &[ObjectStatusTypes], set: bool) {
        let (mut lo, mut hi) = (0u64, 0u64);
        for &b in bits {
            if b < 64 { lo |= 1 << b } else { hi |= 1 << (b - 64) }
        }
        let k = self.k(obj);
        let o = &mut self.s.objs[k];
        if set {
            o.status_lo |= lo;
            o.status_hi |= hi;
        } else {
            o.status_lo &= !lo;
            o.status_hi &= !hi;
        }
        self.line(1, &[obj as u32, lo as u32, (lo >> 32) as u32, hi as u32, u32::from(set)]);
    }
    fn is_effectively_dead(&self, obj: ObjectId) -> bool {
        self.o(obj).flags & F_DEAD != 0
    }
    fn is_destroyed(&self, obj: ObjectId) -> bool {
        self.o(obj).flags & F_DESTROYED != 0
    }
    fn contained_by(&self, obj: ObjectId) -> Option<ObjectId> {
        self.by_index_plus1(self.o(obj).contained_by)
    }
    fn is_airborne_target(&self, obj: ObjectId) -> bool {
        self.o(obj).flags & F_AIRBORNE != 0
    }
    fn is_significantly_above_terrain(&self, obj: ObjectId) -> bool {
        self.o(obj).flags & F_ABOVE != 0
    }
    fn position(&self, obj: ObjectId) -> Coord3D {
        self.o(obj).pos
    }
    fn controlling_player(&self, obj: ObjectId) -> Option<PlayerIndex> {
        let p = self.o(obj).player;
        (p >= 0).then_some(p % 4)
    }
    fn team_attacks_common_target(&self, obj: ObjectId) -> bool {
        let t = self.o(obj).team;
        t != 0 && self.team_common.get(&t).copied().unwrap_or(false)
    }
    fn team_target_object(&self, obj: ObjectId) -> Option<ObjectId> {
        let t = self.o(obj).team;
        self.team_targets.get(&t).copied().filter(|&id| self.find_object(id).is_some())
    }
    fn set_team_target_object(&mut self, obj: ObjectId, target: ObjectId) {
        let t = self.o(obj).team;
        self.team_targets.insert(t, target);
        self.line(8, &[t, target as u32]);
    }
    fn current_weapon(&self, obj: ObjectId) -> Option<WeaponSlotType> {
        let o = self.o(obj);
        let s = o.cur_slot;
        if !(0..=2).contains(&s) || o.weapons >> s & 1 == 0 {
            return None;
        }
        Some([WeaponSlotType::Primary, WeaponSlotType::Secondary, WeaponSlotType::Tertiary][s as usize])
    }
    fn has_weapon_in_slot(&self, obj: ObjectId, slot: WeaponSlotType) -> bool {
        self.o(obj).weapons >> (slot as u32) & 1 != 0
    }
    fn weapon_status(&self, obj: ObjectId, slot: WeaponSlotType) -> WeaponStatus {
        match self.o(obj).wstatus >> (3 * slot as u32) & 7 {
            0 => WeaponStatus::ReadyToFire,
            1 => WeaponStatus::OutOfAmmo,
            2 => WeaponStatus::BetweenFiringShots,
            3 => WeaponStatus::ReloadingClip,
            _ => WeaponStatus::PreAttack,
        }
    }
    fn weapon_is_contact(&self, obj: ObjectId, slot: WeaponSlotType) -> bool {
        self.o(obj).weapons >> (3 + slot as u32) & 1 != 0
    }
    fn weapon_has_leech_range(&self, obj: ObjectId, slot: WeaponSlotType) -> bool {
        self.o(obj).weapons >> (6 + slot as u32) & 1 != 0
    }
    fn weapon_within_range_of_object(&self, obj: ObjectId, slot: WeaponSlotType, victim: ObjectId) -> bool {
        pick(hash(self.s.seed, 45, &[obj as u32, slot as u32, victim as u32]), self.s.rates[1])
    }
    fn weapon_within_range_of_position(&self, obj: ObjectId, slot: WeaponSlotType, pos: &Coord3D) -> bool {
        pick(hash(self.s.seed, 46, &[obj as u32, slot as u32, pos.x.to_bits(), pos.y.to_bits()]), self.s.rates[1])
    }
    fn weapon_continue_attack_range(&self, obj: ObjectId, _slot: WeaponSlotType) -> f32 {
        self.o(obj).cont_range
    }
    fn weapon_attack_range(&self, obj: ObjectId, slot: WeaponSlotType) -> f32 {
        self.o(obj).range[slot as usize]
    }
    fn weapon_set_pre_attack_finished_frame(&mut self, obj: ObjectId, slot: WeaponSlotType, frame: u32) {
        self.line(9, &[obj as u32, slot as u32, frame]);
    }
    fn weapon_fire_at_position(&mut self, obj: ObjectId, slot: WeaponSlotType, pos: &Coord3D) -> bool {
        let r = pick(hash(self.s.seed, 50, &[obj as u32, slot as u32, pos.x.to_bits(), pos.y.to_bits()]), 50);
        self.line(10, &[obj as u32, slot as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), u32::from(r)]);
        r
    }
    fn release_weapon_lock(&mut self, obj: ObjectId, lock: i32) {
        self.line(11, &[obj as u32, lock as u32]);
    }
    fn notify_firing_tracker_shot_fired(&mut self, obj: ObjectId, slot: WeaponSlotType, victim: ObjectId) {
        self.line(12, &[obj as u32, slot as u32, victim as u32]);
    }
    fn pre_fire_current_weapon(&mut self, obj: ObjectId, victim: Option<ObjectId>) {
        self.line(13, &[obj as u32, victim.unwrap_or(0) as u32]);
    }
    fn set_firing_condition_for_current_weapon(&mut self, obj: ObjectId) {
        self.line(14, &[obj as u32]);
    }
    fn fire_current_weapon_at_object(&mut self, obj: ObjectId, victim: Option<ObjectId>) {
        self.line(15, &[obj as u32, victim.unwrap_or(0) as u32]);
    }
    fn fire_current_weapon_at_position(&mut self, obj: ObjectId, pos: &Coord3D) {
        self.line(16, &[obj as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
    }
    fn is_attack_view_blocked(&self, obj: ObjectId, op: &Coord3D, victim: ObjectId, vp: &Coord3D) -> bool {
        pick(hash(self.s.seed, 44, &[obj as u32, op.x.to_bits(), op.y.to_bits(), victim as u32, vp.x.to_bits(), vp.y.to_bits()]), self.s.rates[0])
    }
    fn closest_attackable_of_player(&self, pos: &Coord3D, range: f32, obj: ObjectId, source: CommandSourceType, player: Option<PlayerIndex>) -> Option<ObjectId> {
        let n = self.s.objs.len() as u32;
        let words = [pos.x.to_bits(), pos.y.to_bits(), range.to_bits(), 0, 0, obj as u32, source as u32, player.map_or(NONE, |p| p as u32), obj as u32];
        let k = (hash(self.s.seed, 49, &words) >> 8) % (n + 1);
        (k < n).then(|| self.s.objs[k as usize].id)
    }
}

impl TurretHost for Host {
    fn turret_stack(&mut self) -> &mut Vec<TurretData> {
        &mut self.turrets
    }
    fn set_model_condition(&mut self, obj: ObjectId, flag: ModelConditionFlagType, set: bool) {
        self.line(2, &[obj as u32, flag as u32, u32::from(set)]);
    }
    fn react_to_turret_change(&mut self, obj: ObjectId, turret: WhichTurretType, old_angle: f32, old_pitch: f32) {
        self.line(3, &[obj as u32, turret as u32, old_angle.to_bits(), old_pitch.to_bits()]);
    }
    fn has_ai(&self, obj: ObjectId) -> bool {
        self.o(obj).flags & F_AI != 0
    }
    fn ai_next_mood_check_time(&self, obj: ObjectId) -> u32 {
        self.o(obj).next_mood
    }
    fn ai_reset_next_mood_check_time(&mut self, obj: ObjectId) {
        let k = self.k(obj);
        self.s.objs[k].next_mood = self.frame;
        self.line(4, &[obj as u32]);
    }
    fn ai_turret_sync(&self, obj: ObjectId) -> WhichTurretType {
        self.o(obj).turret_sync
    }
    fn ai_set_turret_sync(&mut self, obj: ObjectId, turret: WhichTurretType) {
        let k = self.k(obj);
        self.s.objs[k].turret_sync = turret;
        self.line(5, &[obj as u32, turret as u32]);
    }
    fn ai_goal_object(&self, obj: ObjectId) -> Option<ObjectId> {
        self.by_index_plus1(self.o(obj).goal)
    }
    fn ai_next_mood_target(&mut self, obj: ObjectId, called_by_ai: bool, called_during_idle: bool) -> Option<ObjectId> {
        let n = self.s.objs.len() as u32;
        let c = self.counter;
        self.counter += 1;
        let h = hash(self.s.seed, 48, &[obj as u32, self.frame, c, u32::from(called_by_ai), u32::from(called_during_idle)]);
        let k = (h >> 8) % (n + 2);
        let r = (k < n).then(|| self.s.objs[k as usize].id);
        self.line(19, &[obj as u32, r.unwrap_or(0) as u32]);
        r
    }
    fn ai_add_targeter(&mut self, target: ObjectId, attacker: ObjectId, add: bool) {
        self.line(6, &[target as u32, attacker as u32, u32::from(add)]);
    }
    fn ai_is_temporarily_preventing_aim_success(&self, obj: ObjectId) -> bool {
        self.o(obj).flags & F_PREVENTING != 0
    }
    fn choose_best_weapon_for_target(&mut self, obj: ObjectId, target: ObjectId) {
        self.line(7, &[obj as u32, target as u32, 0, CommandSourceType::FromAi as u32]);
    }
    fn is_able_to_attack(&self, obj: ObjectId) -> bool {
        !self.test_status(obj, OBJECT_STATUS_NO_ATTACK)
    }
    fn able_to_attack_specific_object(&self, obj: ObjectId, attack_type: i32, target: ObjectId, source: Option<CommandSourceType>) -> CanAttackResult {
        let source = source.map_or(0, |s| s as u32);
        match (hash(self.s.seed, 47, &[obj as u32, attack_type as u32, target as u32, source]) >> 8) % 4 {
            0 => CanAttackResult::NotPossible,
            1 => CanAttackResult::InvalidShot,
            2 => CanAttackResult::PossibleAfterMoving,
            _ => CanAttackResult::Possible,
        }
    }
    fn team(&self, obj: ObjectId) -> Option<TeamId> {
        let t = self.o(obj).team;
        (t != 0).then_some(t)
    }
    fn bridge_attack_points(&self, bridge: ObjectId) -> (Coord3D, Coord3D) {
        let (h1, h2) = (hash(self.s.seed, 42, &[bridge as u32]), hash(self.s.seed, 43, &[bridge as u32]));
        let p = |h: u32| Coord3D { x: ((h >> 8) % 4000) as f32 / 10.0, y: ((h >> 16) % 4000) as f32 / 10.0, z: 0.0 };
        (p(h1), p(h2))
    }
    fn distance_squared_bounding_sphere_3d(&self, obj: ObjectId, pos: &Coord3D) -> f32 {
        ((hash(self.s.seed, 41, &[obj as u32, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits(), 3]) >> 8) % 10000) as f32
    }
    #[allow(clippy::approx_constant)] // (the C++ stand-in's 3.1416f, not pi)
    fn relative_angle_2d(&self, obj: ObjectId, pos: &Coord3D) -> f32 {
        let h = hash(self.s.seed, 40, &[obj as u32, pos.x.to_bits(), pos.y.to_bits()]);
        ((h >> 8) % 62832) as f32 / 10000.0 - 3.1416
    }
    fn vector_to_center_3d(&self, obj: ObjectId, pos: &Coord3D) -> Coord3D {
        let p = self.o(obj).pos;
        Coord3D { x: pos.x - p.x, y: pos.y - p.y, z: pos.z - p.z }
    }
    fn max_height_above_position(&self, obj: ObjectId) -> f32 {
        self.o(obj).max_height
    }
    fn random_value(&mut self, lo: i32, hi: i32) -> i32 {
        let c = self.counter;
        self.counter += 1;
        let h = hash(self.s.seed, 51, &[c, lo as u32, hi as u32]);
        let r = if hi >= lo { lo + ((h >> 8) % ((hi - lo + 1) as u32)) as i32 } else { lo };
        self.line(20, &[lo as u32, hi as u32, r as u32]);
        r
    }
    fn random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        let c = self.counter;
        self.counter += 1;
        let h = hash(self.s.seed, 52, &[c, lo.to_bits(), hi.to_bits()]);
        let r = lo + ((h >> 8) % 10001) as f32 / 10000.0 * (hi - lo);
        self.line(21, &[lo.to_bits(), hi.to_bits(), r.to_bits()]);
        r
    }
    fn audio_is_playing(&self, handle: u32) -> bool {
        self.playing.contains(&handle)
    }
    fn audio_add_turret_sound(&mut self, obj: ObjectId) -> u32 {
        self.next_handle += 1;
        let h = self.next_handle;
        self.playing.insert(h);
        self.line(17, &[obj as u32, h]);
        h
    }
    fn audio_remove(&mut self, handle: u32) {
        self.playing.remove(&handle);
        self.line(18, &[handle]);
    }
}

fn dump(t: &TurretAi<Host>, v: &mut Vec<u32>) {
    let d = &t.data;
    v.extend([d.angle.to_bits(), d.pitch.to_bits(), d.enable_sweep_until, d.victim_initial_team.unwrap_or(0), d.target as u32]);
    v.extend([d.continuous_fire_expiration_frame, d.sleep_until]);
    let flags = [d.play_rot_sound, d.play_pitch_sound, d.positive_sweep, d.did_fire, d.enabled, d.fires_while_turning, d.is_force_attacking, d.target_was_set_by_idle_mood];
    v.push(flags.iter().enumerate().map(|(b, &f)| u32::from(f) << b).sum());
    v.push(d.sound_handle);
    let sm = &t.machine;
    let gp = *sm.goal_position();
    v.extend([sm.get_current_state_id(), sm.peek_sleep_till(), sm.goal_object_id() as u32, gp.x.to_bits(), gp.y.to_bits(), gp.z.to_bits(), u32::from(sm.is_locked())]);
}

fn rust_run(scene: &Scene, ops: &[[u32; 6]]) -> Vec<Vec<u32>> {
    let mut h = Host::new(scene);
    let mut out = Vec::new();
    let Ok(t) = TurretAi::new(&mut h, Arc::new(scene.data.clone()), scene.which) else {
        return vec![vec![0xbadc_0de1]];
    };
    let mut t = t.expect("the machine");
    let mut first = vec![h.log.len() as u32];
    first.append(&mut h.log);
    dump(&t, &mut first);
    out.push(first);
    let mut saved: Vec<u8> = Vec::new();
    let n = scene.objs.len() as u32;
    let mut alive = true;
    for o in ops {
        let mut res: Vec<u32> = Vec::new();
        let id_at = |k: u32| scene.objs[(k % n) as usize].id;
        match o[0] {
            0 => h.frame = o[1],
            1 => res.push(t.update_turret_ai(&mut h).expect("update").0),
            2 => t.set_turret_target_object(&mut h, (o[1] != NONE).then(|| id_at(o[1])), o[2] != 0).expect("target"),
            3 => {
                let p = Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: 0.0 };
                t.set_turret_target_position(&mut h, (o[3] == 0).then_some(&p)).expect("position")
            }
            4 => t.recenter_turret(&mut h).expect("recenter"),
            5 => {
                let hh = &h;
                t.set_turret_enabled(hh, o[1] != 0)
            }
            6 => res.push(u32::from(t.is_turret_in_natural_position(&h))),
            7 => res.push(u32::from(t.is_trying_to_aim_at_target(&mut h, id_at(o[1])))),
            8 => {
                res.push(u32::from(t.is_owners_cur_weapon_on_turret(&h)));
                let slot = [WeaponSlotType::Primary, WeaponSlotType::Secondary, WeaponSlotType::Tertiary][(o[1] % 3) as usize];
                res.push(u32::from(t.is_weapon_slot_on_turret(slot)));
            }
            9 => t.notify_new_victim_chosen(&mut h, id_at(o[1])).expect("victim"),
            10 => {
                let k = (o[1] % n) as usize;
                let v = o[3];
                let ob = &mut h.s.objs[k];
                match o[2] {
                    1 => ob.pos.x = f32::from_bits(v),
                    4 => ob.team = v,
                    5 => ob.kinds = v,
                    6 => {
                        let m = (1u64 << OBJECT_STATUS_UNDER_CONSTRUCTION) | (1 << OBJECT_STATUS_MASKED) | (1 << OBJECT_STATUS_NO_ATTACK);
                        ob.status_lo &= !m;
                        if v & 1 != 0 {
                            ob.status_lo |= 1 << OBJECT_STATUS_UNDER_CONSTRUCTION;
                        }
                        if v & 2 != 0 {
                            ob.status_lo |= 1 << OBJECT_STATUS_MASKED;
                        }
                        if v & 4 != 0 {
                            ob.status_lo |= 1 << OBJECT_STATUS_NO_ATTACK;
                        }
                    }
                    7 => ob.flags = v,
                    10 => ob.mood_idle = v,
                    11 => ob.mood_attack = v,
                    12 => ob.next_mood = v,
                    13 => ob.goal = v as i32,
                    15 => ob.victim = v as i32,
                    16 => ob.cur_slot = v as i32,
                    17 => ob.weapons = v,
                    18 => ob.wstatus = v,
                    _ => {}
                }
            }
            11 => {
                let mut x = XferSave::new();
                x.open(b"turret_t1.bin").expect("open");
                let st = t.xfer(&mut x).is_err();
                let _ = x.close();
                saved = x.bytes().to_vec();
                res.push(u32::from(st));
                res.push(saved.len() as u32);
                res.extend(saved.iter().map(|&b| u32::from(b)));
            }
            12 => {
                let mut x = XferLoad::new(saved.clone());
                x.open(b"turret_t1.bin").expect("open");
                let st = t.xfer(&mut x as &mut dyn Xfer).is_err();
                let _ = x.close();
                t.load_post_process(&h);
                res.push(u32::from(st));
            }
            13 => {
                alive = false;
            }
            _ => unreachable!(),
        }
        if !alive {
            let tt = std::mem::replace(&mut t, TurretAi::new(&mut Host::new(scene), Arc::new(scene.data.clone()), scene.which).expect("data").expect("machine"));
            tt.destroy(&mut h).expect("destroy");
        }
        let mut v = vec![res.len() as u32];
        v.extend(&res);
        v.push(h.log.len() as u32);
        v.append(&mut h.log);
        if alive {
            dump(&t, &mut v);
        }
        out.push(v);
        if !alive {
            break;
        }
    }
    out
}

fn cpp_run(scene: &Scene, ops: &[[u32; 6]]) -> Vec<Vec<u32>> {
    let cfg = scene.cfg();
    let data = scene.data_words();
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let path = CString::new(XFER_FILE_PATH).expect("path");
    let mut buf = vec![0u32; 1 << 20];
    let mut len = 0i32;
    let st = unsafe { cpp::ref_turret_run(cfg.as_ptr(), data.as_ptr(), ops.len() as i32, flat.as_ptr(), path.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len) };
    assert_eq!(st, 0, "the dump fits");
    buf.truncate(len as usize);
    if buf.first() == Some(&0xbadc_0de1) {
        return vec![vec![0xbadc_0de1]];
    }
    let mut out = Vec::new();
    let mut p = 0;
    while p < buf.len() {
        let n = buf[p] as usize;
        out.push(buf[p + 1..p + 1 + n].to_vec());
        p += 1 + n;
    }
    out
}

fn gen_scene(g: &mut Gen) -> Scene {
    let n = 2 + g.below(5) as usize;
    let objs = (0..n)
        .map(|k| {
            let p = |g: &mut Gen, pct: u32| g.below(100) < pct;
            let mut o = TObj {
                id: 10 + k as i32 * 3,
                pos: Coord3D { x: g.uniform(0.0, 300.0), y: g.uniform(0.0, 300.0), z: g.uniform(0.0, 20.0) },
                team: if p(g, 85) { 1 + g.below(3) } else { 0 },
                kinds: [0, 0, 0, 1, 2, 4, 8, 16, 32, 33][g.below(10) as usize],
                status_lo: 0,
                status_hi: 0,
                flags: 0,
                contained_by: if p(g, 15) { 1 + g.below(n as u32) as i32 } else { 0 },
                player: if p(g, 85) { g.below(4) as i32 } else { -1 },
                mood_idle: [1, 1, 1, 0x10, 0x20, 0x41][g.below(6) as usize],
                mood_attack: [1, 1, 1, 0, 2, 0x81][g.below(6) as usize],
                next_mood: g.below(60),
                goal: if p(g, 40) { 1 + g.below(n as u32) as i32 } else { 0 },
                last_cmd: g.below(5),
                victim: if p(g, 40) { 1 + g.below(n as u32) as i32 } else { 0 },
                cur_slot: [0, 0, 1, 2, 3][g.below(5) as usize],
                weapons: g.below(512) | if p(g, 70) { 1 } else { 0 },
                wstatus: (0..3).map(|s| [0u32, 0, 0, 2, 3, 4, 1][g.below(7) as usize] << (3 * s)).sum(),
                range: [g.uniform(0.0, 200.0), g.uniform(0.0, 200.0), g.uniform(0.0, 200.0)],
                cont_range: if p(g, 40) { g.uniform(1.0, 80.0) } else { 0.0 },
                max_height: g.uniform(0.0, 30.0),
                turret_sync: [-1, -1, 0, 1][g.below(4) as usize],
                team_common: p(g, 30),
            };
            if p(g, 10) {
                o.status_lo |= 1 << OBJECT_STATUS_UNDER_CONSTRUCTION;
            }
            if p(g, 10) {
                o.status_lo |= 1 << OBJECT_STATUS_MASKED;
            }
            if p(g, 10) {
                o.status_lo |= 1 << OBJECT_STATUS_NO_ATTACK;
            }
            for (bit, pct) in [(F_DEAD, 10), (F_DESTROYED, 5), (F_AIRBORNE, 20), (F_ABOVE, 20), (F_AI, 75), (F_GROUND, 60), (F_PREVENTING, 10), (F_LINKED, 15)] {
                if p(g, pct) {
                    o.flags |= bit;
                }
            }
            o
        })
        .collect::<Vec<_>>();
    let mut objs = objs;
    // (the owner has a team: the fire state reads it unchecked)
    if objs[0].team == 0 {
        objs[0].team = 1;
    }
    // (the owner has an AI: a turret is its AIUpdate's, and upstream reads the owner's AI unchecked)
    objs[0].flags |= F_AI;
    // (most owners can fire: a ready current weapon in slot 0, the attack mood, able to attack; so the fire state is reached)
    let ready = g.below(10) < 7;
    if ready {
        objs[0].cur_slot = 0;
        objs[0].weapons |= 1;
        objs[0].wstatus &= !7;
        objs[0].mood_attack = 1;
        objs[0].status_lo &= !(1 << OBJECT_STATUS_NO_ATTACK);
        objs[0].flags &= !F_PREVENTING;
    }
    if g.below(10) != 0 {
        objs[0].flags &= !F_DEAD;
    }
    let p = |g: &mut Gen, pct: u32| g.below(100) < pct;
    let mut data = TurretAiData {
        turn_rate: [0.01, 0.05, 0.3, 0.7, 1.5, 3.2, 3.2][g.below(7) as usize],
        pitch_rate: [0.01, 0.05, 0.3, 1.0, 3.2][g.below(5) as usize],
        natural_turret_angle: if p(g, 50) { 0.0 } else { g.uniform(-3.0, 3.0) },
        natural_turret_pitch: if p(g, 70) { 0.0 } else { g.uniform(-0.5, 0.8) },
        ..TurretAiData::default()
    };
    for s in 0..3 {
        data.turret_fire_angle_sweep[s] = if p(g, 30) { g.uniform(0.05, 0.6) } else { 0.0 };
        data.turret_sweep_speed_modifier[s] = if p(g, 30) { g.uniform(0.3, 2.0) } else { 1.0 };
    }
    data.fire_pitch = if p(g, 20) { g.uniform(0.1, 0.9) } else { 0.0 };
    data.min_pitch = if p(g, 40) { g.uniform(-0.4, 0.3) } else { 0.0 };
    data.ground_unit_pitch = if p(g, 40) { g.uniform(0.05, 0.5) } else { 0.0 };
    data.turret_weapon_slots = if p(g, 3) { 0 } else { 1 + g.below(7) };
    if ready && data.turret_weapon_slots != 0 {
        data.turret_weapon_slots |= 1;
    }
    if p(g, 60) {
        data.min_idle_scan_angle = g.uniform(0.0, 1.0);
        data.max_idle_scan_angle = data.min_idle_scan_angle + g.uniform(0.0, 1.5);
    }
    if p(g, 70) {
        data.min_idle_scan_interval = g.below(40);
        data.max_idle_scan_interval = data.min_idle_scan_interval + g.below(60);
    }
    data.recenter_time = [0, 5, 30, 60][g.below(4) as usize];
    data.initially_disabled = p(g, 10);
    data.fires_while_turning = p(g, 30);
    data.is_allows_pitch = p(g, 50);
    let in_range = if ready { 50 + g.below(51) } else { g.below(101) };
    Scene { seed: g.u32(), rates: [g.below(101), in_range, 0], frame: g.below(100), objs, data, which: [0, 1][g.below(2) as usize] }
}

fn gen_op(g: &mut Gen, sc: &Scene, frame: &mut u32) -> [u32; 6] {
    let n = sc.objs.len() as u32;
    match g.below(100) {
        0..=39 => [1, 0, 0, 0, 0, 0],
        40..=44 => {
            let span = if g.below(4) == 0 { 60 } else { 4 };
            *frame += 1 + g.below(span);
            [0, *frame, 0, 0, 0, 0]
        }
        45..=56 => [2, if g.below(8) == 0 { NONE } else { g.below(n) }, g.below(4) / 3, 0, 0, 0],
        57..=62 => [3, g.uniform(0.0, 300.0).to_bits(), g.uniform(0.0, 300.0).to_bits(), u32::from(g.below(6) == 0), 0, 0],
        63..=65 => [4, 0, 0, 0, 0, 0],
        66..=68 => [5, u32::from(g.below(4) != 0), 0, 0, 0, 0],
        69..=71 => [6, 0, 0, 0, 0, 0],
        72..=74 => [7, g.below(n), 0, 0, 0, 0],
        75..=76 => [8, g.below(3), 0, 0, 0, 0],
        77..=78 => [9, g.below(n), 0, 0, 0, 0],
        79..=94 => {
            let k = g.below(n);
            let (field, v) = match g.below(12) {
                0 => (1, g.uniform(0.0, 300.0).to_bits()),
                1 => (4, if k == 0 { 1 + g.below(3) } else { g.below(4) }),
                2 => (5, [0, 1, 2, 4, 8, 16, 32][g.below(7) as usize]),
                3 => (6, g.below(8)),
                4 => (7, g.below(256) | if k == 0 { F_AI } else { 0 }),
                5 => (10, [1, 0x10, 0x20][g.below(3) as usize]),
                6 => (11, [1, 0, 2][g.below(3) as usize]),
                7 => (12, *frame + g.below(30)),
                8 => (13, g.below(n + 1)),
                9 => (15, g.below(n + 1)),
                10 => (16, g.below(4)),
                _ => (18, (0..3).map(|s| [0u32, 0, 2, 3, 4, 1][g.below(6) as usize] << (3 * s)).sum()),
            };
            [10, k, field, v, 0, 0]
        }
        95..=97 => [11, 0, 0, 0, 0, 0],
        _ => [12, 0, 0, 0, 0, 0],
    }
}

#[test]
fn turret_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("TU_T1_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(10_000);
    let mut g = Gen::new(0x0071_7A11);
    let mut check = Check::new("T1 turret");
    let (mut ops_run, mut updates) = (0u64, 0u64);
    let mut calls = [0u64; 24];
    let mut states = [0u64; 7];
    for case in 0..cases {
        let scene = gen_scene(&mut g);
        let n = 1 + g.below(60) as usize;
        let mut frame = scene.frame;
        let mut ops: Vec<[u32; 6]> = Vec::new();
        let mut has_save = false;
        while ops.len() < n {
            let o = gen_op(&mut g, &scene, &mut frame);
            if o[0] == 12 && !has_save {
                continue;
            }
            has_save |= o[0] == 11;
            ops.push(o);
        }
        ops.push([13, 0, 0, 0, 0, 0]);
        let r = rust_run(&scene, &ops);
        let c = cpp_run(&scene, &ops);
        for (i, a) in r.iter().enumerate() {
            let b = c.get(i).cloned().unwrap_or_default();
            if std::env::var("TU_DEBUG").is_ok() && *a != b {
                let k = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
                let op = if i == 0 { None } else { ops.get(i - 1) };
                panic!("case {case} step {i} op {op:?}: first difference at {k}: rust {a:?} c++ {b:?}; ops {:?}; scene {scene:?}", &ops[..i.min(ops.len())]);
            }
            if i > 0 && a.len() > 2 {
                let op = ops[i - 1][0];
                updates += u64::from(op == 1);
                // (the log: after the results; each line its length, code, words)
                let res_len = a[0] as usize;
                let log_len = a[1 + res_len] as usize;
                let mut p = 2 + res_len;
                while p < 2 + res_len + log_len {
                    let code = a[p + 1] as usize;
                    calls[code.min(calls.len() - 1)] += 1;
                    p += 1 + a[p] as usize;
                }
                if let Some(&st) = a.get(2 + res_len + log_len + 9) {
                    states[(st as usize).min(6)] += 1;
                }
            }
            check.eq(a.clone(), b, || format!("case {case} step {i}"));
        }
        if r.len() != c.len() {
            check.eq(vec![r.len() as u32], vec![c.len() as u32], || format!("case {case}: the step count"));
        }
        ops_run += ops.len() as u64;
    }
    println!("T1: {cases} cases, {ops_run} ops ({updates} updates); calls by log code {calls:?}; steps ending in each state {states:?}");
    check.finish();
}
