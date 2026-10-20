// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::world::player` and `player_ops` (slice 1 of `Player`: construction and `init`,
//! relationships, the build list, the team lists and the loops over a player's objects, the AI forwarders, radar and
//! power, special power timers, production changes, sciences and ranks) against upstream's own `Player.h` and the
//! `Player.cpp` functions of that slice (commit 0a05454d). Phase 1 layer 7, p1w10-sonnet.
//!
//! # What is compared against what
//!
//! `build.rs` takes `Player.h`, `Team.h` and every other header unchanged; the functions of the slice from
//! `Player.cpp` (and the few of `Energy.cpp`, `Money.cpp`, `Handicap.cpp`, `MissionStats.cpp` and `PlayerList.cpp`
//! they call), each whole and unchanged, into `player_fns.inc`; `Team.cpp` with only its include block replaced (as
//! the Team differential does) -- so the real teams, prototypes and DLINK member lists are the world the player's loops
//! walk -- and `differential/wrap_player.cpp` compiles them under clang++ on the stand-ins of
//! `differential/shim_player`: the objects (with the special power modules the sciences reach), the AI, the score
//! keeper, the academy statistics, the rank and science stores, the client, the global data and the game state.
//! Money, Handicap, Energy and the player's own fields are the real classes' text. The Rust side runs the same program
//! on `Sim`, a [`PlayerHost`] (and [`TeamHost`]) that answers from the same configuration and writes the same lines.
//!
//! Every call that has an effect (an AI order, a score record, a client event, a script notification, an object
//! disabled, a special power ready) writes a line, formatted alike on both sides; calls that only answer a question
//! write none (their answers show in the `ret` lines and in the state). After every operation both sides write the whole
//! state: the factory's counters, every prototype and team with its members, every object's team and `DLINK`
//! neighbours, and every player's fields (two lines each).
//!
//! # Method
//!
//! A **program** is a header (dictionaries, the services -- global data, game state, ranks, sciences, special power
//! templates, player templates, AI behaviour -- the players, thing templates, objects with their special power
//! modules, scripts, waypoints) and a list of operations: team-world set-up (`initTeam`, `createTeam`,
//! `createInactiveTeam`, an object joining a team, changes to an object), and every `Player` operation of the slice
//! (about 110 cases), each on a player picked among those of the program. Teams and prototypes are named by position
//! among those alive.
//!
//! **Inputs for which upstream is undefined** (a `NULL` dereferenced, a deleted default team, a rank level the table
//! does not have where the answer is dereferenced, a loop that never ends) are never given to the C++: the Rust returns
//! an error and the program is counted as excluded.
//!
//! # Results (2026-10-20, upstream `0a05454d`)
//!
//! `cargo test --release --features differential --test differential_player -- --nocapture` with `PLAYER_CASES=3000`:
//! **3,000 programs, 27 excluded as undefined upstream, 5,254,698 trace lines compared, 0 mismatches.** The exclusions
//! are rank tables for which `addSkillPoints` never leaves its `while (m_skillPoints >= m_levelUp)` loop (the table is not
//! strictly increasing and the rank limit stops the player below the entry that is not above the cap; upstream spins).
//! The programs contain 2,132 relationship queries, 453 rank changes, 411 skill-point additions, 224 science purchases,
//! 100,958 `notifyOfAcquiredScience` calls, 889 AIs made, 250 EVA messages, 632 radar sounds, 235 objects disabled for
//! lack of power, 157 retaliation messages, 352 special powers created by a science, 17,892 `setTeam` calls, and every
//! one of the 77 operations many times.
//!
//! **Planted bugs.** Each was applied to the Rust alone as a runtime switch (`PLAYER_PLANT=<n>`, all 77 in one build, the
//! working tree restored afterwards) and the run repeated at 1,000 programs. **All 77 are caught.** The smallest counts
//! of mismatching lines: plant 36 (`allowedToBuild` testing the units flag for structures): 1; plant 31
//! (`findClosestByKindOf` taking the later of two objects at the same distance): 2; plant 11 (the team override of
//! `getRelationship` skipped): 6; plant 24 (`getPlayerDifficulty` ignoring the AI): 9; plant 33 (`hasAnyBuildings(kind)`
//! ignoring the kind): 11; plant 14 (`removeTeamRelationship` answering true for an entry that is not there): 12;
//! plant 48 (the kind-of production change not compounding): 14. Plants 60 (the rank limit not clamping a new level) and
//! 76 (`resetRank` reading the wrong next rank) make 1,000 programs run too long to finish under the test's watchdog and
//! are caught by 951 and 26,463 mismatches at 100 programs.
//!
//! Four rounds of the generator were needed: the first left four plants alive (`frame > sabotaged` against `>=` needs the
//! sabotage frame to be exactly the current frame; the kind masks were wide random words that never matched; the counts
//! array of `countObjectsByThingTemplate` started at zero so the clearing was invisible; a dead player who is not an
//! observer is unreachable until `killPlayer`), the second left eleven (the kind-of production-change entries never met
//! twice, a science's availability was never changed twice in a row), the third two (`okToPlayRadarEdgeSound`'s
//! `frame > 0` needs frame zero; the EVA's `m_levelUp != 0` needs a rank table whose second entry is zero). The fixes
//! were directed sequences, not more programs: the program's frame is tracked so that an operation can name it; kind
//! masks are none, one or two kinds; the last `Player` operation is repeated with some words re-rolled; the rank table's
//! second entry is sometimes zero; the first frame is sometimes zero. One plant (the removal of the last match of a
//! team prototype from the list) was an equivalent mutant, since the list never holds an entry twice, and was replaced
//! by one that changes the order of the list.
#![cfg(feature = "differential")]

mod support;

