// SPDX-License-Identifier: GPL-3.0-only
//! Differential test of the AI player (`logic::ai::ai_player`, Phase 1 layer 10, `p1w13-sonnet`, batch A28) against AIPlayer.cpp,
//! cut verbatim with AIPlayer.h and GameCommon.h's intrusive lists and compiled over a world of the wrapper's own
//! (`differential/wrap_ai_player.cpp`).
//!
//! Each case is one scene ([`ApScene`]: objects with their AI, supply truck, dozer, production, body, dock, warehouse, rebuild
//! hole, exit and command set; templates; team prototypes and teams; three players with their money, relationships and default
//! teams; player 0's build list; the AI data's timers, resources and sides) and a run of operations ([`ApOp`]) on a new AI player
//! for player 0: every public call (update, newMap, the produced callbacks, the team and building requests, the supply and safety
//! questions, the superweapon target, the bounds, xfer save and load), the protected steps called directly, and changes to the
//! world between them (frames, deaths, flags, money). The answers the world gives that the scene does not hold are one hash of
//! (kind, a, b) both sides compute (`apAnswer`); the host here answers every question by the wrapper's rules, rule for rule.
//! Compared: each operation's answer words, then the AI player (its queues with their work orders, timers, repair queue and
//! base) and the world (objects, teams, players, the next ids, the unit id, the random state, player 0's build list) after it;
//! and the log of every call into the world that changes something or whose order matters (debug messages, scripts, AI commands,
//! team changes, the build assistant's legality questions and new objects, the random values).
//!
//! Not generated: the calls upstream dereferences null for (a missing unit or building for the produced callbacks, a missing team
//! for `guardSupplyCenter` and `buildSpecificBuildingNearestTeam`, a template no template has for `buildBySupplies`): the op
//! answers `u32::MAX` and does nothing on both sides when its object or team is missing. Nor the worlds it does: a dozer, or a
//! building's builder, without an AI (`updateBridgeRepair` and `processBaseBuilding` reach it unchecked), a recruit without a
//! team.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D, Region2D};
use rts_lockstep_engine::common::game_common::{GameDifficulty, Relationship};
use rts_lockstep_engine::common::dict::Dict;
use rts_lockstep_engine::common::game_type::{ObjectId, INVALID_ID};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::object_status_types::*;
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::science::ScienceType;
use rts_lockstep_engine::common::special_power_type::{NUKE_SPECIAL_CLUSTER_MINES, SPECIAL_CLUSTER_MINES, SPECIAL_SNEAK_ATTACK};
use rts_lockstep_engine::common::well_known_keys::WellKnownKeys;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::differential::{ApOp, ApScene, AP_BUILD, AP_OBJECTS, AP_PLAYERS, AP_PROTOS, AP_SIDES, AP_TEAMS, AP_TEMPLATES, AP_UPGRADES};
use rts_lockstep_engine::logic::ai::ai_data::{AiDataEntry, AiSideBuildList, AiSideInfo};
use rts_lockstep_engine::logic::ai::ai_player::{
    AiPlayer, AiPlayerHost, ClosestQuery, PlayerAiCommand, PlayerRef, ProtoRef, TeamRef, TemplateRef, UpgradeRef, WorkOrder,
};
use rts_lockstep_engine::logic::sides_list::BuildListInfo;
use rts_lockstep_engine::logic::world::team::TeamTemplateInfo;
use std::os::raw::c_int;
use support::{Check, Gen};

const CASES: u64 = 100_000;

// the scene's object flags (wrap_ai_player.cpp's AP_F_*)
const F_AI: i32 = 1;
const F_IDLE: i32 = 2;
const F_SUPPLY_AI: i32 = 4;
const F_FERRYING: i32 = 8;
const F_FORCED: i32 = 16;
const F_DOZER_AI: i32 = 32;
const F_TASK: i32 = 64;
const F_TASK_BUILD: i32 = 128;
const F_PRODUCTION: i32 = 256;
const F_BODY: i32 = 512;
const F_LAST_DAMAGE: i32 = 1024;
const F_NO_EFFECT: i32 = 2048;
const F_CENTER_DOCK: i32 = 4096;
const F_WAREHOUSE: i32 = 8192;
const F_HOLE: i32 = 16384;
const F_EXIT: i32 = 32768;
const F_RALLY: i32 = 65536;
const F_ABOVE: i32 = 131072;
const F_DEAD: i32 = 262144;
const F_INSIGNIFICANT: i32 = 524288;
const F_UNMANNED: i32 = 1048576;
const F_MUTABLE: i32 =
    F_IDLE | F_FERRYING | F_FORCED | F_TASK | F_TASK_BUILD | F_LAST_DAMAGE | F_NO_EFFECT | F_RALLY | F_ABOVE | F_DEAD | F_INSIGNIFICANT | F_UNMANNED;
// the prototypes' flags (AP_P_*), the players' (AP_PL_*), the build entries' (AP_B_*)
const P_REINFORCE: i32 = 1;
const P_HOME: i32 = 2;
const P_ACTIONS: i32 = 4;
const P_SINGLETON: i32 = 8;
const P_CONDITION: i32 = 16;
const PL_UNITS: i32 = 1;
const PL_BASE: i32 = 2;
const PL_POWER: i32 = 4;
const PL_HUMAN: i32 = 8;
const PL_SKIRMISH: i32 = 16;
const B_INITIALLY: i32 = 1;
const B_SUPPLY: i32 = 2;
const B_UNDER_CONSTRUCTION: i32 = 4;
const B_UNSELLABLE: i32 = 8;
const B_PRIORITY: i32 = 16;
const B_MANUAL: i32 = 32;

// the log's entries (AP_LOG_*) and AI commands (AP_C_*)
const LOG_RANDOM: u32 = 1;
const LOG_DEBUG: u32 = 2;
const LOG_CACHE: u32 = 3;
const LOG_SCRIPT: u32 = 4;
const LOG_ACTION: u32 = 5;
const LOG_CLEAR_FLAGS: u32 = 6;
const LOG_CAN_BUILD: u32 = 7;
const LOG_PURCHASE: u32 = 8;
const LOG_ADD_BUILD: u32 = 9;
const LOG_ADD_PRIORITY: u32 = 10;
const LOG_SET_POS: u32 = 11;
const LOG_SET_TEAM: u32 = 12;
const LOG_CLEAR_STATUS: u32 = 13;
const LOG_MAP_PROPS: u32 = 14;
const LOG_UPGRADE_MODULES: u32 = 15;
const LOG_AI: u32 = 16;
const LOG_FORCE_WANTING: u32 = 17;
const LOG_QUEUE_UNIT: u32 = 18;
const LOG_QUEUE_UPGRADE: u32 = 19;
const LOG_TEAM_ACTIVE: u32 = 20;
const LOG_TEAM_TRANSFER: u32 = 21;
const LOG_TEAM_DELETE: u32 = 22;
const LOG_TEAM_GUARD: u32 = 23;
const LOG_TEAM_CREATE: u32 = 24;
const LOG_BUILD_NOW: u32 = 25;
const LOG_LEGAL: u32 = 26;
const LOG_BIBS: u32 = 27;
const LOG_ADJUST: u32 = 28;
const LOG_CLEAR_BUILDER: u32 = 29;
const LOG_UNDONE: u32 = 30;
const C_DOCK: u32 = 1;
const C_IDLE: u32 = 2;
const C_MOVE: u32 = 3;
const C_RESUME: u32 = 4;
const C_REPAIR: u32 = 5;
const C_EXIT_PATH: u32 = 6;
const C_JOIN: u32 = 7;
// CommandSourceType
const CMD_FROM_PLAYER: u32 = 0;
const CMD_FROM_SCRIPT: u32 = 1;
const CMD_FROM_AI: u32 = 2;

/// The kinds' bits in a template's `kinds` (the wrapper's KindOfType).
fn kind_bit(k: KindOfType) -> u32 {
    match k {
        KINDOF_STRUCTURE => 0,
        KINDOF_DOZER => 1,
        KINDOF_HARVESTER => 2,
        KINDOF_REBUILD_HOLE => 3,
        KINDOF_SUPPLY_SOURCE => 4,
        KINDOF_CASH_GENERATOR => 5,
        KINDOF_COMMANDCENTER => 6,
        KINDOF_FS_SUPERWEAPON => 7,
        KINDOF_FS_BASE_DEFENSE => 8,
        KINDOF_TECH_BASE_DEFENSE => 9,
        KINDOF_VEHICLE => 10,
        KINDOF_INFANTRY => 11,
        KINDOF_AIRCRAFT => 12,
        KINDOF_CONSERVATIVE_BUILDING => 13,
        KINDOF_BRIDGE => 14,
        KINDOF_FS_POWER => 15,
        _ => panic!("a kind the wrapper has no bit for: {k}"),
    }
}

/// The status bits (the wrapper's ObjectStatusTypes).
fn status_bit(s: ObjectStatusTypes) -> u32 {
    match s {
        OBJECT_STATUS_UNDER_CONSTRUCTION => 0,
        OBJECT_STATUS_SOLD => 1,
        OBJECT_STATUS_RECONSTRUCTING => 2,
        OBJECT_STATUS_STEALTHED => 3,
        OBJECT_STATUS_DETECTED => 4,
        OBJECT_STATUS_DISGUISED => 5,
        _ => panic!("a status the wrapper has no bit for: {s}"),
    }
}

fn bits(f: f32) -> u32 {
    f.to_bits()
}

fn name_of(prefix: &str, i: i32) -> Vec<u8> {
    format!("{prefix}{i}").into_bytes()
}

/// The wrapper's `indexOf`: `prefix` then digits, each prefix of the number below `n`.
fn index_of(name: &[u8], prefix: &str, n: i32) -> i32 {
    let p = prefix.as_bytes();
    if !name.starts_with(p) || name.len() == p.len() {
        return -1;
    }
    let mut v: i32 = 0;
    for &c in &name[p.len()..] {
        if !c.is_ascii_digit() {
            return -1;
        }
        v = v * 10 + (c - b'0') as i32;
        if v >= n {
            return -1;
        }
    }
    v
}

fn script_name(i: i32) -> Vec<u8> {
    if i < 0 {
        Vec::new()
    } else {
        name_of("Cond", i)
    }
}

/// `(Int)f`, x86: the indefinite value out of range.
fn c_int_of(f: f32) -> i32 {
    if f.is_nan() || f64::from(f) < -2_147_483_648.0 || f64::from(f) >= 2_147_483_648.0 {
        return i32::MIN;
    }
    f as i32
}

/// A build list entry of the scene (the wrapper's `entryOf`).
fn entry_of(e: &cpp::ApBuild) -> BuildListInfo {
    BuildListInfo {
        building_name: if e.name < 0 { Vec::new() } else { name_of("Bldg", e.name) },
        template_name: match e.tmpl {
            -1 => Vec::new(),
            t if t < 0 || t >= AP_TEMPLATES as i32 => b"Nothing".to_vec(),
            t => name_of("Tmpl", t),
        },
        script: script_name(e.script),
        location: Coord3D { x: e.loc[0], y: e.loc[1], z: e.loc[2] },
        angle: e.angle,
        rally_point_offset: Coord2D { x: e.rally[0], y: e.rally[1] },
        is_initially_built: e.flags & B_INITIALLY != 0,
        num_rebuilds: e.num_rebuilds as u32,
        health: e.health,
        unsellable: e.flags & B_UNSELLABLE != 0,
        under_construction: e.flags & B_UNDER_CONSTRUCTION != 0,
        is_supply_building: e.flags & B_SUPPLY != 0,
        priority_build: e.flags & B_PRIORITY != 0,
        automatically_build: e.flags & B_MANUAL == 0,
        object_id: e.object_id,
        object_timestamp: e.timestamp as u32,
        desired_gatherers: e.desired,
        current_gatherers: e.current,
        ..Default::default()
    }
}

