// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::world::team`, `team_ops` and `team_xfer` (`Team`, `TeamPrototype`,
//! `TeamFactory`, `TeamRelationMap`, `TeamTemplateInfo`, and `PlayerRelationMap`) against upstream's own
//! `Team.cpp` and `Team.h` (commit 0a05454d). Phase 1 layer 7, p1w9-sonnet.
//!
//! # What is compared against what
//!
//! `build.rs` takes the text of `Team.cpp` with only its include block replaced (asserted to occur once), `Team.h`
//! unchanged, `Player.h`'s `PlayerRelationMap` class and `Player.cpp`'s five `PlayerRelationMap` functions verbatim,
//! and a copy of `GameCommon.h` with one portability edit (clang reads the DLINK iterator's `OBJCLASS::dlink_next_X`
//! only as `&OBJCLASS::dlink_next_X`, as MSVC does), and `differential/wrap_team.cpp` compiles them under clang++ on
//! the stand-ins of `differential/shim_team`: objects (with the `DLINK` of a team's member list), players, the player
//! list, the thing templates, the AI, the contain module, the script engine, the terrain's waypoints, the partition
//! manager, the game logic and the sides list. Dict, the name key generator with its well-known keys, the Xfer classes
//! and the memory-pool shim are the real ones of the other archives. The Rust side runs the same program on `Sim`,
//! a [`TeamHost`] that answers from the same configuration.
//!
//! The stand-ins that stand for other units (what a player thinks of a team, what a script evaluates to, the
//! locomotor surfaces) are small formulas both sides compute the same way: they are not under test, what the team code
//! does with them is. Every call that has an effect (a script run, an object told to move teams, killed, damaged,
//! healed; a player told of a team) writes a line, formatted alike on both sides; calls that only answer a question
//! write none (their answers show in the lines they cause). After every operation both sides write the whole state:
//! the factory's counters and its name-key map, every prototype (with its template info), every team (with its
//! member list, flags and relation maps), every object's team and its `DLINK` neighbours, every player's default team
//! and prototype list. At the end, the name key of each team property name (so the order in which the lazily
//! created `TheKey_*` were first used is compared).
//!
//! # Method
//!
//! A **program** is a header (the players, thing templates, objects, scripts, waypoints and the dictionaries the
//! teams are made from) and a list of operations: the factory's `initTeam`, `initFromSides`, `reset`, `init`,
//! `clear`, `createInactiveTeam`, `createTeam`, `createTeamOnPrototype`, `findTeam`, `findTeamPrototype`,
//! `findTeamByID`, `findTeamPrototypeByID`, `teamAboutToBeDeleted`; an object joining a team (`Object::setTeam`'s
//! list half); every `Team` member function (40 of them); every `TeamPrototype` member function (24); a change to
//! an object (dead, destroyed, status, trigger bits, contain count, AI, position, vision, what happens to it when it
//! is killed, damaged, destroyed or emptied); and the factory's save (CRC, then bytes) and load (the objects then
//! put themselves on the teams they were on, then the post-process of every team and of the factory). Teams and
//! prototypes are named by position among those alive.
//!
//! The kill, damage, destroy and empty reactions send an object to the neutral player's default team, so a member
//! leaves the list that a loop is walking: the `DLINK_ITERATOR` then continues in the other team's list, as
//! upstream's does (`Team::deleteTeam`'s comment).
//!
//! **Inputs for which upstream is undefined** (a `NULL` controlling player dereferenced, a `Team*` that names no
//! team, a loop that never ends, an id shared by two live teams after a `reset()`) are never given to the C++: the
//! Rust returns [`TeamError`] and the program is counted as excluded. Programs that make upstream `throw`
//! (`ERROR_BAD_ARG`, `SC_INVALID_DATA`) are run on both sides up to the throw.
//!
//! # Results (2026-10-20, upstream `0a05454d`)
//!
//! `cargo test --release --features differential --test differential_team -- --nocapture` with `TEAM_CASES=3000`:
//! **3,000 programs, 428 excluded as undefined upstream, 4,410,485 trace lines compared, 0 mismatches.** The
//! exclusions are 308 programs that dereference a `NULL` team or object (almost all `tryToRecruit` meeting an object
//! that is on no team, upstream's `team->isActive()`), 2 whose player list names a prototype that a load left
//! behind, and 118 that give two live teams one id (`reset()` while a prototype outside the name-key map survives).
//! The programs contain 725 `ERROR_BAD_ARG` throws (carried on), 47 `SC_INVALID_DATA` throws (the program ends),
//! 1,367 saves and 908 loads, 46,829 `setTeam` calls, and every one of the 40 `Team` and 24 `TeamPrototype`
//! operations many times.
//!
//! **Planted bugs.** Each was applied to the Rust alone as a runtime switch (`TEAM_PLANT=<n>`, all sixty-one in one
//! build, the working tree restored afterwards), and the run repeated at 3,000 programs. All sixty-one are caught:
//!
//! - by trace mismatches (51; the smallest counts are plant 12 `didAllExit` ignoring `inside`: 1, plant 11
//!   `didAllEnter` ignoring `outside`: 3, plant 18 the `>` of `tryToRecruit` made `>=`: 5, plant 42 the build
//!   variation compare made case-insensitive: 6, plant 66 the held-unit test dropped: 6, plant 19 the
//!   default-team first pick: 7, plant 10 `locoSetMatches` shifting by 1: 12);
//! - by the tripwire in this test (1: plant 8, `Team::setID` that leaves the instance list under the old id, which
//!   makes the arena dereference teams that are not there; the test fails if more than 13 % of the programs do);
//! - by the process dying or not ending (9: plants 1, 2, 3 corrupt the `DLINK`s so a loop never ends, plants 25, 26,
//!   27, 34, 52, 62 corrupt the arena so the test crashes).
//!
//! Three rounds of the generator were needed: the first left 14 plants alive at 1,000 programs, the second 5; the
//! fixes were directed sequences, not more programs (a `setActive`-like prefix before the trigger queries, a
//! second `updateState` and a second production evaluation in a row, a get after each set of the common target,
//! save, change, load blocks that empty and delete a team, every object on a team from the start, a grid for
//! positions so that distances tie, lower-case build variations, a bias to `tryToRecruit`). Five plants that turned
//! out to be equivalent mutants (a clamp whose both sides assign the same value; `any_considered` that `exited`
//! implies; a `NULL` check that the next line makes redundant) were replaced by ones that change the result.
#![cfg(feature = "differential")]

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::dict::Dict;
use rts_lockstep_engine::common::game_common::{GameDifficulty, PlayerType, Relationship};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::kind_of::{
    KindOfMaskType, KindOfType, KINDOF_AIRCRAFT, KINDOF_INERT, KINDOF_INFANTRY, KINDOF_MINE, KINDOF_PROJECTILE, KINDOF_STRUCTURE, KINDOF_TECH_BUILDING,
    KINDOF_VEHICLE,
};
use rts_lockstep_engine::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use rts_lockstep_engine::common::object_status_types::{
    ObjectStatusTypes, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use rts_lockstep_engine::common::well_known_keys::{WellKnownKeys, WELL_KNOWN_KEY_NAMES};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::damage::{DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use rts_lockstep_engine::logic::sides_list::SidesList;
use rts_lockstep_engine::logic::world::ids::{PlayerIndex, TeamId, TeamPrototypeId};
use rts_lockstep_engine::logic::world::team::{ScriptRef, TeamArena, TeamError, WaypointId, WaypointInfo, MAX_GENERIC_SCRIPTS};
use rts_lockstep_engine::logic::world::team_ops::*;
use rts_lockstep_engine::logic::world::team_xfer::*;
use support::{Check, Gen};

/// The C++ side has process-global state (the name keys, the factory, the stand-in world), so the tests take turns.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ---------------------------------------------------------------------------------------------------------------------
// The words of a program
// ---------------------------------------------------------------------------------------------------------------------

struct Reader<'a> {
    w: &'a [u32],
    p: usize,
}

impl<'a> Reader<'a> {
    fn u(&mut self) -> u32 {
        let v = self.w.get(self.p).copied().unwrap_or(0);
        self.p += 1;
        v
    }
    fn i(&mut self) -> i32 {
        self.u() as i32
    }
    fn b(&mut self) -> bool {
        self.u() != 0
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.u())
    }
    fn s(&mut self) -> Vec<u8> {
        let n = self.u();
        (0..n).map(|_| self.u() as u8).collect()
    }
    fn c(&mut self) -> Coord3D {
        let x = self.f();
        let y = self.f();
        let z = self.f();
        Coord3D { x, y, z }
    }
}

#[derive(Default)]
struct W(Vec<u32>);

impl W {
    fn u(&mut self, v: u32) {
        self.0.push(v);
    }
    fn i(&mut self, v: i32) {
        self.0.push(v as u32);
    }
    fn b(&mut self, v: bool) {
        self.0.push(v as u32);
    }
    fn f(&mut self, v: f32) {
        self.0.push(v.to_bits());
    }
    fn c(&mut self, v: [f32; 3]) {
        for x in v {
            self.f(x);
        }
    }
    fn s(&mut self, v: &str) {
        self.u(v.len() as u32);
        for b in v.bytes() {
            self.u(u32::from(b));
        }
    }
}

fn fb(f: f32) -> String {
    format!("{:08x}", f.to_bits())
}

// ---------------------------------------------------------------------------------------------------------------------
// The world of a program: the same configuration as differential/shim_team
// ---------------------------------------------------------------------------------------------------------------------

const KIND_POOL: [KindOfType; 8] = [KINDOF_STRUCTURE, KINDOF_PROJECTILE, KINDOF_MINE, KINDOF_INERT, KINDOF_TECH_BUILDING, KINDOF_AIRCRAFT, KINDOF_INFANTRY, KINDOF_VEHICLE];
const STATUS_POOL: [ObjectStatusTypes; 4] = [OBJECT_STATUS_STEALTHED, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_UNDER_CONSTRUCTION];

fn kind_mask(bits: u32) -> KindOfMaskType {
    let mut m = KindOfMaskType::new();
    for (k, kind) in KIND_POOL.iter().enumerate() {
        if bits & (1 << k) != 0 {
            m.set(*kind, 1);
        }
    }
    m
}

