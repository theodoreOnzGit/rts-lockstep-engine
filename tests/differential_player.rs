// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::world::player`, `player_ops`, `player_objects`, `player_squads` and `player_xfer`
//! (`Player`: slice 1, construction and `init`, relationships, the build list, the team lists and the loops over a player's
//! objects, the AI forwarders, radar and power, special power timers, production changes, sciences and ranks; slice 2, what
//! works through objects, battle plans, vision, bounty, what can be built; slice 2b, the upgrade list and the hotkey squads
//! with `Squad.cpp`; slice 3, `Player::crc`, `xfer` and `loadPostProcess`, with `BuildListInfo::xfer`) against upstream's own
//! `Player.h` and the `Player.cpp` functions of those slices (commit 0a05454d). Phase 1 layer 7, p1w10-sonnet.
//!
//! # What is compared against what
//!
//! `build.rs` takes `Player.h`, `Team.h`, `Squad.h` and every other header unchanged; the functions of the slices from
//! `Player.cpp` (and those of `Squad.cpp`, `Upgrade.cpp`, `SidesList.cpp`, `Energy.cpp`, `Money.cpp`, `Handicap.cpp`,
//! `MissionStats.cpp`, `Science.cpp`, `ResourceGatheringManager.cpp`, `TunnelTracker.cpp` and `PlayerList.cpp` they call), each
//! whole and unchanged, into `player_fns.inc`; `Team.cpp` with only its include block replaced (as the Team differential does)
//! -- so the real teams, prototypes and DLINK member lists are the world the player's loops walk -- and
//! `differential/wrap_player.cpp` compiles them under clang++ on the stand-ins of `differential/shim_player`: the objects (with
//! the special power modules the sciences reach), the AI, the score keeper, the academy statistics, the rank and science
//! stores, the client, the global data and the game state. Money, Handicap, Energy, Squad, the upgrade templates and centre,
//! BuildListInfo (its members and its functions) and the player's own fields are the real classes' text. The **Xfer family**
//! (`Xfer.cpp`, `XferCRC.cpp`, `XferSave.cpp`, `XferLoad.cpp`, `Snapshot.cpp`) is compiled into the same archive, unchanged and
//! renamed, because `Player::xfer` reaches `xferScienceVec` and `xferUpgradeMask`, which look the world of the fixture up. Two
//! mechanical edits, both in `build.rs`, both asserted to apply once: the include block of `Team.cpp`, and `BitFlags::xfer`'s
//! CRC branch, which hashes `sizeof( this )` bytes -- 4 in the shipped Win32 build, 8 in this 64-bit reference
//! (`common/bit_flags.rs`) -- made `4`. The Rust side runs the same program on `Sim`, a [`PlayerHost`] (and [`TeamHost`],
//! [`PlayerObjectHost`], [`PlayerSquadHost`], [`PlayerXferHost`]) that answers from the same configuration and writes the same lines.
//!
//! Every call that has an effect (an AI order, a score record, a client event, a script notification, an object disabled, a
//! special power ready) writes a line, formatted alike on both sides; calls that only answer a question write none (their
//! answers show in the `ret` lines and in the state). After every operation both sides write the whole state: the factory's
//! counters, every prototype and team with its members, every object's team and `DLINK` neighbours, and every player's fields
//! (five lines each: the scalars, the lists, the upgrades with their masks and the squads with their caches, the battle plan
//! bonuses, the build list in full).
//!
//! **Stand-ins that write a marker** (the units verified elsewhere): the score keeper (`differential_score_keeper.rs`) saves a
//! version and its two sums, the AI a version; the real `ResourceGatheringManager`/`TunnelTracker` xfers run on lists that are
//! empty in these programs (the player makes neither until `initFromDict`).
//!
//! # Method
//!
//! A **program** is a header (dictionaries, the services -- global data, game state, ranks, sciences (name keys `Sci0`...,
//! as `ScienceStore` makes them), upgrade templates, special power templates, player templates, AI behaviour -- the players,
//! thing templates, objects with their special power modules, scripts, waypoints) and a list of operations: team-world set-up,
//! and every `Player` operation (about 130 cases), each on a player picked among those of the program. Teams and prototypes are
//! named by position among those alive. Save and load: one save is kept; a load puts it into the player named by the operation
//! (often another player than the one saved: the AI, the team prototypes and the default team then differ), after the
//! operation has emptied what a load refuses to find filled.
//!
//! **Inputs for which upstream is undefined** (a `NULL` dereferenced, a deleted default team, a rank level the table does not
//! have where the answer is dereferenced, a loop that never ends) are never given to the C++: the Rust returns an error and the
//! program is counted as excluded.
//!
//! # Results (2026-10-20, upstream `0a05454d`)
//!
//! `cargo test --release --features differential --test differential_player -- --nocapture` with `PLAYER_CASES=3000`:
//! **3,000 programs, 215 excluded as undefined upstream (56 a `NULL` dereferenced, 10 a loop that never ends, 149 a player without
//! a template asked for its beacon), 9,344,499 trace lines compared, 0 mismatches.** The programs contain 1,464 saves (each with
//! its `Player::crc`) and 510 loads that completed (65 more threw `SC_INVALID_DATA`), 336 `addUpgrade` calls, 390 hotkey squad
//! reads, 931 objects told to recheck their upgrade modules, and every one of the 93 operations many times.
//!
//! **A defect found by the comparison** (kept as a finding): `player_deliver_brown_out_notice` handed `Energy`'s notice to the
//! player whose energy changed; upstream's `Energy` calls its **owner**, and `Energy::xfer` reads the owner's index from the file,
//! so after loading one player's save into another the owner is the other player. Corrected in `player_ops.rs`.
//!
//! **Planted bugs.** Each was applied to the Rust alone as a runtime switch (`PLAYER_PLANT=<n>`, all in one build, the working tree
//! restored afterwards) and the run repeated. **303 switches** (77 of slice 1, 77 of slice 2, 38 of the upgrades and squads, 111 of
//! the save and load and `BuildListInfo::xfer`); of the 293 that can be observed, **all are caught within 6,000 programs**: 273 at
//! 2,000, and the 20 that need more (36, 116, 156, 208, 215, 232, 233, 235, 366, 411 and the rest)
//! by 6,000, with 50 to 144 mismatching lines each; plants 9 and 355 end the run in a panic or a
//! crash, and 60 and 76 make the programs run too long to finish under the watchdog (as in slice 1). The smallest counts of
//! mismatching lines at 2,000 programs: plant 116 (`findMostReadyShortcutSpecialPowerOfType` taking a power ready exactly now as
//! not ready): 51 at 6,000; plant 156 (`calcCostToBuild` multiplying its three factors in another order): 50 at 6,000.
//!
//! Rounds of the generator that the plants forced (slice 3): squad operations were too rare to reach the squads' caches, so a
//! squad operation is a fifth of the operations, with directed pairs (a squad queried then rebuilt; a special power made ready
//! exactly now then searched for); build list entries carry every field a save writes (the first dump showed only six of them,
//! which left the load of the rest unobservable); template costs go above 2^24 so that the order of the factors of a cost shows
//! (an `f32` product of three factors rounds differently when the result is not exactly representable).
//!
//! **Mutants that cannot be observed and are not counted** (the switch stays in the code, `PLAYER_PLANT=212, 227, 228, 354, 356,
//! 363, 369, 370`): 212 and 227 need a hotkey squad that holds an object twice, which `processCreateTeamGameMessage` never
//! builds (it removes the object from every squad first); 228 is the declaration 227 needs; 354 and 356 turn an `SC_INVALID_DATA`
//! into a NULL dereference (an excluded program, by design) and need an upgrade or prototype the file names and the world lacks,
//! which these programs never make; 363 removes a `clear()` that `xferScienceVec` repeats; 369 and 370 need a squad table of another
//! size, or an uninitialised one, which `Player::init` always prevents.
//!
//! # Slice 4: `Player::initFromDict` and `PlayerList` (2026-10-21)
//!
//! **Methodology.** `PlayerList.cpp` joins the unchanged functions of `Player.cpp` that `build.rs` extracts (now with
//! `initFromDict`), and the fixture gains stand-ins for `SidesList` (with the skirmish sides and the build lists it hands over)
//! and `ScriptList` (copy, qualify, delete: each writes a line, the real script lists being verified by
//! `differential_sides_scripts.rs`). The operations added: the 15 `PlayerList` operations (`update`, `newMap`,
//! `updateTeamStates`, `validateTeam`, `setLocalPlayer`, `getPlayerFromMask`, `getEachPlayerFromMask`,
//! `getPlayersWithRelationship`, `init`, `reset`, `newGame`, `crc`, `xfer` save and load, `teamAboutToBeDeleted`) and
//! `Player::initFromDict`, on dictionaries that vary every key it reads (name, faction, display name, colours, start money,
//! `playerIsSkirmish`, `playerIsHuman`, `multiplayerIsLocal`, start index, difficulty, the team dictionaries of the side).
//! Some programs start a new game on the world they are given. Directed sequences: a squad filled, then `initFromDict`;
//! a player initialised from a dictionary, saved, and the save loaded into another player that has not been (and back);
//! neutral's default team made before `validateTeam` is asked for a name that is neither team nor player.
//!
//! **Results (2026-10-21, upstream `0a05454d`).** `PLAYER_CASES=3000`: **3,000 programs, 278 excluded as undefined upstream (156 a
//! `NULL` dereferenced, 83 a player without a template asked for its beacon, 14 a deleted default team, 13 a float out of the
//! `Int` range, 7 a deleted prototype, 5 a loop that never ends), 7,827,629 trace lines compared (456,266 of them the defined
//! prefixes of the excluded programs), 0 mismatches.** 1,616 `newGame` calls, 124 skirmish queries, 561 script lists qualified
//! and 397 deleted, 74 teams added to and 66 removed from the sides, 109 `getPlayerFromMask`, 575 `getEachPlayerFromMask`,
//! 6,380 `getPlayersWithRelationship`, 110 player list saves (109 `crc`) and 18 loads.
//!
//! **The number of excluded programs is pinned** (91 of 1,000, 188 of 2,000, 278 of 3,000: the generator is deterministic). It was
//! added because plant 562 (`newGame` skipping every named side) left 0 mismatches: the faulty Rust failed with a NULL
//! dereference where upstream does not, so the program was *excluded* and compared only on its prefix. A fault that turns defined
//! behaviour into an error now shows as more exclusions.
//!
//! **Planted bugs of slice 4** (`PLAYER_PLANT=501..577` and the `359` of slice 3 that became reachable): 77 + 1 switches. **72
//! are caught** by mismatching lines at 2,000 programs (1 to several hundred lines each; the smallest, 359 and 562, one line:
//! 562's is the pinned exclusion count), and **11** (501, 505, 506, 507, 508, 510, 511, 515, 542, 543, 546) end in a **crash of the
//! C++ reference**: the faulty Rust accepts a dictionary (a skirmish flag, a player with no template, a list slot) on which
//! upstream dereferences `NULL`, the program reaches the C++, and the C++ dies -- a detection, but not a line-by-line one.
//! Rounds the plants forced: the directed squad-then-`initFromDict` and save-load-between-players sequences (359), the
//! `validateTeam` names that are neither team nor player together with neutral's default team (551), the exclusion pin (562).
//!
//! **Mutants that cannot be observed** (the switch stays in the code): `PLAYER_PLANT=540, 541` re-create the squads and the
//! selection that `Player::init`, called a few lines earlier in `initFromDict`, has just made empty; `545` skips
//! `becomingLocalPlayer( false )`, which upstream defines as nothing (`if( yes ) { ... }`); `554` skips the `maskToAdjust = 0`
//! of `getEachPlayerFromMask`, but all 16 slots hold a player, so a non-zero mask is always consumed to zero before the loop
//! ends; `555` skips the early return for `allowedRelationships == 0`, whose loop then adds nothing; `360` needs a save with a
//! resource gathering manager and no tunnel tracker (or the reverse), but `initFromDict` makes both together and 359's check
//! on the manager fires first.
#![cfg(feature = "differential")]

mod support;

use std::collections::{BTreeMap, BTreeSet};

use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D};
use rts_lockstep_engine::common::dict::Dict;
use rts_lockstep_engine::common::game_common::{GameDifficulty, PlayerType, Relationship, VeterancyLevel};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::handicap::{Handicap, HandicapType};
use rts_lockstep_engine::common::kind_of::{
    KindOfMaskType, KindOfType, KINDOF_AIRCRAFT, KINDOF_COMMANDCENTER, KINDOF_COUNT, KINDOF_DOZER, KINDOF_HARVESTER, KINDOF_INERT, KINDOF_INFANTRY, KINDOF_MINE, KINDOF_POWERED,
    KINDOF_PROJECTILE, KINDOF_STRUCTURE, KINDOF_TECH_BUILDING, KINDOF_VEHICLE, KINDOF_DISGUISER, KINDOF_FS_POWER, KINDOF_IGNORES_SELECT_ALL,
};
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use rts_lockstep_engine::common::object_status_types::{
    ObjectStatusTypes, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_SOLD, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use rts_lockstep_engine::common::science::ScienceType;
use rts_lockstep_engine::common::special_power_type::*;
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::common::upgrade::{UpgradeCenter, UpgradeMaskType, UPGRADE_STATUS_COMPLETE, UPGRADE_STATUS_IN_PRODUCTION, UPGRADE_STATUS_INVALID};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::common::well_known_keys::WellKnownKeys;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::{DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use rts_lockstep_engine::logic::sides_list::BuildListInfo;
use rts_lockstep_engine::logic::world::ids::{PlayerIndex, TeamId, TeamPrototypeId};
use rts_lockstep_engine::logic::world::player::*;
use rts_lockstep_engine::logic::world::player_objects::*;
use rts_lockstep_engine::logic::world::player_ops::*;
use rts_lockstep_engine::logic::world::player_squads::*;
use rts_lockstep_engine::logic::world::player_init::*;
use rts_lockstep_engine::logic::world::player_list::*;
use rts_lockstep_engine::logic::world::player_xfer::*;
use rts_lockstep_engine::logic::world::team::{ScriptRef, TeamArena, TeamError, WaypointId, WaypointInfo};
use rts_lockstep_engine::logic::world::team_ops::*;
use support::{Check, Gen};

/// The C++ side has process-global state (the name keys, the factory, the stand-in world), so the tests take turns.
/// The sub of the last `Player` operation, for `PLAYER_WHY`.
static LAST_SUB: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
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

const KIND_POOL: [KindOfType; 15] = [
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
    KINDOF_DISGUISER,
    KINDOF_FS_POWER,
    KINDOF_IGNORES_SELECT_ALL,
];
const STATUS_POOL: [ObjectStatusTypes; 5] = [OBJECT_STATUS_STEALTHED, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_SOLD];

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
    // --- p1w10-sonnet --- (slice 2)
    cost: i32,
    buildable: u8,
    n_prereq: i32,
    prereq_mask: u32,
    max_simul: u32,
    link_key: NameKeyType,
    owning_side: u32,
    // --- end p1w10-sonnet ---
}

/// A special power module of an object.
#[derive(Clone)]
struct ModSim {
    has_sp: bool,
    serial: i32,
    required: i32,
    // --- p1w10-sonnet --- (slice 2)
    ty: i32,
    script_only: bool,
    ready: u32,
    percent: u32,
    shared: bool,
    // --- end p1w10-sonnet ---
}

/// The special powers a program can name (index -> the enumerator); 8 is `SPECIAL_INVALID`.
const SP_POOL: [i32; 9] = [
    SPECIAL_PARTICLE_UPLINK_CANNON,
    SUPW_SPECIAL_PARTICLE_UPLINK_CANNON,
    LAZR_SPECIAL_PARTICLE_UPLINK_CANNON,
    SPECIAL_NEUTRON_MISSILE,
    NUKE_SPECIAL_NEUTRON_MISSILE,
    SUPW_SPECIAL_NEUTRON_MISSILE,
    SPECIAL_SCUD_STORM,
    SPECIAL_DAISY_CUTTER,
    SPECIAL_INVALID,
];

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
    modules: Vec<ModSim>,
    team: Option<TeamId>,
    // --- p1w10-sonnet --- (slice 2)
    modules_ready: bool,
    has_auto_deposit: bool,
    has_drawable: bool,
    disabled: bool,
    faction_structure: bool,
    has_weapon: bool,
    weapon_percent: u32,
    selectable: bool,
    supply_truck: bool,
    disguised: i32,
    night_color: u32,
    indicator_color: u32,
    producer: i32,
    health: f32,
    shroud: f32,
    who_entered: u32,
    can_enter: u32,
    prod_queue: i32,
    wb: u32,
    // --- end p1w10-sonnet ---
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
/// One entry of `TheSidesList`'s side arrays.
struct SideSim {
    dict: Dict,
    scripts: Option<u32>,
    build: Vec<BuildListInfo>,
}

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
    // --- p1w10-sonnet --- (slice 2)
    observer_template: Option<usize>,
    spent: [i32; 16],
    earned: [i32; 16],
    /// The name keys of the programs' sciences 0..7.
    sci_keys: [i32; 8],
    /// `TheUpgradeCenter` and the name keys of its templates, in the order the program made them.
    upgrades: UpgradeCenter,
    upgrade_keys: Vec<NameKeyType>,
    /// The three AI groups' ids (`g_groups`), the messages the squad operations built (their object id arguments).
    groups: Vec<Vec<ObjectId>>,
    msgs: Vec<Vec<ObjectId>>,
    /// `TheSidesList`: the sixteen sides (a side per player index), the skirmish sides, the teams and the skirmish teams.
    sides: Vec<SideSim>,
    skirmish_sides: Vec<SideSim>,
    side_teams: Vec<Dict>,
    skirmish_teams: Vec<Dict>,
    sdicts: Vec<Dict>,
    next_script: u32,
    /// The last save of the whole player list.
    saved_list: Vec<u8>,
    have_list: bool,
    /// The difficulty an AI was given by `setAIDifficulty`, until it is made again.
    ai_diff: [Option<u32>; 16],
    /// `m_academyStats.m_controlGroupsUsed` of each player.
    groups_used: [u32; 16],
    /// The last save of a player, and whether there is one.
    saved: Vec<u8>,
    have_save: bool,
    // --- end p1w10-sonnet ---
    /// Upstream is undefined for something the program did (a deleted default team, a template-less player asked for its
    /// beacon): the program is excluded.
    undefined: std::cell::Cell<Option<&'static str>>,
}