/// The world, by the wrapper's rules.
struct Host {
    w: ApScene,
    seed: u32,
    rand: u32,
    unit_id: u32,
    /// Player 0's build list, head first; the others' are empty.
    build: Vec<BuildListInfo>,
    empty: Vec<BuildListInfo>,
    log: Vec<u32>,
    /// A deleted team was used (undefined upstream): the run is not compared.
    stale: std::cell::Cell<bool>,
}

impl Host {
    fn new(s: &ApScene) -> Self {
        let build = s.b[..s.num_build as usize].iter().map(entry_of).collect();
        Host { w: *s, seed: s.seed as u32, rand: (s.seed as u32) ^ 0x5bd1_e995, unit_id: 0, build, empty: Vec::new(), log: Vec::new(), stale: std::cell::Cell::new(false) }
    }

    fn chance(&self, kind: u32, a: u32, b: u32, pct: u32) -> bool {
        ap_answer(self.seed, kind, a, b) % 100 < pct
    }

    fn log_str(&mut self, kind: u32, s: &[u8]) {
        self.log.push(kind);
        self.log.push(s.len() as u32);
        self.log.extend(s.iter().map(|&c| c as u32));
    }

    fn slot(&self, o: ObjectId) -> usize {
        self.try_slot(o).unwrap_or_else(|| panic!("no object {o}"))
    }

    fn try_slot(&self, o: ObjectId) -> Option<usize> {
        if o == INVALID_ID {
            return None;
        }
        (0..AP_OBJECTS).find(|&i| self.w.o[i].id == o)
    }

    fn team_slot(&self, t: TeamRef) -> Option<usize> {
        if t == 0 {
            return None;
        }
        (0..AP_TEAMS).find(|&k| self.w.tm[k].id as u32 == t)
    }

    /// A team's slot. A deleted team still named (upstream's dangling `Team *`: `aiPreTeamDestroy` skips the queue's new head
    /// after a removal) marks the run stale and answers with slot 0.
    fn tk(&self, t: TeamRef) -> usize {
        self.team_slot(t).unwrap_or_else(|| {
            self.stale.set(true);
            0
        })
    }

    fn has(&self, o: ObjectId, f: i32) -> bool {
        self.w.o[self.slot(o)].flags & f != 0
    }

    fn kinds_of_slot(&self, i: usize) -> u32 {
        self.w.t[self.w.o[i].tmpl as usize].kinds as u32
    }

    fn members(&self, t: TeamRef) -> Vec<usize> {
        if t == 0 {
            return Vec::new();
        }
        (0..AP_OBJECTS).filter(|&i| self.w.o[i].id != 0 && self.w.o[i].team as u32 == t).collect()
    }

    /// Whether a member of an instance of the player's prototypes passes `f` (the wrapper's `anyMember`).
    fn any_member(&self, p: PlayerRef, f: impl Fn(&cpp::ApObj) -> bool) -> bool {
        for t in &self.w.tm {
            if t.id == 0 || self.w.pr[t.proto as usize].player != p {
                continue;
            }
            if self.w.o.iter().any(|o| o.id != 0 && o.team == t.id && f(o)) {
                return true;
            }
        }
        false
    }

    fn proto_player(&self, t: TeamRef) -> i32 {
        self.w.pr[self.w.tm[self.tk(t)].proto as usize].player
    }

    fn off_map(&self, i: usize) -> bool {
        let (o, e) = (&self.w.o[i], &self.w.extent);
        !(e[0] <= o.pos[0] && o.pos[0] <= e[2] && e[1] <= o.pos[1] && o.pos[1] <= e[3])
    }

    fn relationship_by_team_id(&self, p: PlayerRef, team: i32) -> Relationship {
        match self.team_slot(team as u32) {
            None => Relationship::Neutral,
            Some(k) => match self.w.rel[p as usize][self.w.pr[self.w.tm[k].proto as usize].player as usize] {
                0 => Relationship::Enemies,
                1 => Relationship::Neutral,
                _ => Relationship::Allies,
            },
        }
    }
}

fn ap_answer(seed: u32, kind: u32, a: u32, b: u32) -> u32 {
    let mut x = seed ^ kind.wrapping_mul(0x85eb_ca6b);
    x ^= a;
    x = x.wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x ^= b;
    x = x.wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x
}