struct TplSim {
    kinds: KindOfMaskType,
    build_facility: bool,
    equiv: i32,
    name: Vec<u8>,
    variations: Vec<Vec<u8>>,
}

struct PlayerSim {
    ty: PlayerType,
    diff: GameDifficulty,
    rel_salt: u32,
    beacon: Vec<u8>,
    default_team: Option<TeamId>,
    protos: Vec<TeamPrototypeId>,
}

struct ObjSim {
    id: ObjectId,
    tpl: usize,
    player: usize,
    status: BTreeSet<ObjectStatusTypes>,
    dead: bool,
    destroyed: bool,
    contained: bool,
    held: bool,
    enemy_near: bool,
    has_ai: bool,
    ai_idle: bool,
    ai_recruit: bool,
    surfaces: u32,
    contain_count: i32,
    pos: Coord3D,
    vision: f32,
    did_enter: u32,
    did_exit: u32,
    inside: u32,
    reactions: u32,
    team: Option<TeamId>,
}

#[derive(Clone)]
struct ScriptSim {
    serial: u32,
    name: Vec<u8>,
    frame: u32,
    delay: i32,
    easy: bool,
    normal: bool,
    hard: bool,
    one_shot: bool,
    pattern: u32,
    calls: u32,
}

#[derive(Clone)]
struct DictEntry {
    ty: i32,
    key: Vec<u8>,
    bits: u32,
    s: Vec<u8>,
}

struct Sim {
    teams: TeamArena,
    names: NameKeyGenerator,
    keys: WellKnownKeys,
    lines: Vec<String>,
    frame: u32,
    players: Vec<PlayerSim>,
    player_by_key: BTreeMap<u32, usize>,
    tpls: Vec<TplSim>,
    objs: Vec<ObjSim>,
    scripts: Vec<ScriptSim>,
    originals: usize,
    ways: Vec<(WaypointInfo, WaypointId)>,
    dicts: Vec<Vec<DictEntry>>,
    // the last save
    saved: Vec<u8>,
    have_save: bool,
    saved_obj_team: BTreeMap<ObjectId, TeamId>,
    /// A player's prototype list names a prototype that no longer exists (upstream: a dangling pointer).
    dangling: std::cell::Cell<bool>,
}

fn tid(t: Option<TeamId>) -> u32 {
    t.unwrap_or(0)
}

impl Sim {
    fn rec(&mut self, s: String) {
        self.lines.push(s);
    }
    fn obj(&self, o: ObjectId) -> &ObjSim {
        self.objs.iter().find(|x| x.id == o).expect("object")
    }
    fn obj_mut(&mut self, o: ObjectId) -> &mut ObjSim {
        self.objs.iter_mut().find(|x| x.id == o).expect("object")
    }
    fn tpl_of(&self, o: ObjectId) -> &TplSim {
        &self.tpls[self.obj(o).tpl]
    }
    /// `applyReaction`: the object moves to the neutral player's default team.
    fn apply_reaction(&mut self, o: ObjectId, bit: u32) {
        if self.obj(o).reactions & bit != 0 {
            let t = self.players[0].default_team;
            self.obj_set_team(o, t);
        }
    }
    fn build_dict(&mut self, entries: &[DictEntry]) -> Dict {
        let mut d = Dict::new();
        for e in entries {
            let k = self.names.name_to_key(&e.key);
            match e.ty {
                0 => d.set_bool(k, e.bits != 0).unwrap(),
                1 => d.set_int(k, e.bits as i32).unwrap(),
                2 => d.set_real(k, f32::from_bits(e.bits)).unwrap(),
                _ => d.set_ascii_string(k, &e.s).unwrap(),
            }
        }
        d
    }
}