fn tid(t: Option<TeamId>) -> u32 {
    t.unwrap_or(0)
}

impl Sim {
    /// The players the operations address: those of the program that the list still counts (a new game may leave fewer).
    fn np_now(&self) -> usize {
        self.np.min(self.players.player_count().max(1) as usize)
    }
    /// A science of a program (0..7, or -1 for none) as the name key it stands for.
    fn sci_key(&self, i: i32) -> i32 {
        if (0..8).contains(&i) {
            self.sci_keys[i as usize]
        } else {
            i
        }
    }
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
                4 => d.set_unicode_string(k, &e.s.iter().map(|b| u16::from(*b)).collect::<Vec<u16>>()).unwrap(),
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
        // the program's players by their fixed names first, then as the real function does (see shim_player/player_driver.h)
        if let Some(i) = self.player_by_key.get(&key.0).filter(|i| (**i as i32) < self.players.player_count()) {
            return Some(*i as PlayerIndex);
        }
        self.players.find_with_name_key(key.0 as i32).map(|p| p.player_index)
    }
    fn player_list_neutral(&self) -> Option<PlayerIndex> {
        Some(0)
    }
    fn player_list_count(&self) -> i32 {
        self.players.player_count()
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
        let _ = player_list_team_about_to_be_deleted(self, team);
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
            ClientEvent::Player(PlayerClientEvent::Eva(m)) => format!(
                "eva {}",
                match m {
                    EvaMessage::GeneralLevelUp => 1,
                    EvaMessage::SuperweaponDetectedOwnParticleCannon => 2,
                    EvaMessage::SuperweaponDetectedAllyParticleCannon => 3,
                    EvaMessage::SuperweaponDetectedEnemyParticleCannon => 4,
                    EvaMessage::SuperweaponDetectedOwnNuke => 5,
                    EvaMessage::SuperweaponDetectedAllyNuke => 6,
                    EvaMessage::SuperweaponDetectedEnemyNuke => 7,
                    EvaMessage::SuperweaponDetectedOwnScudStorm => 8,
                    EvaMessage::SuperweaponDetectedAllyScudStorm => 9,
                    EvaMessage::SuperweaponDetectedEnemyScudStorm => 10,
                }
            ),
            // --- p1w10-sonnet --- (slice 2)
            ClientEvent::MoneySound { sound, player_index } => format!("audio {} player={player_index}", if sound == rts_lockstep_engine::logic::client_event::MoneySound::Deposit { 1 } else { 2 }),
            ClientEvent::Player(PlayerClientEvent::ControlBarSchemeByPlayer { player_index }) => format!("ui.scheme.player {player_index}"),
            ClientEvent::Player(PlayerClientEvent::ControlBarSchemeByPlayerTemplate { template }) => format!("ui.scheme.template {}", template.map_or(-1, |t| t as i32)),
            ClientEvent::Player(PlayerClientEvent::SetTeamColor { red, green, blue }) => format!("teamcolor {red} {green} {blue}"),
            ClientEvent::Player(PlayerClientEvent::RadarRemoveObject { object }) => format!("radar- {object}"),
            ClientEvent::Player(PlayerClientEvent::RadarAddObject { object }) => format!("radar+ {object}"),
            ClientEvent::Player(PlayerClientEvent::IndicatorColor { object, color }) => format!("indicator {object} {color:08x}"),
            ClientEvent::Player(PlayerClientEvent::FloatingText { amount, pos, color }) => {
                // `TheGameText->fetch("GUI:AddCash")` comes first, then the text is formatted with the amount
                self.rec("text GUI:AddCash".to_string());
                format!("ui.float +${amount} {} {color:08x}", coord_str(&pos))
            }
            // --- end p1w10-sonnet ---
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
        self.groups_used[p as usize] = 0;
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
        self.ai_diff[p as usize] = None;
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
        match self.ai_diff[p as usize].unwrap_or(self.ai_cfg_of(p).diff) {
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
        self.obj(o).modules.iter().filter(|m| m.has_sp).map(|m| m.serial as u32).collect()
    }
    fn sp_required_science(&self, m: SpecialPowerModuleRef) -> ScienceType {
        for o in &self.objs {
            for md in &o.modules {
                if md.has_sp && md.serial as u32 == m {
                    return ScienceType(md.required);
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

// --- p1w10-sonnet --- (slice 2)
impl Sim {
    /// The special power module the opaque reference (its serial) names.
    fn module(&self, m: SpecialPowerModuleRef) -> &ModSim {
        self.objs.iter().flat_map(|o| o.modules.iter()).find(|x| x.has_sp && x.serial as u32 == m).expect("module")
    }
}


impl PlayerObjectHost for Sim {
    fn obj_friend_adjust_power_for_player(&mut self, o: ObjectId, yes: bool) {
        self.rec(format!("adjustpower {o} {}", yes as u32));
    }
    fn obj_has_auto_deposit_update(&self, o: ObjectId, _key: NameKeyType) -> bool {
        self.obj(o).has_auto_deposit
    }
    fn auto_deposit_award_initial_capture_bonus(&mut self, o: ObjectId, p: PlayerIndex) {
        self.rec(format!("capturebonus {o} {p}"));
    }
    fn obj_modules_ready(&self, o: ObjectId) -> bool {
        self.obj(o).modules_ready
    }
    fn ui_add_idle_worker(&mut self, o: ObjectId) {
        self.rec(format!("ui.idle+ {o}"));
    }
    fn ui_remove_idle_worker(&mut self, o: ObjectId, player: PlayerIndex) {
        self.rec(format!("ui.idle- {o} {player}"));
    }
    fn tmpl_default_owning_side(&self, t: TemplateRef) -> Vec<u8> {
        format!("Side{}", self.tpls[t as usize].owning_side).into_bytes()
    }
    fn obj_has_drawable(&self, o: ObjectId) -> bool {
        self.obj(o).has_drawable
    }
    fn obj_stealth_disguised_player_index(&self, o: ObjectId) -> Option<PlayerIndex> {
        let d = self.obj(o).disguised;
        if d >= 0 {
            Some(d)
        } else {
            None
        }
    }
    fn obj_night_indicator_color(&self, o: ObjectId) -> u32 {
        self.obj(o).night_color
    }
    fn obj_indicator_color(&self, o: ObjectId) -> u32 {
        self.obj(o).indicator_color
    }
    fn obj_is_disabled(&self, o: ObjectId) -> bool {
        self.obj(o).disabled
    }
    fn obj_has_any_special_power(&self, o: ObjectId) -> bool {
        self.obj(o).modules.iter().any(|m| m.has_sp)
    }
    fn obj_find_any_shortcut_special_power(&self, o: ObjectId) -> Option<SpecialPowerModuleRef> {
        self.obj(o).modules.iter().find(|m| m.has_sp).map(|m| m.serial as u32)
    }
    fn obj_has_special_power(&self, o: ObjectId, t: SpecialPowerType) -> bool {
        self.obj(o).modules.iter().any(|m| m.has_sp && m.ty == t)
    }
    fn obj_find_special_power(&self, o: ObjectId, t: SpecialPowerType) -> Option<SpecialPowerModuleRef> {
        self.obj(o).modules.iter().find(|m| m.has_sp && m.ty == t).map(|m| m.serial as u32)
    }
    fn sp_is_script_only(&self, m: SpecialPowerModuleRef) -> bool {
        self.module(m).script_only
    }
    fn sp_ready_frame(&self, m: SpecialPowerModuleRef) -> u32 {
        self.module(m).ready
    }
    fn sp_percent_ready(&self, m: SpecialPowerModuleRef) -> u32 {
        self.module(m).percent
    }
    fn sp_is_shared_n_sync(&self, m: SpecialPowerModuleRef) -> bool {
        self.module(m).shared
    }
    fn obj_has_any_weapon(&self, o: ObjectId) -> bool {
        self.obj(o).has_weapon
    }
    fn obj_most_percent_ready_to_fire_any_weapon(&self, o: ObjectId) -> u32 {
        self.obj(o).weapon_percent
    }
    fn obj_leave_group(&mut self, o: ObjectId) {
        self.rec(format!("leavegroup {o}"));
    }
    fn obj_is_any_kind_of(&self, o: ObjectId, mask: &KindOfMaskType) -> bool {
        self.tpl_of(o).kinds.any_intersection_with(mask)
    }
    fn ai_hunt(&mut self, o: ObjectId, source: CommandSource) {
        self.rec(format!("ai.hunt {o} {source}"));
    }
    fn ai_idle(&mut self, o: ObjectId, source: CommandSource) {
        self.rec(format!("ai.idle {o} {source}"));
    }
    fn ai_enter(&mut self, o: ObjectId, building: ObjectId, source: CommandSource) {
        self.rec(format!("ai.enter {o} {building} {source}"));
    }
    fn ai_evacuate(&mut self, o: ObjectId, expose_stealth_units: bool, source: CommandSource) {
        self.rec(format!("ai.evacuate {o} {} {source}", expose_stealth_units as u32));
    }
    fn ai_move_to_position(&mut self, o: ObjectId, pos: Coord3D, source: CommandSource) {
        self.rec(format!("ai.move {o} {} {source}", coord_str(&pos)));
    }
    fn ai_has_supply_truck_interface(&self, o: ObjectId) -> bool {
        self.obj(o).supply_truck
    }
    fn ai_supply_truck_set_force_wanting_state(&mut self, o: ObjectId, v: bool) {
        self.rec(format!("ai.force {o} {}", v as u32));
    }
    fn pathfinder_remove_object(&mut self, o: ObjectId) {
        self.rec(format!("path.remove {o}"));
    }
    fn pathfinder_add_object(&mut self, o: ObjectId) {
        self.rec(format!("path.add {o}"));
    }
    fn partition_get_most_valuable_location(&mut self, p: PlayerIndex) {
        self.rec(format!("partition.valuable {p}"));
    }
    fn partition_structures_near_to_far(&mut self, pos: Coord3D) -> Vec<ObjectId> {
        let mut v: Vec<(f32, ObjectId)> = self
            .objs
            .iter()
            .filter(|o| self.tpls[o.tpl].kinds.test(KINDOF_STRUCTURE))
            .map(|o| {
                let (dx, dy, dz) = (o.pos.x - pos.x, o.pos.y - pos.y, o.pos.z - pos.z);
                (dx * dx + dy * dy + dz * dz, o.id)
            })
            .collect();
        v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        v.into_iter().map(|x| x.1).collect()
    }
    fn partition_all_objects(&self) -> Vec<ObjectId> {
        self.objs.iter().map(|o| o.id).collect()
    }
    fn contain_player_who_entered(&self, building: ObjectId) -> Option<u32> {
        if self.obj(building).contain_count >= 0 {
            Some(self.obj(building).who_entered)
        } else {
            None
        }
    }
    fn contain_recalc_apparent_controlling_player(&mut self, o: ObjectId) {
        self.rec(format!("recalc {o}"));
    }
    fn obj_has_contain(&self, o: ObjectId) -> bool {
        self.obj(o).contain_count >= 0
    }
    fn action_can_enter_object(&mut self, o: ObjectId, building: ObjectId, source: CommandSource) -> bool {
        self.rec(format!("canenter {o} {building} {source}"));
        (self.obj(o).can_enter >> (building & 31)) & 1 != 0
    }
    fn build_assistant_sell_object(&mut self, o: ObjectId) {
        self.rec(format!("sell {o}"));
    }
    fn obj_is_faction_structure(&self, o: ObjectId) -> bool {
        self.obj(o).faction_structure
    }
    fn tmpl_build_cost(&self, t: TemplateRef) -> i32 {
        self.tpls[t as usize].cost
    }
    fn tmpl_kind_of_mask(&self, t: TemplateRef) -> KindOfMaskType {
        self.tpls[t as usize].kinds
    }
    fn obj_producer_id(&self, o: ObjectId) -> ObjectId {
        self.obj(o).producer
    }
    fn obj_body_max_health(&self, o: ObjectId) -> f32 {
        self.obj(o).health
    }
    fn obj_body_set_max_health(&mut self, o: ObjectId, health: f32) {
        self.rec(format!("health {o} {}", fb(health)));
        self.obj_mut(o).health = health;
    }
    fn obj_body_apply_damage_scalar(&mut self, o: ObjectId, scalar: f32) {
        self.rec(format!("armor {o} {}", fb(scalar)));
    }
    fn obj_set_weapon_bonus_condition(&mut self, o: ObjectId, c: i32) {
        self.rec(format!("wb+ {o} {c}"));
        self.obj_mut(o).wb |= 1u32 << c;
    }
    fn obj_clear_weapon_bonus_condition(&mut self, o: ObjectId, c: i32) {
        self.rec(format!("wb- {o} {c}"));
        self.obj_mut(o).wb &= !(1u32 << c);
    }
    fn obj_shroud_clearing_range(&self, o: ObjectId) -> f32 {
        self.obj(o).shroud
    }
    fn obj_set_vision_range(&mut self, o: ObjectId, range: f32) {
        self.rec(format!("vision {o} {}", fb(range)));
        self.obj_mut(o).vision = range;
    }
    fn obj_set_shroud_clearing_range(&mut self, o: ObjectId, range: f32) {
        self.rec(format!("shroud {o} {}", fb(range)));
        self.obj_mut(o).shroud = range;
    }
    fn obj_set_vision_spied(&mut self, o: ObjectId, setting: bool, by_whom: PlayerIndex) {
        self.rec(format!("spied {o} {} {by_whom}", setting as u32));
    }
    fn tmpl_buildable(&self, t: TemplateRef) -> u8 {
        self.tpls[t as usize].buildable
    }
    fn tmpl_prereq_count(&self, t: TemplateRef) -> i32 {
        self.tpls[t as usize].n_prereq
    }
    fn tmpl_prereq_is_satisfied(&self, t: TemplateRef, i: i32, _p: PlayerIndex) -> bool {
        (self.tpls[t as usize].prereq_mask >> (i & 31)) & 1 != 0
    }
    fn tmpl_max_simultaneous_of_type(&self, t: TemplateRef) -> u32 {
        self.tpls[t as usize].max_simul
    }
    fn tmpl_max_simultaneous_link_key(&self, t: TemplateRef) -> NameKeyType {
        self.tpls[t as usize].link_key
    }
    fn obj_production_queue_count(&self, o: ObjectId, _t: TemplateRef) -> Option<u32> {
        let q = self.obj(o).prod_queue;
        if q >= 0 {
            Some(q as u32)
        } else {
            None
        }
    }
    fn player_template_find(&self, key: NameKeyType) -> Option<PlayerTemplateRef> {
        // `ThePlayerTemplateStore->findPlayerTemplate(NAMEKEY("FactionObserver"))`: the key was made by the caller
        let name = self.names.key_to_name(key);
        if name == b"FactionObserver" {
            self.observer_template.map(|i| i as PlayerTemplateRef)
        } else if let Some(digits) = name.strip_prefix(b"Faction") {
            // "Faction" and a number written as `to_string` writes it, naming one of the program's player templates
            std::str::from_utf8(digits).ok().and_then(|t| t.parse::<usize>().ok()).filter(|i| i.to_string().as_bytes() == digits && *i < self.ptpls.len()).map(|i| i as PlayerTemplateRef)
        } else {
            None
        }
    }
    fn thing_factory_find_template(&mut self, name: &[u8]) -> Option<TemplateRef> {
        self.tpls.iter().position(|t| t.name == name).map(|i| i as TemplateRef)
    }
    fn find_object_by_id(&self, id: ObjectId) -> Option<ObjectId> {
        if self.objs.iter().any(|o| o.id == id) {
            Some(id)
        } else {
            None
        }
    }
    fn score_add_money_spent(&mut self, p: PlayerIndex, amount: i32) {
        self.spent[p as usize] = self.spent[p as usize].wrapping_add(amount);
    }
    fn score_add_money_earned(&mut self, p: PlayerIndex, amount: i32) {
        self.earned[p as usize] = self.earned[p as usize].wrapping_add(amount);
    }
}
// --- end p1w10-sonnet ---

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
    for i in 0..sim.np_now() {
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
                "{}/{}/{},{},{}/{}/{}/{}/{}/{}/{}/{}/{}/{}{}{}{}{}{}{}/{}/{},{}/{}",
                e.object_id,
                dash(e.template_name.clone()),
                fb(e.location.x),
                fb(e.location.y),
                fb(e.location.z),
                fb(e.angle),
                e.num_rebuilds as i32,
                e.priority_build as u32,
                dash(e.building_name.clone()),
                dash(e.script.clone()),
                fb(e.rally_point_offset.x),
                fb(e.rally_point_offset.y),
                e.health,
                e.whiner as u32,
                e.unsellable as u32,
                e.repairable as u32,
                e.automatically_build as u32,
                e.is_initially_built as u32,
                e.under_construction as u32,
                e.is_supply_building as u32,
                e.object_timestamp,
                e.desired_gatherers,
                e.current_gatherers,
                e.resource_gatherers.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(",")
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
    // --- p1w10-sonnet --- (slice 2b and 3): the upgrades, their masks, the squads and their caches
    let mask_str = |m: &UpgradeMaskType| -> String {
        let v: Vec<String> = (0..128).filter(|i| m.test(*i)).map(|i| i.to_string()).collect();
        if v.is_empty() { "-".into() } else { v.join(",") }
    };
    let ups = if p.upgrades.is_empty() {
        "-".to_string()
    } else {
        p.upgrades.iter().map(|u| format!("{}:{}", sim.upgrade_keys.iter().position(|k| *k == u.template).map_or(-1, |i| i as i32), u.status)).collect::<Vec<_>>().join(";")
    };
    let squads = if p.squads.is_empty() { "-".to_string() } else { p.squads.iter().map(|s| format!("{}/{}", squad_str(s), id_list(&s.objects_cached))).collect::<Vec<_>>().join("|") };
    let sel = p.current_selection.as_ref().map_or("-".to_string(), |s| format!("{}/{}", squad_str(s), id_list(&s.objects_cached)));
    out.push(format!("D player4 {} gused={} up={} inprog={} comp={} squads={} sel={}", p.player_index, sim.groups_used[idx], ups, mask_str(&p.upgrades_in_progress), mask_str(&p.upgrades_completed), squads, sel));
    // --- end p1w10-sonnet ---
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
    // --- p1w10-sonnet --- (slice 2)
    for o in &sim.objs {
        out.push(format!("D obj2 {} health={} vision={} shroud={} wb={:08x}", o.id, fb(o.health), fb(o.vision), fb(o.shroud), o.wb));
    }
    // --- end p1w10-sonnet ---
    for i in 0..sim.np_now() {
        dump_player(sim, i, &mut out);
        // --- p1w10-sonnet --- (slice 2)
        let p = sim.players.get_nth(i as i32).unwrap();
        let bonus = match &p.battle_plan_bonuses {
            Some(b) => format!(
                "{},{},{},{},{},{},{}",
                fb(b.armor_scalar),
                fb(b.sight_range_scalar),
                b.bombardment,
                b.hold_the_line,
                b.search_and_destroy,
                kind_bits(&b.valid_kind_of),
                kind_bits(&b.invalid_kind_of)
            ),
            None => "-".to_string(),
        };
        out.push(format!("D player3 {i} spent={} earned={} bonus={bonus}", sim.spent[i], sim.earned[i]));
        // --- end p1w10-sonnet ---
    }
    // --- p1w10-sonnet --- (slice 4): the sides list
    {
        let scripts = |v: &Vec<SideSim>| v.iter().map(|x| x.scripts.map_or("-".to_string(), |i| i.to_string())).collect::<Vec<_>>().join(",");
        let builds = sim.sides.iter().map(|x| x.build.len().to_string()).collect::<Vec<_>>().join(",");
        let teams = if sim.side_teams.is_empty() {
            "-".to_string()
        } else {
            let kn = sim.names.name_to_key(b"teamName");
            let ko = sim.names.name_to_key(b"teamOwner");
            sim.side_teams
                .iter()
                .map(|d| format!("{}@{}", String::from_utf8_lossy(&d.get_ascii_string(kn).0), String::from_utf8_lossy(&d.get_ascii_string(ko).0)))
                .collect::<Vec<_>>()
                .join(",")
        };
        out.push(format!("D sides scripts={} sk={} builds={} teams={} count={}", scripts(&sim.sides), scripts(&sim.skirmish_sides), builds, teams, sim.players.player_count()));
    }
    // --- end p1w10-sonnet ---
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
        PlayerError::InvalidData => Ok("threw invaliddata".to_string()),
        PlayerError::Xfer(s) => Ok(format!("threw xfer {}", s as i32)),
        PlayerError::Null => Err(Excluded("a NULL or dangling pointer dereferenced")),
        PlayerError::NameKeysExhausted => Err(Excluded("the name keys ran out")),
        PlayerError::OutOfRange => Err(Excluded("a float converted to an int out of range")),
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
            if ty == 3 || ty == 4 {
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
    // the eight sciences of a program are name keys ("Sci0" ... "Sci7"), as ScienceStore makes them: Player::xfer saves a
    // science as its name, and the key is the only thing the name is looked up by
    let mut sci_keys = [0i32; 8];
    for (i, k) in sci_keys.iter_mut().enumerate() {
        *k = ScienceType::from_name_key(names.name_to_key(format!("Sci{i}").as_bytes())).0;
    }
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
        observer_template: None,
        spent: [0; 16],
        earned: [0; 16],
        sci_keys,
        upgrades: UpgradeCenter::new(),
        upgrade_keys: Vec::new(),
        groups: vec![Vec::new(); 3],
        msgs: Vec::new(),
        saved: Vec::new(),
        have_save: false,
        groups_used: [0; 16],
        sides: Vec::new(),
        skirmish_sides: Vec::new(),
        side_teams: Vec::new(),
        skirmish_teams: Vec::new(),
        sdicts: Vec::new(),
        next_script: 1,
        saved_list: Vec::new(),
        have_list: false,
        ai_diff: [None; 16],
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
        let sciences_granted = (0..ng).map(|_| ScienceType(sim.sci_key(r.i()))).collect();
        sim.ranks.push(RankInfoView { skill_points_needed: needed, science_purchase_points_granted: spp, sciences_granted });
    }
    let nsci = r.i();
    for _ in 0..nsci {
        let id = sim.sci_key(r.i());
        let cost = r.i();
        let grantable = r.b();
        let np2 = r.i();
        let prereqs = (0..np2).map(|_| sim.sci_key(r.i())).collect();
        sim.sciences.insert(id, SciCfg { cost, grantable, prereqs });
    }
    let nup = r.i();
    for i in 0..nup {
        let name = format!("Upg{i}");
        sim.upgrades.new_upgrade(&mut sim.names, name.as_bytes()).expect("new upgrade");
        let key = sim.names.name_to_key(name.as_bytes());
        sim.upgrade_keys.push(key);
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
        let side = match r.i() {
            7 => b"Civilian".to_vec(),
            v => format!("Side{v}").into_bytes(),
        };
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
        let intrinsic_sciences = (0..nc).map(|_| ScienceType(sim.sci_key(r.i()))).collect();
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
            kinds.set(KIND_POOL[(r.u() % 15) as usize], 1);
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
        sim.tpls.push(TplSim {
            kinds,
            build_facility,
            equiv,
            name,
            variations,
            skill,
            cost: 0,
            buildable: 0,
            n_prereq: 0,
            prereq_mask: 0,
            max_simul: 0,
            link_key: NameKeyType::INVALID,
            owning_side: 0,
        });
    }
    let no = r.i();
    for _ in 0..no {
        let id = r.u() as ObjectId;
        let tpl = r.u() as usize % sim.tpls.len();
        let player = r.u() as usize % sim.np;
        let ns = r.i();
        let status = (0..ns).map(|_| STATUS_POOL[(r.u() % 5) as usize]).collect();
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
            modules.push(ModSim { has_sp: r.b(), serial: r.i(), required: sim.sci_key(r.i()), ty: 0, script_only: false, ready: 0, percent: 0, shared: false });
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
            modules_ready: false,
            has_auto_deposit: false,
            has_drawable: false,
            disabled: false,
            faction_structure: false,
            has_weapon: false,
            weapon_percent: 0,
            selectable: false,
            supply_truck: false,
            disguised: -1,
            night_color: 0,
            indicator_color: 0,
            producer: 0,
            health: 0.0,
            shroud: 0.0,
            who_entered: 0,
            can_enter: 0,
            prod_queue: -1,
            wb: 0,
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
    read_header2(r, &mut sim);
    let n_probe = r.i();
    let probes = (0..n_probe).map(|_| r.s()).collect();
    (sim, probes)
}

/// Runs the Rust side of a program, giving the trace lines (as the C++ side writes them), or why it cannot be given
/// to the C++.
/// The second part of the header (slice 2), which follows the frame.
fn read_header2(r: &mut Reader<'_>, sim: &mut Sim) {
    let has_observer = r.b();
    let observer_idx = r.u() as usize;
    sim.observer_template = if has_observer && !sim.ptpls.is_empty() { Some(observer_idx % sim.ptpls.len()) } else { None };
    for t in sim.tpls.iter_mut() {
        t.cost = r.i();
        t.buildable = (r.u() % 4) as u8;
        t.n_prereq = (r.u() % 4) as i32;
        t.prereq_mask = r.u();
        t.max_simul = r.u() % 4;
        let lk = r.i();
        t.link_key = if lk == 0 { NameKeyType::INVALID } else { sim.names.name_to_key(format!("Link{lk}").as_bytes()) };
        t.owning_side = r.u() % 4;
    }
    read_header2_objects(r, sim);
    read_sides(r, sim);
}

fn read_header2_objects(r: &mut Reader<'_>, sim: &mut Sim) {
    for o in sim.objs.iter_mut() {
        o.modules_ready = r.b();
        o.has_auto_deposit = r.b();
        o.has_drawable = r.b();
        o.disabled = r.b();
        o.faction_structure = r.b();
        o.has_weapon = r.b();
        o.weapon_percent = r.u() % 130;
        o.selectable = r.b();
        o.supply_truck = r.b();
        o.disguised = r.i();
        o.night_color = r.u();
        o.indicator_color = r.u();
        o.producer = r.u() as i32;
        o.health = r.f();
        o.shroud = r.f();
        o.who_entered = r.u();
        o.can_enter = r.u();
        o.prod_queue = r.i();
        for m in o.modules.iter_mut() {
            m.ty = SP_POOL[(r.u() % 9) as usize];
            m.script_only = r.b();
            m.ready = r.u();
            m.percent = r.u() % 130;
            m.shared = r.b();
        }
    }
}

/// What of an excluded program is still defined upstream: the operations before the one that made it undefined and the
/// lines the Rust wrote up to there. The C++ is run on those operations alone and its first lines are compared (a program
/// that is undefined at its tenth operation says something about its first nine).
struct Prefix {
    /// The position of the operation count among the program's words.
    pos_n: usize,
    ops: usize,
    lines: Vec<String>,
}

thread_local! {
    static LAST_PREFIX: std::cell::RefCell<Option<Prefix>> = const { std::cell::RefCell::new(None) };
}

fn run_rust(words: &[u32]) -> Result<Vec<String>, Excluded> {
    let mut pre = None;
    let r = run_rust_inner(words, &mut pre);
    LAST_PREFIX.with(|p| *p.borrow_mut() = if r.is_err() { pre } else { None });
    r
}

fn run_rust_inner(words: &[u32], pre: &mut Option<Prefix>) -> Result<Vec<String>, Excluded> {
    let mut r = Reader { w: words, p: 0 };
    let (mut sim, probes) = read_sim(&mut r);
    let pos_n = r.p;
    let n_ops = r.i();
    let mut threw = false;
    dump_state(&mut sim);
    for i in 0..n_ops as usize {
        let ok_lines = sim.lines.len();
        let op = r.u();
        let res = do_op(&mut sim, op, &mut r);
        if let Some(why) = sim.undefined.get() {
            *pre = Some(Prefix { pos_n, ops: i, lines: sim.lines[..ok_lines].to_vec() });
            return Err(Excluded(why));
        }
        match res {
            Ok(()) => dump_state(&mut sim),
            Err(e) => {
                if std::env::var_os("PLAYER_WHY").is_some() {
                    eprintln!("WHY {e:?} at op {op} sub {}", LAST_SUB.load(std::sync::atomic::Ordering::Relaxed));
                }
                let line = match thrown(e) {
                    Ok(line) => line,
                    Err(x) => {
                        *pre = Some(Prefix { pos_n, ops: i, lines: sim.lines[..ok_lines].to_vec() });
                        return Err(x);
                    }
                };
                sim.rec(line);
                // `ERROR_BAD_ARG` is thrown before anything is changed: the program goes on
                if e == PlayerError::Team(TeamError::BadArg) {
                    dump_state(&mut sim);
                    if let Some(why) = sim.undefined.get() {
                        *pre = Some(Prefix { pos_n, ops: i, lines: sim.lines[..ok_lines].to_vec() });
                        return Err(Excluded(why));
                    }
                    continue;
                }
                threw = true;
                break;
            }
        }
        if let Some(why) = sim.undefined.get() {
            *pre = Some(Prefix { pos_n, ops: i, lines: sim.lines[..ok_lines].to_vec() });
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
            let s = STATUS_POOL[(a[0] % 5) as usize];
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
        // --- p1w10-sonnet --- (slice 2)
        9 => sim.obj_mut(o).disabled = a[0] != 0,
        10 => sim.obj_mut(o).modules_ready = a[0] != 0,
        11 => {
            let n = sim.obj(o).modules.len();
            for k in 0..4usize {
                let idx = (k + a[0] as usize) % 4;
                if idx < n {
                    let m = &mut sim.obj_mut(o).modules[idx];
                    m.ready = a[1];
                    m.percent = a[2] % 130;
                    m.script_only = a[3] != 0;
                    break;
                }
            }
        }
        12 => {
            let ob = sim.obj_mut(o);
            ob.has_weapon = a[0] != 0;
            ob.weapon_percent = a[1] % 130;
            ob.faction_structure = a[2] != 0;
        }
        13 => {
            let ob = sim.obj_mut(o);
            ob.prod_queue = (a[0] % 4) as i32 - 1;
            ob.can_enter = a[1];
            ob.who_entered = if !a[2].is_multiple_of(2) { 1u32 << (a[3] % 4) } else { 0 };
        }
        14 => {
            let ob = sim.obj_mut(o);
            ob.health = f32::from_bits(a[0]);
            ob.disguised = (a[1] % 5) as i32 - 1;
            ob.supply_truck = a[2] != 0;
        }
        // --- end p1w10-sonnet ---
        _ => {}
    }
}

fn thing_list(sim: &Sim, a: &[u32; 8]) -> (Vec<Option<TemplateRef>>, Vec<i32>) {
    let n = (a[0] % 5) as usize;
    let things = (0..n).map(|k| if a[1 + k] != 0 { Some(((a[1 + k] - 1) as usize % sim.tpls.len()) as TemplateRef) } else { None }).collect();
    (things, vec![7; n])
}

/// One `Player` operation; `a` are its eight words.
// --- p1w10-sonnet --- (slice 4)
impl PlayerInitHost for Sim {
    fn sides_num_sides(&self) -> i32 {
        self.sides.len() as i32
    }
    fn sides_side_dict(&self, i: i32) -> Dict {
        self.sides[i as usize].dict.clone()
    }
    fn sides_side_scripts(&self, i: i32) -> Option<ScriptListRef> {
        self.sides[i as usize].scripts
    }
    fn sides_set_side_scripts(&mut self, i: i32, l: Option<ScriptListRef>) {
        self.sides[i as usize].scripts = l;
    }
    fn sides_take_side_build_list(&mut self, i: i32) -> Vec<BuildListInfo> {
        std::mem::take(&mut self.sides[i as usize].build)
    }
    fn sides_num_skirmish_sides(&self) -> i32 {
        self.skirmish_sides.len() as i32
    }
    fn sides_skirmish_side_dict(&self, i: i32) -> Dict {
        self.skirmish_sides[i as usize].dict.clone()
    }
    fn sides_skirmish_side_scripts(&self, i: i32) -> Option<ScriptListRef> {
        self.skirmish_sides[i as usize].scripts
    }
    fn sides_set_skirmish_side_scripts(&mut self, i: i32, l: Option<ScriptListRef>) {
        self.skirmish_sides[i as usize].scripts = l;
    }
    fn sides_num_teams(&self) -> i32 {
        self.side_teams.len() as i32
    }
    fn sides_team_dict(&self, i: i32) -> Dict {
        self.side_teams[i as usize].clone()
    }
    fn sides_remove_team(&mut self, i: i32) {
        self.rec(format!("sides.removeteam {i}"));
        self.side_teams.remove(i as usize);
    }
    fn sides_num_skirmish_teams(&self) -> i32 {
        self.skirmish_teams.len() as i32
    }
    fn sides_skirmish_team_dict(&self, i: i32) -> Dict {
        self.skirmish_teams[i as usize].clone()
    }
    fn sides_find_team_info(&mut self, name: &[u8]) -> bool {
        if self.side_teams.is_empty() {
            return false;
        }
        let k = self.names.name_to_key(b"teamName");
        self.side_teams.iter().any(|d| d.get_ascii_string(k).0 == name)
    }
    fn sides_add_team(&mut self, d: &Dict) {
        let get = |sim: &mut Sim, name: &str| -> String {
            let k = sim.names.name_to_key(name.as_bytes());
            String::from_utf8_lossy(&d.get_ascii_string(k).0).into_owned()
        };
        let mut line = format!("sides.addteam owner={} name={}", get(self, "teamOwner"), get(self, "teamName"));
        for n in ["teamOnCreateScript", "teamOnIdleScript", "teamOnUnitDestroyedScript", "teamOnDestroyedScript", "teamEnemySightedScript", "teamAllClearScript", "teamProductionCondition"] {
            line.push(' ');
            line.push_str(&get(self, n));
        }
        for j in 0..16 {
            line.push(' ');
            self.names.name_to_key(b"teamGenericScriptHook"); // keyToName(TheKey_teamGenericScriptHook)
            line.push_str(&get(self, &format!("teamGenericScriptHook{j}")));
        }
        self.rec(line);
        self.side_teams.push(d.clone());
    }
    fn script_list_duplicate_and_qualify(&mut self, list: ScriptListRef, qualifier: &[u8], qual_template_player_name: &[u8], new_player_name: &[u8]) -> ScriptListRef {
        let id = self.next_script;
        self.next_script += 1;
        self.rec(format!(
            "scripts.qualify {list} -> {id} q={} t={} n={}",
            String::from_utf8_lossy(qualifier),
            String::from_utf8_lossy(qual_template_player_name),
            String::from_utf8_lossy(new_player_name)
        ));
        id
    }
    fn script_list_delete(&mut self, list: ScriptListRef) {
        self.rec(format!("scripts.delete {list}"));
    }
    fn script_engine_global_difficulty(&self) -> GameDifficulty {
        self.globals.global_difficulty
    }
    fn ai_set_difficulty(&mut self, p: PlayerIndex, d: GameDifficulty) {
        self.rec(format!("ai.diff {p} {}", d as i32));
        self.ai_diff[p as usize] = Some(match d {
            GameDifficulty::Easy => 0,
            GameDifficulty::Normal => 1,
            GameDifficulty::Hard => 2,
        });
    }
    fn network_present(&self) -> bool {
        false
    }
    fn team_factory_init_from_sides(&mut self) -> PResult<()> {
        let mut sides = rts_lockstep_engine::logic::sides_list::SidesList::new();
        for d in self.side_teams.clone() {
            sides.add_team(&d);
        }
        factory_init_from_sides(self, &sides)?;
        Ok(())
    }
}

/// A side or a team of the program: a dictionary of the program's list (0 is an empty one).
fn sdict_at(sim: &Sim, w: u32) -> Dict {
    if w == 0 || sim.sdicts.is_empty() {
        Dict::new()
    } else {
        sim.sdicts[(w as usize - 1) % sim.sdicts.len()].clone()
    }
}

/// The part of the header that is `TheSidesList` (dictionaries first, then the sixteen sides, the skirmish sides, the teams).
fn read_sides(r: &mut Reader<'_>, sim: &mut Sim) {
    let nsd = r.i();
    for _ in 0..nsd {
        let ne = r.i();
        let mut entries = Vec::new();
        for _ in 0..ne {
            let ty = r.i();
            let key = r.s();
            let (mut bits, mut s) = (0, Vec::new());
            if ty == 3 || ty == 4 {
                s = r.s();
            } else {
                bits = r.u();
            }
            entries.push(DictEntry { ty, key, bits, s });
        }
        let d = sim.build_dict(&entries);
        sim.sdicts.push(d);
    }
    for i in 0..16 {
        let dict = sdict_at(sim, r.u());
        let scripts = if r.b() {
            let id = sim.next_script;
            sim.next_script += 1;
            Some(id)
        } else {
            None
        };
        let bl = r.i();
        let mut build = Vec::new();
        for k in 0..bl {
            let mut e = BuildListInfo::new();
            e.object_id = 100 * i + k;
            e.template_name = thing_name((i + k) as u32);
            build.push(e);
        }
        sim.sides.push(SideSim { dict, scripts, build });
    }
    let nss = r.i();
    for _ in 0..nss {
        let dict = sdict_at(sim, r.u());
        let scripts = if r.b() {
            let id = sim.next_script;
            sim.next_script += 1;
            Some(id)
        } else {
            None
        };
        sim.skirmish_sides.push(SideSim { dict, scripts, build: Vec::new() });
    }
    let nt = r.i();
    for _ in 0..nt {
        let d = sdict_at(sim, r.u());
        sim.side_teams.push(d);
    }
    let nk = r.i();
    for _ in 0..nk {
        let d = sdict_at(sim, r.u());
        sim.skirmish_teams.push(d);
    }
}

/// One operation on the player list.
fn list_op(sim: &mut Sim, a: &[u32; 8], sub: u32) -> Result<(), PlayerError> {
    let np = sim.np_now();
    match sub % 15 {
        0 => player_list_update(sim)?,
        1 => player_list_new_map(sim)?,
        2 => player_list_update_team_states(sim)?,
        3 => {
            let t = player_list_validate_team(sim, &team_name(a[0]))?;
            sim.rec(format!("ret vteam {}", tid(t)));
        }
        4 => {
            let p = if a[0] != 0 { Some(((a[0] - 1) as usize % np) as PlayerIndex) } else { None };
            player_list_set_local_player(sim, p)?;
        }
        5 => {
            let mask: u16 = if a[0].is_multiple_of(3) { 1u16 << (a[1] % 16) } else { (a[1] & 0xffff) as u16 };
            let p = player_list_get_player_from_mask(sim, mask);
            sim.rec(format!("ret pfm {}", p.unwrap_or(-1)));
        }
        6 => {
            let mut m = (a[0] & 0xffff) as u16;
            // until none is left (a mask with bits of no player ends in the none, which empties it)
            for _ in 0..24 {
                let p = player_list_get_each_player_from_mask(sim, &mut m);
                sim.rec(format!("ret each {:04x} {}", m, p.unwrap_or(-1)));
                if p.is_none() {
                    break;
                }
            }
        }
        7 => {
            let r = player_list_get_players_with_relationship(sim, (a[0] % 16) as i32, a[1] & 0xf)?;
            sim.rec(format!("ret pwr {r:04x}"));
        }
        8 => player_list_init(sim)?,
        9 => player_list_reset(sim)?,
        10 => {
            player_list_new_game(sim)?;
            let c = sim.players.player_count();
            sim.rec(format!("ret newgame {c}"));
        }
        11 => {
            let mut xc = XferCrc::new();
            xc.open(b"l").unwrap();
            player_list_crc(sim, &mut xc)?;
            xc.close().unwrap();
            sim.rec(format!("ret lcrc {:08x}", xc.get_crc()));
        }
        12 => {
            let mut save = XferSave::new();
            save.open(b"l").unwrap();
            let r = xfer_nested(sim, &mut save, |h, x| player_list_xfer(h, x));
            save.close().unwrap();
            r?;
            sim.saved_list = save.take_bytes();
            sim.have_list = true;
            sim.rec(format!("ret lsave len={} hash={:08x}", sim.saved_list.len(), hash_bytes(&sim.saved_list)));
        }
        13 => {
            if !sim.have_list {
                sim.rec("skip".into());
                return Ok(());
            }
            for i in 0..sim.players.player_count() {
                let me = sim.players.get_nth_mut(i).unwrap();
                if a[1] & 1 != 0 {
                    me.kind_of_production_changes.clear();
                    me.special_power_timers.clear();
                }
                if a[1] & 2 != 0 {
                    for sq in me.squads.iter_mut().chain(me.current_selection.iter_mut()) {
                        sq.objects_cached.clear();
                    }
                }
            }
            let mut load = XferLoad::new(sim.saved_list.clone());
            load.open(b"l").unwrap();
            let r = xfer_nested(sim, &mut load, |h, x| player_list_xfer(h, x));
            let _ = load.close();
            r?;
            sim.rec(format!("ret lload pp={}", load.post_process_requests()));
            player_list_load_post_process(sim);
        }
        _ => {
            let t = if a[0] != 0 { pick_team(sim, a[0] - 1) } else { None };
            match t {
                Some(t) => player_list_team_about_to_be_deleted(sim, t)?,
                None => sim.rec("skip".into()),
            }
        }
    }
    Ok(())
}
// --- end p1w10-sonnet ---

// --- p1w10-sonnet --- (slice 2b and 3)
impl PlayerSquadHost for Sim {
    fn upgrade_mask(&self, t: NameKeyType) -> UpgradeMaskType {
        self.upgrades.find_upgrade_by_key(t).map(|u| u.upgrade_mask()).expect("an upgrade of the program")
    }
    fn obj_update_upgrade_modules(&mut self, o: ObjectId) {
        self.rec(format!("upgrademods {o}"));
    }
    fn obj_is_selectable(&self, o: ObjectId) -> bool {
        self.obj(o).selectable
    }
    fn ai_group_all_ids(&self, group: GroupRef) -> Vec<ObjectId> {
        self.groups[group as usize - 1].clone()
    }
    fn message_argument_count(&self, msg: MessageRef) -> u8 {
        self.msgs[msg as usize].len() as u8
    }
    fn message_argument_object_id(&self, msg: MessageRef, i: u8) -> ObjectId {
        self.msgs[msg as usize][i as usize]
    }
    fn academy_record_control_groups_used(&mut self, p: PlayerIndex) {
        self.groups_used[p as usize] += 1; // AcademyStats.h line 107, inline: no line, but `m_controlGroupsUsed` is dumped
    }
}

impl PlayerXferHost for Sim {
    fn upgrade_center_and_names(&mut self) -> (&UpgradeCenter, &mut NameKeyGenerator) {
        (&self.upgrades, &mut self.names)
    }
    /// The score keeper is another unit (differential_score_keeper.rs): the fixture's stand-in saves a version and the two sums.
    fn xfer_score_keeper(&mut self, p: PlayerIndex, xfer: &mut dyn Xfer) -> PResult<()> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)?;
        xfer.xfer_int(&mut self.spent[p as usize])?;
        xfer.xfer_int(&mut self.earned[p as usize])?;
        Ok(())
    }
    /// The AI is another unit: the fixture's stand-in saves a version.
    fn xfer_ai(&mut self, _p: PlayerIndex, xfer: &mut dyn Xfer) -> PResult<()> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)?;
        Ok(())
    }
}

fn hash_bytes(v: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for &b in v {
        h = (h ^ u32::from(b)).wrapping_mul(16777619);
    }
    h
}

/// An object of a program's message or group: a word that picks an object, or (a quarter of the time) an id no object has.
fn obj_id_of(sim: &Sim, w: u32) -> ObjectId {
    if w.is_multiple_of(4) {
        900 + ((w / 4) % 7) as i32
    } else {
        sim.objs[((w / 4) as usize) % sim.objs.len()].id
    }
}

/// An object that exists, picked by a word.
fn real_obj_id(sim: &Sim, w: u32) -> ObjectId {
    sim.objs[(w as usize) % sim.objs.len()].id
}

/// The ids of a squad, `-` for none.
fn squad_str(s: &Squad) -> String {
    id_list(&s.object_ids)
}

/// The upgrade masks of a program: none, one template's, or two.
fn upgrade_mask_of(sim: &Sim, a: &[u32; 8], k: usize) -> UpgradeMaskType {
    let n = sim.upgrade_keys.len();
    let mut m = UpgradeMaskType::default();
    if !a[k].is_multiple_of(3) {
        m.set_mask(&sim.upgrade_mask(sim.upgrade_keys[a[k + 1] as usize % n]));
    }
    if a[k] % 3 == 2 {
        m.set_mask(&sim.upgrade_mask(sim.upgrade_keys[a[k + 2] as usize % n]));
    }
    m
}

fn upgrade_op(sim: &mut Sim, p: PlayerIndex, a: &[u32; 8]) -> Result<(), PlayerError> {
    let n = sim.upgrade_keys.len();
    if n == 0 {
        sim.rec("skip noupgrades".into());
        return Ok(());
    }
    let key = sim.upgrade_keys[a[1] as usize % n];
    match a[0] % 8 {
        0 => {
            let status = [UPGRADE_STATUS_INVALID, UPGRADE_STATUS_IN_PRODUCTION, UPGRADE_STATUS_COMPLETE][a[2] as usize % 3];
            let idx = player_add_upgrade(sim, p, key, status)?;
            sim.rec(format!("ret addupg {idx}"));
        }
        1 => {
            let idx = player_find_upgrade(sim, p, key)?;
            sim.rec(format!("ret findupg {}", idx.map_or(-1, |i| i as i32)));
        }
        2 => {
            let r = player_has_upgrade_complete(sim, p, key)?;
            sim.rec(format!("ret upgcomplete {}", r as u32));
        }
        3 => {
            let m = upgrade_mask_of(sim, a, 2);
            let r = player_has_upgrade_complete_mask(sim, p, &m)?;
            sim.rec(format!("ret upgmask {}", r as u32));
        }
        4 => {
            let r = player_has_upgrade_in_production(sim, p, key)?;
            sim.rec(format!("ret upginprod {}", r as u32));
        }
        5 => player_remove_upgrade(sim, p, key)?,
        6 => player_on_upgrade_completed(sim, p, key)?,
        _ => {
            let m = upgrade_mask_of(sim, a, 3);
            let me = sim.players.get_nth_mut(p).unwrap();
            let target = if a[2].is_multiple_of(2) { &mut me.upgrades_in_progress } else { &mut me.upgrades_completed };
            if a[6].is_multiple_of(3) {
                target.clear_mask(&m);
            } else {
                target.set_mask(&m);
            }
        }
    }
    Ok(())
}

fn squad_op(sim: &mut Sim, p: PlayerIndex, a: &[u32; 8]) -> Result<(), PlayerError> {
    // hotkey numbers -1 ..= 11: the last two and -1 are outside the table
    let hk = (a[1] % 13) as i32 - 1;
    let n = (a[2] % 5) as usize;
    let ids: Vec<ObjectId> = (0..n).map(|i| obj_id_of(sim, a[3 + i % 4])).collect();
    let group = |w: u32| -> Option<GroupRef> { if w.is_multiple_of(4) { None } else { Some(w % 3 + 1) } };
    match a[0] % 12 {
        0..=2 => {
            sim.msgs.push(ids);
            let msg = (sim.msgs.len() - 1) as MessageRef;
            match a[0] % 12 {
                0 => player_process_create_team_game_message(sim, p, hk, msg)?,
                1 => player_process_select_team_game_message(sim, p, hk, msg)?,
                _ => player_process_add_team_game_message(sim, p, hk, msg)?,
            }
        }
        3 => player_get_current_selection_as_ai_group(sim, p, group(a[2]))?,
        4 => player_set_currently_selected_ai_group(sim, p, group(a[2]))?,
        5 => {
            let sq = player_get_hotkey_squad(sim, p, hk)?;
            sim.rec(format!("ret hotkey {}", sq.map_or(-1, |s| squad_get_size_of_group(&s))));
        }
        6 => {
            let o = real_obj_id(sim, a[2]);
            let r = player_get_squad_number_for_object(sim, p, o)?;
            sim.rec(format!("ret squadno {r}"));
        }
        7 => {
            let o = if a[2].is_multiple_of(4) { None } else { Some(real_obj_id(sim, a[2])) };
            player_remove_object_from_hotkey_squad(sim, p, o)?;
        }
        8 => player_add_ai_group_to_current_selection(sim, p, group(a[2]))?,
        9 => {
            sim.groups[(a[1] % 3) as usize] = ids;
        }
        10 | 11 => {
            // a squad of the player: the current selection (a[2] & 4) or a hotkey squad
            let use_sel = a[2] & 4 != 0;
            let mut sq = if use_sel {
                match sim.players.get_nth_mut(p).unwrap().current_selection.take() {
                    Some(s) => s,
                    None => {
                        sim.rec("skip nosel".into());
                        return Ok(());
                    }
                }
            } else {
                match sim.players.get_nth_mut(p).unwrap().squads.get_mut(hk as usize).filter(|_| hk >= 0).map(std::mem::take) {
                    Some(s) => s,
                    None => {
                        sim.rec("skip nosquad".into());
                        return Ok(());
                    }
                }
            };
            if a[0] % 12 == 10 {
                match a[2] % 4 {
                    0 => {
                        let v = squad_get_all_objects(sim, &mut sq);
                        sim.rec(format!("ret squadall {}", id_list(&v)));
                    }
                    1 => {
                        let v = squad_get_live_objects(sim, &mut sq);
                        sim.rec(format!("ret squadlive {}", id_list(&v)));
                    }
                    _ => {
                        let o = real_obj_id(sim, a[3]);
                        sim.rec(format!("ret squadsize {} {}", squad_get_size_of_group(&sq), squad_is_on_squad(&sq, o) as u32));
                    }
                }
            } else if a[3].is_multiple_of(2) {
                let t = pick_opt_team(sim, a[4]);
                squad_from_team(sim, &mut sq, t, a[5].is_multiple_of(2));
            } else {
                squad_from_ai_group(sim, &mut sq, group(a[4]), a[5].is_multiple_of(2));
            }
            let me = sim.players.get_nth_mut(p).unwrap();
            if use_sel {
                me.current_selection = Some(sq);
            } else {
                me.squads[hk as usize] = sq;
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn xfer_op(sim: &mut Sim, p: PlayerIndex, a: &[u32; 8]) -> Result<(), PlayerError> {
    match a[0] % 3 {
        0 => {
            let mut xc = XferCrc::new();
            xc.open(b"p").unwrap();
            player_crc(sim, &mut xc, p)?;
            xc.close().unwrap();
            sim.rec(format!("ret pcrc {:08x}", xc.get_crc()));
            if std::env::var_os("PCRC_DBG").is_some() {
                let me = sim.players.get_nth(p).unwrap();
                eprintln!("RUST bonus={} crc={:08x} sp={} spp={}", me.battle_plan_bonuses.is_some() as u32, xc.get_crc(), me.skill_points, me.science_purchase_points);
                if let Some(b) = me.battle_plan_bonuses.clone() {
                    let mut x2 = XferCrc::new();
                    x2.open(b"q").unwrap();
                    let mut v = b.valid_kind_of;
                    v.xfer(&mut x2).unwrap();
                    eprintln!(" valid-only crc={:08x}", x2.get_crc());
                }
            }
            let mut save = XferSave::new();
            save.open(b"p").unwrap();
            xfer_nested(sim, &mut save, |h, x| player_xfer(h, x, p))?;
            save.close().unwrap();
            sim.saved = save.take_bytes();
            sim.have_save = true;
            sim.rec(format!("ret psave len={} hash={:08x}", sim.saved.len(), hash_bytes(&sim.saved)));
        }
        _ if !sim.have_save => sim.rec("skip".into()),
        _ => {
            // what a loading game has not filled yet: the lists a load refuses to find filled, the squads' caches
            let me = sim.players.get_nth_mut(p).unwrap();
            if a[1] & 1 != 0 {
                me.kind_of_production_changes.clear();
                me.special_power_timers.clear();
            }
            if a[1] & 2 != 0 {
                for sq in me.squads.iter_mut().chain(me.current_selection.iter_mut()) {
                    sq.objects_cached.clear();
                }
            }
            let mut load = XferLoad::new(sim.saved.clone());
            load.open(b"p").unwrap();
            let r = xfer_nested(sim, &mut load, |h, x| player_xfer(h, x, p));
            let _ = load.close();
            r?;
            sim.rec(format!("ret pload pp={}", load.post_process_requests()));
            player_load_post_process(sim, p)?;
        }
    }
    Ok(())
}
// --- end p1w10-sonnet ---

fn player_op(sim: &mut Sim, p: PlayerIndex, sub: u32, a: &[u32; 8]) -> Result<(), PlayerError> {
    LAST_SUB.store(sub, std::sync::atomic::Ordering::Relaxed);
    if std::env::var_os("PLAYER_OPS").is_some() {
        eprintln!("OP line {} player {p} sub {sub} {a:?}", sim.lines.len());
    }
    let np = sim.np_now();
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
                // --- p1w10-sonnet --- (slice 3): the other fields of the entry, which a save writes
                e.health = (a[4] % 300) as i32;
                e.whiner = a[4] & 1 != 0;
                e.unsellable = a[4] & 2 != 0;
                e.repairable = a[4] & 4 != 0;
                e.automatically_build = a[4] & 8 != 0;
                e.is_initially_built = a[4] & 16 != 0;
                e.under_construction = a[4] & 32 != 0;
                e.is_supply_building = a[4] & 64 != 0;
                e.building_name = format!("B{}", a[5] % 5).into_bytes();
                e.script = format!("S{}", (a[5] / 8) % 3).into_bytes();
                e.rally_point_offset = Coord2D { x: (a[6] % 50) as f32, y: k as f32 };
                e.desired_gatherers = (a[7] % 6) as i32;
                e.current_gatherers = ((a[7] / 8) % 6) as i32;
                e.object_timestamp = a[7];
                for (j, g) in e.resource_gatherers.iter_mut().enumerate() {
                    *g = (a[5].wrapping_mul(0x0101_0101)).wrapping_add(j as u32 * a[6]) as i32;
                }
                // --- end p1w10-sonnet ---
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
            let s = ScienceType(sim.sci_key((a[1] % 8) as i32));
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
        // --- p1w10-sonnet --- (slice 2)
        77 => {
            let o = pick_opt_obj(sim, a[0]);
            player_becoming_team_member(sim, p, o, a[1] != 0)?;
        }
        78 => player_becoming_local_player(sim, p, a[0] != 0)?,
        79 => {
            let sp = SP_POOL[(a[1] % 9) as usize];
            let th = (a[2] as usize % sim.tpls.len()) as TemplateRef;
            match a[0] % 7 {
                0 => {
                    let o = player_find_natural_command_center(sim, p)?;
                    sim.rec(format!("ret ncc {}", o.unwrap_or(0)));
                }
                1 => {
                    let o = player_find_most_ready_shortcut_special_power_of_type(sim, p, sp)?;
                    sim.rec(format!("ret mrsp {}", o.unwrap_or(0)));
                }
                2 => {
                    let (o, pct) = player_find_most_ready_shortcut_weapon_for_thing(sim, p, th)?;
                    sim.rec(format!("ret mrw {} {pct}", o.unwrap_or(0)));
                }
                3 => {
                    let (o, pct) = player_find_most_ready_shortcut_special_power_for_thing(sim, p, th)?;
                    sim.rec(format!("ret mrst {} {pct}", o.unwrap_or(0)));
                }
                4 => {
                    let o = player_find_any_existing_object_with_thing_template(sim, p, th)?;
                    sim.rec(format!("ret anyobj {}", o.unwrap_or(0)));
                }
                5 => {
                    let r = player_has_any_shortcut_special_power(sim, p)?;
                    sim.rec(format!("ret anysp {}", r as u32));
                }
                _ => {
                    let r = player_count_ready_shortcut_special_powers_of_type(sim, p, sp)?;
                    sim.rec(format!("ret countsp {r}"));
                }
            }
        }
        80 => {
            let b = pick_opt_obj(sim, a[0]);
            let st = pick_obj(sim, a[1]);
            player_on_structure_construction_complete(sim, p, b, st, a[2] != 0)?;
        }
        81 => player_set_units_should_hunt(sim, p, a[0] != 0, (a[1] % 4) as i32)?,
        82 => player_kill_player(sim, p)?,
        83 => {
            let that = (a[0] as usize % np) as PlayerIndex;
            // a player without a template has no beacon to look up (upstream dereferences NULL): skipped
            if sim.players.get_nth(that).unwrap().template.is_none() {
                sim.rec("skip notemplate".into());
            } else {
                player_transfer_assets_from_that(sim, p, that)?;
            }
        }
        84 => match a[0] % 4 {
            0 => player_garrison_all_units(sim, p, (a[1] % 4) as i32)?,
            1 => player_ungarrison_all_units(sim, p, (a[1] % 4) as i32)?,
            2 => player_set_units_should_idle_or_resume(sim, p, a[1] != 0)?,
            _ => player_sell_everything_under_the_sun(sim, p)?,
        },
        85 => {
            let (k, v) = (pick_opt_obj(sim, a[0]), pick_opt_obj(sim, a[1]));
            player_do_bounty_for_kill(sim, p, k, v)?;
        }
        86 => {
            let n = sim.tpls.len();
            let t = if a[0] % 3 == 1 && a[1] != 0 { ((a[1] - 1) as usize % n) as TemplateRef } else { (a[1] as usize % n) as TemplateRef };
            let none = a[0] % 3 == 1 && a[1] == 0;
            match a[0] % 3 {
                0 => {
                    let r = player_can_build_more_of_type(sim, p, t)?;
                    sim.rec(format!("ret morethan {}", r as u32));
                }
                1 => {
                    let r = player_can_build(sim, p, if none { None } else { Some(t) })?;
                    sim.rec(format!("ret canbuild {}", r as u32));
                }
                _ => {
                    let r = player_can_afford_build(sim, p, t)?;
                    sim.rec(format!("ret afford {}", r as u32));
                }
            }
        }
        87 => {
            let o = pick_obj(sim, a[0]);
            player_friend_apply_difficulty_bonuses_for_object(sim, p, o, a[1] != 0)?;
        }
        88 => {
            const SCAL: [f32; 8] = [1.0, 0.5, 2.0, 0.0, 0.7, 1.0, 1.3, 3.0];
            match a[0] % 6 {
                0 | 5 => {
                    let mut b = BattlePlanBonuses {
                        armor_scalar: SCAL[(a[3] % 8) as usize],
                        sight_range_scalar: SCAL[((a[3] >> 4) % 8) as usize],
                        bombardment: (a[4] % 4) as i32,
                        hold_the_line: ((a[4] >> 4) % 4) as i32,
                        search_and_destroy: ((a[4] >> 8) % 4) as i32,
                        valid_kind_of: kind_mask(a[5]),
                        invalid_kind_of: kind_mask(a[6]),
                    };
                    if a[0].is_multiple_of(6) {
                        let plan = [BattlePlanStatus::None, BattlePlanStatus::Bombardment, BattlePlanStatus::HoldTheLine, BattlePlanStatus::SearchAndDestroy][(a[1] % 4) as usize];
                        player_change_battle_plan(sim, p, plan, if !a[2].is_multiple_of(2) { 1 } else { -1 }, &mut b)?;
                    } else {
                        player_apply_battle_plan_bonuses_for_player_objects(sim, p, &b)?;
                    }
                    sim.rec(format!(
                        "bonus {} {} {} {} {} {} {}",
                        fb(b.armor_scalar),
                        fb(b.sight_range_scalar),
                        b.bombardment,
                        b.hold_the_line,
                        b.search_and_destroy,
                        kind_bits(&b.valid_kind_of),
                        kind_bits(&b.invalid_kind_of)
                    ));
                }
                1 => {
                    let o = pick_opt_obj(sim, a[1]);
                    let r = player_does_object_qualify_for_battle_plan(sim, p, o)?;
                    sim.rec(format!("ret qualifies {}", r as u32));
                }
                2 => {
                    let plan = [BattlePlanStatus::None, BattlePlanStatus::Bombardment, BattlePlanStatus::HoldTheLine, BattlePlanStatus::SearchAndDestroy][(a[1] % 4) as usize];
                    let x = player_get_battle_plans_active_specific(sim, p, plan)?;
                    let y = player_get_num_battle_plans_active(sim, p)?;
                    sim.rec(format!("ret plans {x} {y}"));
                }
                // without bonuses upstream dereferences NULL: the operation is skipped (the program is not excluded)
                3 | 4 if sim.players.get_nth(p).unwrap().battle_plan_bonuses.is_none() => sim.rec("skip nobonus".into()),
                3 => {
                    let o = pick_obj(sim, a[1]);
                    player_apply_battle_plan_bonuses_for_object(sim, p, o)?;
                }
                _ => {
                    let o = pick_obj(sim, a[1]);
                    player_remove_battle_plan_bonuses_for_object(sim, p, o)?;
                }
            }
        }
        89 => {
            let mask = kind_mask(a[1]);
            player_set_units_vision_spied(sim, p, a[0] != 0, &mask, (a[2] as usize % np) as PlayerIndex)?;
        }
        90 => {
            let amount = if a[0].is_multiple_of(3) { a[1] } else { a[1] % 4000 };
            let me = sim.players.get_nth_mut(p).unwrap();
            let idx = me.money.player_index();
            me.money = money_of(amount, idx);
        }
        91 => upgrade_op(sim, p, a)?,
        92 => squad_op(sim, p, a)?,
        93 => xfer_op(sim, p, a)?,
        94 => {
            if sim.sdicts.is_empty() {
                sim.rec("skip nosdicts".into());
            } else {
                let d = sim.sdicts[a[0] as usize % sim.sdicts.len()].clone();
                player_init_from_dict(sim, p, &d)?;
            }
        }
        // --- end p1w10-sonnet ---
        _ => sim.rec("ret nop".into()),
    }
    Ok(())
}

fn do_op(sim: &mut Sim, op: u32, r: &mut Reader<'_>) -> Result<(), PlayerError> {
    let np = sim.np_now();
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
        // --- p1w10-sonnet --- (slice 4): the player list
        7 => {
            let sub = r.u();
            let mut a = [0u32; 8];
            for x in a.iter_mut() {
                *x = r.u();
            }
            if std::env::var_os("PLAYER_OPS").is_some() {
                eprintln!("LIST line {} sub {} {a:?}", sim.lines.len(), sub % 15);
            }
            list_op(sim, &a, sub)?;
        }
        // --- end p1w10-sonnet ---
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

fn gen_header(g: &mut Gen, w: &mut W) -> (u32, u32, u32, [u32; 3]) {
    // (a few programs have all sixteen players, so that the last one is reached by the loops over the list)
    let np = if chance(g, 4) { 16 } else { 2 + g.below(3) };
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
    // the upgrade templates ("Upg0" ...)
    w.i(if chance(g, 10) { 0 } else { 1 + g.below(4) } as i32);
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
    NPT.store(npt, std::sync::atomic::Ordering::Relaxed);
    w.i(npt as i32);
    for ti in 0..npt {
        w.i(g.below(8) as i32);
        // the side: 7 is `Civilian`, whose skirmish side a human player takes the scripts of
        w.i(if chance(g, if ti == 0 { 55 } else { 25 }) { 7 } else { g.below(3) as i32 });
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
            // --- p1w10-sonnet --- (slice 2): often one of the kinds the object-facing operations look at
            w.u(if chance(g, 40) { [0u32, 11, 9, 12, 1][g.below(5) as usize] } else { g.below(15) });
        }
        w.b(chance(g, 25));
        w.i(g.below(2) as i32);
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
    let mut mod_counts: Vec<u32> = Vec::new();
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
            w.u(g.below(5));
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
        mod_counts.push(nm);
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
    let palette = gen_header2(g, w, np, nt, &used.iter().copied().collect::<Vec<_>>(), &mod_counts, frame0);
    // the name keys to look at at the end
    let probes = ["", "Player0", "Player3", "Tpl3", "Tpl9", "teamName", "HANDICAP_BUILDCOST_GENERIC", "HANDICAP_BUILDTIME_BUILDINGS", "TeamOwner", "never-used"];
    w.i(probes.len() as i32);
    for name in probes {
        w.s(name);
    }
    (np, no, frame0, palette)
}

// --- p1w10-sonnet --- (slice 2)
/// The second part of the header: what the object-facing operations read (costs, prerequisites, the objects' flags and
/// the special power modules' timers).
/// The number of player templates of the program being generated (`gen_sides` names factions).
static NPT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

fn gen_header2(g: &mut Gen, w: &mut W, np: u32, nt: u32, ids: &[u32], mod_counts: &[u32], frame0: u32) -> [u32; 3] {
    // the special powers of the program come from a palette of three (so that objects share a type)
    let palette = [g.below(9), g.below(9), g.below(9)];
    w.b(chance(g, 60));
    w.u(g.below(8));
    for _ in 0..nt {
        w.i(match g.below(8) {
            0 => 0,
            1..=3 => 50 * g.below(40) as i32,
            4 => -(g.below(300) as i32),
            // above 2^24 an f32 cannot hold every integer, so the order of the factors shows in the cost
            5 | 6 => 20_000_000 + g.below(200_000_000) as i32,
            _ => 100_000 + g.below(400_000) as i32,
        });
        w.u(g.below(4));
        w.u(g.below(4));
        w.u(g.u32());
        w.u(if chance(g, 50) { 0 } else { 1 + g.below(3) });
        w.i(if chance(g, 50) { 0 } else { 1 + g.below(3) as i32 });
        w.u(g.below(4));
    }
    for &nm in mod_counts {
        w.b(chance(g, 70));
        w.b(chance(g, 40));
        w.b(chance(g, 70));
        w.b(chance(g, 30));
        w.b(chance(g, 30));
        w.b(chance(g, 80));
        w.u(if chance(g, 80) { [0u32, 100, 100, 101, 129, 129][g.below(6) as usize] } else { g.below(130) });
        w.b(chance(g, 70));
        w.b(chance(g, 25));
        w.i(if chance(g, 50) { -1 } else { g.below(np) as i32 });
        w.u(g.u32());
        w.u(g.u32());
        w.u(if chance(g, 50) { 0 } else if chance(g, 70) { ids[g.below(ids.len() as u32) as usize] } else { 1 + g.below(200) });
        w.f(g.uniform(50.0, 500.0));
        w.f(g.uniform(0.0, 300.0));
        w.u(if chance(g, 50) { 0 } else { 1 << g.below(np) });
        w.u(g.u32());
        w.i(if chance(g, 50) { -1 } else { g.below(4) as i32 });
        for _ in 0..nm {
            w.u(if chance(g, 85) { palette[g.below(3) as usize] } else { g.below(9) });
            w.b(chance(g, 20));
            w.u(match g.below(8) {
                0 => 0,
                1 | 2 => frame0.saturating_sub(1),
                3 | 4 => frame0,
                5 => frame0 + 1,
                6 => frame0 + 2,
                _ => g.below(2000),
            });
            w.u(if chance(g, 70) { [0u32, 50, 99, 100, 101, 129][g.below(6) as usize] } else { g.below(130) });
            w.b(chance(g, 40));
        }
    }
    gen_sides(g, w);
    palette
}

/// One dictionary entry: a type (0 bool, 1 int, 2 real, 3 ascii, 4 unicode), a key, a value.
fn w_entry_str(w: &mut W, ty: i32, key: &str, v: &str) {
    w.i(ty);
    w.s(key);
    w.s(v);
}

fn w_entry_bits(w: &mut W, ty: i32, key: &str, v: u32) {
    w.i(ty);
    w.s(key);
    w.u(v);
}

/// The sides list of the program: player dictionaries (a faction, names, flags, colours, money, the enemies and allies, a
/// handicap), skirmish side dictionaries, team dictionaries (owner, name, script names, hooks), then the sixteen sides
/// (a dictionary, whether they have a script list, a build list), the skirmish sides and the teams that refer to them.
fn gen_sides(g: &mut Gen, w: &mut W) {
    let npt = NPT.load(std::sync::atomic::Ordering::Relaxed);
    let wide = chance(g, 15);
    WIDE.store(wide, std::sync::atomic::Ordering::Relaxed);
    let npd = if wide { 4 + g.below(2) } else { 1 + g.below(5) };
    let nsk = g.below(3);
    let ntd = g.below(5);
    w.i((npd + nsk + ntd) as i32);
    let faction = |g: &mut Gen| -> String {
        match g.below(10) {
            0 => "FactionObserver".to_string(),
            1 => format!("Faction{npt}"),
            _ => format!("Faction{}", g.below(npt)),
        }
    };
    for _ in 0..npd {
        type Entry = Box<dyn Fn(&mut W)>;
        let mut entries: Vec<Entry> = Vec::new();
        if chance(g, 90) {
            let f = faction(g);
            entries.push(Box::new(move |w| w_entry_str(w, 3, "playerFaction", &f)));
        }
        if chance(g, 70) {
            let n = format!("Name{}", g.below(100));
            entries.push(Box::new(move |w| w_entry_str(w, 4, "playerDisplayName", &n)));
        }
        if chance(g, 85) {
            let n = if chance(g, 8) { String::new() } else { format!("PN{}", g.below(6)) };
            entries.push(Box::new(move |w| w_entry_str(w, 3, "playerName", &n)));
        }
        for key in ["playerIsSkirmish", "playerIsHuman", "playerIsPreorder", "multiplayerIsLocal"] {
            if chance(g, 40) {
                let v = chance(g, 55) as u32;
                entries.push(Box::new(move |w| w_entry_bits(w, 0, key, v)));
            }
        }
        if chance(g, 60) {
            let v = g.below(8);
            entries.push(Box::new(move |w| w_entry_bits(w, 1, "multiplayerStartIndex", v)));
        }
        if chance(g, 50) {
            // a value the enum names (upstream stores any int in it; the Rust enum holds the three)
            let v = g.below(3);
            entries.push(Box::new(move |w| w_entry_bits(w, 1, "skirmishDifficulty", v)));
        }
        for key in ["playerColor", "playerNightColor"] {
            if chance(g, 40) {
                let v = g.below(0x100_0000);
                entries.push(Box::new(move |w| w_entry_bits(w, 1, key, v)));
            }
        }
        if chance(g, 50) {
            let v = g.below(3000);
            entries.push(Box::new(move |w| w_entry_bits(w, 1, "playerStartMoney", v)));
        }
        for key in ["playerEnemies", "playerAllies"] {
            if chance(g, 45) {
                let n = 1 + g.below(3);
                let v = (0..n).map(|_| format!("PN{}", g.below(6))).collect::<Vec<_>>().join(" ");
                entries.push(Box::new(move |w| w_entry_str(w, 3, key, &v)));
            }
        }
        if chance(g, 40) {
            let v = gen_real(g).to_bits();
            let key = ["HANDICAP_BUILDCOST_GENERIC", "HANDICAP_BUILDTIME_BUILDINGS", "HANDICAP_BUILDCOST_BUILDINGS"][g.below(3) as usize];
            entries.push(Box::new(move |w| w_entry_bits(w, 2, key, v)));
        }
        w.i(entries.len() as i32);
        for e in &entries {
            e(w);
        }
    }
    for k in 0..nsk {
        // a skirmish side: a faction (mostly the first), its name
        let f = format!("Faction{}", if chance(g, 60) { 0 } else { g.below(npt) });
        w.i(2);
        w_entry_str(w, 3, "playerFaction", &f);
        w_entry_str(w, 3, "playerName", &format!("SK{k}"));
    }
    for _ in 0..ntd {
        // a team: an owner (a player of the map or a skirmish side), a name, script names, hooks
        let mut entries: Vec<(i32, String, String)> = Vec::new();
        let owner = if chance(g, 50) && nsk > 0 { format!("SK{}", g.below(nsk)) } else { format!("PN{}", g.below(6)) };
        entries.push((3, "teamOwner".into(), owner));
        entries.push((3, "teamName".into(), format!("ST{}", g.below(4))));
        for key in ["teamOnCreateScript", "teamOnIdleScript", "teamOnUnitDestroyedScript", "teamOnDestroyedScript", "teamEnemySightedScript", "teamAllClearScript", "teamProductionCondition"] {
            if chance(g, 40) {
                entries.push((3, key.into(), if chance(g, 15) { String::new() } else { format!("Scr{}", g.below(5)) }));
            }
        }
        for j in 0..16 {
            if chance(g, 12) {
                entries.push((3, format!("teamGenericScriptHook{j}"), format!("Hook{}", g.below(4))));
            }
        }
        w.i(entries.len() as i32);
        for (ty, k, v) in &entries {
            w_entry_str(w, *ty, k, v);
        }
    }
    let total = npd + nsk + ntd;
    // the sixteen sides: a player dictionary for some of the first, a script list for some, a build list for some
    for i in 0..16u32 {
        w.u(if i < if wide { 16 } else { 6 } && chance(g, if wide { 92 } else { 55 }) { 1 + g.below(npd) } else { 0 });
        w.b(chance(g, 30));
        w.i(if chance(g, 30) { 1 + g.below(2) as i32 } else { 0 });
    }
    // the skirmish sides and the teams
    w.i(nsk as i32);
    for k in 0..nsk {
        w.u(npd + 1 + k);
        w.b(chance(g, 75));
    }
    let nst = g.below(4);
    w.i(nst as i32);
    for _ in 0..nst {
        w.u(if ntd > 0 { 1 + npd + nsk + g.below(ntd) } else { 0 });
    }
    let nkt = g.below(4);
    w.i(nkt as i32);
    for _ in 0..nkt {
        w.u(if ntd > 0 { 1 + npd + nsk + g.below(ntd) } else { 0 });
    }
    let _ = total;
}
// --- end p1w10-sonnet ---

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
        4..=7 => 1 << g.below(15),
        _ => (1 << g.below(15)) | (1 << g.below(15)),
    }
}

/// The eight words of a `Player` operation.
fn gen_player_args(g: &mut Gen, sub: u32, st: &GenState) -> [u32; 8] {
    let frame = st.frame;
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
            // --- p1w10-sonnet --- (slice 3): the other fields of the entries
            for x in a[4..].iter_mut() {
                *x = g.u32();
            }
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
            a[2] = [0.25f32, -0.5, 0.25, 1.0, 0.1, -0.3, 0.7, 0.15][g.below(8) as usize].to_bits();
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
        // --- p1w10-sonnet --- (slice 2)
        79 => {
            a[0] = [0u32, 1, 1, 1, 2, 3, 4, 5, 6, 6][g.below(10) as usize];
            a[1] = if chance(g, 75) { st.palette[g.below(3) as usize] } else { g.below(9) };
            a[2] = g.below(8);
        }
        84 => a[0] = g.below(4),
        86 => {
            a[0] = g.below(3);
            a[1] = g.below(8);
        }
        88 => {
            a[0] = g.below(6);
            a[1] = g.below(8);
            a[2] = g.below(2);
            a[3] = g.u32();
            a[4] = g.u32();
            // a valid mask of many kinds (so that objects qualify), an invalid one of few
            a[5] = (0..15).fold(0u32, |m, k| if chance(g, 45) { m | (1 << k) } else { m });
            a[6] = if chance(g, 55) { 0 } else { gen_mask(g) };
        }
        89 => a[1] = gen_mask(g),
        90 => {
            a[0] = g.below(3);
            a[1] = g.below(5000);
        }
        91 => {
            for x in a.iter_mut() {
                *x = g.u32();
            }
            a[0] = g.below(8);
        }
        92 => {
            for x in a.iter_mut() {
                *x = g.u32();
            }
            a[0] = g.below(12);
            a[1] = if chance(g, 85) { 1 + g.below(10) } else { g.below(13) };
            // most squad operations come as a fill (a group, a message with objects) and a use of what was filled
            a[2] = if chance(g, 50) { 1 + g.below(3) + 4 * g.below(2) } else { g.u32() };
            if chance(g, 40) {
                a[0] = [0u32, 9, 8, 4, 2, 1][g.below(6) as usize];
            }
        }
        94 => {
            a[0] = g.u32();
        }
        93 => {
            for x in a.iter_mut() {
                *x = g.u32();
            }
            a[0] = if chance(g, 35) { 0 } else { 1 + g.below(2) };
            a[1] = if chance(g, 70) { 3 } else { g.below(4) };
        }
        // --- end p1w10-sonnet ---
        _ => {}
    }
    a
}

/// The operations that make the world a Player has something to do in: a team of the player, objects on it.
fn gen_setup(g: &mut Gen, w: &mut W, n: &mut u32, np: u32) {
    // --- p1w10-sonnet --- (slice 2): most players start with a template (`init`), as in a game
    for k in 0..np {
        if chance(g, 85) {
            w.u(6);
            w.u(k);
            w.u(13);
            w.u(1 + g.below(8));
            for _ in 0..7 {
                w.u(0);
            }
            *n += 1;
        }
    }
    // --- end p1w10-sonnet ---
    let mut made = 0u32;
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
            // --- p1w10-sonnet --- (slice 2): a few objects on the team, so that the player has objects to search
            made += 1;
            for _ in 0..(4 + g.below(5)) {
                w.u(4);
                w.u(g.below(32));
                w.u(made);
                *n += 1;
            }
            // --- end p1w10-sonnet ---
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
    // --- p1w10-sonnet --- (slice 2)
    /// The three special powers the program's objects share.
    palette: [u32; 3],
    /// The battle plans the generator has switched on (`PLANSTATUS_*` index), so that it can switch them off again.
    plans: [[i32; 4]; 16],
    /// The number of players.
    np: u32,
    // --- end p1w10-sonnet ---
}

/// One `Player` operation with its first two arguments given and the rest random.
fn emit_player_op(g: &mut Gen, w: &mut W, pk: u32, sub: u32, a0: u32, a1: u32) {
    w.u(6);
    w.u(pk);
    w.u(sub);
    w.u(a0);
    w.u(a1);
    for _ in 2..8 {
        w.u(g.u32());
    }
}

/// One `Player` operation with all eight arguments given.
fn emit_player_args(w: &mut W, pk: u32, sub: u32, a: [u32; 8]) {
    w.u(6);
    w.u(pk);
    w.u(sub);
    for x in a {
        w.u(x);
    }
}

/// A directed sequence of `Player` operations that the plain random operations reach too seldom, as a list of
/// (player, sub, arguments); the caller counts the operations. `kind` picks the sequence.
fn directed_sequence(g: &mut Gen, np: u32, kind: u32) -> Vec<(u32, u32, [u32; 8])> {
    let mut v: Vec<(u32, u32, [u32; 8])> = Vec::new();
    let pk = g.below(np.max(1));
    let rnd = |g: &mut Gen| -> [u32; 8] { [g.u32(), g.u32(), g.u32(), g.u32(), g.u32(), g.u32(), g.u32(), g.u32()] };
    match kind {
        // hotkey squads: filled from an AI group that holds objects that do not exist, then asked for their objects
        0 => {
            let (gi, hk) = (g.below(3), g.below(10));
            let mut a = rnd(g);
            a[1] = gi;
            a[2] = 1 + g.below(4);
            v.push((pk, 92, [9, a[1], a[2], a[3], a[4], a[5], a[6], a[7]]));
            // (from the AI group: an odd a[3]; the group word is gi modulo 3 and not a multiple of 4)
            let mut gw = gi + 3 * g.below(30);
            while gw.is_multiple_of(4) {
                gw += 3;
            }
            let use_sel = chance(g, 30);
            let a2 = if use_sel { 4 } else { 0 };
            let mut b = rnd(g);
            b[1] = hk + 1;
            b[2] = a2;
            b[3] = 1;
            b[4] = gw;
            b[5] = g.below(2);
            v.push((pk, 92, [11, b[1], b[2], b[3], b[4], b[5], b[6], b[7]]));
            for q in 0..(2 + g.below(3)) {
                let mut c = rnd(g);
                c[1] = hk + 1;
                c[2] = a2 + (q % 2);
                v.push((pk, 92, [10, c[1], c[2], c[3], c[4], c[5], c[6], c[7]]));
            }
            // the selection as an AI group (the squad's live objects into a group)
            let mut d = rnd(g);
            d[2] = 1 + g.below(3);
            v.push((pk, 92, [3, d[1], d[2], d[3], d[4], d[5], d[6], d[7]]));
            // slice 4: half of the time the player is then initialised from a dictionary again, which makes its squads and its
            // selection afresh (plants 540, 541)
            if chance(g, 50) {
                v.push((pk, 94, [g.u32(), 0, 0, 0, 0, 0, 0, 0]));
            }
        }
        // slice 4: a player initialised from a dictionary (it then has a resource gathering manager and a tunnel tracker)
        // saved, and the save loaded into another player that has not (plants 359, 360), and the other way round
        6 => {
            let pk2 = if np > 1 { (pk + 1 + g.below(np - 1)) % np } else { pk };
            let dsel = g.u32();
            v.push((pk, 94, [dsel, 0, 0, 0, 0, 0, 0, 0]));
            v.push((pk, 93, [0, 3, 0, 0, 0, 0, 0, 0]));
            v.push((pk2, 93, [1 + g.below(2), 3, 0, 0, 0, 0, 0, 0]));
            v.push((pk2, 94, [g.u32(), 0, 0, 0, 0, 0, 0, 0]));
            v.push((pk, 93, [0, 3, 0, 0, 0, 0, 0, 0]));
            v.push((pk2, 93, [1 + g.below(2), 3, 0, 0, 0, 0, 0, 0]));
        }
        // save and load: with the caches and the lists filled, loaded without clearing them
        1 => {
            for _ in 0..g.below(3) {
                let mut a = rnd(g);
                a[0] = 0;
                a[2] = g.uniform(0.2, 1.8).to_bits();
                v.push((pk, 70, a));
            }
            let hk = g.below(10);
            for q in 0..2 {
                let mut c = rnd(g);
                c[1] = hk + 1;
                c[2] = q;
                v.push((pk, 92, [10, c[1], c[2], c[3], c[4], c[5], c[6], c[7]]));
            }
            let mut s = rnd(g);
            s[0] = 0;
            v.push((pk, 93, s));
            for _ in 0..(1 + g.below(3)) {
                let mut l = rnd(g);
                l[0] = 2;
                l[1] = g.below(4);
                v.push((g.below(np.max(1)), 93, l));
            }
        }
        // upgrades: added in production or complete, asked about, removed (the masks of the player follow)
        3 => {
            let key = g.u32();
            for _ in 0..(1 + g.below(2)) {
                let mut a = rnd(g);
                a[0] = 0;
                a[1] = key;
                a[2] = g.below(3);
                v.push((pk, 91, a));
            }
            let mut q = rnd(g);
            q[0] = 4;
            q[1] = key;
            v.push((pk, 91, q));
            let mut r = rnd(g);
            r[0] = 5;
            r[1] = key;
            v.push((pk, 91, r));
            let mut c = rnd(g);
            c[0] = 2;
            c[1] = key;
            v.push((pk, 91, c));
        }
        // team relationships: set, read, removed (a team that is not in the map, then one that is, then again)
        4 => {
            let (t1, t2) = (1 + g.below(12), 1 + g.below(12));
            for (sub, team, rel) in [(8u32, t1, g.below(3)), (8, t2, g.below(3)), (5, t1, 0), (5, t2, 0), (9, 1 + g.below(12), 0), (9, t1, 0), (9, t1, 0), (5, t1, 0), (5, t2, 0)] {
                let mut a = rnd(g);
                a[0] = team;
                a[1] = rel;
                v.push((pk, sub, a));
            }
        }
        // what a build costs: changes of the production cost by kind on every player (the victim's owner's count), a bounty
        // for the killer, then bounties and the affording of builds
        _ => {
            for q in 0..np.clamp(1, 6) {
                for _ in 0..(1 + g.below(2)) {
                    let mut a = rnd(g);
                    a[0] = 0;
                    // (a template matches when it has every kind of the change: one or two kinds)
                    a[1] = (1 << g.below(15)) | if chance(g, 30) { 1 << g.below(15) } else { 0 };
                    a[2] = g.uniform(-0.6, 1.4).to_bits();
                    v.push((q, 70, a));
                }
            }
            let mut a = rnd(g);
            a[0] = 1;
            v.push((pk, 76, a));
            for _ in 0..4 {
                let mut a = rnd(g);
                a[0] = 2;
                v.push((pk, 86, a));
                let mut b = rnd(g);
                b[0] = 1 + g.below(30);
                b[1] = 1 + g.below(30);
                v.push((pk, 85, b));
            }
        }
    }
    v
}

/// One player-list operation (top-level operation 7) with its first two arguments given and the rest random.
fn emit_list_op(g: &mut Gen, w: &mut W, sub: u32, a0: u32, a1: u32) {
    w.u(7);
    w.u(sub);
    w.u(a0);
    w.u(a1);
    for _ in 2..8 {
        w.u(g.u32());
    }
}

/// Set when the program's sides list fills all sixteen sides (so that a new game makes sixteen players).
static WIDE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn gen_one(g: &mut Gen, w: &mut W, n: &mut u32, st: &mut GenState) {
    let r = g.below(100);
    *n += 1;
    match r {
        0..=5 => {
            w.u(0);
            let v = [1u32, 1, 1, 2, 29, 30, 31, 60, 300][g.below(9) as usize];
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
            let sub = g.below(15);
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
                // --- p1w10-sonnet --- (slice 2)
                2 => a[0] = g.below(5),
                11 => {
                    a[0] = g.below(4);
                    a[1] = if chance(g, 50) { 0 } else { st.frame.saturating_sub(1) + g.below(3) };
                    a[2] = g.below(130);
                }
                12 => a[1] = g.below(130),
                13 => {
                    a[0] = g.below(4);
                    a[1] = g.u32();
                    a[3] = g.below(4);
                }
                14 => {
                    a[0] = g.uniform(1.0, 500.0).to_bits();
                    a[1] = g.below(5);
                }
                // --- end p1w10-sonnet ---
                _ => {}
            }
            for x in a {
                w.u(x);
            }
        }
        // --- p1w10-sonnet --- (slice 4): an operation on the player list
        35..=37 => {
            // the heavy ones (init, reset, newGame) are rarer than the queries
            let sub = if chance(g, 25) { [8u32, 9, 10, 10, 10][g.below(5) as usize] } else { g.below(15) };
            let mut a = [0u32; 8];
            for x in a.iter_mut() {
                *x = g.u32();
            }
            a[0] = match sub {
                // slice 4: a name that is no team and no player reaches validateTeam's last resort (neutral's default team)
                3 => if chance(g, 30) { 100 + g.below(40) } else { g.below(8) },
                4 => g.below(6),
                14 => g.below(8),
                _ => g.u32(),
            };
            if sub == 13 {
                a[1] = if chance(g, 70) { 3 } else { g.below(4) };
            }
            if sub == 5 && chance(g, 50) {
                a[0] = 0;
            }
            if sub == 7 {
                // the players of the program, and a mix of the four relationships
                a[0] = g.below(6);
                a[1] = 1 + g.below(15);
            }
            // slice 4: neutral's default team made first, so that validateTeam's last resort has an answer (plant 551)
            if sub == 3 && chance(g, 60) {
                emit_player_op(g, w, 0, 3, 0, 0);
                *n += 1;
            }
            w.u(7);
            w.u(sub);
            for x in a {
                w.u(x);
            }
            // a save is often loaded straight back (the interesting case: the state is the one that was saved)
            if sub == 12 && chance(g, 25) {
                // ... or after the list has changed size (init, new game): the load refuses it
                *n += 1;
                w.u(7);
                w.u([8u32, 10][g.below(2) as usize]);
                for _ in 0..8 {
                    w.u(g.u32());
                }
            }
            if sub == 12 && chance(g, 60) {
                *n += 1;
                w.u(7);
                w.u(13);
                for k in 0..8 {
                    w.u(if k == 1 { 3 } else { g.u32() });
                }
            }
        }
        // --- end p1w10-sonnet ---
        // --- p1w10-sonnet --- directed sequences for the plants the plain random operations reached too seldom
        // (relationships set, then the players asked for by relationship; what a player may build set, then asked)
        38..=40 => {
            if chance(g, 50) {
                for _ in 0..2 {
                    let pk = g.below(8);
                    for _ in 0..(2 + g.below(4)) {
                        let (o, rel) = (1 + g.below(8), g.below(3));
                        emit_player_op(g, w, pk, 6, o, rel);
                        *n += 1;
                    }
                }
                for flags in [2u32, 4, 8, 6, 12, 10] {
                    let src = g.below(st.np);
                    emit_list_op(g, w, 7, src, flags);
                    *n += 1;
                }
                *n -= 1;
            } else {
                let pk = g.below(8);
                let (units, base) = if chance(g, 50) { (0, 1) } else { (1, 0) };
                emit_player_op(g, w, pk, 51, units, base);
                for _ in 0..6 {
                    let t = g.u32();
                    emit_player_op(g, w, pk, 52, t, 0);
                    *n += 1;
                }
            }
        }
        41..=44 => {
            let kind = if chance(g, 25) { 6 } else { g.below(6) };
            if kind == 3 && st.np == 16 {
                // relations of the last player to a team, which the list is then told is about to be deleted
                let team = 1 + g.below(12);
                for _ in 0..3 {
                    let rel = g.below(3);
                    emit_player_op(g, w, 15, 8, team, rel);
                    *n += 1;
                }
                emit_list_op(g, w, 14, team, 0);
                return;
            }
            // (the last player's relations needed sixteen players: otherwise the sequence is a hotkey squad one)
            let kind = match kind {
                3 => 0,
                4 => 3,
                5 => 4,
                k => k,
            };
            let seq = directed_sequence(g, st.np, kind);
            *n += seq.len() as u32 - 1;
            for (pk, sub, a) in seq {
                emit_player_args(w, pk, sub, a);
            }
        }
        // --- end p1w10-sonnet ---
        _ => {
            w.u(6);
            let (pk, sub, a) = if let (true, Some((pk, sub, last))) = (chance(g, 25), st.last) {
                let fresh = gen_player_args(g, sub, st);
                let mut a = last;
                for k in 0..8 {
                    if chance(g, 40) {
                        a[k] = fresh[k];
                    }
                }
                (pk, sub, a)
            } else {
                let pk = g.below(16);
                // --- p1w10-sonnet --- directed pairs: a special power ready exactly now, then the search for it; a squad queried, then rebuilt
                if chance(g, 5) {
                    let ok = g.below(32);
                    let mut oa = [0u32; 8];
                    oa[0] = g.below(4);
                    oa[1] = st.frame;
                    oa[2] = g.below(130);
                    w.u(5);
                    w.u(ok);
                    w.u(11);
                    for x in oa {
                        w.u(x);
                    }
                    *n += 1;
                    let mut pa = gen_player_args(g, 79, st);
                    pa[0] = [1u32, 3, 0, 6][g.below(4) as usize];
                    st.last = Some((pk, 79, pa));
                    w.u(6);
                    w.u(pk);
                    w.u(79);
                    for x in pa {
                        w.u(x);
                    }
                    return;
                }
                if chance(g, 4) {
                    let mut qa = gen_player_args(g, 92, st);
                    qa[0] = 10;
                    qa[2] &= !4;
                    let hk = qa[1];
                    w.u(6);
                    w.u(pk);
                    w.u(92);
                    for x in qa {
                        w.u(x);
                    }
                    *n += 1;
                    let mut ca = gen_player_args(g, 92, st);
                    ca[0] = 0;
                    ca[1] = hk;
                    st.last = Some((pk, 92, ca));
                    w.u(6);
                    w.u(pk);
                    w.u(92);
                    for x in ca {
                        w.u(x);
                    }
                    return;
                }
                // --- end p1w10-sonnet ---
                // the subs: all, with a bias to the loops over objects and the sciences
                let sub = if chance(g, 14) { [92u32, 92, 92, 92, 92, 91, 93, 93, 94, 94][g.below(10) as usize] } else if chance(g, 20) { [0u32, 33, 34, 35, 36, 37, 38, 44, 68, 74, 75, 3, 69, 69, 62, 63, 64, 65, 70, 50, 14, 13, 2, 5, 6, 8, 76, 85, 85, 79, 79, 79, 79, 79, 79][g.below(35) as usize] } else if chance(g, 45) { 77 + g.below(18) } else { g.below(95) };
                let mut a = gen_player_args(g, sub, st);
                if sub == 88 && chance(g, 50) {
                    a[0] = 0;
                }
                if sub == 88 && a[0].is_multiple_of(6) {
                    // switch a plan off again that was switched on, half of the time
                    let on: Vec<u32> = (1..4u32).filter(|k| st.plans[(pk % st.np) as usize][*k as usize] > 0).collect();
                    let neg: Vec<u32> = (1..4u32).filter(|k| st.plans[(pk % st.np) as usize][*k as usize] < 0).collect();
                    if !neg.is_empty() && chance(g, 70) {
                        // a plan that was switched off without being on: switched on, back to zero
                        a[1] = neg[g.below(neg.len() as u32) as usize];
                        a[2] = 1;
                    } else if chance(g, 15) {
                        // a plan switched off that is not on, or on twice
                        a[1] = 1 + g.below(3);
                        a[2] = g.below(2);
                    } else if !on.is_empty() && chance(g, 55) {
                        let k = on[g.below(on.len() as u32) as usize];
                        a[1] = k;
                        a[2] = 0; // delta -1
                    } else {
                        a[1] = 1 + g.below(3);
                        a[2] = 1; // delta +1
                    }
                    let delta = if !a[2].is_multiple_of(2) { 1 } else { -1 };
                    st.plans[(pk % st.np) as usize][(a[1] % 4) as usize] += delta;
                }
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

fn gen_ops(g: &mut Gen, w: &mut W, np: u32, frame0: u32, palette: [u32; 3]) {
    let mut ops = W::default();
    let mut n = 0u32;
    // --- p1w10-sonnet --- (slice 4): some programs start a new game (or reset, or init the list) on the world they are given,
    // before anything exists that its teams would take down (those programs are excluded as undefined once teams and objects
    // are on the players' default teams)
    let wide = WIDE.load(std::sync::atomic::Ordering::Relaxed);
    if chance(g, 35) || wide {
        // (a local player other than the neutral one first, so that init and new game have a local player to replace)
        if chance(g, 50) {
            ops.u(7);
            ops.u(4);
            ops.u(1 + g.below(np));
            for _ in 1..8 {
                ops.u(g.u32());
            }
            n += 1;
        }
        let one = |g: &mut Gen, ops: &mut W, n: &mut u32, sub: u32| {
            ops.u(7);
            ops.u(sub);
            for k in 0..8 {
                ops.u(if k == 1 { 3 } else { g.u32() });
            }
            *n += 1;
        };
        one(g, &mut ops, &mut n, 10);
        if chance(g, 60) {
            // players a new game made, saved and loaded into the neutral player (which has no resource gathering manager and no
            // tunnel tracker; a human player has no AI either, so that the first check that differs is the manager's), and the
            // neutral player's save loaded into them
            for pk in 1..np.clamp(2, 4) {
                emit_player_op(g, &mut ops, pk, 93, 0, 0);
                emit_player_op(g, &mut ops, 0, 93, 2, 3);
                emit_player_op(g, &mut ops, 0, 93, 0, 0);
                emit_player_op(g, &mut ops, pk, 93, 2, 3);
                n += 4;
            }
        }
        if wide {
            // sixteen players: the last one's new map, team states, the teams about to be deleted, the players it has relations to
            for (sub, a0, a1) in [(1u32, 0u32, 0u32), (2, 0, 0), (14, 1 + g.below(8), 0), (14, 1 + g.below(8), 0), (7, 15, 1 + g.below(15)), (7, 15, 14)] {
                emit_list_op(g, &mut ops, sub, a0, a1);
                n += 1;
            }
        }
        // who the new game's players take for enemies, allies and neutral
        for flags in [4u32, 2, 8, 1] {
            if chance(g, 70) {
                ops.u(7);
                ops.u(7);
                ops.u(g.below(6));
                ops.u(flags);
                for _ in 2..8 {
                    ops.u(g.u32());
                }
                n += 1;
            }
        }
        if chance(g, 50) {
            let sub = [1u32, 0, 2, 12, 11, 8, 9][g.below(7) as usize];
            one(g, &mut ops, &mut n, sub);
        }
        if chance(g, 35) {
            one(g, &mut ops, &mut n, 10);
        }
    }
    // --- end p1w10-sonnet ---
    gen_setup(g, &mut ops, &mut n, np);
    let mut st = GenState { frame: frame0, last: None, palette, plans: [[0; 4]; 16], np };
    // --- p1w10-sonnet --- (slice 2): most programs start with a battle plan on for a player or two (the bonuses it hands out
    // are what later operations remove, apply to new team members and read back)
    for pk in 0..np {
        if chance(g, 50) {
            let plan = 1 + g.below(3);
            ops.u(6);
            ops.u(pk);
            ops.u(88);
            for x in [0, plan, 1, g.u32(), g.u32(), (0..15).fold(0u32, |m, k| if chance(g, 45) { m | (1 << k) } else { m }), if chance(g, 55) { 0 } else { gen_mask(g) }, 0] {
                ops.u(x);
            }
            n += 1;
            st.plans[pk as usize][plan as usize] += 1;
        }
    }
    // --- end p1w10-sonnet ---
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
    let (np, _no, frame0, palette) = gen_header(&mut g, &mut w);
    gen_ops(&mut g, &mut w, np, frame0, palette);
    w.0
}

// ---------------------------------------------------------------------------------------------------------------------
// The comparison
// ---------------------------------------------------------------------------------------------------------------------

fn cpp_run(words: &[u32]) -> Vec<String> {
    let path = std::ffi::CString::new(format!("{}/player_{}.bin", std::env::temp_dir().display(), std::process::id())).unwrap();
    let mut cap = 1 << 20;
    loop {
        let mut buf = vec![0u8; cap];
        let n = unsafe { cpp::ref_player_run(words.as_ptr(), words.len() as i32, path.as_ptr(), buf.as_mut_ptr() as *mut std::os::raw::c_char, cap as i32) } as usize;
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
    let (mut prefix_programs, mut prefix_lines) = (0u64, 0u64);
    let mut why_counts: BTreeMap<&str, u64> = BTreeMap::new();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let markers = [
        "ret rel ", "ret rmprel ", "ret rmtrel ", "ret skirmish", "ret super ", "ret enemy", "ret diff", "ret bridges", "ret base ", "ret safe", "ret attacked ", "ret count ", "ret buildings",
        "ret objects", "ret closest", "ret anybuildings", "ret anyunits", "ret anyobjects", "ret buildfacility", "ret local", "ret prod ", "ret allowed", "ret zone ", "ret radar ", "ret radarok",
        "ret spready", "ret kopc", "ret attackedby", "ret status", "ret hasscience", "ret purchase", "ret grant", "ret capable", "ret prereqs", "ret avail", "ret setrank", "ret addskill",
        "ret killskill", "audio ", "ui.dirty", "ui.spp", "ui.rank", "eva ", "msg.append", "script.science", "script.objchange", "sp.created", "sp.ready", "disable ", "enable ", "script-status",
        "ai.new", "ai.delete", "ai.update", "ai.buildteam",
        // --- p1w10-sonnet --- (slice 2)
        "ret ncc", "ret mrsp", "ret mrw", "ret mrst", "ret anyobj ", "ret anysp", "ret countsp", "ret morethan", "ret canbuild", "ret afford", "ret qualifies", "ret plans", "bonus ", "skip nobonus",
        "skip notemplate", "adjustpower", "capturebonus", "ui.idle+", "ui.idle-", "leavegroup", "ai.hunt", "ai.idle", "ai.enter", "ai.evacuate", "ai.move", "ai.force", "path.remove", "path.add",
        "partition.valuable", "recalc", "canenter", "sell ", "health ", "armor ", "wb+", "wb-", "vision ", "shroud ", "spied", "radar+", "radar-", "indicator", "teamcolor", "ui.scheme.player",
        "ui.scheme.template", "ui.float", "D obj2", "D player3 ",
        // --- p1w10-sonnet --- (slice 2b and 3)
        "ret addupg", "ret findupg", "ret upgcomplete", "ret upgmask", "ret upginprod", "upgrademods", "ret hotkey", "ret squadno", "ret squadall", "ret squadlive", "ret squadsize", "group.add", "skip nosel",
        "skip nosquad", "skip noupgrades", "ret pcrc", "ret psave", "ret pload", "threw invaliddata", "threw xfer", "D player4 ",
        // --- p1w10-sonnet --- (slice 4)
        "ret newgame", "scripts.qualify", "scripts.delete", "sides.removeteam", "sides.addteam", "ai.diff", "ret vteam", "ret pfm", "ret each", "ret pwr", "ret lcrc", "ret lsave", "ret lload", "skip nosdicts", "D sides",
        // --- end p1w10-sonnet --- "score.", "academy.", "setteam ", "kill ", "damage ", "heal ", "iter ", "skip", "threw badarg", "D team ",
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
                // the operations before the undefined one are defined: the C++ runs those and is compared on them
                if let Some(pre) = LAST_PREFIX.with(|p| p.borrow_mut().take()) {
                    let mut short = words.clone();
                    short[pre.pos_n] = pre.ops as u32;
                    let cpp_lines = cpp_run(&short);
                    prefix_programs += 1;
                    prefix_lines += pre.lines.len() as u64;
                    check.eq(cpp_lines.len() >= pre.lines.len(), true, || format!("program {case}: prefix line count"));
                    for (i, (a, b)) in pre.lines.iter().zip(cpp_lines.iter()).enumerate() {
                        check.eq(a.as_str(), b.as_str(), || format!("program {case} prefix line {i}"));
                    }
                }
                continue;
            }
        };
        if std::env::var_os("PLAYER_DUMP").is_some() {
            for l in &rust {
                eprintln!("RUST {l}");
            }
        }
        let cpp_lines = cpp_run(&words);
        lines += rust.len() as u64;
        for l in &rust {
            // --- p1w10-sonnet --- (slice 2): how often a search found something, a build was allowed, a plan was on
            for m in ["ret ncc ", "ret mrsp ", "ret mrw ", "ret mrst ", "ret anyobj ", "ret anysp ", "ret countsp ", "ret morethan ", "ret canbuild ", "ret afford ", "ret qualifies "] {
                if let Some(v) = l.strip_prefix(m) {
                    if !v.starts_with('0') {
                        *counts.entry(format!("{m}(non-zero)")).or_insert(0) += 1;
                    }
                }
            }
            // --- end p1w10-sonnet ---
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
        // `PLAYER_SHOW=1`: the lines before the first difference of a program (what the operations did), for debugging
        if std::env::var_os("PLAYER_SHOW").is_some() {
            if let Some(i) = rust.iter().zip(cpp_lines.iter()).position(|(a, b)| a != b) {
                eprintln!("--- program {case}, first difference at line {i}");
                for k in i.saturating_sub(40)..=i {
                    let (a, b) = (&rust[k], &cpp_lines[k]);
                    if a == b {
                        eprintln!("  {k}: {a}");
                    } else {
                        eprintln!("  {k}: RUST {a}\n  {k}: C++  {b}");
                    }
                }
            }
        }
    }
    println!("RESULT programs {programs}, excluded (upstream undefined) {excluded}, lines compared {lines}");
    println!("RESULT excluded programs compared on their defined prefix: {prefix_programs} ({prefix_lines} lines)");
    for (why, c) in &why_counts {
        println!("RESULT   excluded: {why}: {c}");
    }
    // --- p1w10-sonnet --- (slice 4) The generator is deterministic, so the number of programs excluded as undefined upstream is
    // too. A fault in the Rust that makes it fail where upstream does not (plant 562) shows as MORE exclusions, not as a
    // mismatch -- an excluded program is compared only on its defined prefix -- so the count is pinned for the usual sizes.
    if from == 0 {
        for (n, expected) in [(1000u64, 91u64), (2000, 188), (3000, 278)] {
            if cases() == n {
                check.eq(excluded, expected, || format!("programs excluded as undefined upstream, of {n}"));
            }
        }
    }
    // --- end p1w10-sonnet ---
    for (m, c) in &counts {
        println!("RESULT   {m:<24} {c}");
    }
    check.finish();
}