impl AiPlayerHost for Host {
    fn frame(&mut self) -> u32 {
        self.w.frame as u32
    }
    fn logic_random_value(&mut self, lo: i32, hi: i32) -> i32 {
        self.rand = self.rand.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let r = lo.wrapping_add(((self.rand >> 8) % (hi.wrapping_sub(lo).wrapping_add(1) as u32)) as i32);
        self.log.extend([LOG_RANDOM, lo as u32, hi as u32, r as u32]);
        r
    }
    fn debug_ai(&mut self) -> bool {
        self.w.debug_ai != 0
    }
    fn base_value_per_supply_box(&mut self) -> i32 {
        self.w.box_value
    }
    fn append_debug_message(&mut self, msg: &[u8]) {
        self.log_str(LOG_DEBUG, msg);
    }
    fn add_object_to_cache(&mut self, o: ObjectId) {
        self.log.extend([LOG_CACHE, o as u32]);
    }
    fn run_object_script(&mut self, script: &[u8], o: ObjectId) {
        self.log_str(LOG_SCRIPT, script);
        self.log.push(o as u32);
    }
    fn find_script(&mut self, name: &[u8]) -> (bool, bool) {
        let k = index_of(name, "Cond", 32);
        if k < 0 || !self.chance(88, k as u32, 0, 70) {
            return (false, false);
        }
        (true, self.chance(89, k as u32, 0, 60))
    }
    fn execute_action(&mut self, script: &[u8], team: TeamRef) {
        let k = index_of(script, "Cond", 32);
        self.log.extend([LOG_ACTION, k as u32, team]);
    }
    fn clear_team_flags(&mut self) {
        self.log.push(LOG_CLEAR_FLAGS);
    }
    fn skirmish_enemy_player(&mut self) -> PlayerRef {
        self.w.enemy
    }
    fn player_exists(&mut self, p: PlayerRef) -> bool {
        (0..AP_PLAYERS as i32).contains(&p)
    }
    fn money(&mut self, p: PlayerRef) -> u32 {
        self.w.pl[p as usize].money as u32
    }
    fn can_build_units(&mut self, p: PlayerRef) -> bool {
        self.w.pl[p as usize].flags & PL_UNITS != 0
    }
    fn set_can_build_units(&mut self, p: PlayerRef, b: bool) {
        self.log.extend([LOG_CAN_BUILD, p as u32, b as u32]);
        if b {
            self.w.pl[p as usize].flags |= PL_UNITS;
        } else {
            self.w.pl[p as usize].flags &= !PL_UNITS;
        }
    }
    fn can_build_base(&mut self, p: PlayerRef) -> bool {
        self.w.pl[p as usize].flags & PL_BASE != 0
    }
    fn side(&mut self, p: PlayerRef) -> Vec<u8> {
        name_of("Side", self.w.pl[p as usize].side)
    }
    fn attacked_frame(&mut self, p: PlayerRef) -> u32 {
        self.w.pl[p as usize].attacked_frame as u32
    }
    fn science_purchase_points(&mut self, p: PlayerRef) -> i32 {
        self.w.pl[p as usize].science_points
    }
    fn is_capable_of_purchasing_science(&mut self, p: PlayerRef, s: ScienceType) -> bool {
        self.chance(85, p as u32, s.0 as u32, 60)
    }
    fn attempt_to_purchase_science(&mut self, p: PlayerRef, s: ScienceType) -> bool {
        let ok = self.chance(86, p as u32, s.0 as u32, 50);
        self.log.extend([LOG_PURCHASE, p as u32, s.0 as u32, ok as u32]);
        ok
    }
    fn science_internal_name(&mut self, s: ScienceType) -> Vec<u8> {
        name_of("Sci", s.0)
    }
    fn player_name(&mut self, p: PlayerRef) -> Vec<u8> {
        name_of("Player", p)
    }
    fn has_upgrade_in_production(&mut self, p: PlayerRef, u: UpgradeRef) -> bool {
        self.chance(83, p as u32, u, 25)
    }
    fn has_upgrade_complete(&mut self, p: PlayerRef, u: UpgradeRef) -> bool {
        self.chance(84, p as u32, u, 25)
    }
    fn default_team(&mut self, p: PlayerRef) -> TeamRef {
        let id = self.w.pl[p as usize].default_team as u32;
        if self.team_slot(id).is_some() {
            id
        } else {
            0
        }
    }
    fn relationship_to_team(&mut self, p: PlayerRef, team: TeamRef) -> Relationship {
        self.relationship_by_team_id(p, team as i32)
    }
    fn player_teams(&mut self, p: PlayerRef) -> Vec<ProtoRef> {
        (0..AP_PROTOS as u32).filter(|&i| self.w.pr[i as usize].player == p).collect()
    }
    fn build_list(&mut self, p: PlayerRef) -> &mut Vec<BuildListInfo> {
        if p == 0 {
            &mut self.build
        } else {
            &mut self.empty
        }
    }
    fn add_to_build_list(&mut self, p: PlayerRef, o: ObjectId) {
        assert_eq!(p, 0, "only player 0 has a build list here");
        self.log.extend([LOG_ADD_BUILD, p as u32, o as u32]);
        let i = self.slot(o);
        let ob = self.w.o[i];
        let b = BuildListInfo {
            object_id: o,
            template_name: name_of("Tmpl", ob.tmpl),
            location: Coord3D { x: ob.pos[0], y: ob.pos[1], z: ob.pos[2] },
            angle: (ob.id % 8) as f32 * 0.5,
            num_rebuilds: 0,
            ..Default::default()
        };
        self.build.insert(0, b);
    }
    fn add_to_priority_build_list(&mut self, p: PlayerRef, name: &[u8], loc: &Coord3D, angle: f32) {
        assert_eq!(p, 0, "only player 0 has a build list here");
        self.log.extend([LOG_ADD_PRIORITY, p as u32, bits(loc.x), bits(loc.y), bits(loc.z), bits(angle)]);
        let b = BuildListInfo { template_name: name.to_vec(), location: *loc, angle, priority_build: true, num_rebuilds: 1, ..Default::default() };
        self.build.insert(0, b);
    }
    fn find_template(&mut self, name: &[u8]) -> Option<TemplateRef> {
        let i = index_of(name, "Tmpl", AP_TEMPLATES as i32);
        (i >= 0).then_some(i as u32)
    }
    fn first_template(&mut self) -> Option<TemplateRef> {
        Some(0)
    }
    fn next_template(&mut self, t: TemplateRef) -> Option<TemplateRef> {
        ((t as usize + 1) < AP_TEMPLATES).then_some(t + 1)
    }
    fn template_name(&mut self, t: TemplateRef) -> Vec<u8> {
        name_of("Tmpl", t as i32)
    }
    fn template_is_kind_of(&mut self, t: TemplateRef, k: KindOfType) -> bool {
        (self.w.t[t as usize].kinds as u32 >> kind_bit(k)) & 1 != 0
    }
    fn template_cost(&mut self, t: TemplateRef, _p: PlayerRef) -> i32 {
        self.w.t[t as usize].cost
    }
    fn template_equivalent(&mut self, a: TemplateRef, b: TemplateRef) -> bool {
        self.w.t[a as usize].equiv == self.w.t[b as usize].equiv
    }
    fn template_bounding_radius(&mut self, t: TemplateRef) -> f32 {
        self.w.t[t as usize].radius
    }
    fn template_placement_angle(&mut self, t: TemplateRef) -> f32 {
        self.w.t[t as usize].angle
    }
    fn objects(&mut self) -> Vec<ObjectId> {
        self.w.o.iter().filter(|o| o.id != 0).map(|o| o.id).collect()
    }
    fn object_exists(&mut self, o: ObjectId) -> bool {
        self.try_slot(o).is_some()
    }
    fn object_template(&mut self, o: ObjectId) -> TemplateRef {
        self.w.o[self.slot(o)].tmpl as u32
    }
    fn object_is_kind_of(&mut self, o: ObjectId, k: KindOfType) -> bool {
        (self.kinds_of_slot(self.slot(o)) >> kind_bit(k)) & 1 != 0
    }
    fn position(&mut self, o: ObjectId) -> Coord3D {
        let p = self.w.o[self.slot(o)].pos;
        Coord3D { x: p[0], y: p[1], z: p[2] }
    }
    fn set_position(&mut self, o: ObjectId, pos: &Coord3D) {
        self.log.extend([LOG_SET_POS, o as u32, bits(pos.x), bits(pos.y), bits(pos.z)]);
        let i = self.slot(o);
        self.w.o[i].pos = [pos.x, pos.y, pos.z];
    }
    fn bounding_radius(&mut self, o: ObjectId) -> f32 {
        self.w.o[self.slot(o)].radius
    }
    fn controlling_player(&mut self, o: ObjectId) -> Option<PlayerRef> {
        let p = self.w.o[self.slot(o)].player;
        (p >= 0).then_some(p)
    }
    fn object_team(&mut self, o: ObjectId) -> TeamRef {
        let t = self.w.o[self.slot(o)].team as u32;
        if self.team_slot(t).is_some() {
            t
        } else {
            0
        }
    }
    fn set_team(&mut self, o: ObjectId, team: TeamRef) {
        self.log.extend([LOG_SET_TEAM, o as u32, team]);
        let i = self.slot(o);
        self.w.o[i].team = team as i32;
    }
    fn status_test(&mut self, o: ObjectId, s: ObjectStatusTypes) -> bool {
        (self.w.o[self.slot(o)].status as u32 >> status_bit(s)) & 1 != 0
    }
    fn clear_construction_status(&mut self, o: ObjectId) {
        let m = (1u32 << status_bit(OBJECT_STATUS_UNDER_CONSTRUCTION)) | (1u32 << status_bit(OBJECT_STATUS_RECONSTRUCTING));
        self.log.extend([LOG_CLEAR_STATUS, o as u32, m]);
        let i = self.slot(o);
        self.w.o[i].status &= !(m as i32);
    }
    fn builder_id(&mut self, o: ObjectId) -> ObjectId {
        self.w.o[self.slot(o)].builder
    }
    fn is_significantly_above_terrain(&mut self, o: ObjectId) -> bool {
        self.has(o, F_ABOVE)
    }
    fn update_obj_values_from_map_properties(&mut self, o: ObjectId, name: &[u8], script: &[u8], health: i32, unsellable: bool) {
        self.log.extend([LOG_MAP_PROPS, o as u32, health as u32, unsellable as u32, name.len() as u32]);
        self.log.extend(name.iter().map(|&c| c as u32));
        self.log.push(script.len() as u32);
        self.log.extend(script.iter().map(|&c| c as u32));
    }
    fn update_upgrade_modules(&mut self, o: ObjectId) {
        self.log.extend([LOG_UPGRADE_MODULES, o as u32]);
    }
    fn has_ai(&mut self, o: ObjectId) -> bool {
        self.has(o, F_AI)
    }
    fn ai_is_idle(&mut self, o: ObjectId) -> bool {
        self.has(o, F_IDLE)
    }
    fn ai_goal_position(&mut self, o: ObjectId) -> Coord3D {
        self.position(o)
    }
    fn ai_command(&mut self, o: ObjectId, cmd: PlayerAiCommand) {
        let id = o as u32;
        match cmd {
            PlayerAiCommand::Dock(t) => self.log.extend([LOG_AI, id, C_DOCK, t as u32, CMD_FROM_PLAYER]),
            PlayerAiCommand::Idle => self.log.extend([LOG_AI, id, C_IDLE, CMD_FROM_AI]),
            PlayerAiCommand::MoveToPosition(p) => self.log.extend([LOG_AI, id, C_MOVE, bits(p.x), bits(p.y), bits(p.z), CMD_FROM_AI]),
            PlayerAiCommand::ResumeConstruction(t) => self.log.extend([LOG_AI, id, C_RESUME, t as u32, CMD_FROM_AI]),
            PlayerAiCommand::Repair(t) => self.log.extend([LOG_AI, id, C_REPAIR, t as u32, CMD_FROM_AI]),
            PlayerAiCommand::FollowExitProductionPath(path) => {
                self.log.extend([LOG_AI, id, C_EXIT_PATH, path.len() as u32]);
                for c in &path {
                    self.log.extend([bits(c.x), bits(c.y), bits(c.z)]);
                }
                self.log.push(CMD_FROM_AI);
            }
            PlayerAiCommand::JoinTeam => self.log.extend([LOG_AI, id, C_JOIN]),
        }
    }
    fn has_supply_truck_ai(&mut self, o: ObjectId) -> bool {
        self.has(o, F_SUPPLY_AI)
    }
    fn preferred_dock(&mut self, o: ObjectId) -> ObjectId {
        self.w.o[self.slot(o)].dock
    }
    fn ferrying_supplies(&mut self, o: ObjectId) -> bool {
        self.has(o, F_FERRYING)
    }
    fn forced_into_wanting_state(&mut self, o: ObjectId) -> bool {
        self.has(o, F_FORCED)
    }
    fn set_force_wanting_state(&mut self, o: ObjectId, b: bool) {
        self.log.extend([LOG_FORCE_WANTING, o as u32, b as u32]);
        let i = self.slot(o);
        if b {
            self.w.o[i].flags |= F_FORCED;
        } else {
            self.w.o[i].flags &= !F_FORCED;
        }
    }
    fn has_dozer_ai(&mut self, o: ObjectId) -> bool {
        self.has(o, F_DOZER_AI)
    }
    fn dozer_any_task_pending(&mut self, o: ObjectId) -> bool {
        self.has(o, F_TASK)
    }
    fn dozer_build_task_pending(&mut self, o: ObjectId) -> bool {
        self.has(o, F_TASK_BUILD)
    }
    fn has_production(&mut self, o: ObjectId) -> bool {
        self.has(o, F_PRODUCTION)
    }
    fn production_count(&mut self, o: ObjectId) -> i32 {
        self.w.o[self.slot(o)].prod_count
    }
    fn queue_create_unit(&mut self, o: ObjectId, t: TemplateRef) -> bool {
        self.unit_id = self.unit_id.wrapping_add(1);
        let ok = self.chance(80, o as u32, t, 80);
        self.log.extend([LOG_QUEUE_UNIT, o as u32, t, self.unit_id, ok as u32]);
        if ok {
            let i = self.slot(o);
            self.w.o[i].prod_count += 1;
        }
        ok
    }
    fn queue_upgrade(&mut self, o: ObjectId, u: UpgradeRef) -> bool {
        let ok = self.chance(81, o as u32, u, 70);
        self.log.extend([LOG_QUEUE_UPGRADE, o as u32, u, ok as u32]);
        ok
    }
    fn damage_state(&mut self, o: ObjectId) -> Option<i32> {
        let ob = self.w.o[self.slot(o)];
        (ob.flags & F_BODY != 0).then_some(ob.damage_state)
    }
    fn last_damage(&mut self, o: ObjectId) -> Option<(bool, u32)> {
        let ob = self.w.o[self.slot(o)];
        (ob.flags & F_BODY != 0 && ob.flags & F_LAST_DAMAGE != 0).then_some((ob.flags & F_NO_EFFECT != 0, ob.damage_stamp as u32))
    }
    fn has_supply_center_dock(&mut self, o: ObjectId) -> bool {
        self.has(o, F_CENTER_DOCK)
    }
    fn warehouse_boxes(&mut self, o: ObjectId) -> Option<i32> {
        let ob = self.w.o[self.slot(o)];
        (ob.flags & F_WAREHOUSE != 0).then_some(ob.boxes)
    }
    fn rebuild_hole(&mut self, o: ObjectId) -> Option<(ObjectId, ObjectId)> {
        let ob = self.w.o[self.slot(o)];
        (ob.flags & F_HOLE != 0).then_some((ob.spawner, ob.reconstructed))
    }
    fn exit_natural_rally_point(&mut self, o: ObjectId) -> Option<Option<Coord3D>> {
        let ob = self.w.o[self.slot(o)];
        if ob.flags & F_EXIT == 0 {
            return None;
        }
        Some((ob.flags & F_RALLY != 0).then_some(Coord3D { x: ob.rally[0], y: ob.rally[1], z: ob.rally[2] }))
    }
    fn command_set_upgrades(&mut self, o: ObjectId) -> Option<Vec<Vec<u8>>> {
        let bits = self.w.o[self.slot(o)].cmd_buttons;
        if bits < 0 {
            return None;
        }
        let mut v = Vec::new();
        for j in 0..18 {
            if (bits >> j) & 1 == 0 || j % 7 == 6 || j % 5 == 4 {
                continue;
            }
            v.push(name_of("Up", j % AP_UPGRADES as i32));
        }
        Some(v)
    }
    fn team_proto(&mut self, team: TeamRef) -> ProtoRef {
        self.w.tm[self.tk(team)].proto as u32
    }
    fn team_name(&mut self, team: TeamRef) -> Vec<u8> {
        name_of("Proto", self.w.tm[self.tk(team)].proto)
    }
    fn team_members(&mut self, team: TeamRef) -> Vec<ObjectId> {
        self.members(team).into_iter().map(|i| self.w.o[i].id).collect()
    }
    fn team_has_any_units(&mut self, team: TeamRef) -> bool {
        self.members(team).into_iter().any(|i| (self.kinds_of_slot(i) >> kind_bit(KINDOF_STRUCTURE)) & 1 == 0)
    }
    fn team_has_any_objects(&mut self, team: TeamRef) -> bool {
        !self.members(team).is_empty()
    }
    fn team_is_idle(&mut self, team: TeamRef) -> bool {
        self.w.tm[self.tk(team)].idle != 0
    }
    fn team_set_active(&mut self, team: TeamRef) {
        self.log.extend([LOG_TEAM_ACTIVE, team]);
        let k = self.tk(team);
        self.w.tm[k].active = 1;
    }
    fn team_transfer_units_to(&mut self, team: TeamRef, to: TeamRef) {
        self.log.extend([LOG_TEAM_TRANSFER, team, to]);
        for i in self.members(team) {
            self.w.o[i].team = to as i32;
        }
    }
    fn team_delete(&mut self, team: TeamRef) {
        self.log.extend([LOG_TEAM_DELETE, team]);
        for i in self.members(team) {
            self.w.o[i].team = 0;
        }
        let k = self.tk(team);
        self.w.tm[k].id = 0;
    }
    fn team_try_to_recruit(&mut self, team: TeamRef, t: TemplateRef, origin: &Coord3D, radius: f32) -> Option<ObjectId> {
        let owner = self.proto_player(team);
        let mut best: Option<(usize, f32)> = None;
        for i in 0..AP_OBJECTS {
            let o = self.w.o[i];
            if o.id == 0 || o.flags & F_DEAD != 0 || o.flags & F_AI == 0 || o.team as u32 == team || self.team_slot(o.team as u32).is_none() || o.player != owner {
                continue;
            }
            if self.w.t[o.tmpl as usize].equiv != self.w.t[t as usize].equiv {
                continue;
            }
            let (dx, dy) = (o.pos[0] - origin.x, o.pos[1] - origin.y);
            let d = dx * dx + dy * dy;
            if d > radius * radius {
                continue;
            }
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best.map(|(i, _)| self.w.o[i].id)
    }
    fn team_count_objects_by_template(&mut self, team: TeamRef, t: TemplateRef) -> i32 {
        let e = self.w.t[t as usize].equiv;
        self.members(team).into_iter().filter(|&i| self.w.t[self.w.o[i].tmpl as usize].equiv == e).count() as i32
    }
    fn team_estimate_position(&mut self, team: TeamRef) -> Option<Coord3D> {
        self.members(team).first().map(|&i| {
            let p = self.w.o[i].pos;
            Coord3D { x: p[0], y: p[1], z: p[2] }
        })
    }
    fn team_guard_position(&mut self, team: TeamRef, pos: &Coord3D) {
        self.log.extend([LOG_TEAM_GUARD, team, bits(pos.x), bits(pos.y), bits(pos.z), 0, CMD_FROM_SCRIPT]);
    }
    fn proto_name(&mut self, proto: ProtoRef) -> Vec<u8> {
        name_of("Proto", proto as i32)
    }
    fn proto_info(&mut self, proto: ProtoRef) -> TeamTemplateInfo {
        let p = self.w.pr[proto as usize];
        let mut info = TeamTemplateInfo::from_dict(&Dict::new(), &mut NameKeyGenerator::new(), &WellKnownKeys::new(), &[]);
        info.num_units_info = p.num_units;
        for i in 0..4 {
            info.units_info[i].min_units = p.unit_min[i];
            info.units_info[i].max_units = p.unit_max[i];
            info.units_info[i].unit_thing_name = if p.unit_tmpl[i] < 0 { b"Nothing".to_vec() } else { name_of("Tmpl", p.unit_tmpl[i]) };
        }
        info.home_location = Coord3D { x: p.home[0], y: p.home[1], z: p.home[2] };
        info.has_home_location = p.flags & P_HOME != 0;
        info.initial_idle_frames = p.idle_frames;
        info.automatically_reinforce = p.flags & P_REINFORCE != 0;
        info.execute_actions = p.flags & P_ACTIONS != 0;
        info.production_condition = script_name(p.condition);
        info.max_instances = p.max_instances;
        info.production_priority = p.priority;
        info
    }
    fn proto_is_singleton(&mut self, proto: ProtoRef) -> bool {
        self.w.pr[proto as usize].flags & P_SINGLETON != 0
    }
    fn proto_controlling_player(&mut self, proto: ProtoRef) -> PlayerRef {
        self.w.pr[proto as usize].player
    }
    fn proto_instances(&mut self, proto: ProtoRef) -> Vec<TeamRef> {
        self.w.tm.iter().filter(|t| t.id != 0 && t.proto as u32 == proto).map(|t| t.id as u32).collect()
    }
    fn proto_evaluate_production_condition(&mut self, proto: ProtoRef) -> bool {
        self.w.pr[proto as usize].flags & P_CONDITION == 0 || self.chance(87, proto, self.w.frame as u32, 50)
    }
    fn find_team(&mut self, name: &[u8]) -> Option<TeamRef> {
        let p = index_of(name, "Proto", AP_PROTOS as i32);
        if p < 0 {
            return None;
        }
        self.w.tm.iter().find(|t| t.id != 0 && t.proto == p).map(|t| t.id as u32)
    }
    fn create_inactive_team(&mut self, name: &[u8]) -> TeamRef {
        let p = index_of(name, "Proto", AP_PROTOS as i32);
        let k = (0..AP_TEAMS).find(|&k| self.w.tm[k].id == 0).expect("the generator keeps a free team slot");
        let id = self.w.next_team_id;
        self.w.next_team_id += 1;
        let idle = self.chance(94, id as u32, 0, 75);
        self.w.tm[k] = cpp::ApTeam { id, proto: p, active: 0, idle: idle as i32 };
        self.log.extend([LOG_TEAM_CREATE, p as u32, id as u32]);
        id as u32
    }
    fn team_by_id(&mut self, id: u32) -> TeamRef {
        if self.team_slot(id).is_some() {
            id
        } else {
            0
        }
    }
    fn build_object_now(&mut self, builder: Option<ObjectId>, t: TemplateRef, pos: &Coord3D, angle: f32, p: PlayerRef) -> Option<ObjectId> {
        let slot = (0..AP_OBJECTS).find(|&i| self.w.o[i].id == 0);
        let new_id = if slot.is_some() { self.w.next_object_id as u32 } else { 0 };
        self.log.extend([LOG_BUILD_NOW, builder.unwrap_or(0) as u32, t, bits(pos.x), bits(pos.y), bits(pos.z), bits(angle), p as u32, new_id]);
        let i = slot?;
        let tm = self.w.t[t as usize];
        self.w.o[i] = cpp::ApObj {
            id: self.w.next_object_id,
            tmpl: t as i32,
            player: p,
            team: self.w.pl[p as usize].default_team,
            flags: tm.new_flags,
            status: if builder.is_some() { 1 << status_bit(OBJECT_STATUS_UNDER_CONSTRUCTION) } else { 0 },
            pos: [pos.x, pos.y, pos.z],
            radius: tm.radius,
            builder: builder.unwrap_or(INVALID_ID),
            cmd_buttons: -1,
            ..Default::default()
        };
        self.w.next_object_id += 1;
        Some(self.w.o[i].id)
    }
    fn is_location_legal_to_build(&mut self, pos: &Coord3D, t: TemplateRef, angle: f32, options: u32, builder: Option<ObjectId>, p: PlayerRef) -> bool {
        let ok = ap_answer(self.seed, 90, bits(pos.x) ^ bits(pos.y).wrapping_mul(31), t.wrapping_mul(16).wrapping_add(options)) % 100
            < self.w.legal_pct as u32;
        self.log.extend([LOG_LEGAL, bits(pos.x), bits(pos.y), bits(pos.z), t, bits(angle), options, builder.unwrap_or(0) as u32, p as u32, ok as u32]);
        ok
    }
    fn is_possible_to_make_unit(&mut self, factory: ObjectId, t: TemplateRef) -> bool {
        self.chance(91, factory as u32, t, 75)
    }
    fn remove_all_bibs(&mut self) {
        self.log.push(LOG_BIBS);
    }
    fn closest_object(&mut self, q: ClosestQuery, p: PlayerRef, center: &Coord3D, radius: f32) -> Option<ObjectId> {
        let kb = |k: KindOfType| 1u32 << kind_bit(k);
        let sb = |s: ObjectStatusTypes| 1u32 << status_bit(s);
        let mut best: Option<(usize, f32)> = None;
        for i in 0..AP_OBJECTS {
            let o = self.w.o[i];
            if o.id == 0 {
                continue;
            }
            let kinds = self.kinds_of_slot(i);
            let ok = match q {
                ClosestQuery::SupplySourceNotMine => kinds & kb(KINDOF_SUPPLY_SOURCE) != 0 && o.player != p && !self.off_map(i),
                ClosestQuery::MyCashGenerator => kinds & kb(KINDOF_CASH_GENERATOR) != 0 && o.player == p && !self.off_map(i),
                ClosestQuery::Threat => {
                    let st = o.status as u32;
                    self.relationship_by_team_id(p, o.team) == Relationship::Enemies
                        && o.flags & F_DEAD == 0
                        && !(st & sb(OBJECT_STATUS_STEALTHED) != 0 && st & (sb(OBJECT_STATUS_DETECTED) | sb(OBJECT_STATUS_DISGUISED)) == 0)
                        && kinds & kb(KINDOF_HARVESTER) == 0
                        && kinds & kb(KINDOF_DOZER) == 0
                }
            };
            if !ok {
                continue;
            }
            let (dx, dy) = (o.pos[0] - center.x, o.pos[1] - center.y);
            let (d, r) = (dx * dx + dy * dy, radius + o.radius);
            if d > r * r {
                continue;
            }
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best.map(|(i, _)| self.w.o[i].id)
    }
    fn quick_does_path_exist(&mut self, o: ObjectId, to: &Coord3D) -> bool {
        self.chance(92, bits(to.x) ^ bits(to.y), o as u32, 85)
    }
    fn adjust_to_possible_destination(&mut self, o: ObjectId, pos: &mut Coord3D) {
        self.log.extend([LOG_ADJUST, o as u32, bits(pos.x), bits(pos.y), bits(pos.z)]);
        pos.x += 5.0;
    }
    fn ground_height(&mut self, x: f32, y: f32) -> f32 {
        (c_int_of((x / 50.0).floor()).wrapping_add(2i32.wrapping_mul(c_int_of((y / 50.0).floor()))) & 7) as f32
    }
    fn extent(&mut self) -> Region2D {
        let e = self.w.extent;
        Region2D { lo: Coord2D { x: e[0], y: e[1] }, hi: Coord2D { x: e[2], y: e[3] } }
    }
    fn find_upgrade(&mut self, name: &[u8]) -> Option<UpgradeRef> {
        let i = index_of(name, "Up", AP_UPGRADES as i32);
        (i >= 0).then_some(i as u32)
    }
    fn upgrade_is_object_type(&mut self, u: UpgradeRef) -> bool {
        u & 1 != 0
    }
    fn upgrade_name(&mut self, u: UpgradeRef) -> Vec<u8> {
        name_of("Up", u as i32)
    }
    fn can_afford_upgrade(&mut self, p: PlayerRef, u: UpgradeRef) -> bool {
        self.chance(93, p as u32, u, 70)
    }
    fn has_sufficient_power(&mut self, p: PlayerRef) -> bool {
        self.w.pl[p as usize].flags & PL_POWER != 0
    }
    fn is_disabled_unmanned(&mut self, o: ObjectId) -> bool {
        self.has(o, F_UNMANNED)
    }
    fn clear_builder(&mut self, o: ObjectId) {
        self.log.extend([LOG_CLEAR_BUILDER, o as u32, 0]);
        let i = self.slot(o);
        self.w.o[i].builder = INVALID_ID;
    }
    fn can_make_unit(&mut self, builder: ObjectId, t: Option<TemplateRef>) -> bool {
        t.is_some_and(|t| self.chance(96, builder as u32, t, 70))
    }
    fn player_count(&mut self) -> i32 {
        AP_PLAYERS as i32
    }
    fn player_is_human(&mut self, p: PlayerRef) -> bool {
        self.w.pl[p as usize].flags & PL_HUMAN != 0
    }
    fn player_has_any_units(&mut self, p: PlayerRef) -> bool {
        self.any_member(p, |o| (self.w.t[o.tmpl as usize].kinds as u32 >> kind_bit(KINDOF_STRUCTURE)) & 1 == 0)
    }
    fn player_has_any_build_facility(&mut self, p: PlayerRef) -> bool {
        self.any_member(p, |o| o.flags & F_PRODUCTION != 0)
    }
    fn player_has_any_objects(&mut self, p: PlayerRef) -> bool {
        self.any_member(p, |_| true)
    }
    fn player_is_skirmish_ai(&mut self, p: PlayerRef) -> bool {
        assert_ne!(p, 0, "player 0's AI answers for itself");
        self.w.pl[p as usize].flags & PL_SKIRMISH != 0
    }
    fn player_current_enemy(&mut self, p: PlayerRef) -> Option<PlayerRef> {
        assert_ne!(p, 0, "player 0's AI answers for itself");
        let pl = self.w.pl[p as usize];
        (pl.flags & PL_SKIRMISH != 0 && (0..AP_PLAYERS as i32).contains(&pl.enemy)).then_some(pl.enemy)
    }
    fn mp_start_index(&mut self, p: PlayerRef) -> i32 {
        self.w.pl[p as usize].mp_start
    }
    fn closest_waypoint_on_path(&mut self, pos: &Coord3D, label: &[u8]) -> Option<Coord3D> {
        let k = ["Center", "Flank", "Backdoor"].iter().position(|n| name_of(n, self.w.path_start) == label)?;
        let mut best: Option<(usize, f32)> = None;
        for j in 0..self.w.path_len[k] as usize {
            let w = self.w.paths[k][j];
            let (dx, dy) = (w[0] - pos.x, w[1] - pos.y);
            let d = dx * dx + dy * dy;
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((j, d));
            }
        }
        best.map(|(j, _)| {
            let w = self.w.paths[k][j];
            Coord3D { x: w[0], y: w[1], z: w[2] }
        })
    }
    fn maximum_pathfind_extent(&mut self) -> Region2D {
        self.extent()
    }
    fn find_broken_bridge(&mut self, unit: ObjectId, to: &Coord3D) -> Option<ObjectId> {
        if !self.chance(95, bits(to.x) ^ bits(to.y), unit as u32, 40) {
            return None;
        }
        (0..AP_OBJECTS).find(|&i| self.w.o[i].id != 0 && (self.kinds_of_slot(i) >> kind_bit(KINDOF_BRIDGE)) & 1 != 0).map(|i| self.w.o[i].id)
    }
    fn undo_and_destroy_structure(&mut self, o: ObjectId) {
        self.log.extend([LOG_UNDONE, o as u32]);
        let i = self.slot(o);
        self.w.o[i].id = 0;
    }
}

/// The AI data of the scene (the wrapper's `setUp`).
fn ai_data(s: &ApScene) -> AiDataEntry {
    let mut e = AiDataEntry::default();
    let d = &mut e.data;
    d.structure_seconds = s.structure_seconds;
    d.team_seconds = s.team_seconds;
    d.resources_wealthy = s.resources_wealthy;
    d.resources_poor = s.resources_poor;
    d.structures_wealthy_mod = s.structures_wealthy_mod;
    d.team_wealthy_mod = s.team_wealthy_mod;
    d.structures_poor_mod = s.structures_poor_mod;
    d.team_poor_mod = s.team_poor_mod;
    d.team_resources_to_build = s.team_resources_to_build;
    d.max_recruit_distance = s.max_recruit_distance;
    d.supply_center_safe_radius = s.safe_radius;
    d.rebuild_delay_seconds = s.rebuild_delay_seconds;
    e.lists.side_info.clear();
    for side in &s.sides[..AP_SIDES] {
        let mut si = AiSideInfo { side: name_of("Side", side.side), easy: side.easy, normal: side.normal, hard: side.hard, ..Default::default() };
        for k in 0..5 {
            si.skill_sets[k].num_skills = side.num_skills[k];
            for j in 0..4 {
                si.skill_sets[k].skills[j] = ScienceType(side.skills[k][j]);
            }
        }
        si.base_defense_structure1 = if side.base_defense < 0 { Vec::new() } else { name_of("Tmpl", side.base_defense) };
        e.lists.side_info.push(si);
    }
    e.lists.side_build_lists = (0..AP_SIDES)
        .map(|k| AiSideBuildList { side: name_of("Side", s.sides[k].side), build_list: s.side_build[k][..s.side_build_count[k] as usize].iter().map(entry_of).collect() })
        .collect();
    e.data.rotate_skirmish_bases = s.rotate_bases != 0;
    e.data.skirmish_base_defense_extra_distance = s.defense_extra;
    e
}

/// The run's AI player: an `AISkirmishPlayer` for scene.skirmish 2, else an `AIPlayer` answering `isSkirmishAI()` as the scene
/// says (the wrapper's `newAI`).
fn new_ai(h: &mut Host, data: &AiDataEntry) -> AiPlayer {
    let d = difficulty(h.w.difficulty);
    if h.w.skirmish == 2 {
        AiPlayer::new_skirmish(h, 0, d, data)
    } else {
        AiPlayer::new(h, 0, h.w.skirmish != 0, d, data)
    }
}

fn difficulty(d: i32) -> GameDifficulty {
    match d {
        0 => GameDifficulty::Easy,
        1 => GameDifficulty::Normal,
        _ => GameDifficulty::Hard,
    }
}

/// The dump of `ref_ai_player_run`: the AI player, then the world.
fn dump(a: &AiPlayer, h: &Host, out: &mut Vec<u32>) {
    let team = |out: &mut Vec<u32>, t: &rts_lockstep_engine::logic::ai::ai_player::TeamInQueue| {
        out.extend([t.priority_build as u32, t.team, t.frame_started as u32, t.sent_to_start_location as u32, t.stop_queueing as u32]);
        out.extend([t.reinforcement as u32, t.reinforcement_id as u32, t.work_orders.len() as u32]);
        for w in &t.work_orders {
            out.extend([w.thing.unwrap_or(u32::MAX), w.factory_id as u32, w.num_completed as u32, w.num_required as u32, w.required as u32]);
            out.push(w.is_resource_gatherer as u32);
        }
    };
    for t in a.build_queue.iter().chain(&a.ready_queue) {
        if t.team != 0 && h.team_slot(t.team).is_none() {
            h.stale.set(true); // (a queued team deleted: upstream's dangling pointer)
        }
    }
    out.push(a.build_queue.len() as u32);
    for t in &a.build_queue {
        team(out, t);
    }
    out.push(a.ready_queue.len() as u32);
    for t in &a.ready_queue {
        team(out, t);
    }
    out.extend([a.ready_to_build_team as u32, a.ready_to_build_structure as u32, a.team_timer as u32, a.structure_timer as u32]);
    out.extend([a.team_seconds as u32, a.build_delay as u32, a.team_delay as u32, a.frame_last_building_built as u32]);
    out.extend([a.difficulty as u32, a.skillset_selector as u32, bits(a.base_center.x), bits(a.base_center.y), bits(a.base_center.z)]);
    out.extend([a.base_center_set as u32, bits(a.base_radius), a.structures_to_repair[0] as u32, a.structures_to_repair[1] as u32]);
    out.extend([a.repair_dozer as u32, bits(a.repair_dozer_origin.x), bits(a.repair_dozer_origin.y), bits(a.repair_dozer_origin.z)]);
    out.extend([a.structures_in_queue as u32, a.dozer_queued_for_repair as u32, a.dozer_is_repairing as u32, a.bridge_timer as u32]);
    out.extend([a.supply_source_attack_check_frame, a.attacked_supply_center as u32, a.cur_warehouse_id as u32]);
    out.push(a.skirmish_state.is_some() as u32);
    if let Some(s) = &a.skirmish_state {
        out.extend([s.cur_front_base_defense as u32, s.cur_flank_base_defense as u32, bits(s.cur_front_left_defense_angle), bits(s.cur_front_right_defense_angle)]);
        out.extend([bits(s.cur_left_flank_left_defense_angle), bits(s.cur_left_flank_right_defense_angle), bits(s.cur_right_flank_left_defense_angle)]);
        out.extend([bits(s.cur_right_flank_right_defense_angle), s.frame_to_check_enemy, s.current_enemy.map_or(u32::MAX, |e| e as u32)]);
    }
    for o in &h.w.o {
        out.extend([o.id, o.tmpl, o.player, o.team, o.flags, o.status].map(|v| v as u32));
        out.extend([bits(o.pos[0]), bits(o.pos[1]), bits(o.pos[2]), bits(o.radius)]);
        out.extend([o.builder, o.spawner, o.reconstructed, o.dock, o.boxes, o.prod_count, o.damage_state, o.damage_stamp, o.cmd_buttons].map(|v| v as u32));
        out.extend([bits(o.rally[0]), bits(o.rally[1]), bits(o.rally[2])]);
    }
    for t in &h.w.tm {
        out.extend([t.id, t.proto, t.active, t.idle].map(|v| v as u32));
    }
    for p in &h.w.pl {
        out.extend([p.money, p.flags, p.side, p.attacked_frame, p.science_points, p.default_team, p.enemy, p.mp_start].map(|v| v as u32));
    }
    out.extend([h.w.next_object_id as u32, h.w.next_team_id as u32, h.unit_id, h.rand]);
    out.push(h.build.len() as u32);
    let s = |out: &mut Vec<u32>, v: &[u8]| {
        out.push(v.len() as u32);
        out.extend(v.iter().map(|&c| c as u32));
    };
    for b in &h.build {
        s(out, &b.building_name);
        s(out, &b.template_name);
        s(out, &b.script);
        out.extend([bits(b.location.x), bits(b.location.y), bits(b.location.z), bits(b.rally_point_offset.x), bits(b.rally_point_offset.y)]);
        out.extend([bits(b.angle), b.is_initially_built as u32, b.num_rebuilds, b.automatically_build as u32, b.health as u32, b.unsellable as u32]);
        out.extend([b.under_construction as u32, b.is_supply_building as u32, b.priority_build as u32, b.object_id as u32, b.object_timestamp]);
        out.extend([b.desired_gatherers as u32, b.current_gatherers as u32]);
    }
}

/// One operation's answer words (`None`: undefined upstream, not compared).
fn rust_op(a: &mut AiPlayer, h: &mut Host, data: &AiDataEntry, op: &ApOp, seen: &mut [u64; 8]) -> Option<Vec<u32>> {
    let mut r = Vec::new();
    let mut pos = Coord3D { x: op.x, y: op.y, z: op.z };
    let tmpl_name = |i: i32| if (0..AP_TEMPLATES as i32).contains(&i) { name_of("Tmpl", i) } else { b"Nothing".to_vec() };
    let tmpl = |i: i32| (0..AP_TEMPLATES as i32).contains(&i).then_some(i as u32);
    let proto = |i: i32| (0..AP_PROTOS as i32).contains(&i).then_some(i as u32);
    let id = |h: &mut Host, o: ObjectId| -> u32 {
        let _ = h;
        o as u32
    };
    let team_of = |h: &Host, t: i32| -> TeamRef {
        if h.team_slot(t as u32).is_some() {
            t as u32
        } else {
            0
        }
    };
    match op.kind {
        0 => a.update(h, data),
        1 => a.new_map(h, data),
        2 => {
            if !h.object_exists(op.b) {
                r.push(u32::MAX);
            } else {
                let f = h.object_exists(op.a).then_some(op.a);
                a.on_unit_produced(h, f, op.b);
            }
        }
        3 => {
            if !h.object_exists(op.b) {
                r.push(u32::MAX);
            } else {
                a.on_structure_produced(h, data, op.b);
            }
        }
        4 => a.build_specific_ai_team(h, data, proto(op.a), op.b != 0),
        5 => a.recruit_specific_ai_team(h, proto(op.a), op.x),
        6 | 7 => {
            let before = h.build.len();
            if op.kind == 6 {
                a.build_ai_base_defense(h, data, op.a != 0);
            } else {
                a.build_ai_base_defense_structure(h, data, &tmpl_name(op.a), op.b != 0);
            }
            seen[4] += (h.build.len() - before) as u64;
        }
        8 => a.build_specific_ai_building(h, &tmpl_name(op.a)),
        9 => a.repair_structure(h, op.a),
        10 => a.select_skillset(op.a),
        11 => a.build_by_supplies(h, data, op.a, &tmpl_name(op.b)),
        12 => {
            let t = team_of(h, op.b);
            if t == 0 {
                r.push(u32::MAX);
            } else {
                a.build_specific_building_nearest_team(h, &tmpl_name(op.a), t);
            }
        }
        13 => a.build_upgrade(h, &if op.a >= 0 { name_of("Up", op.a) } else { b"Nothing".to_vec() }),
        14 => {
            let t = team_of(h, op.a);
            a.ai_pre_team_destroy(h, t);
        }
        15 => r.push(a.is_supply_source_safe(h, data, op.a) as u32),
        16 => r.push(a.is_supply_source_attacked(h) as u32),
        17 => r.push(a.is_location_safe(h, data, &pos, tmpl(op.a)) as u32),
        18 => {
            let t = team_of(h, op.a);
            if t == 0 {
                r.push(u32::MAX);
            } else {
                a.guard_supply_center(h, data, t, op.b);
            }
        }
        19 => {
            let ok = a.calc_closest_construction_zone_location(h, tmpl(op.a), &mut pos);
            r.extend([ok as u32, bits(pos.x), bits(pos.y), bits(pos.z)]);
        }
        20 => match a.compute_superweapon_target(h, [0, SPECIAL_SNEAK_ATTACK, SPECIAL_CLUSTER_MINES, NUKE_SPECIAL_CLUSTER_MINES][(op.a & 3) as usize], op.b, op.x) {
            Ok((p, ok)) => {
                seen[0] += 1;
                seen[6] += (a.skirmish_state.is_some() && op.a & 3 >= 2) as u64;
                r.extend([ok as u32, bits(p.x), bits(p.y), bits(p.z)]);
            }
            Err(_) => {
                seen[1] += 1;
                return None;
            }
        },
        21 => r.push(AiPlayer::get_player_superweapon_value(h, &pos, op.a, op.w, op.b != 0) as u32),
        22 => {
            let b = AiPlayer::get_player_structure_bounds(h, op.a, op.b != 0);
            r.extend([bits(b.lo.x), bits(b.lo.y), bits(b.hi.x), bits(b.hi.y)]);
        }
        23 => h.w.frame += op.a,
        24 => {
            if (0..AP_OBJECTS as i32).contains(&op.a) {
                h.w.o[op.a as usize].id = 0;
            }
        }
        25 => {
            let k = if op.a < 0 { h.try_slot(a.repair_dozer).map_or(-1, |k| k as i32) } else { op.a };
            if (0..AP_OBJECTS as i32).contains(&k) {
                h.w.o[k as usize].flags ^= op.b & F_MUTABLE;
                h.w.o[k as usize].status ^= op.c;
            }
        }
        26 => {
            if (0..AP_PLAYERS as i32).contains(&op.a) {
                h.w.pl[op.a as usize].money = op.b;
            }
        }
        27 => {
            let mut s = XferSave::new();
            s.open(b"ai").expect("open");
            a.xfer(h, &mut s).expect("save");
            s.close().expect("close");
            let bytes = s.take_bytes();
            r.push(bytes.len() as u32);
            r.extend(bytes.iter().map(|&c| c as u32));
            a.clear_teams_in_queue(h);
            *a = new_ai(h, data);
            let mut l = XferLoad::new(bytes);
            l.open(b"ai").expect("open");
            a.xfer(h, &mut l).expect("load");
            seen[2] += 1;
        }
        28 => {
            if (0..AP_TEAMS as i32).contains(&op.a) {
                h.w.tm[op.a as usize].idle ^= 1;
            }
        }
        29 => r.push(a.get_ai_difficulty() as u32),
        30 => a.process_base_building(h, data),
        31 => a.process_team_building(h, data),
        32 => a.queue_units(h, data),
        33 => a.check_ready_teams(h),
        34 => a.check_queued_teams(h),
        35 => a.do_upgrades_and_skills(h, data),
        36 => a.queue_supply_truck(h, data),
        37 => a.queue_dozer(h),
        38 => a.update_bridge_repair(h, data),
        39 => {
            let o = a.find_dozer(h, data, &pos);
            r.push(o.map_or(0, |o| id(h, o)));
        }
        40 => {
            let o = a.find_supply_center(h, data, op.a);
            r.push(o.map_or(0, |o| id(h, o)));
        }
        41 => {
            let (c, rad) = a.compute_center_and_radius_of_base(h);
            r.extend([bits(c.x), bits(c.y), bits(c.z), bits(rad)]);
        }
        42 => r.push(a.dozer_in_queue(h) as u32),
        43 => r.push(a.select_team_to_build(h, data) as u32),
        44 => r.push(a.select_team_to_reinforce(h, data, op.a) as u32),
        45 => r.push(a.is_a_good_idea_to_build_team(h, data, op.a as u32) as u32),
        46 => {
            let (ok, money) = a.is_possible_to_build_team(h, data, op.a as u32, op.b != 0);
            r.extend([ok as u32, money as u32]);
        }
        47 => {
            let o = AiPlayer::find_factory(h, a.player, op.a as u32, op.b != 0);
            r.push(o.map_or(0, |o| o as u32));
        }
        48 => a.clear_teams_in_queue(h),
        49 | 50 => {
            if op.b < 0 || op.b as usize >= h.build.len() {
                r.push(u32::MAX);
            } else {
                let o = if op.kind == 49 {
                    a.build_structure_now(h, data, op.a as u32, op.b as usize)
                } else {
                    a.build_structure_with_dozer(h, data, op.a as u32, op.b as usize)
                };
                r.push(o.map_or(0, |o| o as u32));
            }
        }
        51 => {
            if op.b < 0 || op.b as usize >= h.build.len() || !h.object_exists(op.a) {
                r.push(u32::MAX);
            } else {
                a.check_for_supply_center(h, data, op.b as usize, op.a);
            }
        }
        52 => a.do_base_building(h, data),
        53 => a.do_team_building(h, data),
        54 => {
            let mut w = WorkOrder::new();
            w.thing = Some(op.a as u32);
            w.required = op.c != 0;
            let ok = AiPlayer::start_training(h, a.player, &mut w, op.b != 0, &name_of("Proto", 0));
            r.extend([ok as u32, w.factory_id as u32]);
        }
        55 => {
            if (0..AP_PLAYERS as i32).contains(&op.a) {
                h.w.pl[op.a as usize].flags ^= op.b;
            }
        }
        56 => {
            let order = a.build_queue.iter().flat_map(|t| &t.work_orders).find(|o| o.factory_id != INVALID_ID && o.num_completed < o.num_required).cloned();
            let f = order.as_ref().map(|o| o.factory_id).filter(|&f| h.object_exists(f));
            let slot = (0..AP_OBJECTS).find(|&i| h.w.o[i].id == 0);
            match (order, f, slot) {
                (Some(order), Some(f), Some(i)) => {
                    let t = order.thing.expect("a work order's template") as usize;
                    let fp = h.w.o[h.slot(f)].pos;
                    h.w.o[i] = cpp::ApObj {
                        id: h.w.next_object_id,
                        tmpl: t as i32,
                        player: 0,
                        team: h.w.pl[0].default_team,
                        flags: h.w.t[t].new_flags | (op.b & (F_AI | F_SUPPLY_AI | F_IDLE | F_DOZER_AI)),
                        pos: fp,
                        radius: h.w.t[t].radius,
                        cmd_buttons: -1,
                        ..Default::default()
                    };
                    h.w.next_object_id += 1;
                    let u = h.w.o[i].id;
                    r.push(u as u32);
                    seen[3] += 1;
                    a.on_unit_produced(h, Some(f), u);
                }
                _ => r.push(u32::MAX),
            }
        }
        58 => {
            let k = if op.a < 0 {
                match a.build_queue.iter().flat_map(|t| &t.work_orders).find(|o| o.factory_id != INVALID_ID) {
                    Some(o) => h.try_slot(o.factory_id).map_or(-2, |k| k as i32),
                    None => -1,
                }
            } else {
                op.a
            };
            if (0..AP_OBJECTS as i32).contains(&k) {
                h.w.o[k as usize].player = op.b;
            }
        }
        59 => {
            if !h.object_exists(op.a) || !(0..3).contains(&op.b) {
                r.push(u32::MAX);
            } else {
                let k = op.b as usize;
                let way: Vec<Coord3D> = h.w.paths[k][..h.w.path_len[k] as usize].iter().map(|w| Coord3D { x: w[0], y: w[1], z: w[2] }).collect();
                r.push(a.check_bridges(h, op.a, &way) as u32);
            }
        }
        60 => {
            let e = a.get_ai_enemy(h);
            seen[5] += e.is_some() as u64;
            r.push(e.map_or(u32::MAX, |e| e as u32));
        }
        61 => r.push(if a.skirmish_state.is_some() { a.get_my_enemy_player_index(h) as u32 } else { u32::MAX }),
        57 => {
            let first = (a.structures_in_queue > 0).then(|| a.structures_to_repair[0]);
            let k = if op.a < 0 { first.and_then(|id| h.try_slot(id)).map_or(-1, |k| k as i32) } else { op.a };
            if (0..AP_OBJECTS as i32).contains(&k) {
                h.w.o[k as usize].damage_state = op.b;
            }
        }
        k => panic!("op kind {k}"),
    }
    Some(r)
}

/// One op's answer (`None` when undefined) and the dump after it.
type OpResult = (Option<Vec<u32>>, Vec<u32>);

/// The run as `ref_ai_player_run` makes it: per op its answer and the dump; the log.
fn rust_run(s: &ApScene, ops: &[ApOp], seen: &mut [u64; 8]) -> (Vec<OpResult>, Vec<u32>, bool) {
    let mut h = Host::new(s);
    let data = ai_data(s);
    let mut a = new_ai(&mut h, &data);
    let mut v = Vec::new();
    for op in ops {
        let r = rust_op(&mut a, &mut h, &data, op, seen);
        let mut d = Vec::new();
        dump(&a, &h, &mut d);
        v.push((r, d));
    }
    a.clear_teams_in_queue(&mut h);
    let stale = h.stale.get();
    (v, h.log, stale)
}

/// Tallies the log's entries by kind (AI commands by command, at 32 + the command); panics on a malformed log.
fn tally(log: &[u32], kinds: &mut [u64; 48]) {
    let mut i = 0;
    while i < log.len() {
        let k = log[i];
        let len = match k {
            LOG_RANDOM | LOG_PURCHASE | LOG_QUEUE_UPGRADE => 4,
            LOG_DEBUG => 2 + log[i + 1] as usize,
            LOG_SCRIPT => 3 + log[i + 1] as usize,
            LOG_CACHE | LOG_UPGRADE_MODULES | LOG_TEAM_ACTIVE | LOG_TEAM_DELETE => 2,
            LOG_ACTION | LOG_CAN_BUILD | LOG_ADD_BUILD | LOG_SET_TEAM | LOG_CLEAR_STATUS | LOG_FORCE_WANTING | LOG_TEAM_TRANSFER | LOG_TEAM_CREATE => 3,
            LOG_CLEAR_FLAGS | LOG_BIBS => 1,
            LOG_ADD_PRIORITY => 6,
            LOG_SET_POS | LOG_QUEUE_UNIT | LOG_ADJUST => 5,
            LOG_MAP_PROPS => {
                let n = log[i + 4] as usize;
                6 + n + log[i + 5 + n] as usize
            }
            LOG_AI => {
                let c = log[i + 2];
                kinds[32 + c as usize] += 1;
                match c {
                    C_DOCK | C_RESUME | C_REPAIR => 5,
                    C_IDLE => 4,
                    C_MOVE => 7,
                    C_EXIT_PATH => 5 + 3 * log[i + 3] as usize,
                    C_JOIN => 3,
                    _ => panic!("AI command {c}"),
                }
            }
            LOG_TEAM_GUARD => 7,
            LOG_CLEAR_BUILDER => 3,
            LOG_UNDONE => 2,
            LOG_BUILD_NOW => 9,
            LOG_LEGAL => 10,
            _ => panic!("log kind {k} at {i}"),
        };
        kinds[k as usize] += 1;
        i += len;
    }
    assert_eq!(i, log.len(), "the log's last entry");
}

/// The C++ output split per op: (answer, dump).
fn split(out: &[u32], n: usize) -> Vec<(Vec<u32>, Vec<u32>)> {
    let mut v = Vec::new();
    let mut i = 0;
    for _ in 0..n {
        let k = out[i] as usize;
        let ans = out[i + 1..i + 1 + k].to_vec();
        i += 1 + k;
        let k = out[i] as usize;
        let d = out[i + 1..i + 1 + k].to_vec();
        i += 1 + k;
        v.push((ans, d));
    }
    assert_eq!(i, out.len(), "the output's length");
    v
}

fn gen_pos(g: &mut Gen) -> [f32; 3] {
    [g.uniform(-100.0, 1100.0), g.uniform(-100.0, 1100.0), g.uniform(0.0, 20.0)]
}

fn gen_scene(g: &mut Gen) -> ApScene {
    let mut s = ApScene::default();
    // templates: kinds by a few patterns
    for t in s.t.iter_mut() {
        let mut k = 0i32;
        for b in 0..16 {
            if g.below(5) == 0 {
                k |= 1 << b;
            }
        }
        if g.below(2) == 0 {
            k |= 1; // a structure
        }
        let structural = F_AI | F_SUPPLY_AI | F_DOZER_AI | F_PRODUCTION | F_BODY | F_CENTER_DOCK | F_WAREHOUSE | F_HOLE | F_EXIT;
        *t = cpp::ApTmpl {
            kinds: k,
            cost: g.below(1500) as i32,
            equiv: g.below(6) as i32,
            new_flags: (g.u32() as i32) & (structural | F_MUTABLE & !F_DEAD),
            radius: g.uniform(2.0, 40.0),
            angle: g.uniform(-3.2, 3.2),
        };
    }
    // prototypes
    for p in s.pr.iter_mut() {
        *p = cpp::ApProto {
            player: if g.below(3) == 0 { g.below(3) as i32 } else { 0 },
            max_instances: g.below(4) as i32,
            priority: g.below(8) as i32,
            idle_frames: g.below(400) as i32 - 50,
            flags: g.below(32) as i32,
            num_units: g.below(5) as i32,
            condition: g.below(8) as i32 - 2,
            unit_tmpl: [0; 4].map(|_: i32| g.below(12) as i32 - 1),
            unit_min: [0; 4].map(|_: i32| g.below(3) as i32),
            unit_max: [0; 4].map(|_: i32| g.below(4) as i32),
            home: gen_pos(g),
        };
    }
    // teams: the players' default teams (ids 1 to 3), then a few more
    let mut next_team = 1;
    let mut team_ids = Vec::new();
    for p in 0..AP_PLAYERS {
        let protos: Vec<usize> = (0..AP_PROTOS).filter(|&i| s.pr[i].player == p as i32).collect();
        let proto = if protos.is_empty() { g.below(AP_PROTOS as u32) as usize } else { protos[g.below(protos.len() as u32) as usize] };
        s.tm[p] = cpp::ApTeam { id: next_team, proto: proto as i32, active: 1, idle: g.below(2) as i32 };
        s.pl[p].default_team = next_team;
        team_ids.push(next_team);
        next_team += 1;
    }
    for k in AP_PLAYERS..AP_PLAYERS + g.below(9) as usize {
        if g.below(4) == 0 {
            continue; // a free slot among the teams
        }
        s.tm[k] = cpp::ApTeam { id: next_team, proto: g.below(AP_PROTOS as u32) as i32, active: g.below(2) as i32, idle: g.below(3).min(1) as i32 };
        team_ids.push(next_team);
        next_team += 1;
    }
    s.next_team_id = 50;
    // objects
    let n = 4 + g.below(17) as usize;
    let mut ids = Vec::new();
    for i in 0..n {
        if g.below(6) == 0 {
            continue;
        }
        ids.push(100 + i as i32);
    }
    let mut k = 0;
    for i in 0..n {
        if k >= ids.len() {
            break;
        }
        if g.below(8) == 0 {
            continue; // a free slot
        }
        let pick = |g: &mut Gen, ids: &[i32]| -> i32 {
            match g.below(4) {
                0 => 0,
                1 => 999,
                _ => ids[g.below(ids.len() as u32) as usize],
            }
        };
        let tmpl = g.below(AP_TEMPLATES as u32) as i32;
        let flags = (g.u32() as i32) & ((1 << 20) - 1) & !(if g.below(4) == 0 { 0 } else { F_DEAD }) | if g.below(8) == 0 { F_UNMANNED } else { 0 };
        s.o[i] = cpp::ApObj {
            id: ids[k],
            tmpl,
            player: g.below(4) as i32 - 1 + (g.below(3) == 0) as i32,
            team: if g.below(6) == 0 { 0 } else { team_ids[g.below(team_ids.len() as u32) as usize] },
            flags: flags | (F_AI * (g.below(2) as i32)),
            status: if g.below(3) == 0 { g.below(64) as i32 } else { 0 },
            pos: gen_pos(g),
            radius: g.uniform(1.0, 40.0),
            builder: pick(g, &ids),
            spawner: pick(g, &ids),
            reconstructed: pick(g, &ids),
            dock: pick(g, &ids),
            boxes: g.below(30) as i32,
            prod_count: g.below(3) as i32,
            damage_state: g.below(4) as i32,
            damage_stamp: g.below(600) as i32,
            cmd_buttons: if g.below(3) == 0 { -1 } else { g.below(1 << 18) as i32 },
            rally: gen_pos(g),
        };
        s.o[i].player = s.o[i].player.min(2);
        k += 1;
    }
    let live: Vec<i32> = s.o.iter().filter(|o| o.id != 0).map(|o| o.id).collect();
    s.next_object_id = 1000;
    // players
    for p in s.pl.iter_mut() {
        p.money = match g.below(4) {
            0 => 0,
            1 => g.below(200) as i32,
            _ => g.below(8000) as i32,
        };
        p.flags = g.below(4) as i32;
        p.side = g.below(3) as i32;
        p.attacked_frame = g.below(1000) as i32;
        p.science_points = g.below(4) as i32;
        p.flags |= (g.below(8) as i32) << 2; // power, human, skirmish AI
        p.enemy = g.below(4) as i32 - 1;
        p.mp_start = g.below(4) as i32;
    }
    // player 0's build list
    s.num_build = g.below(AP_BUILD as u32 + 1) as i32;
    for e in s.b.iter_mut().take(s.num_build as usize) {
        *e = cpp::ApBuild {
            tmpl: match g.below(10) {
                0 => -1,
                1 => -2,
                _ => g.below(AP_TEMPLATES as u32) as i32,
            },
            object_id: match g.below(3) {
                0 => 0,
                1 => 999,
                _ if live.is_empty() => 0,
                _ => live[g.below(live.len() as u32) as usize],
            },
            timestamp: g.below(1000) as i32,
            num_rebuilds: match g.below(5) {
                0 => -1,
                n => n as i32 - 1,
            },
            flags: g.below(32) as i32,
            desired: g.below(5) as i32,
            current: g.below(5) as i32,
            health: g.below(101) as i32,
            name: g.below(6) as i32 - 1,
            script: g.below(8) as i32 - 2,
            loc: gen_pos(g),
            angle: g.uniform(-3.2, 3.2),
            rally: [g.uniform(-3.0, 3.0), g.uniform(-3.0, 3.0)],
        };
    }
    // (upstream reaches a dozer's AI, and a building's builder's, without a test: they always have one)
    for t in s.t.iter_mut() {
        if (t.kinds as u32 >> kind_bit(KINDOF_DOZER)) & 1 != 0 {
            t.new_flags |= F_AI;
        }
    }
    let builders: Vec<i32> = s.o.iter().filter(|o| o.id != 0).map(|o| o.builder).collect();
    for i in 0..AP_OBJECTS {
        let o = s.o[i];
        if o.id != 0 && ((s.t[o.tmpl as usize].kinds as u32 >> kind_bit(KINDOF_DOZER)) & 1 != 0 || builders.contains(&o.id)) {
            s.o[i].flags |= F_AI;
        }
    }
    // most of the build list's objects are player 0's working factories
    for e in s.b[..s.num_build as usize].iter().copied() {
        if let Some(o) = s.o.iter_mut().find(|o| o.id != 0 && o.id == e.object_id) {
            if g.below(3) != 0 {
                o.player = 0;
                o.flags |= F_PRODUCTION | F_BODY | F_EXIT | F_AI;
                o.flags &= !F_DEAD;
                o.status = 0;
            }
        }
    }
    for (k, side) in s.sides.iter_mut().enumerate() {
        side.side = k as i32;
        side.easy = g.below(4) as i32;
        side.normal = g.below(4) as i32;
        side.hard = g.below(4) as i32;
        for j in 0..5 {
            side.num_skills[j] = if g.below(3) == 0 { 0 } else { g.below(5) as i32 };
            side.skills[j] = [0; 4].map(|_: i32| g.below(8) as i32 - 1);
        }
        side.base_defense = g.below(AP_TEMPLATES as u32 + 1) as i32 - 1;
    }
    s.frame = g.below(2000) as i32;
    s.seed = g.u32() as i32;
    s.debug_ai = g.below(3) as i32;
    s.box_value = 50 + g.below(150) as i32;
    s.difficulty = g.below(3) as i32;
    s.skirmish = g.below(3) as i32;
    // batch A29: the factions' skirmish build lists (never empty: upstream copies the list unchecked), mostly command centre first;
    // the approach paths; the base rotation and defence distance
    let cc: Vec<i32> = (0..AP_TEMPLATES as i32).filter(|&t| (s.t[t as usize].kinds as u32 >> kind_bit(KINDOF_COMMANDCENTER)) & 1 != 0).collect();
    for k in 0..AP_SIDES {
        let n = 1 + g.below(6) as usize;
        s.side_build_count[k] = n as i32;
        for i in 0..n {
            let tmpl = if i == 0 && !cc.is_empty() && g.below(4) != 0 { cc[g.below(cc.len() as u32) as usize] } else { g.below(AP_TEMPLATES as u32 + 1) as i32 - 1 };
            s.side_build[k][i] = cpp::ApBuild {
                tmpl,
                object_id: 0,
                timestamp: 0,
                num_rebuilds: g.below(3) as i32,
                flags: g.below(64) as i32 & !B_UNDER_CONSTRUCTION,
                desired: g.below(4) as i32,
                current: 0,
                health: 100,
                name: g.below(6) as i32 - 1,
                script: g.below(8) as i32 - 2,
                loc: gen_pos(g),
                angle: g.uniform(-3.2, 3.2),
                rally: [g.uniform(-3.0, 3.0), g.uniform(-3.0, 3.0)],
            };
        }
    }
    for k in 0..3 {
        s.path_len[k] = g.below(5) as i32;
        for j in 0..4 {
            s.paths[k][j] = gen_pos(g);
        }
    }
    s.path_start = 1 + g.below(4) as i32;
    s.rotate_bases = g.below(2) as i32;
    s.defense_extra = g.uniform(0.0, 100.0);
    s.enemy = g.below(3) as i32;
    s.legal_pct = 10 + g.below(80) as i32;
    for p in 0..AP_PLAYERS {
        for q in 0..AP_PLAYERS {
            s.rel[p][q] = if p == q { 2 } else { g.below(3) as i32 };
        }
    }
    s.extent = [0.0, 0.0, g.uniform(500.0, 1000.0), g.uniform(500.0, 1000.0)];
    s.rebuild_delay_seconds = g.below(20) as i32;
    s.resources_poor = g.below(2000) as i32;
    s.resources_wealthy = 2000 + g.below(6000) as i32;
    s.structure_seconds = g.uniform(0.0, 10.0);
    s.structures_poor_mod = g.uniform(0.5, 2.0);
    s.structures_wealthy_mod = g.uniform(0.5, 2.0);
    s.team_resources_to_build = g.uniform(0.0, 1.0);
    s.max_recruit_distance = g.uniform(0.0, 800.0);
    s.safe_radius = g.uniform(0.0, 400.0);
    s.team_seconds = g.uniform(0.0, 20.0);
    s.team_poor_mod = g.uniform(0.5, 2.0);
    s.team_wealthy_mod = g.uniform(0.5, 2.0);
    s
}

/// Mode 3's scene: player 0 a skirmish AI with approach paths for its start, the other players skirmish AIs with enemies, some
/// of the build list's buildings power plants under construction, and a rebuild hole for an entry with its rebuilt building.
fn skirmish_scene(g: &mut Gen, s: &mut ApScene) {
    s.skirmish = 2;
    s.path_start = s.pl[0].mp_start + 1;
    for k in 0..3 {
        s.path_len[k] = 1 + g.below(4) as i32;
    }
    for p in 1..AP_PLAYERS {
        s.pl[p].flags |= PL_SKIRMISH;
        s.pl[p].enemy = g.below(AP_PLAYERS as u32) as i32;
    }
    for e in s.b[..s.num_build as usize].iter().copied() {
        if let Some(o) = s.o.iter_mut().find(|o| o.id != 0 && o.id == e.object_id && o.player == 0) {
            if g.below(3) == 0 {
                o.status |= 1 << 0; // under construction
                let t = o.tmpl as usize;
                s.t[t].kinds |= 1 << kind_bit(KINDOF_FS_POWER);
            }
        }
    }
    // a hole (its template a rebuild hole's) for an entry, and the building it spawned
    let live: Vec<usize> = (0..AP_OBJECTS).filter(|&i| s.o[i].id != 0).collect();
    if live.len() >= 2 && s.num_build > 0 {
        let (hi, bi) = (live[g.below(live.len() as u32) as usize], live[g.below(live.len() as u32) as usize]);
        if hi != bi {
            let ht = s.o[hi].tmpl as usize;
            s.t[ht].kinds |= 1 << kind_bit(KINDOF_REBUILD_HOLE);
            s.o[hi].flags |= F_HOLE;
            s.o[hi].reconstructed = s.o[bi].id;
            let e = g.below(s.num_build as u32) as usize;
            s.b[e].tmpl = s.o[bi].tmpl;
            s.b[e].object_id = s.o[hi].id;
        }
    }
}

/// The kinds of runs: 0 every op alike; 1 team building (updates, frames, production); 2 repairs (repair requests, the bridge
/// repair step, the dozers' tasks and the damage states); 3 a skirmish AI (its enemy, defences, cluster mines, build list, base
/// building).
fn gen_op(g: &mut Gen, s: &ApScene, mode: u32) -> ApOp {
    let kind = match (mode, g.below(10)) {
        (0, 0..=2) | (1, 0..=3) => 0, // update
        (1, 4..=6) => [23, 56, 56, 34, 33, 32, 31, 1, 58][g.below(9) as usize],
        (2, 0) => 9,
        (2, 1..=7) => 38,
        (2, 8) => [25, 57, 37, 0][g.below(4) as usize],
        (3, 0..=1) => [0, 30, 52, 60][g.below(4) as usize],
        (3, 2..=3) => [23, 60, 20, 6, 7, 1, 3, 61][g.below(8) as usize],
        (_, 3) => [1, 23, 23, 25, 26, 28, 55, 24, 56, 57][g.below(10) as usize],
        _ => g.below(62) as i32,
    };
    let any_id = |g: &mut Gen| -> i32 {
        let live: Vec<i32> = s.o.iter().filter(|o| o.id != 0).map(|o| o.id).collect();
        if live.is_empty() || g.below(6) == 0 {
            [0, 999, 1000, 1001][g.below(4) as usize]
        } else {
            live[g.below(live.len() as u32) as usize]
        }
    };
    let any_team = |g: &mut Gen| -> i32 { if g.below(5) == 0 { 77 } else { 1 + g.below(12) as i32 } };
    let (a, b) = match kind {
        // (in skirmish runs a rebuilt hole's building now and then)
        3 if mode == 3 && g.below(2) == 0 => (0, s.o.iter().find(|o| o.id != 0 && o.flags & F_HOLE != 0).map_or(0, |o| o.reconstructed)),
        2 | 3 => (any_id(g), any_id(g)),
        4 | 5 => (g.below(AP_PROTOS as u32 + 1) as i32 - (g.below(6) == 0) as i32, g.below(2) as i32),
        7 | 8 | 17 | 19 => (g.below(AP_TEMPLATES as u32 + 2) as i32 - 1, g.below(2) as i32),
        9 => (any_id(g), 0),
        10 => (g.below(6) as i32 - 1, 0),
        11 => (g.below(3000) as i32, g.below(AP_TEMPLATES as u32) as i32),
        12 => (g.below(AP_TEMPLATES as u32 + 1) as i32, any_team(g)),
        13 => (g.below(AP_UPGRADES as u32 + 1) as i32 - 1, 0),
        14 | 18 => (any_team(g), g.below(3000) as i32),
        15 | 40 => (g.below(3000) as i32, 0),
        21 | 22 => (g.below(AP_PLAYERS as u32) as i32, g.below(2) as i32),
        23 if mode == 3 => (150 + g.below(300) as i32, 0),
        23 => (if g.below(3) == 0 { g.below(1500) as i32 } else { g.below(60) as i32 }, 0),
        56 => (0, (g.u32() as i32) & (F_AI | F_SUPPLY_AI | F_IDLE | F_DOZER_AI)),
        // (in repair runs mostly the first structure to repair)
        57 if mode == 2 && g.below(2) == 0 => (-1, g.below(2) as i32),
        58 => (if g.below(2) == 0 { -1 } else { g.below(AP_OBJECTS as u32 / 2) as i32 }, g.below(4) as i32 - 1),
        6 => (g.below(2) as i32, 0),
        20 if mode == 3 => (2 + g.below(2) as i32, g.below(AP_PLAYERS as u32) as i32),
        20 => (g.below(4) as i32, g.below(AP_PLAYERS as u32) as i32),
        59 => (any_id(g), g.below(4) as i32),
        57 => (g.below(AP_OBJECTS as u32 / 2) as i32, g.below(4) as i32),
        24 => (g.below(AP_OBJECTS as u32 / 2) as i32, 0),
        // (in repair runs mostly the repair dozer's task flags)
        25 if mode == 2 && g.below(2) == 0 => (-1, F_TASK),
        25 => (g.below(AP_OBJECTS as u32 / 2) as i32, g.below(1 << 20) as i32),
        26 => (g.below(AP_PLAYERS as u32) as i32, g.below(9000) as i32),
        28 => (g.below(16) as i32, 0),
        44 => (g.below(9) as i32 - 1, 0),
        45 | 46 => (g.below(AP_PROTOS as u32) as i32, g.below(2) as i32),
        47 | 54 => (g.below(AP_TEMPLATES as u32) as i32, g.below(2) as i32),
        49 | 50 => (g.below(AP_TEMPLATES as u32) as i32, g.below(AP_BUILD as u32 + 1) as i32 - (g.below(8) == 0) as i32),
        51 => (any_id(g), g.below(AP_BUILD as u32 + 1) as i32),
        55 => (g.below(AP_PLAYERS as u32) as i32, g.below(4) as i32),
        _ => (g.below(4) as i32, g.below(2) as i32),
    };
    ApOp {
        kind,
        a,
        b,
        c: if kind == 25 { if g.below(2) == 0 { 0 } else { 1 << g.below(6) } } else { g.below(2) as i32 },
        x: if kind == 5 || kind == 20 { g.uniform(0.0, 600.0) } else { g.uniform(-100.0, 1100.0) },
        y: g.uniform(-100.0, 1100.0),
        z: g.uniform(0.0, 20.0),
        w: g.uniform(0.0, 600.0),
    }
}

#[test]
fn ai_player_matches_upstream() {
    assert_eq!(unsafe { cpp::ref_ai_player_scene_size() } as usize, std::mem::size_of::<ApScene>(), "ApScene's layout");
    let cases: u64 = std::env::var("AI_PLAYER_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(CASES);
    let mut results = Check::new("AI player results (each operation's answer; the AI player and the world after it)");
    let mut logs = Check::new("AI player calls (debug messages, scripts, AI commands, teams, the build assistant, random values)");
    let mut by_kind = [0u64; 62];
    let mut seen = [0u64; 8];
    let mut log_words = 0u64;
    let mut kinds = [0u64; 48];
    let (mut queued, mut ready) = (0u64, 0u64);
    let mut stale_cases = 0u64;
    for case in 0..cases {
        let mut g = Gen::new(0xA28_0000 + case);
        let s = gen_scene(&mut g);
        assert_eq!(ap_answer(s.seed as u32, 7, 3, 11), unsafe { cpp::apAnswer(s.seed as u32, 7, 3, 11) }, "the hash");
        let mode = g.below(4);
        let mut s = s;
        if mode == 3 {
            skirmish_scene(&mut g, &mut s);
        }
        // (a bridge repair checks once a second: 30 steps between its checks)
        let n = match mode {
            0 => 4 + g.below(21),
            1 => 4 + g.below(40),
            2 => 60 + g.below(60),
            _ => 30 + g.below(25),
        } as usize;
        let mut ops: Vec<ApOp> = (0..n).map(|_| gen_op(&mut g, &s, mode)).collect();
        if mode != 0 || g.below(2) == 0 {
            // player 0 builds units (the AI player's constructor turned it off)
            ops.insert(0, ApOp { kind: 55, a: 0, b: 1, ..Default::default() });
        }
        for op in &ops {
            by_kind[op.kind as usize] += 1;
        }
        if std::env::var("AI_PLAYER_TRACE").is_ok() {
            eprintln!("case {case} ops {:?}", ops.iter().map(|o| o.kind).collect::<Vec<_>>());
        }
        let (r, rl, stale) = rust_run(&s, &ops, &mut seen);
        let mut out = vec![0u32; 1 << 20];
        let mut log = vec![0u32; 1 << 20];
        let (mut out_len, mut log_len): (c_int, c_int) = (0, 0);
        let status = unsafe {
            cpp::ref_ai_player_run(&s, ops.as_ptr(), ops.len() as c_int, out.as_mut_ptr(), out.len() as c_int, &mut out_len, log.as_mut_ptr(), log.len() as c_int, &mut log_len)
        };
        assert_eq!(status, 0, "case {case}: the C++ run threw");
        assert!(out_len as usize <= out.len() && log_len as usize <= log.len(), "output too long");
        out.truncate(out_len as usize);
        log.truncate(log_len as usize);
        log_words += log.len() as u64;
        tally(&log, &mut kinds);
        let c = split(&out, ops.len());
        if stale {
            stale_cases += 1;
            if std::env::var("AI_PLAYER_STALE").is_ok() {
                println!("stale case {case}: ops {:?}", ops.iter().map(|o| o.kind).collect::<Vec<_>>());
            }
            continue;
        }
        // the ops after which a team was being built, or gathering
        for (_, d) in &r {
            let mut i = 1;
            for _ in 0..d[0] {
                i += 8 + 6 * d[i + 7] as usize;
            }
            queued += (d[0] > 0) as u64;
            ready += (d[i] > 0) as u64;
        }
        // an undefined answer is not compared
        let rr: Vec<(Vec<u32>, Vec<u32>)> = r.iter().zip(&c).map(|((ra, rd), (ca, _))| (ra.clone().unwrap_or_else(|| ca.clone()), rd.clone())).collect();
        if std::env::var("AI_PLAYER_DEBUG").is_ok() && (rr != c || rl != log) {
            let first = rr.iter().zip(&c).position(|(x, y)| x != y);
            println!("case {case}: ops {:?}\n first differing op {first:?}", ops.iter().map(|o| o.kind).collect::<Vec<_>>());
            if let Some(i) = first {
                let (ra, rd) = &rr[i];
                let (ca, cd) = &c[i];
                println!(" answer rust {ra:?} cpp {ca:?}");
                if let Some(j) = rd.iter().zip(cd).position(|(x, y)| x != y).or((rd.len() != cd.len()).then_some(rd.len().min(cd.len()))) {
                    let lo = j.saturating_sub(6);
                    println!(" dump differs at {j} (of {}, {}): rust {:?}\n cpp {:?}", rd.len(), cd.len(), &rd[lo..(j + 12).min(rd.len())], &cd[lo..(j + 12).min(cd.len())]);
                }
            }
            let lp = rl.iter().zip(&log).position(|(x, y)| x != y).unwrap_or(rl.len().min(log.len()));
            println!(" log differs at {lp}: rust {:?}\n log cpp {:?}", &rl[lp.saturating_sub(4)..(lp + 40).min(rl.len())], &log[lp.saturating_sub(4)..(lp + 40).min(log.len())]);
        }
        let tag = || format!("case {case} ops {ops:?}");
        results.eq(rr, c, tag);
        logs.eq(rl, log, tag);
    }
    println!("COVERAGE: {cases} cases; operations per kind {by_kind:?}; {log_words} log words");
    println!("COVERAGE (superweapon targets defined, undefined; xfer round trips; units produced; skirmish: base defences placed, enemies answered, cluster-mine targets): {seen:?}");
    println!("NOT COMPARED: {stale_cases} cases that used a deleted team (undefined upstream)");
    println!("COVERAGE (log entries by kind, AI commands by command from 33): {kinds:?}; ops after which teams were being built {queued}, gathering {ready}");
    results.finish();
    logs.finish();
}