impl TeamHost for Sim {
    fn teams(&self) -> &TeamArena {
        &self.teams
    }
    fn teams_mut(&mut self) -> &mut TeamArena {
        &mut self.teams
    }
    fn names(&mut self) -> (&mut NameKeyGenerator, &WellKnownKeys) {
        (&mut self.names, &self.keys)
    }
    fn frame(&self) -> u32 {
        self.frame
    }
    fn game_logic_objects(&self) -> Vec<ObjectId> {
        self.objs.iter().map(|o| o.id).collect()
    }
    fn game_logic_find_object(&self, id: ObjectId) -> bool {
        self.objs.iter().any(|o| o.id == id)
    }
    fn game_logic_destroy_object(&mut self, o: ObjectId) {
        self.rec(format!("destroy {o}"));
        self.apply_reaction(o, 2);
    }
    fn player_list_find_with_name_key(&mut self, key: NameKeyType) -> Option<PlayerIndex> {
        self.player_by_key.get(&key.0).map(|i| *i as PlayerIndex)
    }
    fn player_list_neutral(&self) -> Option<PlayerIndex> {
        Some(0)
    }
    fn player_list_count(&self) -> i32 {
        self.players.len() as i32
    }
    fn player_list_nth(&self, i: i32) -> Option<PlayerIndex> {
        if i >= 0 && (i as usize) < self.players.len() {
            Some(i)
        } else {
            None
        }
    }
    fn player_list_team_about_to_be_deleted(&mut self, team: TeamId) {
        self.rec(format!("pl.about team={team}"));
    }
    fn player_add_team_to_list(&mut self, p: PlayerIndex, proto: TeamPrototypeId) {
        self.rec(format!("p.add {p} proto={proto}"));
        self.players[p as usize].protos.push(proto);
    }
    fn player_remove_team_from_list(&mut self, p: PlayerIndex, proto: TeamPrototypeId) {
        self.rec(format!("p.remove {p} proto={proto}"));
        let v = &mut self.players[p as usize].protos;
        if let Some(i) = v.iter().position(|x| *x == proto) {
            v.remove(i);
        }
    }
    fn player_pre_team_destroy(&mut self, p: PlayerIndex, team: TeamId) {
        self.rec(format!("p.pre {p} team={team}"));
        // a player's default team that is deleted reads as none afterwards (the stand-in Player; see team_driver.h)
        if self.players[p as usize].default_team == Some(team) {
            self.players[p as usize].default_team = None;
        }
    }
    fn player_default_team(&self, p: PlayerIndex) -> Option<TeamId> {
        self.players[p as usize].default_team
    }
    fn player_relationship(&self, p: PlayerIndex, that: Option<TeamId>) -> Relationship {
        let v = self.players[p as usize].rel_salt.wrapping_add(5u32.wrapping_mul(p as u32)).wrapping_add(11u32.wrapping_mul(that.unwrap_or(0))) % 3;
        match v {
            0 => Relationship::Enemies,
            1 => Relationship::Neutral,
            _ => Relationship::Allies,
        }
    }
    fn player_type(&self, p: PlayerIndex) -> PlayerType {
        self.players[p as usize].ty
    }
    fn player_difficulty(&self, p: PlayerIndex) -> GameDifficulty {
        self.players[p as usize].diff
    }
    fn player_beacon_template(&self, p: PlayerIndex) -> Option<TemplateRef> {
        let b = &self.players[p as usize].beacon;
        self.tpls.iter().position(|t| t.name == *b).map(|i| i as TemplateRef)
    }
    fn script_find_by_name(&mut self, name: &[u8]) -> Option<ScriptRef> {
        self.scripts[..self.originals].iter().find(|s| s.name == name).map(|s| s.serial)
    }
    fn script_duplicate(&mut self, s: ScriptRef) -> ScriptRef {
        let mut c = self.scripts[s as usize - 1].clone();
        c.serial = self.scripts.len() as u32 + 1;
        c.calls = 0;
        let serial = c.serial;
        self.scripts.push(c);
        self.rec(format!("script.dup {s} {serial}"));
        serial
    }
    fn script_delete(&mut self, s: ScriptRef) {
        self.rec(format!("script.delete {s}"));
    }
    fn script_frame_to_evaluate(&self, s: ScriptRef) -> u32 {
        self.scripts[s as usize - 1].frame
    }
    fn script_set_frame_to_evaluate(&mut self, s: ScriptRef, frame: u32) {
        self.rec(format!("script.frame {s} {frame}"));
        self.scripts[s as usize - 1].frame = frame;
    }
    fn script_delay_eval_seconds(&self, s: ScriptRef) -> i32 {
        self.scripts[s as usize - 1].delay
    }
    fn script_is_easy(&self, s: ScriptRef) -> bool {
        self.scripts[s as usize - 1].easy
    }
    fn script_is_normal(&self, s: ScriptRef) -> bool {
        self.scripts[s as usize - 1].normal
    }
    fn script_is_hard(&self, s: ScriptRef) -> bool {
        self.scripts[s as usize - 1].hard
    }
    fn script_is_one_shot(&self, s: ScriptRef) -> bool {
        self.scripts[s as usize - 1].one_shot
    }
    fn script_name(&self, s: ScriptRef) -> Vec<u8> {
        self.scripts[s as usize - 1].name.clone()
    }
    fn script_evaluate_conditions(&mut self, s: ScriptRef, team: Option<TeamId>, player: Option<PlayerIndex>) -> bool {
        let sc = &mut self.scripts[s as usize - 1];
        let r = (sc.pattern >> (sc.calls & 31)) & 1 != 0;
        sc.calls += 1;
        self.rec(format!("eval {s} team={} player={} -> {}", tid(team), player.unwrap_or(-1), r as u32));
        r
    }
    fn script_execute_action(&mut self, s: ScriptRef, team: Option<TeamId>) {
        self.rec(format!("exec {s} team={}", tid(team)));
    }
    fn script_run(&mut self, name: &[u8], team: TeamId) {
        self.rec(format!("run {} team={team}", String::from_utf8_lossy(name)));
    }
    fn script_notify_of_team_destruction(&mut self, team: TeamId) {
        self.rec(format!("notify-destroy team={team}"));
    }
    fn script_append_debug_message(&mut self, msg: &[u8], force_pause: bool) {
        self.rec(format!("dbg {} {}", String::from_utf8_lossy(msg), force_pause as u32));
    }
    fn terrain_waypoints(&self) -> Vec<WaypointInfo> {
        self.ways.iter().map(|(w, _)| w.clone()).collect()
    }
    fn terrain_waypoint_by_id(&self, id: WaypointId) -> Option<WaypointId> {
        self.ways.iter().find(|(_, i)| *i == id).map(|(_, i)| *i)
    }
    fn obj_template(&self, o: ObjectId) -> Option<TemplateRef> {
        Some(self.obj(o).tpl as TemplateRef)
    }
    fn tmpl_is_equivalent_to(&self, a: TemplateRef, b: Option<TemplateRef>) -> bool {
        match b {
            None => false,
            Some(b) => a == b || self.tpls[a as usize].equiv == self.tpls[b as usize].equiv,
        }
    }
    fn tmpl_is_kind_of(&self, t: TemplateRef, kind: KindOfType) -> bool {
        self.tpls[t as usize].kinds.test(kind)
    }
    fn tmpl_is_kind_of_multi(&self, t: TemplateRef, set: &KindOfMaskType, clear: &KindOfMaskType) -> bool {
        self.tpls[t as usize].kinds.test_set_and_clear(set, clear)
    }
    fn tmpl_is_build_facility(&self, t: TemplateRef) -> bool {
        self.tpls[t as usize].build_facility
    }
    fn tmpl_name(&self, t: TemplateRef) -> Vec<u8> {
        self.tpls[t as usize].name.clone()
    }
    fn tmpl_build_variations(&self, t: TemplateRef) -> Vec<Vec<u8>> {
        self.tpls[t as usize].variations.clone()
    }
    fn obj_is_effectively_dead(&self, o: ObjectId) -> bool {
        self.obj(o).dead
    }
    fn obj_is_destroyed(&self, o: ObjectId) -> bool {
        self.obj(o).destroyed
    }
    fn obj_is_kind_of(&self, o: ObjectId, kind: KindOfType) -> bool {
        self.tpl_of(o).kinds.test(kind)
    }
    fn obj_is_kind_of_multi(&self, o: ObjectId, set: &KindOfMaskType, clear: &KindOfMaskType) -> bool {
        self.tpl_of(o).kinds.test_set_and_clear(set, clear)
    }
    fn obj_test_status(&self, o: ObjectId, status: ObjectStatusTypes) -> bool {
        self.obj(o).status.contains(&status)
    }
    fn obj_is_disabled_held(&self, o: ObjectId) -> bool {
        self.obj(o).held
    }
    fn obj_is_contained(&self, o: ObjectId) -> bool {
        self.obj(o).contained
    }
    fn obj_controlling_player(&self, o: ObjectId) -> Option<PlayerIndex> {
        Some(self.obj(o).player as PlayerIndex)
    }
    fn obj_team(&self, o: ObjectId) -> Option<TeamId> {
        self.obj(o).team
    }
    fn obj_position(&self, o: ObjectId) -> Coord3D {
        self.obj(o).pos
    }
    fn obj_vision_range(&self, o: ObjectId) -> f32 {
        self.obj(o).vision
    }
    fn obj_heal_completely(&mut self, o: ObjectId) {
        self.rec(format!("heal {o}"));
    }
    fn obj_kill(&mut self, o: ObjectId) {
        self.rec(format!("kill {o}"));
        self.apply_reaction(o, 1);
    }
    fn obj_attempt_damage(&mut self, o: ObjectId, amount: f32) {
        self.rec(format!("damage {o} amt={} type={} death={} src={}", fb(amount), DAMAGE_UNRESISTABLE, DEATH_NORMAL, 0));
        self.apply_reaction(o, 8);
    }
    fn obj_set_team(&mut self, o: ObjectId, team: Option<TeamId>) {
        self.rec(format!("setteam obj={o} team={}", tid(team)));
        let cur = self.obj(o).team;
        if cur == team {
            return;
        }
        if let Some(c) = cur {
            if self.teams.is_in_member_list(c, o) {
                self.teams.remove_member(c, o);
            }
        }
        self.obj_mut(o).team = team;
        if let Some(n) = team {
            if !self.teams.is_in_member_list(n, o) {
                self.teams.prepend_member(n, o);
            }
        }
    }
    fn obj_handle_partition_cell_maintenance(&mut self, o: ObjectId) {
        self.rec(format!("pcm {o}"));
    }
    fn obj_contain_count(&self, o: ObjectId) -> Option<i32> {
        let c = self.obj(o).contain_count;
        if c >= 0 {
            Some(c)
        } else {
            None
        }
    }
    fn obj_remove_all_contained(&mut self, o: ObjectId) {
        self.rec(format!("remove-all-contained {o}"));
        self.apply_reaction(o, 4);
    }
    fn obj_did_enter(&self, o: ObjectId, t: TriggerRef) -> bool {
        (self.obj(o).did_enter >> t) & 1 != 0
    }
    fn obj_did_exit(&self, o: ObjectId, t: TriggerRef) -> bool {
        (self.obj(o).did_exit >> t) & 1 != 0
    }
    fn obj_is_inside(&self, o: ObjectId, t: TriggerRef) -> bool {
        (self.obj(o).inside >> t) & 1 != 0
    }
    fn obj_has_ai(&self, o: ObjectId) -> bool {
        self.obj(o).has_ai
    }
    fn ai_is_idle(&self, o: ObjectId) -> bool {
        self.obj(o).ai_idle
    }
    fn ai_valid_surfaces(&self, o: ObjectId) -> u32 {
        self.obj(o).surfaces
    }
    fn ai_is_recruitable(&self, o: ObjectId) -> bool {
        self.obj(o).ai_recruit
    }
    fn ai_group_add(&mut self, group: GroupRef, o: ObjectId) {
        self.rec(format!("group.add {group} {o}"));
    }
    fn team_renamed(&mut self, old: TeamId, new: TeamId) {
        for o in &mut self.objs {
            if o.team == Some(old) {
                o.team = Some(new);
            }
        }
        for p in &mut self.players {
            if p.default_team == Some(old) {
                p.default_team = Some(new);
            }
        }
    }
    fn partition_closest_enemy(&mut self, o: ObjectId, range: f32) -> bool {
        self.rec(format!("closest {o} range={}", fb(range)));
        self.obj(o).enemy_near
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The state dump (the same text as dumpState of team_driver.h)
// ---------------------------------------------------------------------------------------------------------------------

fn id_list<T: std::fmt::Display>(v: &[T]) -> String {
    if v.is_empty() {
        return "-".into();
    }
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
}

fn rel_map_str<K: std::fmt::Display>(m: &BTreeMap<K, Relationship>) -> String {
    if m.is_empty() {
        return "-".into();
    }
    m.iter().map(|(k, v)| format!("{k}:{}", *v as i32)).collect::<Vec<_>>().join(",")
}

/// The prototypes alive: the union of the players' lists, in id order.
fn alive_protos(sim: &Sim) -> Vec<TeamPrototypeId> {
    let mut v: Vec<TeamPrototypeId> = Vec::new();
    for p in &sim.players {
        for pr in &p.protos {
            if !v.contains(pr) {
                v.push(*pr);
            }
        }
    }
    v.sort();
    // a prototype deleted while a player still lists it (a load assigns the owner without telling the players)
    let before = v.len();
    v.retain(|p| sim.teams.prototype(*p).is_some());
    if v.len() != before {
        sim.dangling.set(true);
    }
    v
}

/// The teams alive: every instance list of the prototypes alive, in id order.
fn alive_teams(sim: &Sim) -> Vec<TeamId> {
    let mut v = Vec::new();
    for p in alive_protos(sim) {
        v.extend(sim.teams.instances(p));
    }
    v.sort();
    v
}

fn dump_state(sim: &mut Sim) {
    let mut out: Vec<String> = Vec::new();
    out.push(format!(
        "D frame={} tid={} pid={} nprotos={}",
        sim.frame,
        sim.teams.last_team_id(),
        sim.teams.last_prototype_id(),
        sim.teams.prototype_count()
    ));
    for (nk, p) in sim.teams.by_key() {
        out.push(format!("D map nk={} proto={}", nk.0, p));
    }
    for pid in alive_protos(sim) {
        let p = sim.teams.prototype(pid).expect("alive prototype");
        let insts = sim.teams.instances(pid);
        let gens = p.generic_scripts_to_run.iter().map(|s| s.unwrap_or(0).to_string()).collect::<Vec<_>>().join(",");
        let t = &p.team_template;
        let units = if t.num_units_info == 0 {
            "-".to_string()
        } else {
            (0..t.num_units_info as usize)
                .map(|k| format!("{}/{}/{}", t.units_info[k].min_units, t.units_info[k].max_units, String::from_utf8_lossy(&t.units_info[k].unit_thing_name)))
                .collect::<Vec<_>>()
                .join(",")
        };
        out.push(format!(
            "D proto id={} name={} owner={} flags={} fals={} cond={} retr={} gen={} head={} insts={} attack={}",
            p.id,
            String::from_utf8_lossy(&p.name),
            p.owning_player.unwrap_or(-1),
            p.flags,
            p.production_condition_always_false as u32,
            p.production_condition_script.unwrap_or(0),
            p.retrieved_generic_scripts as u32,
            gens,
            tid(sim.teams.first_instance(pid)),
            id_list(&insts),
            String::from_utf8_lossy(&p.attack_priority_name)
        ));
        out.push(format!(
            "D tmpl id={} units={} n={} home={},{},{} has={} idle={} dthr={} rec={} bd={} pd={} ar={} tr={} av={} ac={} maxi={} prio={} psi={} pfd={} att={}",
            p.id,
            units,
            t.num_units_info,
            fb(t.home_location.x),
            fb(t.home_location.y),
            fb(t.home_location.z),
            t.has_home_location as u32,
            t.initial_idle_frames,
            fb(t.destroyed_threshold),
            t.is_ai_recruitable as u32,
            t.is_base_defense as u32,
            t.is_perimeter_defense as u32,
            t.automatically_reinforce as u32,
            t.transports_return as u32,
            t.avoid_threats as u32,
            t.attack_common_target as u32,
            t.max_instances,
            t.production_priority,
            t.production_priority_success_increase,
            t.production_priority_failure_decrease,
            t.initial_team_attitude
        ));
        let s = |v: &[u8]| String::from_utf8_lossy(v).into_owned();
        let g: String = t.team_generic_scripts.iter().map(|x| format!("{};", s(x))).collect();
        out.push(format!(
            "D tmpl2 id={} create={} idlescr={} sight={} clear={} unitdead={} dead={} trans={} wp={} full={} exit={} vet={} cond={} exec={} gs={}",
            p.id,
            s(&t.script_on_create),
            s(&t.script_on_idle),
            s(&t.script_on_enemy_sighted),
            s(&t.script_on_all_clear),
            s(&t.script_on_unit_destroyed),
            s(&t.script_on_destroyed),
            s(&t.transport_unit_type),
            s(&t.start_reinforce_waypoint),
            t.team_starts_full as u32,
            t.transports_exit as u32,
            t.veterancy,
            s(&t.production_condition),
            t.execute_actions as u32,
            g
        ));
    }
    for id in alive_teams(sim) {
        let t = sim.teams.team(id).expect("alive team");
        let mem = sim.teams.members(id);
        let gen: String = t.should_attempt_generic_script.iter().map(|b| if *b { '1' } else { '0' }).collect();
        let tr: BTreeMap<u32, Relationship> = t.team_relations.map.clone();
        let pr: BTreeMap<i32, Relationship> = t.player_relations.map.clone();
        out.push(format!(
            "D team id={} proto={} head={} members={} state={} eoe={} act={} cre={} ces={} see={} psee={} widle={} dth={} cur={} way={} gen={} rs={} r={} tgt={} trel={} prel={} xm={}",
            t.id,
            t.proto,
            mem.first().copied().unwrap_or(0),
            id_list(&mem),
            String::from_utf8_lossy(&t.state),
            t.entered_or_exited as u32,
            t.active as u32,
            t.created as u32,
            t.check_enemy_sighted as u32,
            t.see_enemy as u32,
            t.prev_see_enemy as u32,
            t.was_idle as u32,
            t.destroy_threshold,
            t.cur_units,
            t.current_waypoint.unwrap_or(0),
            gen,
            t.is_recruitability_set as u32,
            t.is_recruitable as u32,
            t.common_attack_target,
            rel_map_str(&tr),
            rel_map_str(&pr),
            id_list(&t.xfer_member_id_list)
        ));
    }
    for o in &sim.objs {
        out.push(format!(
            "D obj id={} team={} next={} prev={}",
            o.id,
            tid(o.team),
            sim.teams.next_member(o.id).unwrap_or(0),
            sim.teams.prev_member(o.id).unwrap_or(0)
        ));
    }
    for (i, p) in sim.players.iter().enumerate() {
        out.push(format!("D player {i} default={} protos={}", tid(p.default_team), id_list(&p.protos)));
    }
    sim.lines.extend(out);
}

// ---------------------------------------------------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------------------------------------------------

struct Excluded(&'static str);

fn hash_bytes(v: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for &b in v {
        h = (h ^ u32::from(b)).wrapping_mul(16777619);
    }
    h
}

fn team_name(id: u32) -> Vec<u8> {
    format!("Team{id}").into_bytes()
}

fn count_str(c: &[i32]) -> String {
    c.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
}

/// How an error ends a program: a throw upstream makes (the line to write), or an exclusion.
fn thrown(e: TeamError) -> Result<String, Excluded> {
    match e {
        TeamError::BadArg => Ok("threw badarg".to_string()),
        TeamError::InvalidData => Ok("threw invaliddata".to_string()),
        TeamError::Xfer(s) => Ok(format!("threw xfer {}", s as i32)),
        TeamError::Null => Err(Excluded("a NULL or dangling pointer dereferenced")),
        TeamError::Hang => Err(Excluded("a loop that never ends")),
        TeamError::IdCollision => Err(Excluded("an id shared by two live teams after reset")),
    }
}

fn pick_team(sim: &Sim, k: u32) -> Option<TeamId> {
    let v = alive_teams(sim);
    if v.is_empty() {
        None
    } else {
        Some(v[k as usize % v.len()])
    }
}

fn pick_proto(sim: &Sim, k: u32) -> Option<TeamPrototypeId> {
    let v = alive_protos(sim);
    if v.is_empty() {
        None
    } else {
        Some(v[k as usize % v.len()])
    }
}

fn read_sim(r: &mut Reader<'_>) -> (Sim, Vec<Vec<u8>>) {
    let mut names = NameKeyGenerator::new();
    names.reset();
    let mut sim = Sim {
        teams: TeamArena::new(),
        names,
        keys: WellKnownKeys::new(),
        lines: Vec::new(),
        frame: 0,
        players: Vec::new(),
        player_by_key: BTreeMap::new(),
        tpls: Vec::new(),
        objs: Vec::new(),
        scripts: Vec::new(),
        originals: 0,
        ways: Vec::new(),
        dicts: Vec::new(),
        saved: Vec::new(),
        have_save: false,
        saved_obj_team: BTreeMap::new(),
        dangling: std::cell::Cell::new(false),
    };
    let np = r.i();
    for i in 0..np {
        let ty = if r.i() == 0 { PlayerType::Human } else { PlayerType::Computer };
        let diff = match r.i() {
            0 => GameDifficulty::Easy,
            1 => GameDifficulty::Normal,
            _ => GameDifficulty::Hard,
        };
        let rel_salt = r.u();
        let bt = r.i();
        let beacon = if bt >= 0 { format!("Tpl{bt}").into_bytes() } else { b"NoSuchTemplate".to_vec() };
        sim.players.push(PlayerSim { ty, diff, rel_salt, beacon, default_team: None, protos: Vec::new() });
        let k = sim.names.name_to_key(format!("Player{i}").as_bytes());
        sim.player_by_key.insert(k.0, i as usize);
    }
    let nt = r.i();
    for _ in 0..nt {
        let nk = r.i();
        let mut kinds = KindOfMaskType::new();
        for _ in 0..nk {
            kinds.set(KIND_POOL[(r.u() % 8) as usize], 1);
        }
        let build_facility = r.b();
        let equiv = r.i();
        let name = format!("Tpl{}", r.i()).into_bytes();
        let nv = r.i();
        let variations = (0..nv)
            .map(|_| {
                let v = r.i();
                if v >= 8 {
                    format!("tpl{}", v - 8).into_bytes()
                } else {
                    format!("Tpl{v}").into_bytes()
                }
            })
            .collect();
        sim.tpls.push(TplSim { kinds, build_facility, equiv, name, variations });
    }
    let no = r.i();
    for _ in 0..no {
        let id = r.u() as ObjectId;
        let tpl = r.u() as usize % sim.tpls.len();
        let player = r.u() as usize % sim.players.len();
        let ns = r.i();
        let status = (0..ns).map(|_| STATUS_POOL[(r.u() % 4) as usize]).collect();
        let (dead, destroyed, contained, held, enemy_near) = (r.b(), r.b(), r.b(), r.b(), r.b());
        let (has_ai, ai_idle, ai_recruit) = (r.b(), r.b(), r.b());
        let surfaces = r.u();
        let contain_count = r.i();
        let pos = r.c();
        let vision = r.f();
        let (did_enter, did_exit, inside, reactions) = (r.u(), r.u(), r.u(), r.u());
        sim.objs.push(ObjSim {
            id,
            tpl,
            player,
            status,
            dead,
            destroyed,
            contained,
            held,
            enemy_near,
            has_ai,
            ai_idle,
            ai_recruit,
            surfaces,
            contain_count,
            pos,
            vision,
            did_enter,
            did_exit,
            inside,
            reactions,
            team: None,
        });
    }
    let ns = r.i();
    sim.originals = ns as usize;
    for i in 0..ns {
        let frame = r.u();
        let delay = r.i();
        let (easy, normal, hard, one_shot) = (r.b(), r.b(), r.b(), r.b());
        let pattern = r.u();
        sim.scripts.push(ScriptSim { serial: i as u32 + 1, name: format!("Scr{i}").into_bytes(), frame, delay, easy, normal, hard, one_shot, pattern, calls: 0 });
    }
    let nw = r.i();
    for _ in 0..nw {
        let name = format!("Way{}", r.i()).into_bytes();
        let id = r.u();
        let location = r.c();
        sim.ways.push((WaypointInfo { name, location }, id));
    }
    let nd = r.i();
    for _ in 0..nd {
        let ne = r.i();
        let mut entries = Vec::new();
        for _ in 0..ne {
            let ty = r.i();
            let key = r.s();
            let (mut bits, mut s) = (0, Vec::new());
            if ty == 3 {
                s = r.s();
            } else {
                bits = r.u();
            }
            entries.push(DictEntry { ty, key, bits, s });
        }
        sim.dicts.push(entries);
    }
    sim.frame = r.u();
    let n_probe = r.i();
    let probes = (0..n_probe).map(|_| r.s()).collect();
    (sim, probes)
}

/// Runs the Rust side of a program, giving the trace lines (as the C++ side writes them), or why it cannot be given
/// to the C++.
fn run_rust(words: &[u32], tmp: &mut Vec<u8>) -> Result<Vec<String>, Excluded> {
    let _ = tmp;
    let mut r = Reader { w: words, p: 0 };
    let (mut sim, probes) = read_sim(&mut r);
    let n_ops = r.i();
    let mut threw = false;
    for _ in 0..n_ops {
        let op = r.u();
        let args_at = r.p;
        match do_op(&mut sim, op, &mut r) {
            Ok(()) => {
                dump_state(&mut sim);
                if sim.dangling.get() {
                    return Err(Excluded("a player's list names a prototype that was deleted"));
                }
            }
            Err(e) => {
                if std::env::var_os("TEAM_WHY").is_some() {
                    eprintln!("WHY {e:?} at op {op} sub {}", if op == 15 || op == 16 { words.get(args_at + 1).copied().unwrap_or(0) } else { 0 });
                }
                let line = thrown(e)?;
                sim.rec(line);
                // `ERROR_BAD_ARG` is thrown before anything is changed: the program goes on
                if e == TeamError::BadArg {
                    dump_state(&mut sim);
                    continue;
                }
                threw = true;
                break;
            }
        }
    }
    if !threw {
        for p in &probes {
            let k = sim.names.name_to_key(p);
            sim.rec(format!("K {} {}", String::from_utf8_lossy(p), k.0));
        }
    }
    Ok(sim.lines)
}

fn thing_list(sim: &Sim, a: &[u32; 8]) -> (Vec<Option<TemplateRef>>, Vec<i32>) {
    let n = (a[0] % 5) as usize;
    let things = (0..n).map(|k| if a[1 + k] != 0 { Some(((a[1 + k] - 1) as usize % sim.tpls.len()) as TemplateRef) } else { None }).collect();
    (things, vec![0; n])
}

fn do_op(sim: &mut Sim, op: u32, r: &mut Reader<'_>) -> Result<(), TeamError> {
    let np = sim.players.len();
    match op {
        0 => {
            let n = r.u();
            sim.frame = sim.frame.wrapping_add(n);
        }
        1 => {
            let (name_id, owner_sel, singleton, dict_idx) = (r.u(), r.u(), r.b(), r.u());
            let owner = if (owner_sel as usize % (np + 1)) < np { format!("Player{}", owner_sel as usize % (np + 1)) } else { "Nobody".to_string() };
            let d = if sim.dicts.is_empty() {
                Dict::new()
            } else {
                let spec = sim.dicts[dict_idx as usize % sim.dicts.len()].clone();
                sim.build_dict(&spec)
            };
            factory_init_team(sim, &team_name(name_id), owner.as_bytes(), singleton, &d)?;
        }
        2 => {
            let n = r.u() % 4;
            let mut sides = SidesList::new();
            for _ in 0..n {
                let di = r.u();
                let d = if sim.dicts.is_empty() {
                    Dict::new()
                } else {
                    let spec = sim.dicts[di as usize % sim.dicts.len()].clone();
                    sim.build_dict(&spec)
                };
                sides.add_team(&d);
            }
            factory_init_from_sides(sim, &sides)?;
        }
        3 => factory_reset(sim)?,
        4 => factory_init(sim)?,
        5 => factory_clear(sim)?,
        6 => {
            let id = r.u();
            let t = factory_create_inactive_team(sim, &team_name(id))?;
            sim.rec(format!("ret team {t}"));
        }
        7 => {
            let id = r.u();
            let t = factory_create_team(sim, &team_name(id))?;
            sim.rec(format!("ret team {t}"));
        }
        8 => {
            let sel = r.u();
            let p = if sel != 0 { pick_proto(sim, sel - 1) } else { None };
            let t = factory_create_team_on_prototype(sim, p)?;
            sim.rec(format!("ret team {t}"));
        }
        9 => {
            let id = r.u();
            let t = factory_find_team(sim, &team_name(id))?;
            sim.rec(format!("ret team {}", tid(t)));
        }
        10 => {
            let id = r.u();
            let p = factory_find_team_prototype(sim, &team_name(id));
            let id = p.and_then(|p| sim.teams.prototype(p)).map_or(0, |p| p.id);
            sim.rec(format!("ret proto {id}"));
        }
        11 => {
            let raw = r.u();
            let t = factory_find_team_by_id(sim, raw);
            sim.rec(format!("ret team {}", tid(t)));
        }
        12 => {
            let raw = r.u();
            let p = factory_find_team_prototype_by_id(sim, raw);
            let id = p.and_then(|p| sim.teams.prototype(p)).map_or(0, |p| p.id);
            sim.rec(format!("ret proto {id}"));
        }
        13 => {
            let (ok, ts) = (r.u(), r.u());
            let t = if ts != 0 { pick_team(sim, ts - 1) } else { None };
            if ts != 0 && t.is_none() {
                sim.rec("skip".into());
            } else {
                let o = sim.objs[ok as usize % sim.objs.len()].id;
                sim.obj_set_team(o, t);
            }
        }
        14 => {
            let (pk, ts) = (r.u(), r.u());
            let t = if ts != 0 { pick_team(sim, ts - 1) } else { None };
            if ts != 0 && t.is_none() {
                sim.rec("skip".into());
            } else {
                sim.players[pk as usize % np].default_team = t;
            }
        }
        15 => {
            let (tk, sub) = (r.u(), r.u());
            let mut a = [0u32; 8];
            for x in a.iter_mut() {
                *x = r.u();
            }
            match pick_team(sim, tk) {
                None => sim.rec("skip".into()),
                Some(t) => team_op(sim, t, sub, &a)?,
            }
        }
        16 => {
            let (pk, sub) = (r.u(), r.u());
            let mut a = [0u32; 8];
            for x in a.iter_mut() {
                *x = r.u();
            }
            match pick_proto(sim, pk) {
                None => sim.rec("skip".into()),
                Some(p) => proto_op(sim, p, sub, &a)?,
            }
        }
        17 => {
            let (ok, sub) = (r.u(), r.u());
            let mut a = [0u32; 8];
            for x in a.iter_mut() {
                *x = r.u();
            }
            let o = sim.objs[ok as usize % sim.objs.len()].id;
            obj_op(sim, o, sub, &a);
        }
        18 => {
            let sub = r.u();
            if sub == 0 {
                let mut xc = XferCrc::new();
                xc.open(b"t").unwrap();
                xfer_nested(sim, &mut xc, |h, x| xfer_team_factory(h, x))?;
                xc.close().unwrap();
                sim.rec(format!("ret crc {:08x}", xc.get_crc()));
                let mut save = XferSave::new();
                save.open(b"t").unwrap();
                xfer_nested(sim, &mut save, |h, x| xfer_team_factory(h, x))?;
                save.close().unwrap();
                let bytes = save.take_bytes();
                sim.saved = bytes;
                sim.have_save = true;
                sim.saved_obj_team = sim.objs.iter().map(|o| (o.id, tid(o.team))).collect();
                sim.rec(format!("ret xfer-save len={} hash={:08x}", sim.saved.len(), hash_bytes(&sim.saved)));
            } else if !sim.have_save {
                sim.rec("skip".into());
            } else {
                let mut load = XferLoad::new(sim.saved.clone());
                load.open(b"t").unwrap();
                xfer_nested(sim, &mut load, |h, x| xfer_team_factory(h, x))?;
                let _ = load.close();
                sim.rec(format!("ret xfer-load pp={}", load.post_process_requests()));
                // the objects' own xfer: each puts itself on the team it was saved on
                let ids: Vec<ObjectId> = sim.objs.iter().map(|o| o.id).collect();
                for id in ids {
                    let saved = sim.saved_obj_team.get(&id).copied().unwrap_or(0);
                    let t = if saved != 0 { factory_find_team_by_id(sim, saved) } else { None };
                    sim.obj_set_team(id, t);
                }
                // post process: the teams (in id order), then the factory
                for t in alive_teams(sim) {
                    team_load_post_process(sim, t)?;
                }
                factory_load_post_process(sim);
            }
        }
        19 => {
            let ts = r.u();
            let t = if ts != 0 { pick_team(sim, ts - 1) } else { None };
            if ts != 0 && t.is_none() {
                sim.rec("skip".into());
            } else {
                factory_team_about_to_be_deleted(sim, t)?;
            }
        }
        _ => sim.rec("ret nop".into()),
    }
    Ok(())
}

fn team_op(sim: &mut Sim, t: TeamId, sub: u32, a: &[u32; 8]) -> Result<(), TeamError> {
    let np = sim.players.len();
    match sub {
        0 => {
            let v = team_get_targetable_count(sim, t);
            sim.rec(format!("ret targetable {v}"));
        }
        1 => {
            let that = if a[0] != 0 { pick_team(sim, a[0] - 1) } else { None };
            let v = team_get_relationship(sim, t, that)?;
            sim.rec(format!("ret rel {}", v as i32));
        }
        2 => {
            let o = if a[0] != 0 { Some(sim.objs[(a[0] - 1) as usize % sim.objs.len()].id) } else { None };
            team_set_team_target_object(sim, t, o)?;
        }
        3 => {
            let o = team_get_team_target_object(sim, t)?;
            sim.rec(format!("ret target {}", o.unwrap_or(0)));
        }
        4 => team_set_override_team_relationship(sim, t, a[0], rel_of(a[1]))?,
        5 => {
            let v = team_remove_override_team_relationship(sim, t, a[0])?;
            sim.rec(format!("ret rmteamrel {}", v as u32));
        }
        6 => team_set_override_player_relationship(sim, t, a[0] as i32, rel_of(a[1]))?,
        7 => {
            let v = team_remove_override_player_relationship(sim, t, a[0] as i32)?;
            sim.rec(format!("ret rmplayerrel {}", v as u32));
        }
        8 => {
            let (things, mut counts) = thing_list(sim, a);
            team_count_objects_by_thing_template(sim, t, &things, a[5] != 0, &mut counts, a[6] != 0);
            sim.rec(format!("ret count {}", count_str(&counts)));
        }
        9 => {
            let v = team_count_buildings(sim, t);
            sim.rec(format!("ret buildings {v}"));
        }
        10 => {
            let v = team_count_objects(sim, t, &kind_mask(a[0]), &kind_mask(a[1]));
            sim.rec(format!("ret objects {v}"));
        }
        11 => team_heal_all_objects(sim, t),
        12 => team_iterate_objects(sim, t, |h, o| h.rec(format!("iter {o}"))),
        13 => {
            let v = team_has_any_buildings(sim, t);
            sim.rec(format!("ret anybuildings {}", v as u32));
        }
        14 => {
            let v = team_has_any_buildings_kind_of(sim, t, kind_mask(a[0]));
            sim.rec(format!("ret anybuildingskind {}", v as u32));
        }
        15 => {
            let v = team_has_any_units(sim, t);
            sim.rec(format!("ret anyunits {}", v as u32));
        }
        16 => {
            let v = team_has_any_objects(sim, t);
            sim.rec(format!("ret anyobjects {}", v as u32));
        }
        17 => {
            let v = team_is_idle(sim, t);
            sim.rec(format!("ret idle {}", v as u32));
        }
        18 => team_update_state(sim, t)?,
        19 => team_notify_team_of_object_death(sim, t)?,
        20 => {
            let trg = a[1] % 2;
            let which = a[2] % 4;
            let v = match a[0] % 7 {
                0 => team_did_all_enter(sim, t, trg, which)?,
                1 => team_did_partial_enter(sim, t, trg, which)?,
                2 => team_did_partial_exit(sim, t, trg, which)?,
                3 => team_did_all_exit(sim, t, trg, which)?,
                4 => team_all_inside(sim, t, trg, which)?,
                5 => team_none_inside(sim, t, trg, which)?,
                _ => team_some_inside_some_outside(sim, t, trg, which)?,
            };
            sim.rec(format!("ret trigger {} {}", a[0] % 7, v as u32));
        }
        21 => match team_get_estimate_team_position(sim, t) {
            Some(p) => sim.rec(format!("ret pos {},{},{}", fb(p.x), fb(p.y), fb(p.z))),
            None => sim.rec("ret pos null".into()),
        },
        22 => team_delete_team(sim, t, a[0] != 0)?,
        23 => {
            let other = if a[0] != 0 { pick_team(sim, a[0] - 1) } else { None };
            team_transfer_units_to(sim, t, other)?;
        }
        24 => {
            let home = Coord3D { x: f32::from_bits(a[1]), y: f32::from_bits(a[2]), z: f32::from_bits(a[3]) };
            let md = f32::from_bits(a[4]);
            let tpl = (a[0] as usize % sim.tpls.len()) as TemplateRef;
            let o = team_try_to_recruit(sim, t, tpl, home, md)?;
            sim.rec(format!("ret recruit {}", o.unwrap_or(0)));
        }
        25 => team_evacuate_team(sim, t),
        26 => team_kill_team(sim, t)?,
        27 => {
            let v = team_damage_team_members(sim, t, f32::from_bits(a[0]));
            sim.rec(format!("ret damage {}", v as u32));
        }
        28 => team_move_team_to(sim, t, Coord3D { x: f32::from_bits(a[0]), y: f32::from_bits(a[1]), z: f32::from_bits(a[2]) }),
        29 => {
            let v = team_has_any_build_facility(sim, t)?;
            sim.rec(format!("ret buildfacility {}", v as u32));
        }
        30 => team_update_generic_scripts(sim, t)?,
        31 => team_set_active(sim, t)?,
        32 => {
            sim.teams.team_mut(t).ok_or(TeamError::Null)?.set_state(&team_name(a[0]));
            let s = String::from_utf8_lossy(sim.teams.team(t).ok_or(TeamError::Null)?.get_state()).into_owned();
            sim.rec(format!("ret state {s}"));
        }
        33 => sim.teams.team_mut(t).ok_or(TeamError::Null)?.set_recruitable(a[0] != 0),
        34 => sim.teams.team_mut(t).ok_or(TeamError::Null)?.set_entered_exited(),
        35 => {
            let w = if a[0] != 0 && !sim.ways.is_empty() { Some(sim.ways[(a[0] - 1) as usize % sim.ways.len()].1) } else { None };
            sim.teams.team_mut(t).ok_or(TeamError::Null)?.set_current_waypoint(w);
            let w = sim.teams.team(t).ok_or(TeamError::Null)?.get_current_waypoint();
            sim.rec(format!("ret waypoint {}", w.unwrap_or(0)));
        }
        36 => team_set_attack_priority_name(sim, t, &team_name(a[0]))?,
        37 => {
            let p = team_get_controlling_player(sim, t)?;
            let name = String::from_utf8_lossy(sim.teams.team_name(t).ok_or(TeamError::Null)?).into_owned();
            let tm = sim.teams.team(t).ok_or(TeamError::Null)?;
            let line = format!(
                "ret getters name={} player={} active={} created={} eoe={} proto={}",
                name,
                p.unwrap_or(-1),
                tm.active as u32,
                tm.created as u32,
                tm.entered_or_exited as u32,
                sim.teams.prototype(tm.proto).ok_or(TeamError::Null)?.id
            );
            sim.rec(line);
        }
        38 => {
            let p = if a[0] != 0 { Some(((a[0] - 1) as usize % np) as PlayerIndex) } else { None };
            team_set_controlling_player(sim, t, p)?;
        }
        39 => {
            let g = if a[0] != 0 { Some((a[0] - 1) % 3 + 1) } else { None };
            team_get_team_as_ai_group(sim, t, g);
        }
        _ => sim.rec("ret nop".into()),
    }
    Ok(())
}

fn rel_of(v: u32) -> Relationship {
    match v % 3 {
        0 => Relationship::Enemies,
        1 => Relationship::Neutral,
        _ => Relationship::Allies,
    }
}

fn proto_op(sim: &mut Sim, p: TeamPrototypeId, sub: u32, a: &[u32; 8]) -> Result<(), TeamError> {
    let np = sim.players.len();
    match sub {
        0 => {
            let pl = proto_get_controlling_player(sim, p)?;
            sim.rec(format!("ret owner {}", pl.unwrap_or(-1)));
        }
        1 => {
            let t = proto_find_team_by_id(sim, p, a[0]);
            sim.rec(format!("ret findteam {}", tid(t)));
        }
        2 => {
            let pl = if a[0] != 0 { Some(((a[0] - 1) as usize % np) as PlayerIndex) } else { None };
            proto_set_controlling_player(sim, p, pl)?;
        }
        3 => {
            let (things, mut counts) = thing_list(sim, a);
            proto_count_objects_by_thing_template(sim, p, &things, a[5] != 0, &mut counts, a[6] != 0);
            sim.rec(format!("ret count {}", count_str(&counts)));
        }
        4 => {
            let s = proto_get_generic_script(sim, p, (a[0] % MAX_GENERIC_SCRIPTS as u32) as i32)?;
            sim.rec(format!("ret generic {}", s.unwrap_or(0)));
        }
        5 => {
            proto_increase_ai_priority_for_success(sim, p)?;
            let v = sim.teams.prototype(p).ok_or(TeamError::Null)?.team_template.production_priority;
            sim.rec(format!("ret prio {v}"));
        }
        6 => {
            proto_decrease_ai_priority_for_failure(sim, p)?;
            let v = sim.teams.prototype(p).ok_or(TeamError::Null)?.team_template.production_priority;
            sim.rec(format!("ret prio {v}"));
        }
        7 => {
            let v = proto_count_buildings(sim, p);
            sim.rec(format!("ret buildings {v}"));
        }
        8 => {
            let v = proto_count_objects(sim, p, &kind_mask(a[0]), &kind_mask(a[1]));
            sim.rec(format!("ret objects {v}"));
        }
        9 => proto_heal_all_objects(sim, p),
        10 => proto_iterate_objects(sim, p, |h, o| h.rec(format!("iter {o}"))),
        11 => {
            let v = proto_count_team_instances(sim, p);
            sim.rec(format!("ret instances {v}"));
        }
        12 => {
            let v = proto_has_any_buildings(sim, p);
            sim.rec(format!("ret anybuildings {}", v as u32));
        }
        13 => {
            let v = proto_has_any_buildings_kind_of(sim, p, &kind_mask(a[0]));
            sim.rec(format!("ret anybuildingskind {}", v as u32));
        }
        14 => {
            let v = proto_has_any_units(sim, p);
            sim.rec(format!("ret anyunits {}", v as u32));
        }
        15 => {
            let v = proto_has_any_objects(sim, p);
            sim.rec(format!("ret anyobjects {}", v as u32));
        }
        16 => {
            let v = proto_has_any_build_facility(sim, p)?;
            sim.rec(format!("ret buildfacility {}", v as u32));
        }
        17 => proto_damage_team_members(sim, p, f32::from_bits(a[0])),
        18 => proto_move_team_to(sim, p, Coord3D { x: f32::from_bits(a[0]), y: f32::from_bits(a[1]), z: f32::from_bits(a[2]) }),
        19 => proto_update_state(sim, p)?,
        20 => {
            let v = proto_evaluate_production_condition(sim, p)?;
            sim.rec(format!("ret production {}", v as u32));
        }
        21 => {
            let pr = sim.teams.prototype(p).ok_or(TeamError::Null)?;
            let line = format!("ret getters id={} name={} singleton={} maxi={}", pr.id, String::from_utf8_lossy(&pr.name), pr.get_is_singleton() as u32, pr.team_template.max_instances);
            sim.rec(line);
        }
        22 => {
            let t = if a[0] != 0 { pick_team(sim, a[0] - 1) } else { None };
            proto_team_about_to_be_deleted(sim, p, t)?;
        }
        23 => {
            sim.teams.prototype_mut(p).ok_or(TeamError::Null)?.attack_priority_name = team_name(a[0]);
            let s = String::from_utf8_lossy(&sim.teams.prototype(p).ok_or(TeamError::Null)?.attack_priority_name).into_owned();
            sim.rec(format!("ret attack {s}"));
        }
        _ => sim.rec("ret nop".into()),
    }
    Ok(())
}

fn obj_op(sim: &mut Sim, o: ObjectId, sub: u32, a: &[u32; 8]) {
    let ob = sim.obj_mut(o);
    match sub {
        0 => ob.dead = a[0] != 0,
        1 => ob.destroyed = a[0] != 0,
        2 => {
            let s = STATUS_POOL[(a[0] % 4) as usize];
            if a[1] != 0 {
                ob.status.insert(s);
            } else {
                ob.status.remove(&s);
            }
        }
        3 => ob.enemy_near = a[0] != 0,
        4 => {
            ob.did_enter = a[0];
            ob.did_exit = a[1];
            ob.inside = a[2];
        }
        5 => ob.contain_count = a[0] as i32,
        6 => ob.held = a[0] != 0,
        7 => ob.contained = a[0] != 0,
        8 => {
            ob.ai_idle = a[0] != 0;
            ob.ai_recruit = a[1] != 0;
            ob.surfaces = a[2];
            ob.has_ai = a[3] != 0;
        }
        9 => ob.pos = Coord3D { x: f32::from_bits(a[0]), y: f32::from_bits(a[1]), z: f32::from_bits(a[2]) },
        10 => ob.reactions = a[0],
        11 => ob.vision = f32::from_bits(a[0]),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------------------------------------------------

fn chance(g: &mut Gen, percent: u32) -> bool {
    g.below(100) < percent
}

fn gen_coord(g: &mut Gen, spread: f32) -> [f32; 3] {
    [g.uniform(-spread, spread), g.uniform(-spread, spread), g.uniform(0.0, 20.0)]
}

/// A dictionary: a random subset of the properties of a team, with values of the right type.
fn gen_dict(g: &mut Gen, w: &mut W, n_players: u32, n_tpls: u32, n_ways: u32, n_scripts: u32) {
    let mut entries: Vec<(i32, String, u32, String)> = Vec::new();
    let mut add = |ty: i32, key: &str, bits: u32, s: &str| entries.push((ty, key.to_string(), bits, s.to_string()));
    let script = |g: &mut Gen| format!("Scr{}", g.below(n_scripts.max(1) + 1));
    if chance(g, 60) {
        add(3, "teamName", 0, &format!("Team{}", g.below(4)));
    }
    if chance(g, 60) {
        add(3, "teamOwner", 0, &format!("Player{}", g.below(n_players + 1)));
    }
    if chance(g, 40) {
        add(0, "teamIsSingleton", g.below(2), "");
    }
    if chance(g, 40) && n_ways > 0 {
        add(3, "teamHome", 0, &format!("Way{}", g.below(4)));
    }
    for k in 1..=7u32 {
        if chance(g, 25) {
            add(3, &format!("teamUnitType{k}"), 0, &format!("Tpl{}", g.below(n_tpls + 2)));
        }
        if chance(g, 25) {
            add(1, &format!("teamUnitMinCount{k}"), g.below(5), "");
        }
        if chance(g, 25) {
            add(1, &format!("teamUnitMaxCount{k}"), (g.below(7) as i32 - 1) as u32, "");
        }
    }
    for key in ["teamOnCreateScript", "teamOnIdleScript", "teamOnUnitDestroyedScript", "teamOnDestroyedScript", "teamEnemySightedScript", "teamAllClearScript", "teamProductionCondition"] {
        if chance(g, 50) {
            let s = script(g);
            add(3, key, 0, &s);
        }
    }
    for key in [
        "teamAutoReinforce",
        "teamIsAIRecruitable",
        "teamIsBaseDefense",
        "teamIsPerimeterDefense",
        "teamTransportsReturn",
        "teamAvoidThreats",
        "teamAttackCommonTarget",
        "teamStartsFull",
        "teamTransportsExit",
        "teamExecutesActionsOnCreate",
    ] {
        if chance(g, 35) {
            add(0, key, g.below(2), "");
        }
    }
    for key in ["teamInitialIdleFrames", "teamMaxInstances", "teamProductionPriority", "teamProductionPrioritySuccessIncrease", "teamProductionPriorityFailureDecrease", "teamVeterancy"] {
        if chance(g, 40) {
            add(1, key, (g.below(12) as i32 - 3) as u32, "");
        }
    }
    if chance(g, 35) {
        add(1, "teamAggressiveness", (g.below(6) as i32 - 2) as u32, "");
    }
    if chance(g, 50) {
        let v = *[0.0f32, 0.25, 0.5, 0.75, 1.0, 1.5, -0.5].get(g.below(7) as usize).unwrap();
        add(2, "teamDestroyedThreshold", v.to_bits(), "");
    }
    for key in ["teamTransport", "teamReinforcementOrigin"] {
        if chance(g, 20) {
            add(3, key, 0, &format!("Tpl{}", g.below(n_tpls)));
        }
    }
    for k in 0..16u32 {
        if chance(g, 18) {
            let s = script(g);
            add(3, &format!("teamGenericScriptHook{k}"), 0, &s);
        }
    }
    // a shuffle, so the order in which the names reach the generator varies
    for i in (1..entries.len()).rev() {
        let j = g.below(i as u32 + 1) as usize;
        entries.swap(i, j);
    }
    w.i(entries.len() as i32);
    for (ty, key, bits, s) in entries {
        w.i(ty);
        w.s(&key);
        if ty == 3 {
            w.s(&s);
        } else {
            w.u(bits);
        }
    }
}

fn gen_header(g: &mut Gen, w: &mut W) -> u32 {
    let np = 2 + g.below(3);
    w.i(np as i32);
    for _ in 0..np {
        w.i(g.below(2) as i32);
        w.i(g.below(3) as i32);
        w.u(g.u32());
        w.i(if chance(g, 60) { g.below(8) as i32 } else { -1 });
    }
    let nt = 2 + g.below(5);
    w.i(nt as i32);
    for _ in 0..nt {
        let nk = g.below(4);
        w.i(nk as i32);
        for _ in 0..nk {
            w.u(g.below(8));
        }
        w.b(chance(g, 25));
        w.i(g.below(3) as i32);
        w.i(g.below(8) as i32);
        let nv = if chance(g, 70) { 1 + g.below(2) } else { 0 };
        w.i(nv as i32);
        for _ in 0..nv {
            w.i(g.below(16) as i32);
        }
    }
    let no = 3 + g.below(12);
    w.i(no as i32);
    let mut used = BTreeSet::new();
    for _ in 0..no {
        let mut id = 1 + g.below(200);
        while !used.insert(id) {
            id = 1 + g.below(200);
        }
        w.u(id);
        w.u(g.below(64));
        w.u(g.below(64));
        let ns = g.below(3);
        w.i(ns as i32);
        for _ in 0..ns {
            w.u(g.below(4));
        }
        w.b(chance(g, 15));
        w.b(chance(g, 10));
        w.b(chance(g, 15));
        w.b(chance(g, 10));
        w.b(chance(g, 30));
        w.b(chance(g, 70));
        w.b(chance(g, 50));
        w.b(chance(g, 70));
        w.u(g.below(32));
        w.i(if chance(g, 50) { -1 } else { g.below(4) as i32 });
        if chance(g, 50) {
            w.c([(g.below(3) as f32 - 1.0) * 10.0, (g.below(3) as f32 - 1.0) * 10.0, 0.0]);
        } else {
            w.c(gen_coord(g, 300.0));
        }
        w.f(g.uniform(5.0, 200.0));
        w.u(g.below(4));
        w.u(g.below(4));
        w.u(g.below(4));
        w.u(if chance(g, 55) { 0 } else { g.below(16) });
    }
    let nscr = g.below(6);
    w.i(nscr as i32);
    for _ in 0..nscr {
        w.u(g.below(200));
        w.i(g.below(14) as i32 - 3);
        w.b(chance(g, 70));
        w.b(chance(g, 70));
        w.b(chance(g, 70));
        w.b(chance(g, 40));
        w.u(g.u32() | g.u32());
    }
    let nw = g.below(4);
    w.i(nw as i32);
    for k in 0..nw {
        w.i(g.below(4) as i32);
        w.u(1 + k * 7 + g.below(5));
        w.c(gen_coord(g, 300.0));
    }
    let nd = 3 + g.below(6);
    w.i(nd as i32);
    for _ in 0..nd {
        gen_dict(g, w, np, nt, nw, nscr);
    }
    w.u(g.below(1000));
    // the name keys to look at at the end: every team property
    let n_probe = 54;
    w.i(n_probe as i32);
    for name in WELL_KNOWN_KEY_NAMES.iter().take(n_probe) {
        w.s(name);
    }
    no
}

/// A grid coordinate (ties in distances) or a free one.
fn gen_pos(g: &mut Gen) -> [f32; 3] {
    if chance(g, 50) {
        [(g.below(3) as f32 - 1.0) * 10.0, (g.below(3) as f32 - 1.0) * 10.0, 0.0]
    } else {
        [g.uniform(-300.0, 300.0), g.uniform(-300.0, 300.0), g.uniform(0.0, 20.0)]
    }
}

/// `TEAM_OP`: the team (position among those alive), the sub-operation and its eight arguments.
fn gen_team_op(g: &mut Gen, w: &mut W, tk: u32, sub: u32) {
    w.u(15);
    w.u(tk);
    w.u(sub);
    let mut a = [0u32; 8];
    for x in a.iter_mut() {
        *x = g.u32();
    }
    match sub {
        1 | 2 | 23 | 38 | 39 | 35 => a[0] = if chance(g, 25) { 0 } else { 1 + g.below(12) },
        4 | 5 => a[0] = g.below(12),
        6 | 7 => a[0] = g.below(6).wrapping_sub(1),
        8 => {
            a[0] = g.below(5);
            for slot in a.iter_mut().take(5).skip(1) {
                *slot = if chance(g, 15) { 0 } else { 1 + g.below(8) };
            }
            a[5] = g.below(2);
            a[6] = g.below(2);
        }
        10 | 14 => {
            a[0] = g.below(256);
            a[1] = g.below(256);
        }
        20 => {
            a[0] = g.below(7);
            a[1] = g.below(2);
            a[2] = g.below(4);
        }
        22 => a[0] = g.below(2),
        24 => {
            let p = gen_pos(g);
            a[0] = g.below(8);
            a[1] = p[0].to_bits();
            a[2] = p[1].to_bits();
            a[3] = p[2].to_bits();
            a[4] = if chance(g, 50) { (g.below(4) as f32 * 10.0).to_bits() } else { g.uniform(0.0, 400.0).to_bits() };
        }
        27 => a[0] = if chance(g, 15) { 0f32.to_bits() } else { g.uniform(-5.0, 120.0).to_bits() },
        28 => {
            let p = gen_pos(g);
            a[0] = p[0].to_bits();
            a[1] = p[1].to_bits();
            a[2] = 0f32.to_bits();
        }
        32 | 36 => a[0] = g.below(6),
        33 => a[0] = g.below(2),
        _ => {}
    }
    for x in a {
        w.u(x);
    }
}

fn gen_proto_op(g: &mut Gen, w: &mut W, pk: u32, sub: u32) {
    w.u(16);
    w.u(pk);
    w.u(sub);
    let mut a = [0u32; 8];
    for x in a.iter_mut() {
        *x = g.u32();
    }
    match sub {
        1 => a[0] = g.below(12),
        2 => a[0] = if chance(g, 25) { 0 } else { 1 + g.below(12) },
        3 => {
            a[0] = g.below(5);
            for slot in a.iter_mut().take(5).skip(1) {
                *slot = if chance(g, 15) { 0 } else { 1 + g.below(8) };
            }
            a[5] = g.below(2);
            a[6] = g.below(2);
        }
        4 => a[0] = g.below(16),
        8 | 13 => {
            a[0] = g.below(256);
            a[1] = g.below(256);
        }
        17 => a[0] = if chance(g, 15) { 0f32.to_bits() } else { g.uniform(-5.0, 120.0).to_bits() },
        18 => {
            let p = gen_pos(g);
            a[0] = p[0].to_bits();
            a[1] = p[1].to_bits();
            a[2] = 0f32.to_bits();
        }
        22 => a[0] = if chance(g, 25) { 0 } else { 1 + g.below(12) },
        23 => a[0] = g.below(6),
        _ => {}
    }
    for x in a {
        w.u(x);
    }
}

fn gen_obj_op(g: &mut Gen, w: &mut W) {
    w.u(17);
    w.u(g.below(16));
    let sub = g.below(12);
    w.u(sub);
    let mut a = [0u32; 8];
    match sub {
        0 | 1 | 3 | 6 | 7 => a[0] = g.below(2),
        2 => {
            a[0] = g.below(4);
            a[1] = g.below(2);
        }
        4 => {
            a[0] = g.below(4);
            a[1] = g.below(4);
            a[2] = g.below(4);
        }
        5 => a[0] = (g.below(5) as i32 - 1) as u32,
        8 => {
            a[0] = g.below(2);
            a[1] = g.below(2);
            a[2] = g.below(32);
            a[3] = g.below(2);
        }
        9 => {
            let p = gen_pos(g);
            a[0] = p[0].to_bits();
            a[1] = p[1].to_bits();
            a[2] = p[2].to_bits();
        }
        10 => a[0] = g.below(16),
        _ => a[0] = g.uniform(5.0, 200.0).to_bits(),
    }
    for x in a {
        w.u(x);
    }
}

/// One random operation (not an early one); `n` counts the operations written.
fn gen_one(g: &mut Gen, w: &mut W, n: &mut u32) {
    let opt = |g: &mut Gen| if chance(g, 15) { 0 } else { 1 + g.below(12) };
    let pick = g.below(100);
    *n += 1;
    match pick {
        0..=3 => {
            w.u(1);
            w.u(g.below(4));
            w.u(g.below(5));
            w.b(chance(g, 30));
            w.u(g.below(20));
        }
        4 => {
            w.u(2);
            let k = g.below(4);
            w.u(k);
            for _ in 0..k {
                w.u(g.below(20));
            }
        }
        5 => {
            if chance(g, 40) {
                w.u(*[3, 4, 5].get(g.below(3) as usize).unwrap());
            } else {
                w.u(0);
                w.u(g.below(40));
            }
        }
        6..=9 => {
            w.u(6 + g.below(2));
            w.u(g.below(4));
        }
        10 | 11 => {
            w.u(8);
            w.u(opt(g));
        }
        12 | 13 => {
            w.u(9);
            w.u(g.below(4));
        }
        14 => {
            if chance(g, 50) {
                w.u(10);
                w.u(g.below(4));
            } else {
                w.u(11 + g.below(2));
                w.u(g.below(10));
            }
        }
        15..=28 => {
            w.u(13);
            w.u(g.below(16));
            w.u(opt(g));
        }
        29 | 30 => {
            w.u(14);
            w.u(g.below(4));
            w.u(opt(g));
        }
        31..=66 => {
            let tk = g.below(12);
            let sub = if chance(g, 12) { 24 } else { g.below(40) };
            if sub == 20 && chance(g, 60) {
                // the members only enter or exit when the team was told
                gen_team_op(g, w, tk, 34);
                *n += 1;
            }
            gen_team_op(g, w, tk, sub);
            if sub == 18 && chance(g, 40) {
                gen_team_op(g, w, tk, 18);
                *n += 1;
            }
            if sub == 2 && chance(g, 70) {
                gen_team_op(g, w, tk, 3);
                *n += 1;
            }
            if (sub == 6 || sub == 4) && chance(g, 60) {
                gen_team_op(g, w, tk, 1);
                *n += 1;
            }
        }
        67..=80 => {
            let pk = g.below(12);
            let sub = g.below(25);
            gen_proto_op(g, w, pk, sub);
            if sub == 20 && chance(g, 60) {
                gen_proto_op(g, w, pk, 20);
                *n += 1;
            }
        }
        81..=90 => gen_obj_op(g, w),
        91..=93 => {
            w.u(18);
            w.u(if g.below(3) == 0 { 0 } else { 1 });
        }
        94 => {
            // a save, some operations, and the load of that save
            w.u(18);
            w.u(0);
            let m = 3 + g.below(6);
            for _ in 0..m {
                // changes that can leave a team out or add one
                let kind = g.below(7);
                *n += 1;
                match kind {
                    6 => {
                        // empty a team, then let the prototype drop its empty teams
                        let tk = g.below(12);
                        w.u(15);
                        w.u(tk);
                        w.u(23);
                        w.u(1 + g.below(12));
                        for _ in 0..7 {
                            w.u(0);
                        }
                        for pk in 0..4 {
                            w.u(16);
                            w.u(pk);
                            w.u(19);
                            for _ in 0..8 {
                                w.u(0);
                            }
                            *n += 1;
                        }
                    }
                    0 => {
                        let tk = g.below(12);
                        gen_team_op(g, w, tk, 22)
                    }
                    1 => {
                        w.u(15);
                        w.u(g.below(12));
                        w.u(18);
                        for _ in 0..8 {
                            w.u(0);
                        }
                    }
                    2 => {
                        w.u(16);
                        w.u(g.below(12));
                        w.u(19);
                        for _ in 0..8 {
                            w.u(0);
                        }
                    }
                    3 => {
                        w.u(7);
                        w.u(g.below(4));
                    }
                    4 => {
                        w.u(13);
                        w.u(g.below(16));
                        w.u(opt(g));
                    }
                    _ => gen_obj_op(g, w),
                }
            }
            w.u(18);
            w.u(1);
            *n += 1;
        }
        95 | 96 => {
            w.u(19);
            w.u(opt(g));
        }
        _ => {
            w.u(0);
            w.u(g.below(40));
        }
    }
}

fn gen_ops(g: &mut Gen, w: &mut W, n_objects: u32) {
    let mut ops = W::default();
    let mut n = 0u32;
    // the first operations make the prototypes (one for each name), teams on them, each player's default team and
    // objects on teams, so that there is something to ask about
    for k in 0..4 {
        ops.u(1);
        ops.u(k);
        ops.u(g.below(5));
        ops.b(chance(g, 30));
        ops.u(g.below(20));
        n += 1;
    }
    for _ in 0..4 {
        ops.u(7);
        ops.u(g.below(4));
        n += 1;
    }
    for p in 0..3 {
        ops.u(14);
        ops.u(p);
        ops.u(1 + g.below(8));
        n += 1;
    }
    // every object joins one of the first teams
    for o in 0..n_objects {
        ops.u(13);
        ops.u(o);
        ops.u(1 + g.below(4));
        n += 1;
    }
    let target = 25 + g.below(55);
    while n < target {
        gen_one(g, &mut ops, &mut n);
    }
    w.i(n as i32);
    w.0.extend(ops.0);
}

fn gen_program(seed: u64) -> Vec<u32> {
    let mut g = Gen::new(seed);
    let mut w = W::default();
    let no = gen_header(&mut g, &mut w);
    gen_ops(&mut g, &mut w, no);
    w.0
}

// ---------------------------------------------------------------------------------------------------------------------
// The comparison
// ---------------------------------------------------------------------------------------------------------------------

fn cpp_run(words: &[u32]) -> Vec<String> {
    let path = CString::new(format!("{}/team_{}.bin", std::env::temp_dir().display(), std::process::id())).unwrap();
    let mut cap = 1 << 20;
    loop {
        let mut buf = vec![0u8; cap];
        let n = unsafe { cpp::ref_team_run(words.as_ptr(), words.len() as i32, path.as_ptr(), buf.as_mut_ptr() as *mut std::os::raw::c_char, cap as i32) } as usize;
        if n <= cap {
            buf.truncate(n);
            return String::from_utf8_lossy(&buf).lines().map(str::to_owned).collect();
        }
        cap = n + 1;
    }
}

fn cases() -> u64 {
    std::env::var("TEAM_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(1500)
}

#[test]
fn team_matches_upstream() {
    let _guard = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("team, trace lines");
    let (mut programs, mut excluded, mut lines) = (0u64, 0u64, 0u64);
    let mut why_counts: BTreeMap<&str, u64> = BTreeMap::new();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let markers = [
        "ret team ", "ret proto ", "ret rel ", "ret count ", "ret trigger ", "ret recruit ", "ret xfer-load", "ret xfer-save", "threw badarg", "threw invaliddata",
        "setteam ", "kill ", "damage ", "heal ", "destroy ", "remove-all-contained", "group.add", "script.dup", "eval ", "exec ", "run ", "dbg ", "closest ", "pcm ",
        "p.add", "p.remove", "p.pre", "pl.about", "notify-destroy", "iter ", "skip", "D team ",
    ];
    let from: u64 = std::env::var("TEAM_FROM").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut tmp = Vec::new();
    for case in from..from + cases() {
        let words = gen_program(0x7ea0_0000 + case);
        programs += 1;
        let rust = match run_rust(&words, &mut tmp) {
            Ok(l) => l,
            Err(Excluded(why)) => {
                excluded += 1;
                *why_counts.entry(why).or_insert(0) += 1;
                continue;
            }
        };
        let cpp_lines = cpp_run(&words);
        lines += rust.len() as u64;
        for l in &rust {
            for m in markers {
                if l.starts_with(m) {
                    *counts.entry(m.to_string()).or_insert(0) += 1;
                }
            }
        }
        check.eq(rust.len(), cpp_lines.len(), || format!("program {case}: line count"));
        for (i, (a, b)) in rust.iter().zip(cpp_lines.iter()).enumerate() {
            check.eq(a.as_str(), b.as_str(), || format!("program {case} line {i}"));
        }
    }
    println!("RESULT programs {programs}, excluded (upstream undefined) {excluded}, lines compared {lines}");
    // A corruption of the Rust arena surfaces as a dereference of a `NULL` or dangling team, which is also what an
    // undefined upstream input looks like; the clean translation has a steady rate of the latter, so a jump in it is a defect.
    let nulls = why_counts.get("a NULL or dangling pointer dereferenced").copied().unwrap_or(0);
    assert!(nulls * 100 <= programs * 13, "{nulls} of {programs} programs dereference a NULL or dangling team: the arena is corrupt");
    for (why, c) in &why_counts {
        println!("RESULT   excluded: {why}: {c}");
    }
    for (m, c) in &counts {
        println!("RESULT   {m:<24} {c}");
    }
    check.finish();
}