use std::collections::{BTreeMap, BTreeSet};

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::dict::Dict;
use rts_lockstep_engine::common::game_common::{GameDifficulty, PlayerType, Relationship, VeterancyLevel};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::handicap::{Handicap, HandicapType};
use rts_lockstep_engine::common::kind_of::{
    KindOfMaskType, KindOfType, KINDOF_AIRCRAFT, KINDOF_COMMANDCENTER, KINDOF_COUNT, KINDOF_DOZER, KINDOF_HARVESTER, KINDOF_INERT, KINDOF_INFANTRY, KINDOF_MINE, KINDOF_POWERED,
    KINDOF_PROJECTILE, KINDOF_STRUCTURE, KINDOF_TECH_BUILDING, KINDOF_VEHICLE,
};
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use rts_lockstep_engine::common::object_status_types::{
    ObjectStatusTypes, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use rts_lockstep_engine::common::science::ScienceType;
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::common::well_known_keys::WellKnownKeys;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::{DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use rts_lockstep_engine::logic::sides_list::BuildListInfo;
use rts_lockstep_engine::logic::world::ids::{PlayerIndex, TeamId, TeamPrototypeId};
use rts_lockstep_engine::logic::world::player::*;
use rts_lockstep_engine::logic::world::player_ops::*;
use rts_lockstep_engine::logic::world::team::{ScriptRef, TeamArena, TeamError, WaypointId, WaypointInfo};
use rts_lockstep_engine::logic::world::team_ops::*;
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
// The world of a program: the same configuration as differential/shim_player
// ---------------------------------------------------------------------------------------------------------------------

const KIND_POOL: [KindOfType; 12] = [
    KINDOF_STRUCTURE,
    KINDOF_PROJECTILE,
    KINDOF_MINE,
    KINDOF_INERT,
    KINDOF_TECH_BUILDING,
    KINDOF_AIRCRAFT,
    KINDOF_INFANTRY,
    KINDOF_VEHICLE,
    KINDOF_POWERED,
    KINDOF_DOZER,
    KINDOF_HARVESTER,
    KINDOF_COMMANDCENTER,
];
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
    skill: [i32; 4],
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
    orientation: f32,
    vet: i32,
    /// (has a special power interface, serial, required science)
    modules: Vec<(bool, i32, i32)>,
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

#[derive(Clone)]
struct SciCfg {
    cost: i32,
    grantable: bool,
    prereqs: Vec<i32>,
}

#[derive(Clone, Copy)]
struct AiCfg {
    safe: bool,
    attacked: bool,
    bridges: bool,
    has_base: bool,
    base: Coord3D,
    super_ok: bool,
    construct: bool,
    at: Coord3D,
    enemy: i32,
    diff: u32,
}

struct Sim {
    teams: TeamArena,
    players: PlayerArena,
    names: NameKeyGenerator,
    keys: WellKnownKeys,
    lines: Vec<String>,
    frame: u32,
    np: usize,
    player_by_key: BTreeMap<u32, usize>,
    tpls: Vec<TplSim>,
    objs: Vec<ObjSim>,
    scripts: Vec<ScriptSim>,
    originals: usize,
    ways: Vec<(WaypointInfo, WaypointId)>,
    dicts: Vec<Vec<DictEntry>>,
    // the services
    globals: PlayerGlobals,
    victory: [bool; 16],
    ranks: Vec<RankInfoView>,
    sciences: BTreeMap<i32, SciCfg>,
    sp_templates: Vec<SpecialPowerView>,
    ptpls: Vec<PlayerTemplateView>,
    ai_cfg: Vec<AiCfg>,
    academy_points: Vec<u32>,
    /// Upstream is undefined for something the program did (a deleted default team, a template-less player asked for its
    /// beacon): the program is excluded.
    undefined: std::cell::Cell<Option<&'static str>>,
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
            let t = self.players.get_nth(0).and_then(|p| p.default_team);
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
    fn undef(&self, why: &'static str) {
        if self.undefined.get().is_none() {
            self.undefined.set(Some(why));
        }
    }
    fn ai_cfg_of(&self, p: PlayerIndex) -> AiCfg {
        self.ai_cfg[p as usize % self.ai_cfg.len()]
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
        self.player_by_key.get(&key.0).filter(|i| **i < self.np).map(|i| *i as PlayerIndex)
    }
    fn player_list_neutral(&self) -> Option<PlayerIndex> {
        Some(0)
    }
    fn player_list_count(&self) -> i32 {
        self.np as i32
    }
    fn player_list_nth(&self, i: i32) -> Option<PlayerIndex> {
        if (0..16).contains(&i) {
            Some(i)
        } else {
            None
        }
    }
    fn player_list_team_about_to_be_deleted(&mut self, team: TeamId) {
        // a player's default team that is deleted is a dangling pointer upstream
        if (0..16).any(|i| self.players.get_nth(i).and_then(|p| p.default_team) == Some(team)) {
            self.undef("a player's default team was deleted");
        }
        // PlayerList::teamAboutToBeDeleted: every player drops its relationship override for the team
        for i in 0..16 {
            let _ = player_remove_team_relationship(self, i, Some(team));
        }
    }
    fn player_add_team_to_list(&mut self, p: PlayerIndex, proto: TeamPrototypeId) {
        let _ = player_add_team_to_list(self, p, proto);
    }
    fn player_remove_team_from_list(&mut self, p: PlayerIndex, proto: TeamPrototypeId) {
        let _ = player_remove_team_from_list(self, p, proto);
    }
    fn player_pre_team_destroy(&mut self, p: PlayerIndex, team: TeamId) {
        let _ = player_pre_team_destroy(self, p, Some(team));
    }
    fn player_default_team(&self, p: PlayerIndex) -> Option<TeamId> {
        self.players.get_nth(p).and_then(|x| x.default_team)
    }
    fn player_relationship(&self, p: PlayerIndex, that: Option<TeamId>) -> Relationship {
        match player_get_relationship(self, p, that) {
            Ok(r) => r,
            Err(_) => {
                self.undef("a relationship asked of a team that is not there");
                Relationship::Neutral
            }
        }
    }
    fn player_type(&self, p: PlayerIndex) -> PlayerType {
        self.players.get_nth(p).map(|x| x.player_type).unwrap_or(PlayerType::Human)
    }
    fn player_difficulty(&self, p: PlayerIndex) -> GameDifficulty {
        player_get_player_difficulty(self, p).unwrap_or(GameDifficulty::Normal)
    }
    fn player_beacon_template(&self, p: PlayerIndex) -> Option<TemplateRef> {
        let Some(t) = self.players.get_nth(p).and_then(|x| x.template) else {
            self.undef("a player without a template asked for its beacon");
            return None;
        };
        let b = &self.ptpls[t as usize].beacon_template;
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
        for i in 0..16 {
            if let Some(p) = self.players.get_nth_mut(i) {
                if p.default_team == Some(old) {
                    p.default_team = Some(new);
                }
            }
        }
    }
    fn partition_closest_enemy(&mut self, o: ObjectId, range: f32) -> bool {
        self.rec(format!("closest {o} range={}", fb(range)));
        self.obj(o).enemy_near
    }
}

impl PlayerHost for Sim {
    fn players(&self) -> &PlayerArena {
        &self.players
    }
    fn players_mut(&mut self) -> &mut PlayerArena {
        &mut self.players
    }
    fn globals(&self) -> PlayerGlobals {
        self.globals
    }
    fn player_template(&self, t: PlayerTemplateRef) -> PlayerTemplateView {
        self.ptpls[t as usize].clone()
    }
    fn client_event(&mut self, e: ClientEvent) {
        let line = match e {
            ClientEvent::Player(PlayerClientEvent::RadarSound { online, player_index }) => format!("audio {} player={player_index}", if online { 3 } else { 4 }),
            ClientEvent::Player(PlayerClientEvent::ControlBarDirty) => "ui.dirty".to_string(),
            ClientEvent::Player(PlayerClientEvent::SciencePurchasePointsChanged { player_index }) => format!("ui.spp {player_index}"),
            ClientEvent::Player(PlayerClientEvent::RankChanged { player_index }) => format!("ui.rank {player_index}"),
            ClientEvent::Player(PlayerClientEvent::Eva(EvaMessage::GeneralLevelUp)) => "eva 1".to_string(),
            other => panic!("unexpected client event {other:?}"),
        };
        self.rec(line);
    }
    fn rank_level_count(&self) -> i32 {
        self.ranks.len() as i32
    }
    fn rank_info(&self, level: i32) -> Option<RankInfoView> {
        if level >= 1 && (level as usize) <= self.ranks.len() {
            Some(self.ranks[level as usize - 1].clone())
        } else {
            None
        }
    }
    fn science_is_grantable(&self, science: ScienceType) -> bool {
        self.sciences.get(&science.0).is_some_and(|c| c.grantable)
    }
    fn science_purchase_cost(&self, science: ScienceType) -> i32 {
        self.sciences.get(&science.0).map_or(0, |c| c.cost)
    }
    fn science_has_prereqs(&self, p: PlayerIndex, science: ScienceType) -> bool {
        let Some(c) = self.sciences.get(&science.0) else { return false };
        c.prereqs.iter().all(|s| player_has_science(self, p, ScienceType(*s)).unwrap_or(false))
    }
    fn game_text_general_name(&mut self) -> UnicodeString {
        self.rec("text SCIENCE:GeneralName".to_string());
        let chars: Vec<u16> = "General".encode_utf16().collect();
        UnicodeString::try_from_chars(&chars).unwrap()
    }
    fn script_notify_of_object_creation_or_destruction(&mut self) {
        self.rec("script.objchange".to_string());
    }
    fn script_notify_of_acquired_science(&mut self, player_index: PlayerIndex, science: ScienceType) {
        self.rec(format!("script.science {player_index} {}", science.0));
    }
    fn victory_has_single_player_been_defeated(&self, p: PlayerIndex) -> bool {
        self.victory[(p & 15) as usize]
    }
    fn message_enable_retaliation_mode(&mut self, player_index: PlayerIndex, enabled: bool) {
        self.rec("msg.append 1".to_string());
        self.rec(format!("msg.int {player_index}"));
        self.rec(format!("msg.bool {}", enabled as u32));
    }
    fn academy_init(&mut self, p: PlayerIndex) {
        self.rec(format!("academy.init {p}"));
        self.academy_points[p as usize] = 0;
    }
    fn academy_update(&mut self, _p: PlayerIndex) {
        self.rec("academy.update".to_string());
    }
    fn academy_record_income(&mut self, _p: PlayerIndex) {
        self.rec("academy.income".to_string());
    }
    fn academy_record_generals_points_spent(&mut self, p: PlayerIndex, points: i32) {
        self.academy_points[p as usize] = self.academy_points[p as usize].wrapping_add(points as u32);
    }
    fn score_reset(&mut self, _p: PlayerIndex, player_idx: PlayerIndex) {
        self.rec(format!("score.reset {player_idx}"));
    }
    fn score_add_object_built(&mut self, _p: PlayerIndex, o: Option<ObjectId>) {
        self.rec(format!("score.built {}", o.unwrap_or(0)));
    }
    fn score_remove_object_built(&mut self, _p: PlayerIndex, o: Option<ObjectId>) {
        self.rec(format!("score.unbuilt {}", o.unwrap_or(0)));
    }
    fn ai_create(&mut self, p: PlayerIndex, _skirmish: bool) {
        self.rec(format!("ai.new {p}"));
    }
    fn ai_delete(&mut self, p: PlayerIndex) {
        self.rec(format!("ai.delete {p}"));
    }
    fn ai_update(&mut self, p: PlayerIndex) {
        self.rec(format!("ai.update {p}"));
    }
    fn ai_new_map(&mut self, p: PlayerIndex) {
        self.rec(format!("ai.newmap {p}"));
    }
    fn ai_difficulty(&self, p: PlayerIndex) -> GameDifficulty {
        match self.ai_cfg_of(p).diff {
            0 => GameDifficulty::Easy,
            1 => GameDifficulty::Normal,
            _ => GameDifficulty::Hard,
        }
    }
    fn ai_is_skirmish(&self, p: PlayerIndex) -> bool {
        self.players.get_nth(p).and_then(|x| x.ai).is_some_and(|a| a.skirmish)
    }
    fn ai_compute_superweapon_target(&mut self, p: PlayerIndex, _power: SpecialPowerTemplateRef, pos: &mut Coord3D, player_ndx: i32, weapon_radius: f32) -> bool {
        self.rec(format!("ai.super {p} {player_ndx} {}", fb(weapon_radius)));
        let c = self.ai_cfg_of(p);
        if c.super_ok {
            *pos = c.base;
        }
        c.super_ok
    }
    fn ai_enemy(&self, p: PlayerIndex) -> Option<PlayerIndex> {
        let e = self.ai_cfg_of(p).enemy;
        if e >= 0 {
            Some(e % self.np as i32)
        } else {
            None
        }
    }
    fn ai_check_bridges(&mut self, p: PlayerIndex, unit: Option<ObjectId>, way: Option<WaypointId>) -> bool {
        self.rec(format!("ai.bridges {p} {} {}", unit.unwrap_or(0), way.unwrap_or(0)));
        self.ai_cfg_of(p).bridges
    }
    fn ai_base_center(&mut self, p: PlayerIndex, pos: &mut Coord3D) -> bool {
        self.rec(format!("ai.base {p}"));
        let c = self.ai_cfg_of(p);
        if c.has_base {
            *pos = c.base;
        }
        c.has_base
    }
    fn ai_repair_structure(&mut self, p: PlayerIndex, id: ObjectId) {
        self.rec(format!("ai.repair {p} {id}"));
    }
    fn ai_on_unit_produced(&mut self, p: PlayerIndex, factory: Option<ObjectId>, unit: Option<ObjectId>) {
        self.rec(format!("ai.unit {p} {} {}", factory.unwrap_or(0), unit.unwrap_or(0)));
    }
    fn ai_on_structure_produced(&mut self, p: PlayerIndex, builder: Option<ObjectId>, structure: Option<ObjectId>) {
        self.rec(format!("ai.structure {p} {} {}", builder.unwrap_or(0), structure.unwrap_or(0)));
    }
    fn ai_is_supply_source_safe(&mut self, p: PlayerIndex, min_supplies: i32) -> bool {
        self.rec(format!("ai.safe {p} {min_supplies}"));
        self.ai_cfg_of(p).safe
    }
    fn ai_is_supply_source_attacked(&mut self, p: PlayerIndex) -> bool {
        self.rec(format!("ai.attacked {p}"));
        self.ai_cfg_of(p).attacked
    }
    fn ai_set_team_delay_seconds(&mut self, p: PlayerIndex, delay: i32) {
        self.rec(format!("ai.delay {p} {delay}"));
    }
    fn ai_guard_supply_center(&mut self, p: PlayerIndex, team: Option<TeamId>, min_supplies: i32) {
        self.rec(format!("ai.guard {p} {} {min_supplies}", tid(team)));
    }
    fn ai_pre_team_destroy(&mut self, p: PlayerIndex, team: Option<TeamId>) {
        self.rec(format!("ai.preteam {p} {}", tid(team)));
    }
    fn ai_select_skillset(&mut self, p: PlayerIndex, skill_set: i32) {
        self.rec(format!("ai.skillset {p} {skill_set}"));
    }
    fn ai_build_specific_team(&mut self, p: PlayerIndex, proto: Option<TeamPrototypeId>, priority: bool) {
        self.rec(format!("ai.buildteam {p} {} {}", proto.unwrap_or(0), priority as u32));
    }
    fn ai_build_base_defense(&mut self, p: PlayerIndex, flank: bool) {
        self.rec(format!("ai.basedef {p} {}", flank as u32));
    }
    fn ai_build_base_defense_structure(&mut self, p: PlayerIndex, name: &[u8], flank: bool) {
        self.rec(format!("ai.basedefstruct {p} {} {}", String::from_utf8_lossy(name), flank as u32));
    }
    fn ai_build_specific_building(&mut self, p: PlayerIndex, name: &[u8]) {
        self.rec(format!("ai.building {p} {}", String::from_utf8_lossy(name)));
    }
    fn ai_build_by_supplies(&mut self, p: PlayerIndex, minimum_cash: i32, name: &[u8]) {
        self.rec(format!("ai.bysupplies {p} {minimum_cash} {}", String::from_utf8_lossy(name)));
    }
    fn ai_build_specific_building_nearest_team(&mut self, p: PlayerIndex, name: &[u8], team: Option<TeamId>) {
        self.rec(format!("ai.nearest {p} {} {}", String::from_utf8_lossy(name), tid(team)));
    }
    fn ai_build_upgrade(&mut self, p: PlayerIndex, name: &[u8]) {
        self.rec(format!("ai.upgrade {p} {}", String::from_utf8_lossy(name)));
    }
    fn ai_recruit_specific_team(&mut self, p: PlayerIndex, proto: Option<TeamPrototypeId>, radius: f32) {
        self.rec(format!("ai.recruit {p} {} {}", proto.unwrap_or(0), fb(radius)));
    }
    fn ai_calc_closest_construction_zone_location(&mut self, p: PlayerIndex, template: TemplateRef, location: &mut Coord3D) -> bool {
        let name = String::from_utf8_lossy(&self.tpls[template as usize].name).into_owned();
        self.rec(format!("ai.zone {p} {name}"));
        let c = self.ai_cfg_of(p);
        if c.construct {
            *location = c.at;
        }
        c.construct
    }
    fn obj_veterancy_level(&self, o: ObjectId) -> i32 {
        self.obj(o).vet
    }
    fn tmpl_skill_point_value(&self, t: TemplateRef, level: i32) -> i32 {
        self.tpls[t as usize].skill[(level & 3) as usize]
    }
    fn obj_orientation(&self, o: ObjectId) -> f32 {
        self.obj(o).orientation
    }
    fn obj_set_script_status_disabled(&mut self, o: ObjectId, set: bool) {
        self.rec(format!("script-status {o} 1 {}", set as u32));
    }
    fn obj_set_disabled_underpowered(&mut self, o: ObjectId) {
        self.rec(format!("disable {o} {}", DISABLED_UNDERPOWERED));
    }
    fn obj_clear_disabled_underpowered(&mut self, o: ObjectId) {
        self.rec(format!("enable {o} {}", DISABLED_UNDERPOWERED));
    }
    fn obj_special_power_modules(&self, o: ObjectId) -> Vec<SpecialPowerModuleRef> {
        self.obj(o).modules.iter().filter(|m| m.0).map(|m| m.1 as u32).collect()
    }
    fn sp_required_science(&self, m: SpecialPowerModuleRef) -> ScienceType {
        for o in &self.objs {
            for md in &o.modules {
                if md.0 && md.1 as u32 == m {
                    return ScienceType(md.2);
                }
            }
        }
        ScienceType(-1)
    }
    fn sp_on_special_power_creation(&mut self, m: SpecialPowerModuleRef) {
        self.rec(format!("sp.created {m}"));
    }
    fn sp_set_ready_frame(&mut self, m: SpecialPowerModuleRef, frame: u32) {
        self.rec(format!("sp.ready {m} {frame}"));
    }
    fn special_power_template(&self, t: SpecialPowerTemplateRef) -> SpecialPowerView {
        self.sp_templates[t as usize]
    }
    fn partition_distance_squared_2d(&self, a: ObjectId, b: ObjectId) -> f32 {
        let (pa, pb) = (self.obj(a).pos, self.obj(b).pos);
        let (dx, dy) = (pa.x - pb.x, pa.y - pb.y);
        dx * dx + dy * dy
    }
}

/// `DISABLED_UNDERPOWERED`.
const DISABLED_UNDERPOWERED: i32 = rts_lockstep_engine::common::disabled_types::DISABLED_UNDERPOWERED;

// ---------------------------------------------------------------------------------------------------------------------
// The state dump (the same text as dumpState of player_driver.h)
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

fn dash(v: Vec<u8>) -> String {
    if v.is_empty() {
        "-".into()
    } else {
        String::from_utf8_lossy(&v).into_owned()
    }
}

fn uni_str(u: &UnicodeString) -> String {
    let s: String = u.str().iter().map(|c| if *c < 128 { *c as u8 as char } else { '?' }).collect();
    if s.is_empty() {
        "-".into()
    } else {
        s
    }
}

fn sci_str(v: &[ScienceType]) -> String {
    if v.is_empty() {
        "-".into()
    } else {
        v.iter().map(|s| s.0.to_string()).collect::<Vec<_>>().join(",")
    }
}

fn pmap_str(m: &BTreeMap<NameKeyType, f32>) -> String {
    if m.is_empty() {
        return "-".into();
    }
    m.iter().map(|(k, v)| format!("{}:{}", k.0, fb(*v))).collect::<Vec<_>>().join(",")
}

fn vmap_str(m: &BTreeMap<NameKeyType, VeterancyLevel>) -> String {
    if m.is_empty() {
        return "-".into();
    }
    m.iter().map(|(k, v)| format!("{}:{}", k.0, *v as i32)).collect::<Vec<_>>().join(",")
}

fn kind_bits(m: &KindOfMaskType) -> String {
    let v: Vec<String> = (0..KINDOF_COUNT).filter(|k| m.test(*k)).map(|k| k.to_string()).collect();
    if v.is_empty() {
        "-".into()
    } else {
        v.join("+")
    }
}

/// The prototypes alive: the union of the players' lists, in id order.
fn alive_protos(sim: &Sim) -> Vec<TeamPrototypeId> {
    let mut v: Vec<TeamPrototypeId> = Vec::new();
    for i in 0..sim.np {
        for pr in &sim.players.get_nth(i as i32).unwrap().team_prototypes {
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
        sim.undef("a player's list names a prototype that was deleted");
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

fn dump_player(sim: &mut Sim, idx: usize, out: &mut Vec<String>) {
    let frame = sim.frame;
    let p = sim.players.get_nth(idx as i32).expect("player").clone();
    let mut build = p
        .build_list
        .iter()
        .map(|e: &BuildListInfo| {
            format!(
                "{}/{}/{},{},{}/{}/{}/{}",
                e.object_id,
                dash(e.template_name.clone()),
                fb(e.location.x),
                fb(e.location.y),
                fb(e.location.z),
                fb(e.angle),
                e.num_rebuilds as i32,
                e.priority_build as u32
            )
        })
        .collect::<Vec<_>>()
        .join(";");
    if build.is_empty() {
        build = "-".into();
    }
    let ai = match p.ai {
        Some(a) => format!("{}/{}", a.skirmish as u32, sim.ai_difficulty(idx as i32) as i32),
        None => "-".into(),
    };
    let tpl = p.template.map_or(-1, |t| t as i32);
    out.push(format!(
        "D player {} tpl={} name={} key={} disp={} side={} base={} type={} money={}/{} hc={},{},{},{} radar={},{},{} plans={},{},{} bonus={} up={} energy={},{},{},{} build={} color={:08x},{:08x} pcost={} ptime={} pvet={} ai={} mp={} rgm={} tun={} def={}",
        p.player_index,
        tpl,
        dash(p.player_name.clone()),
        p.name_key,
        uni_str(&p.display_name),
        dash(p.side.clone()),
        dash(p.base_side.clone()),
        p.player_type as i32,
        p.money.count_money(),
        p.money.player_index(),
        fb(p.handicap.get_handicap(HandicapType::BuildCost, false)),
        fb(p.handicap.get_handicap(HandicapType::BuildCost, true)),
        fb(p.handicap.get_handicap(HandicapType::BuildTime, false)),
        fb(p.handicap.get_handicap(HandicapType::BuildTime, true)),
        p.radar_count,
        p.disable_proof_radar_count,
        p.radar_disabled as u32,
        p.bombard_battle_plans,
        p.hold_the_line_battle_plans,
        p.search_and_destroy_battle_plans,
        p.battle_plan_bonuses.is_some() as u32,
        (!p.upgrades.is_empty()) as u32,
        p.energy.production(frame),
        p.energy.consumption(),
        p.energy.power_sabotaged_till_frame(),
        p.energy.owner().unwrap_or(-1),
        build,
        p.color,
        p.night_color,
        pmap_str(&p.production_cost_changes),
        pmap_str(&p.production_time_changes),
        vmap_str(&p.production_veterancy_levels),
        ai,
        p.mp_start_index,
        p.resource_gathering_manager.is_some() as u32,
        p.tunnel_system.is_some() as u32,
        tid(p.default_team),
    ));
    let att: String = p.attacked_by.iter().map(|b| if *b { '1' } else { '0' }).collect();
    let kop = if p.kind_of_production_changes.is_empty() {
        "-".to_string()
    } else {
        p.kind_of_production_changes.iter().map(|t| format!("{}/{}/{}", kind_bits(&t.kind_of), fb(t.percent), t.ref_count)).collect::<Vec<_>>().join(";")
    };
    let spt = if p.special_power_timers.is_empty() {
        "-".to_string()
    } else {
        p.special_power_timers.iter().map(|t| format!("{}/{}", t.template_id, t.ready_frame)).collect::<Vec<_>>().join(";")
    };
    out.push(format!(
        "D player2 {} sci={} dis={} hid={} rank={},{},{},{},{} gen={} protos={} prel={} trel={} flags={}{}{}{}{}{}{}{} mod={} att={} attf={} bounty={} kop={} spt={} squads={},{} gpts={}",
        p.player_index,
        sci_str(&p.sciences),
        sci_str(&p.sciences_disabled),
        sci_str(&p.sciences_hidden),
        p.rank_level,
        p.skill_points,
        p.science_purchase_points,
        p.level_up,
        p.level_down,
        uni_str(&p.general_name),
        id_list(&p.team_prototypes),
        rel_map_str(&p.player_relations.map),
        rel_map_str(&p.team_relations.map),
        p.can_build_units as u32,
        p.can_build_base as u32,
        p.observer as u32,
        p.is_preorder as u32,
        p.list_in_score_screen as u32,
        p.units_should_hunt as u32,
        p.is_player_dead as u32,
        p.logical_retaliation_mode_enabled as u32,
        fb(p.skill_points_modifier),
        att,
        p.attacked_frame,
        fb(p.cash_bounty_percent),
        kop,
        spt,
        p.squads.len(),
        p.current_selection.is_some() as u32,
        sim.academy_points[idx],
    ));
}

fn dump_state(sim: &mut Sim) {
    let mut out: Vec<String> = Vec::new();
    out.push(format!(
        "D frame={} tid={} pid={} nprotos={} local={}",
        sim.frame,
        sim.teams.last_team_id(),
        sim.teams.last_prototype_id(),
        sim.teams.prototype_count(),
        sim.players.local_index().unwrap_or(-1)
    ));
    for pid in alive_protos(sim) {
        let p = sim.teams.prototype(pid).expect("alive prototype");
        let insts = sim.teams.instances(pid);
        out.push(format!("D proto id={} name={} owner={} insts={}", p.id, String::from_utf8_lossy(&p.name), p.owning_player.unwrap_or(-1), id_list(&insts)));
    }
    for tid_ in alive_teams(sim) {
        let t = sim.teams.team(tid_).expect("alive team");
        let mem = sim.teams.members(tid_);
        out.push(format!("D team id={} proto={} members={} act={}", t.id, t.proto, id_list(&mem), t.active as u32));
    }
    for o in &sim.objs {
        let next = sim.teams.next_member(o.id).unwrap_or(0);
        let prev = sim.teams.prev_member(o.id).unwrap_or(0);
        out.push(format!("D obj id={} team={} next={} prev={}", o.id, tid(o.team), next, prev));
    }
    for i in 0..sim.np {
        dump_player(sim, i, &mut out);
    }
    sim.lines.extend(out);
}

// ---------------------------------------------------------------------------------------------------------------------
// The program: the header and the operations
// ---------------------------------------------------------------------------------------------------------------------

struct Excluded(&'static str);

fn team_name(id: u32) -> Vec<u8> {
    if id >= 100 {
        format!("teamP{}", id - 100).into_bytes()
    } else {
        format!("Team{id}").into_bytes()
    }
}

fn thing_name(v: u32) -> Vec<u8> {
    if v >= 8 {
        format!("tpl{}", v - 8).into_bytes()
    } else {
        format!("Tpl{v}").into_bytes()
    }
}

fn count_str(c: &[i32]) -> String {
    c.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
}

/// How an error ends a program: a throw upstream makes (the line to write), or an exclusion.
fn thrown(e: PlayerError) -> Result<String, Excluded> {
    match e {
        PlayerError::Team(TeamError::BadArg) => Ok("threw badarg".to_string()),
        PlayerError::Team(TeamError::InvalidData) => Ok("threw invaliddata".to_string()),
        PlayerError::Team(TeamError::Xfer(s)) => Ok(format!("threw xfer {}", s as i32)),
        PlayerError::Team(TeamError::Null) | PlayerError::NoPlayer => Err(Excluded("a NULL or dangling pointer dereferenced")),
        PlayerError::Team(TeamError::Hang) | PlayerError::Hang => Err(Excluded("a loop that never ends")),
        PlayerError::Team(TeamError::IdCollision) => Err(Excluded("an id shared by two live teams after reset")),
        PlayerError::NoRank => Err(Excluded("a rank level the table does not have, dereferenced")),
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

fn pick_obj(sim: &Sim, k: u32) -> ObjectId {
    sim.objs[k as usize % sim.objs.len()].id
}

fn pick_opt_obj(sim: &Sim, k: u32) -> Option<ObjectId> {
    if k == 0 {
        None
    } else {
        Some(pick_obj(sim, k - 1))
    }
}

fn pick_opt_team(sim: &Sim, k: u32) -> Option<TeamId> {
    if k == 0 {
        None
    } else {
        pick_team(sim, k - 1)
    }
}

fn pick_opt_proto(sim: &Sim, k: u32) -> Option<TeamPrototypeId> {
    if k == 0 {
        None
    } else {
        pick_proto(sim, k - 1)
    }
}

fn money_of(amount: u32, idx: i32) -> Money {
    let mut m = Money::new();
    let mut ev = Vec::new();
    m.deposit(amount, false, &mut ev);
    m.set_player_index(idx);
    m
}

fn read_money(r: &mut Reader<'_>) -> Money {
    let amount = r.u();
    let idx = r.i();
    money_of(amount, idx)
}

fn read_dicts(r: &mut Reader<'_>, sim: &mut Sim) {
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
}

fn read_sim(r: &mut Reader<'_>) -> (Sim, Vec<Vec<u8>>) {
    let mut names = NameKeyGenerator::new();
    names.reset();
    let mut sim = Sim {
        teams: TeamArena::new(),
        players: PlayerArena::new(),
        names,
        keys: WellKnownKeys::new(),
        lines: Vec::new(),
        frame: 0,
        np: 0,
        player_by_key: BTreeMap::new(),
        tpls: Vec::new(),
        objs: Vec::new(),
        scripts: Vec::new(),
        originals: 0,
        ways: Vec::new(),
        dicts: Vec::new(),
        globals: PlayerGlobals {
            client_retaliation_mode_enabled: false,
            base_value_per_supply_box: 0,
            solo_player_health_bonus: [[1.0; 3]; 2],
            default_starting_cash: Money::new(),
            time_of_day_is_night: false,
            game_info_starting_cash: None,
            force_skirmish_ai: false,
            in_single_player_game: false,
            in_shell_game: false,
            in_game_logic_update: false,
            rank_level_limit: 0,
            client_quiet: false,
            global_difficulty: GameDifficulty::Normal,
        },
        victory: [false; 16],
        ranks: Vec::new(),
        sciences: BTreeMap::new(),
        sp_templates: Vec::new(),
        ptpls: Vec::new(),
        ai_cfg: Vec::new(),
        academy_points: vec![0; 16],
        undefined: std::cell::Cell::new(None),
    };
    // the dictionaries
    read_dicts(r, &mut sim);
    // the services
    let g = &mut sim.globals;
    g.client_retaliation_mode_enabled = r.b();
    g.base_value_per_supply_box = r.u();
    for a in 0..2 {
        for d in 0..3 {
            g.solo_player_health_bonus[a][d] = r.f();
        }
    }
    g.default_starting_cash = read_money(r);
    g.time_of_day_is_night = r.b();
    let has_info = r.b();
    let cash = read_money(r);
    g.game_info_starting_cash = if has_info { Some(cash) } else { None };
    g.force_skirmish_ai = r.b();
    g.in_single_player_game = r.b();
    g.in_shell_game = r.b();
    g.in_game_logic_update = r.b();
    g.rank_level_limit = r.i();
    g.client_quiet = r.b();
    g.global_difficulty = match r.u() % 3 {
        0 => GameDifficulty::Easy,
        1 => GameDifficulty::Normal,
        _ => GameDifficulty::Hard,
    };
    for k in 0..16 {
        sim.victory[k] = r.b();
    }
    let nr = r.i();
    for _ in 0..nr {
        let needed = r.i();
        let spp = r.i();
        let ng = r.i();
        let sciences_granted = (0..ng).map(|_| ScienceType(r.i())).collect();
        sim.ranks.push(RankInfoView { skill_points_needed: needed, science_purchase_points_granted: spp, sciences_granted });
    }
    let nsci = r.i();
    for _ in 0..nsci {
        let id = r.i();
        let cost = r.i();
        let grantable = r.b();
        let np2 = r.i();
        let prereqs = (0..np2).map(|_| r.i()).collect();
        sim.sciences.insert(id, SciCfg { cost, grantable, prereqs });
    }
    let nsp = r.i();
    for _ in 0..nsp {
        let id = r.u();
        let reload_time = r.u();
        let shared_n_sync = r.b();
        sim.sp_templates.push(SpecialPowerView { id, reload_time, shared_n_sync });
    }
    let npt = r.i();
    for _ in 0..npt {
        let beacon = format!("Tpl{}", r.i()).into_bytes();
        let side = format!("Side{}", r.i()).into_bytes();
        let base_side = format!("Base{}", r.i()).into_bytes();
        let color = r.i();
        let money = read_money(r);
        let di = r.u();
        let mut handicap = Handicap::new();
        if !sim.dicts.is_empty() {
            let spec = sim.dicts[di as usize % sim.dicts.len()].clone();
            let d = sim.build_dict(&spec);
            handicap.read_from_dict(&mut sim.names, &d);
        }
        let mut cost = BTreeMap::new();
        let nc = r.i();
        for _ in 0..nc {
            let name = format!("Tpl{}", r.i());
            let key = sim.names.name_to_key(name.as_bytes());
            cost.insert(key, r.f());
        }
        let mut time = BTreeMap::new();
        let nc = r.i();
        for _ in 0..nc {
            let name = format!("Tpl{}", r.i());
            let key = sim.names.name_to_key(name.as_bytes());
            time.insert(key, r.f());
        }
        let mut vet = BTreeMap::new();
        let nc = r.i();
        for _ in 0..nc {
            let name = format!("Tpl{}", r.i());
            let key = sim.names.name_to_key(name.as_bytes());
            let level = match r.u() % 4 {
                0 => VeterancyLevel::Regular,
                1 => VeterancyLevel::Veteran,
                2 => VeterancyLevel::Elite,
                _ => VeterancyLevel::Heroic,
            };
            vet.insert(key, level);
        }
        let observer = r.b();
        let playable_side = r.b();
        let nc = r.i();
        let intrinsic_sciences = (0..nc).map(|_| ScienceType(r.i())).collect();
        let intrinsic_science_purchase_points = r.i();
        sim.ptpls.push(PlayerTemplateView {
            side,
            base_side,
            preferred_color_as_int: color,
            money,
            handicap,
            production_cost_changes: cost,
            production_time_changes: time,
            production_veterancy_levels: vet,
            observer,
            playable_side,
            intrinsic_sciences,
            intrinsic_science_purchase_points,
            beacon_template: beacon,
        });
    }
    let np = r.i();
    for _ in 0..np {
        let (safe, attacked, bridges, has_base) = (r.b(), r.b(), r.b(), r.b());
        let base = r.c();
        let (super_ok, construct) = (r.b(), r.b());
        let at = r.c();
        let enemy = r.i();
        let diff = r.u() % 3;
        sim.ai_cfg.push(AiCfg { safe, attacked, bridges, has_base, base, super_ok, construct, at, enemy, diff });
    }
    // PlayerList::PlayerList makes all MAX_PLAYER_COUNT players, each ending in init( NULL )
    sim.np = np as usize;
    for i in 0..16 {
        player_init(&mut sim, i, None).expect("init");
        let k = sim.names.name_to_key(format!("Player{i}").as_bytes());
        sim.player_by_key.insert(k.0, i as usize);
    }
    sim.players.set_player_count(np);
    sim.players.set_local_player(Some(0));

    let nt = r.i();
    for _ in 0..nt {
        let nk = r.i();
        let mut kinds = KindOfMaskType::new();
        for _ in 0..nk {
            kinds.set(KIND_POOL[(r.u() % 12) as usize], 1);
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
        let skill = [r.i(), r.i(), r.i(), r.i()];
        sim.tpls.push(TplSim { kinds, build_facility, equiv, name, variations, skill });
    }
    let no = r.i();
    for _ in 0..no {
        let id = r.u() as ObjectId;
        let tpl = r.u() as usize % sim.tpls.len();
        let player = r.u() as usize % sim.np;
        let ns = r.i();
        let status = (0..ns).map(|_| STATUS_POOL[(r.u() % 4) as usize]).collect();
        let (dead, destroyed, contained, held, enemy_near) = (r.b(), r.b(), r.b(), r.b(), r.b());
        let (has_ai, ai_idle, ai_recruit) = (r.b(), r.b(), r.b());
        let surfaces = r.u();
        let contain_count = r.i();
        let pos = r.c();
        let vision = r.f();
        let (did_enter, did_exit, inside, reactions) = (r.u(), r.u(), r.u(), r.u());
        let orientation = r.f();
        let vet = (r.u() % 4) as i32;
        let nm = (r.i() % 4) as usize;
        let mut modules = Vec::new();
        for _ in 0..nm {
            modules.push((r.b(), r.i(), r.i()));
        }
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
            orientation,
            vet,
            modules,
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
    sim.frame = r.u();
    let n_probe = r.i();
    let probes = (0..n_probe).map(|_| r.s()).collect();
    (sim, probes)
}

/// Runs the Rust side of a program, giving the trace lines (as the C++ side writes them), or why it cannot be given
/// to the C++.
fn run_rust(words: &[u32]) -> Result<Vec<String>, Excluded> {
    let mut r = Reader { w: words, p: 0 };
    let (mut sim, probes) = read_sim(&mut r);
    let n_ops = r.i();
    let mut threw = false;
    dump_state(&mut sim);
    for _ in 0..n_ops {
        let op = r.u();
        let res = do_op(&mut sim, op, &mut r);
        if let Some(why) = sim.undefined.get() {
            return Err(Excluded(why));
        }
        match res {
            Ok(()) => dump_state(&mut sim),
            Err(e) => {
                if std::env::var_os("PLAYER_WHY").is_some() {
                    eprintln!("WHY {e:?} at op {op}");
                }
                let line = thrown(e)?;
                sim.rec(line);
                // `ERROR_BAD_ARG` is thrown before anything is changed: the program goes on
                if e == PlayerError::Team(TeamError::BadArg) {
                    dump_state(&mut sim);
                    if let Some(why) = sim.undefined.get() {
                        return Err(Excluded(why));
                    }
                    continue;
                }
                threw = true;
                break;
            }
        }
        if let Some(why) = sim.undefined.get() {
            return Err(Excluded(why));
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

fn coord_of(a: u32, b: u32, c: u32) -> Coord3D {
    Coord3D { x: f32::from_bits(a), y: f32::from_bits(b), z: f32::from_bits(c) }
}

fn sci_of(v: u32) -> ScienceType {
    ScienceType((v % 8) as i32)
}

fn rel_of(v: u32) -> Relationship {
    match v % 3 {
        0 => Relationship::Enemies,
        1 => Relationship::Neutral,
        _ => Relationship::Allies,
    }
}

fn coord_str(c: &Coord3D) -> String {
    format!("{},{},{}", fb(c.x), fb(c.y), fb(c.z))
}

fn obj_op(sim: &mut Sim, o: ObjectId, sub: u32, a: &[u32; 8]) {
    match sub {
        0 => sim.obj_mut(o).dead = a[0] != 0,
        1 => sim.obj_mut(o).destroyed = a[0] != 0,
        2 => {
            let s = STATUS_POOL[(a[0] % 4) as usize];
            if a[1] != 0 {
                sim.obj_mut(o).status.insert(s);
            } else {
                sim.obj_mut(o).status.remove(&s);
            }
        }
        3 => sim.obj_mut(o).enemy_near = a[0] != 0,
        4 => sim.obj_mut(o).pos = coord_of(a[0], a[1], a[2]),
        5 => sim.obj_mut(o).reactions = a[0],
        6 => sim.obj_mut(o).orientation = f32::from_bits(a[0]),
        7 => sim.obj_mut(o).vet = (a[0] % 4) as i32,
        8 => sim.obj_mut(o).held = a[0] != 0,
        _ => {}
    }
}

fn thing_list(sim: &Sim, a: &[u32; 8]) -> (Vec<Option<TemplateRef>>, Vec<i32>) {
    let n = (a[0] % 5) as usize;
    let things = (0..n).map(|k| if a[1 + k] != 0 { Some(((a[1 + k] - 1) as usize % sim.tpls.len()) as TemplateRef) } else { None }).collect();
    (things, vec![7; n])
}

/// One `Player` operation; `a` are its eight words.
fn player_op(sim: &mut Sim, p: PlayerIndex, sub: u32, a: &[u32; 8]) -> Result<(), PlayerError> {
    let np = sim.np;
    match sub {
        0 => player_update(sim, p)?,
        1 => player_new_map(sim, p)?,
        2 => {
            let t = if a[0].is_multiple_of(2) { PlayerType::Human } else { PlayerType::Computer };
            player_set_player_type(sim, p, t, a[1] != 0)?;
        }
        3 => player_set_default_team(sim, p)?,
        4 => {
            sim.players.get_nth_mut(p).unwrap().player_name = format!("P{}", a[0] % 4).into_bytes();
        }
        5 => {
            let t = pick_opt_team(sim, a[0]);
            let r = player_get_relationship(sim, p, t)?;
            sim.rec(format!("ret rel {}", r as i32));
        }
        6 => {
            let o = if a[0] != 0 { Some(((a[0] - 1) as usize % np) as PlayerIndex) } else { None };
            player_set_player_relationship(sim, p, o, rel_of(a[1]))?;
        }
        7 => {
            let o = if a[0] != 0 { Some(((a[0] - 1) as usize % np) as PlayerIndex) } else { None };
            let r = player_remove_player_relationship(sim, p, o)?;
            sim.rec(format!("ret rmprel {}", r as u32));
        }
        8 => {
            let t = pick_opt_team(sim, a[0]);
            player_set_team_relationship(sim, p, t, rel_of(a[1]))?;
        }
        9 => {
            let t = pick_opt_team(sim, a[0]);
            let r = player_remove_team_relationship(sim, p, t)?;
            sim.rec(format!("ret rmtrel {}", r as u32));
        }
        10 => {
            let n = a[0] % 4;
            let mut list = Vec::new();
            for k in 0..n {
                let mut e = BuildListInfo::new();
                e.object_id = a[1].wrapping_add(k) as i32;
                e.template_name = thing_name(a[2].wrapping_add(k));
                e.angle = k as f32;
                e.num_rebuilds = (a[3] as i32).wrapping_add(k as i32) as u32;
                list.push(e);
            }
            player_set_build_list(sim, p, list)?;
        }
        11 => {
            let o = pick_obj(sim, a[0]);
            player_add_to_build_list(sim, p, o)?;
        }
        12 => {
            let pos = coord_of(a[1], a[2], a[3]);
            player_add_to_priority_build_list(sim, p, &thing_name(a[0] % 16), pos, f32::from_bits(a[4]))?;
        }
        13 => {
            let pt = if a[0] != 0 && !sim.ptpls.is_empty() { Some((a[0] as usize - 1) % sim.ptpls.len()) } else { None };
            player_init(sim, p, pt.map(|t| t as PlayerTemplateRef))?;
        }
        14 => {
            let frame = sim.frame;
            let notice = sim.players.get_nth_mut(p).unwrap().energy.adjust_power(frame, a[0] as i32, a[1] != 0);
            player_deliver_brown_out_notice(sim, p, notice)?;
        }
        15 => sim.players.get_nth_mut(p).unwrap().energy.set_power_sabotaged_till_frame(a[0]),
        16 => {
            let r = player_is_skirmish_ai_player(sim, p)?;
            sim.rec(format!("ret skirmish {}", r as u32));
        }
        17 => {
            let t = (a[0] as usize % sim.sp_templates.len()) as SpecialPowerTemplateRef;
            let mut pos = coord_of(a[2], a[3], a[4]);
            let r = player_compute_superweapon_target(sim, p, t, &mut pos, (a[5] % 4) as i32, f32::from_bits(a[1]))?;
            sim.rec(format!("ret super {} {}", r as u32, coord_str(&pos)));
        }
        18 => {
            let e = player_get_current_enemy(sim, p)?;
            sim.rec(format!("ret enemy {}", e.unwrap_or(-1)));
        }
        19 => {
            let d = player_get_player_difficulty(sim, p)?;
            sim.rec(format!("ret diff {}", d as i32));
        }
        20 => {
            let o = pick_opt_obj(sim, a[0]);
            let w = if a[1] != 0 && !sim.ways.is_empty() { Some(sim.ways[(a[1] as usize - 1) % sim.ways.len()].1) } else { None };
            let r = player_check_bridges(sim, p, o, w)?;
            sim.rec(format!("ret bridges {}", r as u32));
        }
        21 => {
            let mut pos = coord_of(a[0], a[1], a[2]);
            let r = player_get_ai_base_center(sim, p, &mut pos)?;
            sim.rec(format!("ret base {} {}", r as u32, coord_str(&pos)));
        }
        22 => player_repair_structure(sim, p, a[0] as ObjectId)?,
        23 => {
            let (f, u) = (pick_opt_obj(sim, a[0]), pick_opt_obj(sim, a[1]));
            player_on_unit_created(sim, p, f, u)?;
        }
        24 => {
            let r = player_is_supply_source_safe(sim, p, a[0] as i32)?;
            sim.rec(format!("ret safe {}", r as u32));
        }
        25 => {
            let r = player_is_supply_source_attacked(sim, p)?;
            sim.rec(format!("ret attacked {}", r as u32));
        }
        26 => player_set_team_delay_seconds(sim, p, a[0] as i32)?,
        27 => {
            let t = pick_opt_team(sim, a[0]);
            player_guard_supply_center(sim, p, t, a[1] as i32)?;
        }
        28 => {
            let t = pick_opt_team(sim, a[0]);
            player_pre_team_destroy(sim, p, t)?;
        }
        29 => {
            let (b, s) = (pick_opt_obj(sim, a[0]), pick_opt_obj(sim, a[1]));
            player_on_structure_created(sim, p, b, s);
        }
        30 => {
            let s = pick_opt_obj(sim, a[0]);
            player_on_structure_undone(sim, p, s);
        }
        31 => match pick_proto(sim, a[0]) {
            None => sim.rec("skip".into()),
            Some(pr) => player_add_team_to_list(sim, p, pr)?,
        },
        32 => match pick_proto(sim, a[0]) {
            None => sim.rec("skip".into()),
            Some(pr) => player_remove_team_from_list(sim, p, pr)?,
        },
        33 => player_heal_all_objects(sim, p)?,
        34 => {
            let mut seen = Vec::new();
            player_iterate_objects(sim, p, |_, o| seen.push(o))?;
            for o in seen {
                sim.rec(format!("iter {o}"));
            }
        }
        35 => {
            let (things, mut counts) = thing_list(sim, a);
            player_count_objects_by_thing_template(sim, p, &things, a[5] != 0, &mut counts, a[6] != 0)?;
            sim.rec(format!("ret count {}", count_str(&counts)));
        }
        36 => {
            let r = player_count_buildings(sim, p)?;
            sim.rec(format!("ret buildings {r}"));
        }
        37 => {
            let r = player_count_objects(sim, p, &kind_mask(a[0]), &kind_mask(a[1]))?;
            sim.rec(format!("ret objects {r}"));
        }
        38 => {
            let q = pick_opt_obj(sim, a[0]);
            let c = player_find_closest_by_kind_of(sim, p, q, &kind_mask(a[1]), &kind_mask(a[2]))?;
            sim.rec(format!("ret closest {}", c.unwrap_or(0)));
        }
        39 => {
            let r = player_has_any_buildings(sim, p)?;
            sim.rec(format!("ret anybuildings {}", r as u32));
        }
        40 => {
            let r = player_has_any_buildings_kind_of(sim, p, &kind_mask(a[0]))?;
            sim.rec(format!("ret anybuildingskind {}", r as u32));
        }
        41 => {
            let r = player_has_any_units(sim, p)?;
            sim.rec(format!("ret anyunits {}", r as u32));
        }
        42 => {
            let r = player_has_any_objects(sim, p)?;
            sim.rec(format!("ret anyobjects {}", r as u32));
        }
        43 => {
            let r = player_has_any_build_facility(sim, p)?;
            sim.rec(format!("ret buildfacility {}", r as u32));
        }
        44 => player_update_team_states(sim, p)?,
        45 => {
            let r = player_is_local_player(sim, p);
            sim.rec(format!("ret local {}", r as u32));
        }
        46 => {
            player_set_list_in_score_screen(sim, p, a[0] != 0)?;
            let r = sim.players.get_nth(p).unwrap().list_in_score_screen;
            sim.rec(format!("ret score {}", r as u32));
        }
        47 => {
            let r = player_get_supply_box_value(sim);
            sim.rec(format!("ret box {r}"));
        }
        48 => {
            let n = thing_name(a[0] % 16);
            let c = player_get_production_cost_change_percent(sim, p, &n)?;
            let t = player_get_production_time_change_percent(sim, p, &n)?;
            let v = player_get_production_veterancy_level(sim, p, &n)?;
            sim.rec(format!("ret prod {} {} {}", fb(c), fb(t), v as i32));
        }
        49 => player_friend_set_skillset(sim, p, a[0] as i32)?,
        50 => player_set_objects_enabled(sim, p, &thing_name(a[0] % 16), a[1] != 0)?,
        51 => {
            let me = sim.players.get_nth_mut(p).unwrap();
            me.can_build_units = a[0] != 0;
            me.can_build_base = a[1] != 0;
        }
        52 => {
            let t = (a[0] as usize % sim.tpls.len()) as TemplateRef;
            let r = player_allowed_to_build(sim, p, t)?;
            sim.rec(format!("ret allowed {}", r as u32));
        }
        53 => {
            let pr = pick_opt_proto(sim, a[0]);
            player_build_specific_team(sim, p, pr)?;
        }
        54 => player_build_base_defense(sim, p, a[0] != 0)?,
        55 => player_build_base_defense_structure(sim, p, &thing_name(a[0] % 16), a[1] != 0)?,
        56 => player_build_specific_building(sim, p, &thing_name(a[0] % 16))?,
        57 => player_build_by_supplies(sim, p, a[0] as i32, &thing_name(a[1] % 16))?,
        58 => {
            let t = pick_opt_team(sim, a[1]);
            player_build_specific_building_nearest_team(sim, p, &thing_name(a[0] % 16), t)?;
        }
        59 => player_build_upgrade(sim, p, &thing_name(a[0] % 16))?,
        60 => {
            let pr = pick_opt_proto(sim, a[0]);
            player_recruit_specific_team(sim, p, pr, f32::from_bits(a[1]))?;
        }
        61 => {
            let t = (a[0] as usize % sim.tpls.len()) as TemplateRef;
            let mut loc = coord_of(a[1], a[2], a[3]);
            let r = player_calc_closest_construction_zone_location(sim, p, t, &mut loc)?;
            sim.rec(format!("ret zone {} {}", r as u32, coord_str(&loc)));
        }
        62 => player_add_radar(sim, p, a[0] != 0)?,
        63 => player_remove_radar(sim, p, a[0] != 0)?,
        64 => player_disable_radar(sim, p)?,
        65 => player_enable_radar(sim, p)?,
        66 => {
            let r = player_has_radar(sim, p)?;
            sim.rec(format!("ret radar {}", r as u32));
        }
        67 => {
            let r = player_ok_to_play_radar_edge_sound(sim, p)?;
            sim.rec(format!("ret radarok {}", r as u32));
        }
        68 => player_on_power_brown_out_change(sim, p, a[0] != 0)?,
        69 => {
            let t = (a[1] as usize % sim.sp_templates.len()) as SpecialPowerTemplateRef;
            match a[0] % 4 {
                0 => player_reset_or_start_special_power_ready_frame(sim, p, t)?,
                1 => player_express_special_power_ready_frame(sim, p, t, a[2])?,
                2 => {
                    let r = player_get_or_start_special_power_ready_frame(sim, p, t)?;
                    sim.rec(format!("ret spready {r}"));
                }
                _ => player_add_new_shared_special_power_timer(sim, p, t, a[2])?,
            }
        }
        70 => {
            let k = kind_mask(a[1]);
            let pc = f32::from_bits(a[2]);
            match a[0] % 3 {
                0 => player_add_kind_of_production_cost_change(sim, p, k, pc)?,
                1 => player_remove_kind_of_production_cost_change(sim, p, k, pc)?,
                _ => {
                    let r = player_get_production_cost_change_based_on_kind_of(sim, p, &k)?;
                    sim.rec(format!("ret kopc {}", fb(r)));
                }
            }
        }
        71 => {
            if a[0].is_multiple_of(2) {
                player_set_attacked_by(sim, p, (a[1] % 16) as i32)?;
            } else {
                let r = player_get_attacked_by(sim, p, (a[1] % 16) as i32)?;
                sim.rec(format!("ret attackedby {}", r as u32));
            }
        }
        72 => {
            let (o, d, ac, pl_) = (player_is_player_observer(sim, p)?, player_is_player_dead(sim, p)?, player_is_player_active(sim, p)?, player_is_playable_side(sim, p)?);
            sim.rec(format!("ret status {}{}{}{}", o as u32, d as u32, ac as u32, pl_ as u32));
        }
        73 => {
            sim.players.set_local_player(Some((a[0] as usize % np) as PlayerIndex));
        }
        74 => {
            let s = sci_of(a[1]);
            match a[0] % 10 {
                0 => {
                    let r = player_has_science(sim, p, s)?;
                    sim.rec(format!("ret hasscience {}", r as u32));
                }
                1 => {
                    let ty = match a[2] % 3 {
                        0 => ScienceAvailabilityType::Available,
                        1 => ScienceAvailabilityType::Disabled,
                        _ => ScienceAvailabilityType::Hidden,
                    };
                    player_set_science_availability(sim, p, s, ty)?;
                }
                2 => {
                    let r = player_attempt_to_purchase_science(sim, p, s)?;
                    sim.rec(format!("ret purchase {}", r as u32));
                }
                3 => {
                    let r = player_grant_science(sim, p, s)?;
                    sim.rec(format!("ret grant {}", r as u32));
                }
                4 => {
                    let r = player_is_capable_of_purchasing_science(sim, p, s)?;
                    sim.rec(format!("ret capable {}", r as u32));
                }
                5 => {
                    let (d, h) = (player_is_science_disabled(sim, p, s)?, player_is_science_hidden(sim, p, s)?);
                    sim.rec(format!("ret disabledhidden {}{}", d as u32, h as u32));
                }
                6 => {
                    let r = player_has_prereqs_for_science(sim, p, s);
                    sim.rec(format!("ret prereqs {}", r as u32));
                }
                7 => {
                    let names: [&[u8]; 4] = [b"Available", b"disabled", b"HIDDEN", b"other"];
                    let r = player_get_science_availability_type_from_string(names[(a[2] % 4) as usize]);
                    sim.rec(format!("ret avail {}", r as i32));
                }
                8 => player_add_science_purchase_points(sim, p, a[2] as i32)?,
                _ => player_reset_sciences(sim, p)?,
            }
        }
        75 => match a[0] % 5 {
            0 => {
                let r = player_set_rank_level(sim, p, (a[1] as i32) % 8)?;
                sim.rec(format!("ret setrank {}", r as u32));
            }
            1 => {
                let r = player_add_skill_points(sim, p, (a[1] as i32) % 5000)?;
                sim.rec(format!("ret addskill {}", r as u32));
            }
            2 => {
                let (k, v) = (pick_opt_obj(sim, a[1]), pick_opt_obj(sim, a[2]));
                let r = player_add_skill_points_for_kill(sim, p, k, v)?;
                sim.rec(format!("ret killskill {}", r as u32));
            }
            3 => player_reset_rank(sim, p)?,
            _ => {
                let m = match a[1] % 3 {
                    0 => 1.0f32,
                    1 => 0.5,
                    _ => 2.25,
                };
                sim.players.get_nth_mut(p).unwrap().skill_points_modifier = m;
            }
        },
        76 => {
            let me = sim.players.get_nth_mut(p).unwrap();
            me.cash_bounty_percent = if !a[0].is_multiple_of(2) { 0.25 } else { 0.0 };
            me.units_should_hunt = a[1] != 0;
            me.is_player_dead = a[2] != 0;
        }
        _ => sim.rec("ret nop".into()),
    }
    Ok(())
}

fn do_op(sim: &mut Sim, op: u32, r: &mut Reader<'_>) -> Result<(), PlayerError> {
    let np = sim.np;
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
            let id = r.u();
            let t = factory_create_team(sim, &team_name(id))?;
            sim.rec(format!("ret team {t}"));
        }
        3 => {
            let id = r.u();
            let t = factory_create_inactive_team(sim, &team_name(id))?;
            sim.rec(format!("ret team {t}"));
        }
        4 => {
            let (ok, ts) = (r.u(), r.u());
            let t = if ts != 0 { pick_team(sim, ts - 1) } else { None };
            if ts != 0 && t.is_none() {
                sim.rec("skip".into());
            } else {
                let o = pick_obj(sim, ok);
                sim.obj_set_team(o, t);
            }
        }
        5 => {
            let (ok, sub) = (r.u(), r.u());
            let mut a = [0u32; 8];
            for x in a.iter_mut() {
                *x = r.u();
            }
            let o = pick_obj(sim, ok);
            obj_op(sim, o, sub, &a);
        }
        6 => {
            let (pk, sub) = (r.u(), r.u());
            let mut a = [0u32; 8];
            for x in a.iter_mut() {
                *x = r.u();
            }
            let p = (pk as usize % np) as PlayerIndex;
            player_op(sim, p, sub, &a)?;
        }
        _ => sim.rec("ret nop".into()),
    }
    Ok(())
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

/// A real from a short list (so values tie and repeat) or a free one.
fn gen_real(g: &mut Gen) -> f32 {
    const POOL: [f32; 9] = [-0.5, -0.25, 0.0, 0.25, 0.5, 1.0, 1.5, 2.0, 100.0];
    if chance(g, 75) {
        POOL[g.below(POOL.len() as u32) as usize]
    } else {
        g.uniform(-3.0, 3.0)
    }
}

/// A dictionary: a random subset of the properties of a team and of a handicap, with values of the right type.
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
    // the handicap keys (Handicap::readFromDict reads these four)
    for key in ["HANDICAP_BUILDCOST_GENERIC", "HANDICAP_BUILDCOST_BUILDINGS", "HANDICAP_BUILDTIME_GENERIC", "HANDICAP_BUILDTIME_BUILDINGS"] {
        if chance(g, 45) {
            let v = gen_real(g);
            add(2, key, v.to_bits(), "");
        } else if chance(g, 8) {
            add(1, key, g.below(3), ""); // a value of the wrong type reads as "does not exist"
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

fn gen_money(g: &mut Gen, w: &mut W) {
    const AMOUNTS: [u32; 5] = [0, 0, 500, 2000, 10000];
    w.u(AMOUNTS[g.below(5) as usize]);
    w.i(g.below(16) as i32);
}

fn gen_header(g: &mut Gen, w: &mut W) -> (u32, u32, u32) {
    let np = 2 + g.below(3);
    let nt = 2 + g.below(5);
    let nscr = g.below(6);
    let nw = g.below(4);
    // dictionaries
    let nd = 3 + g.below(6);
    w.i(nd as i32);
    for _ in 0..nd {
        gen_dict(g, w, np, nt, nw, nscr);
    }
    // the global data and the game state
    w.b(chance(g, 50));
    w.u(g.below(300));
    for _ in 0..6 {
        let v = *[0.5f32, 1.0, 1.0, 1.25, 1.5, 2.0].get(g.below(6) as usize).unwrap();
        w.f(v);
    }
    gen_money(g, w);
    w.b(chance(g, 30));
    w.b(chance(g, 30));
    gen_money(g, w);
    w.b(chance(g, 30));
    w.b(chance(g, 50));
    w.b(chance(g, 20));
    w.b(chance(g, 75));
    let nr = 2 + g.below(4);
    w.i(1 + g.below(nr + 2) as i32);
    w.b(chance(g, 30));
    w.u(g.below(3));
    for _ in 0..16 {
        w.b(chance(g, 15));
    }
    // the ranks: strictly increasing points
    w.i(nr as i32);
    let mut needed = 0i32;
    for ri in 0..nr {
        w.i(needed);
        w.i(g.below(4) as i32);
        let ng = g.below(3);
        w.i(ng as i32);
        for _ in 0..ng {
            w.i(g.below(8) as i32);
        }
        needed += if (ri == 0 && chance(g, 30)) || chance(g, 10) { 0 } else { 1 + g.below(300) as i32 };
    }
    // the sciences 0..7
    w.i(8);
    for id in 0..8i32 {
        w.i(id);
        w.i(g.below(4) as i32);
        w.b(chance(g, 70));
        let npre = if id == 0 { 0 } else { g.below(3) };
        w.i(npre as i32);
        for _ in 0..npre {
            w.i(g.below(id as u32) as i32);
        }
    }
    // special power templates
    let nsp = 1 + g.below(3);
    w.i(nsp as i32);
    for k in 0..nsp {
        w.u(10 + k * 3 + g.below(3));
        w.u(g.below(2000));
        w.b(chance(g, 40));
    }
    // player templates
    let npt = 1 + g.below(3);
    w.i(npt as i32);
    for _ in 0..npt {
        w.i(g.below(8) as i32);
        w.i(g.below(3) as i32);
        w.i(g.below(3) as i32);
        w.i(g.below(0x100_0000) as i32);
        gen_money(g, w);
        w.u(g.u32());
        for map in 0..3 {
            let n = g.below(4);
            w.i(n as i32);
            for _ in 0..n {
                w.i(g.below(16) as i32);
                if map < 2 {
                    let v = gen_real(g);
                    w.f(v);
                } else {
                    w.u(g.below(4));
                }
            }
        }
        w.b(chance(g, 20));
        w.b(chance(g, 60));
        let ni = g.below(4);
        w.i(ni as i32);
        for _ in 0..ni {
            w.i(g.below(8) as i32);
        }
        w.i(g.below(5) as i32);
    }
    // what the AI answers, per player
    w.i(np as i32);
    for _ in 0..np {
        w.b(chance(g, 50));
        w.b(chance(g, 50));
        w.b(chance(g, 50));
        w.b(chance(g, 50));
        w.c(gen_coord(g, 300.0));
        w.b(chance(g, 50));
        w.b(chance(g, 50));
        w.c(gen_coord(g, 300.0));
        w.i(if chance(g, 40) { -1 } else { g.below(np) as i32 });
        w.u(g.below(3));
    }
    // the thing templates
    w.i(nt as i32);
    for _ in 0..nt {
        let nk = g.below(4);
        w.i(nk as i32);
        for _ in 0..nk {
            w.u(g.below(12));
        }
        w.b(chance(g, 25));
        w.i(g.below(3) as i32);
        w.i(g.below(8) as i32);
        let nv = if chance(g, 70) { 1 + g.below(2) } else { 0 };
        w.i(nv as i32);
        for _ in 0..nv {
            w.i(g.below(16) as i32);
        }
        for _ in 0..4 {
            w.i(g.below(150) as i32 - 10);
        }
    }
    // the objects
    let no = 3 + g.below(12);
    w.i(no as i32);
    let mut used = BTreeSet::new();
    for oi in 0..no {
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
        w.f(g.uniform(-3.0, 3.0));
        w.u(g.below(4));
        let nm = g.below(4);
        w.i(nm as i32);
        for k in 0..nm {
            w.b(chance(g, 70));
            w.i((oi * 4 + k + 1) as i32);
            w.i(g.below(8) as i32);
        }
    }
    // scripts and waypoints
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
    w.i(nw as i32);
    for k in 0..nw {
        w.i(g.below(4) as i32);
        w.u(1 + k * 7 + g.below(5));
        w.c(gen_coord(g, 300.0));
    }
    // the frame: often a whole number of seconds, which is when Player::update posts its message
    let frame0 = if chance(g, 15) { 0 } else if chance(g, 60) { 30 * g.below(40) } else { g.below(1000) };
    w.u(frame0);
    // the name keys to look at at the end
    let probes = ["", "Player0", "Player3", "Tpl3", "Tpl9", "teamName", "HANDICAP_BUILDCOST_GENERIC", "HANDICAP_BUILDTIME_BUILDINGS", "TeamOwner", "never-used"];
    w.i(probes.len() as i32);
    for name in probes {
        w.s(name);
    }
    (np, no, frame0)
}

/// A real from the pools of the operations, as bits.
fn gen_bits(g: &mut Gen) -> u32 {
    gen_real(g).to_bits()
}

/// A grid coordinate (ties in distances) or a free one.
fn gen_pos(g: &mut Gen) -> [f32; 3] {
    if chance(g, 50) {
        [(g.below(3) as f32 - 1.0) * 10.0, (g.below(3) as f32 - 1.0) * 10.0, 0.0]
    } else {
        [g.uniform(-300.0, 300.0), g.uniform(-300.0, 300.0), g.uniform(0.0, 20.0)]
    }
}

/// A kind mask: often none, a single kind, or two.
fn gen_mask(g: &mut Gen) -> u32 {
    match g.below(10) {
        0..=3 => 0,
        4..=7 => 1 << g.below(12),
        _ => (1 << g.below(12)) | (1 << g.below(12)),
    }
}

/// The eight words of a `Player` operation.
fn gen_player_args(g: &mut Gen, sub: u32, frame: u32) -> [u32; 8] {
    let mut a = [0u32; 8];
    for x in a.iter_mut() {
        *x = if chance(g, 20) { 0 } else { g.below(6) };
    }
    let bits3 = |g: &mut Gen, a: &mut [u32; 8], at: usize| {
        let p = gen_pos(g);
        for (k, v) in p.iter().enumerate() {
            a[at + k] = v.to_bits();
        }
    };
    match sub {
        10 => {
            a[1] = g.below(200);
            a[2] = g.below(16);
            a[3] = g.below(4);
        }
        12 => {
            a[0] = g.below(16);
            bits3(g, &mut a, 1);
            a[4] = gen_bits(g);
        }
        14 => a[0] = (g.below(101) as i32 - 50) as u32,
        15 => a[0] = match g.below(4) {
            0 => frame,
            1 => frame + 1,
            2 => frame.saturating_sub(1),
            _ => g.below(2000),
        },
        17 => {
            a[1] = gen_bits(g);
            bits3(g, &mut a, 2);
            a[5] = g.below(4);
        }
        21 => bits3(g, &mut a, 0),
        22 => a[0] = g.below(200),
        61 => {
            a[0] = g.below(8);
            bits3(g, &mut a, 1);
        }
        60 => a[1] = gen_bits(g),
        69 => {
            a[2] = g.below(3000);
        }
        70 => {
            a[1] = gen_mask(g);
            a[2] = [0.25f32, -0.5, 0.25, 1.0][g.below(4) as usize].to_bits();
        }
        35 => {
            a[0] = g.below(5);
            for x in a.iter_mut().skip(1).take(4) {
                *x = if chance(g, 20) { 0 } else { g.below(8) };
            }
        }
        37 => {
            a[0] = gen_mask(g);
            a[1] = gen_mask(g);
        }
        38 => {
            a[1] = gen_mask(g);
            a[2] = gen_mask(g);
        }
        40 => a[0] = gen_mask(g),
        48 | 50 | 55 | 56 | 57 | 58 | 59 => {
            a[0] = g.below(16);
            a[1] = g.below(16);
        }
        74 => {
            a[0] = g.below(10);
            a[1] = if chance(g, 70) { g.below(4) } else { g.below(8) };
            a[2] = if a[0] == 8 { (g.below(13) as i32 - 6) as u32 } else { g.below(8) };
        }
        75 => {
            a[0] = g.below(5);
            a[1] = match a[0] {
                0 => g.below(8),
                1 => (g.below(700) as i32 - 100) as u32,
                _ => g.below(16),
            };
        }
        _ => {}
    }
    a
}

/// The operations that make the world a Player has something to do in: a team of the player, objects on it.
fn gen_setup(g: &mut Gen, w: &mut W, n: &mut u32, np: u32) {
    for k in 0..np {
        if chance(g, 70) {
            // a team for a player, named for its default team
            w.u(6);
            w.u(k);
            w.u(4);
            w.u(k);
            for _ in 0..7 {
                w.u(0);
            }
            *n += 1;
            w.u(1);
            w.u(100 + k);
            w.u(k);
            w.b(chance(g, 30));
            w.u(g.u32());
            *n += 1;
            w.u(2);
            w.u(100 + k);
            *n += 1;
        }
    }
    for k in 0..(1 + g.below(3)) {
        w.u(1);
        w.u(k);
        w.u(g.below(np + 1));
        w.b(chance(g, 20));
        w.u(g.u32());
        *n += 1;
        w.u(2);
        w.u(k);
        *n += 1;
    }
}

/// What the generator remembers of the program so far.
struct GenState {
    frame: u32,
    /// The last `Player` operation: the player, the sub and the words (a repeat with some words re-rolled makes pairs of
    /// operations on the same science, kind mask or team).
    last: Option<(u32, u32, [u32; 8])>,
}

fn gen_one(g: &mut Gen, w: &mut W, n: &mut u32, st: &mut GenState) {
    let r = g.below(100);
    *n += 1;
    match r {
        0..=5 => {
            w.u(0);
            let v = [1u32, 29, 30, 31, 60, 300][g.below(6) as usize];
            st.frame = st.frame.wrapping_add(v);
            w.u(v);
        }
        6..=9 => {
            w.u(1);
            w.u(g.below(4));
            w.u(g.below(5));
            w.b(chance(g, 30));
            w.u(g.u32());
        }
        10..=13 => {
            w.u(2);
            w.u(if chance(g, 30) { 100 + g.below(4) } else { g.below(4) });
        }
        14..=15 => {
            w.u(3);
            w.u(g.below(4));
        }
        16..=26 => {
            w.u(4);
            w.u(g.below(32));
            w.u(if chance(g, 15) { 0 } else { 1 + g.below(8) });
        }
        27..=34 => {
            w.u(5);
            w.u(g.below(32));
            let sub = g.below(9);
            w.u(sub);
            let mut a = [0u32; 8];
            for x in a.iter_mut() {
                *x = g.below(2);
            }
            match sub {
                4 => {
                    let p = gen_pos(g);
                    a[0] = p[0].to_bits();
                    a[1] = p[1].to_bits();
                    a[2] = p[2].to_bits();
                }
                5 => a[0] = [0u32, 1, 2, 4, 8, 15][g.below(6) as usize],
                6 => a[0] = g.uniform(-3.0, 3.0).to_bits(),
                7 => a[0] = g.below(4),
                _ => {}
            }
            for x in a {
                w.u(x);
            }
        }
        _ => {
            w.u(6);
            let (pk, sub, a) = if let (true, Some((pk, sub, last))) = (chance(g, 25), st.last) {
                let fresh = gen_player_args(g, sub, st.frame);
                let mut a = last;
                for k in 0..8 {
                    if chance(g, 40) {
                        a[k] = fresh[k];
                    }
                }
                (pk, sub, a)
            } else {
                let pk = g.below(8);
                // the subs: all, with a bias to the loops over objects and the sciences
                let sub = if chance(g, 30) { [0u32, 33, 34, 35, 36, 37, 38, 44, 68, 74, 75, 3, 69, 69, 62, 63, 64, 65, 70, 50, 14, 13, 2, 5, 6, 8][g.below(26) as usize] } else { g.below(77) };
                let a = gen_player_args(g, sub, st.frame);
                (pk, sub, a)
            };
            st.last = Some((pk, sub, a));
            w.u(pk);
            w.u(sub);
            for x in a {
                w.u(x);
            }
        }
    }
}

fn gen_ops(g: &mut Gen, w: &mut W, np: u32, frame0: u32) {
    let mut ops = W::default();
    let mut n = 0u32;
    gen_setup(g, &mut ops, &mut n, np);
    let mut st = GenState { frame: frame0, last: None };
    let target = n + 25 + g.below(60);
    while n < target {
        gen_one(g, &mut ops, &mut n, &mut st);
    }
    w.i(n as i32);
    w.0.extend(ops.0);
}

fn gen_program(seed: u64) -> Vec<u32> {
    let mut g = Gen::new(seed);
    let mut w = W::default();
    let (np, _no, frame0) = gen_header(&mut g, &mut w);
    gen_ops(&mut g, &mut w, np, frame0);
    w.0
}

// ---------------------------------------------------------------------------------------------------------------------
// The comparison
// ---------------------------------------------------------------------------------------------------------------------

fn cpp_run(words: &[u32]) -> Vec<String> {
    let mut cap = 1 << 20;
    loop {
        let mut buf = vec![0u8; cap];
        let n = unsafe { cpp::ref_player_run(words.as_ptr(), words.len() as i32, buf.as_mut_ptr() as *mut std::os::raw::c_char, cap as i32) } as usize;
        if n <= cap {
            buf.truncate(n);
            return String::from_utf8_lossy(&buf).lines().map(str::to_owned).collect();
        }
        cap = n + 1;
    }
}

fn cases() -> u64 {
    std::env::var("PLAYER_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(1000)
}

#[test]
fn player_matches_upstream() {
    let _guard = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("player, trace lines");
    let (mut programs, mut excluded, mut lines) = (0u64, 0u64, 0u64);
    let mut why_counts: BTreeMap<&str, u64> = BTreeMap::new();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let markers = [
        "ret rel ", "ret rmprel ", "ret rmtrel ", "ret skirmish", "ret super ", "ret enemy", "ret diff", "ret bridges", "ret base ", "ret safe", "ret attacked ", "ret count ", "ret buildings",
        "ret objects", "ret closest", "ret anybuildings", "ret anyunits", "ret anyobjects", "ret buildfacility", "ret local", "ret prod ", "ret allowed", "ret zone ", "ret radar ", "ret radarok",
        "ret spready", "ret kopc", "ret attackedby", "ret status", "ret hasscience", "ret purchase", "ret grant", "ret capable", "ret prereqs", "ret avail", "ret setrank", "ret addskill",
        "ret killskill", "audio ", "ui.dirty", "ui.spp", "ui.rank", "eva ", "msg.append", "script.science", "script.objchange", "sp.created", "sp.ready", "disable ", "enable ", "script-status",
        "ai.new", "ai.delete", "ai.update", "ai.buildteam", "score.", "academy.", "setteam ", "kill ", "damage ", "heal ", "iter ", "skip", "threw badarg", "D team ",
    ];
    let from: u64 = std::env::var("PLAYER_FROM").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    for case in from..from + cases() {
        let words = gen_program(0x91a7_0000 + case);
        programs += 1;
        let rust = match run_rust(&words) {
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
    for (why, c) in &why_counts {
        println!("RESULT   excluded: {why}: {c}");
    }
    for (m, c) in &counts {
        println!("RESULT   {m:<24} {c}");
    }
    check.finish();
}
