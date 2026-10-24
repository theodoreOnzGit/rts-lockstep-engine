// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::world::build_assistant` (`BuildAssistant`: the footprint samples, the legal-to-build and
//! clear-of-objects queries, the tiled line, `buildObjectNow`/`buildObjectLineNow`, `canMakeUnit`/`isPossibleToMakeUnit`, `sellObject`
//! with the sell list and its `update`, and `xferTheSellList`) against upstream's own `BuildAssistant.h` and `BuildAssistant.cpp`
//! (commit 0a05454d). Phase 1 layer 7, p1w10-sonnet.
//!
//! # What is compared against what
//!
//! `build.rs` takes `BuildAssistant.h` unchanged and `BuildAssistant.cpp` whole, with one mechanical edit made in memory (its include
//! block is replaced; asserted to apply once) -- so every function of the file, with its statics, is the upstream text -- and
//! `differential/wrap_build_assistant.cpp` compiles them under clang++ on the stand-ins of `differential/shim_buildassist`: the objects
//! and the interfaces `BuildAssistant.cpp` reaches (AI, production, contain, parking place, create modules, drawable), the thing templates,
//! the player (seven calls), the terrain and its visual, the partition manager's queries, the pathfinder's cells, the control bar's command
//! sets, the audio, the game logic and the global data. `Geometry.h` (`GeometryInfo`), `KindOf.h`, `ObjectStatusTypes.h`, `ModelState.h`,
//! `GameCommon.h`, `Xfer.h`/`Xfer.cpp` (the sell list's xfer runs on the real `Xfer` under a memory buffer), the random value generator
//! (the objects told to move out of the way pick their destination with `GameLogicRandomValueReal`) and WWMath's `Matrix3D`/`Vector3` are the
//! real code. The Rust side runs the same program on `Sim`, a [`BuildAssistantHost`] that answers from the same configuration and writes
//! the same lines.
//!
//! Every call that has an effect (an object destroyed, a bib added, an AI order, a status or model condition set, a sale's construction
//! percent, a deposit, an object made) writes a line, formatted alike on both sides; calls that only answer a question write none (their
//! answers show in the `ret` lines). After every operation both sides write the whole state: the frame, every object that still exists
//! (position, orientation, construction percent, status, flags, producer, model conditions), the assistant's scratch array and sell list,
//! and every player's money.
//!
//! # Method
//!
//! A **program** is a header (the global data, the terrain's functions and densities, the thing templates, the players and their
//! relationships, the command sets, the objects) and a list of operations (about 20 kinds, 16 argument words each). The world is a small
//! deterministic one: the terrain height, the terrain tile, the pathfinder cell, the shroud and the layer at a point are hash functions of the
//! cell and a seed; the partition manager's queries are the 2D bounding-circle rules written out in `ba_standins_decl.h` and in `Sim` (the
//! real partition manager is verified by `differential_partition.rs`; the iteration order -- ascending object id -- is the host's choice
//! and decides which failure is reported first). Operations name objects by a selector word; one that names an object a
//! `processDestroyList` has removed is skipped on both sides.
//!
//! **Inputs for which upstream is undefined** (the position array has no element when a line is tiled before `init` or with
//! `m_maxLineBuildObjects == 0`; a builder with no controlling player asked what it can make; a player with no default team given to
//! `buildObjectNow`) make the C++ crash. The driver catches that (SIGSEGV, `sigsetjmp`), drops the lines of the operation and ends the
//! program with `UNDEF op=<k>`; the Rust, which returns an `is_undefined()` error there, writes the same line. So the two must agree on
//! where upstream is undefined: a Rust that misses a place shows as a difference (the C++ ends the program, the Rust goes on) and
//! one that sees one too many shows too.
//!
//! # Results (2026-10-21, upstream `0a05454d`)
//!
//! `cargo test --release --features differential --test differential_build_assistant -- --nocapture` with `BA_CASES=3000`: **3,000
//! programs, 470 of them ending in `UNDEF` (upstream undefined at the same operation on both sides), 1,955,904 trace lines compared, 0
//! mismatches.** The programs make 2,359 saves and 745 loads of the sell list, 9,261 updates (the sell countdown runs from `99.9` to the
//! refund at `-50` in 160-frame runs), 8,926 sales, 9,123 tiled lines, 13,600 `buildObjectNow` calls (5,846 objects made; 157 orders to a dozer to
//! `construct`), 6,898 + 7,096 + 548 + 6,773 + 890 + 361 + 106 location checks answering each of the seven `LegalBuildCode`s, 4,646 + 4,625 +
//! 17 clear-of-objects checks answering OK, objects in the way and generic failure, 17,550 bibs, 7,254 `removeTreesAndPropsForConstruction`
//! calls, and every `CanMakeType` (375 OK, 2,762 no prerequisites, 136 no money, 953 factory disabled, 10 queue full, 6 parking full, 4,067
//! maxed out).
//!
//! **Planted bugs.** `BA_PLANT=<n>` switches one fault into the Rust alone (all in one build; ids 1 to 172, of which 47 and 77 were
//! replaced while the set was written and have no code). **170 faults: 163 are caught by mismatching lines within 3,000 programs** (3 to
//! 1.4 million lines each; the weakest, 51, drops the last footprint row only when it falls within 3 units of the extent: 3 lines), **2 by a panic
//! of the Rust itself** (130 and 133 leave the position array one element short and index past it), and **5 are equivalent mutants** that
//! cannot change the answer in this fixture: 49 (a sphere or cylinder footprint takes `getMajorRadius` instead of the bounding circle,
//! which are the same number); 55 (the `z` handed to `Transform_Vector` is overwritten by the ground height afterwards); 86 (a negative
//! exit width is never tested for being positive); 96 (the minor radius of a non-box exit rectangle, which the bounding-circle collision
//! of the stand-in ignores); 101 (skipping the "no exit space" shortcut when only the template's own bib is padded, where every exit
//! check is off anyway).
//!
//! Rounds of the generator that the plants forced: the sell countdown runs one update per frame (160 per operation) so that the whole
//! sale is reached; an object's construction percent is sometimes set to exactly `0` mid-sale (plant 4); a line's end has a height of its
//! own (plant 131, `placementVector` is 3D); placement queries for templates with a factory exit, by a skirmish AI player, at a random
//! distance and bearing from an immobile structure, with exit widths up to 100 (plant 85, the skirmish padding of the exit width); and
//! the `UNDEF` line, which turned the C++ crashes of faulty Rust (a position array with no element) into line mismatches.
#![cfg(feature = "differential")]

mod support;

use std::collections::BTreeMap;
use std::ffi::CString;

use rts_lockstep_engine::common::base_type::{Coord3D, Region3D};
use rts_lockstep_engine::common::game_common::{CellShroudStatus, PlayerType, Relationship};
use rts_lockstep_engine::common::game_type::{ObjectId, PathfindLayer, INVALID_ID};
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::model_state::ModelConditionFlagType;
use rts_lockstep_engine::common::object_status_types::*;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::build_assistant::*;
use rts_lockstep_engine::logic::world::ids::PlayerIndex;
use rts_lockstep_engine::logic::world::team_ops::TemplateRef;
use support::{Check, Gen};

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ---------------------------------------------------------------------------------------------------------------------
// the program
// ---------------------------------------------------------------------------------------------------------------------

/// The kinds a template's mask names, bit by bit (the table of `ba_driver.h`).
const KIND_TABLE: [KindOfType; 11] = [
    KINDOF_DOZER,
    KINDOF_STRUCTURE,
    KINDOF_INERT,
    KINDOF_SHRUBBERY,
    KINDOF_CLEARED_BY_BUILD,
    KINDOF_MINE,
    KINDOF_ALWAYS_SELECTABLE,
    KINDOF_IMMOBILE,
    KINDOF_LINEBUILD,
    KINDOF_CANNOT_BUILD_NEAR_SUPPLIES,
    KINDOF_SUPPLY_SOURCE,
];

struct W(Vec<u32>);
impl W {
    fn u(&mut self, v: u32) {
        self.0.push(v);
    }
    fn i(&mut self, v: i32) {
        self.0.push(v as u32);
    }
    fn f(&mut self, v: f32) {
        self.0.push(v.to_bits());
    }
}

#[derive(Clone)]
struct TmplCfg {
    kind_mask: u32,
    geom_type: u32,
    small: u32,
    height: f32,
    major: f32,
    minor: f32,
    exit_width: f32,
    extra_bib: f32,
    vision: f32,
    refund: i32,
    cost: i32,
    equiv: u32,
    flags: u32,
}

#[derive(Clone)]
struct PlayerCfg {
    computer: u32,
    skirmish: u32,
    has_team: u32,
    money: u32,
    can_mask: u32,
    more_mask: u32,
}

#[derive(Clone)]
struct ObjCfg {
    tmpl: u32,
    owner: u32,
    pos: [f32; 3],
    orient: f32,
    status: u64,
    disabled: u32,
    dead: u32,
    script_bits: u32,
    ai_busy: u32,
    cmd_set: u32,
    quick_path: u32,
    cpct: f32,
    producer: u32,
    sp_cons: i32,
    can_queue: u32,
    geom_override: u32,
    geom_h: f32,
    geom_major: f32,
    geom_minor: f32,
    geom_type: u32,
}

#[derive(Clone)]
struct Prog {
    seed: u32,
    max_line: i32,
    min_edge: f32,
    height_var: f32,
    supply_border: f32,
    sell_pct: f32,
    extent: [f32; 6],
    pf_extent: [f32; 6],
    seed_h: u32,
    tile_density: u32,
    cell_density: u32,
    shroud_density: u32,
    layer_density: u32,
    frame0: u32,
    tmpls: Vec<TmplCfg>,
    players: Vec<PlayerCfg>,
    rel: Vec<u32>,
    sets: Vec<Vec<Option<(i32, u32)>>>,
    objs: Vec<ObjCfg>,
    ops: Vec<(u32, [u32; 16])>,
}

fn words(p: &Prog, n_ops: usize) -> Vec<u32> {
    let mut w = W(Vec::new());
    w.u(p.seed);
    w.i(p.max_line);
    w.f(p.min_edge);
    w.f(p.height_var);
    w.f(p.supply_border);
    w.f(p.sell_pct);
    for v in p.extent {
        w.f(v);
    }
    for v in p.pf_extent {
        w.f(v);
    }
    w.u(p.seed_h);
    w.u(p.tile_density);
    w.u(p.cell_density);
    w.u(p.shroud_density);
    w.u(p.layer_density);
    w.u(p.frame0);
    w.i(p.tmpls.len() as i32);
    for t in &p.tmpls {
        w.u(t.kind_mask);
        w.u(t.geom_type);
        w.u(t.small);
        w.f(t.height);
        w.f(t.major);
        w.f(t.minor);
        w.f(t.exit_width);
        w.f(t.extra_bib);
        w.f(t.vision);
        w.i(t.refund);
        w.i(t.cost);
        w.u(t.equiv);
        w.u(t.flags);
    }
    w.i(p.players.len() as i32);
    for q in &p.players {
        w.u(q.computer);
        w.u(q.skirmish);
        w.u(q.has_team);
        w.u(q.money);
        w.u(q.can_mask);
        w.u(q.more_mask);
    }
    for r in &p.rel {
        w.u(*r);
    }
    w.i(p.sets.len() as i32);
    for s in &p.sets {
        for slot in s {
            match slot {
                None => w.u(0),
                Some((ty, t)) => {
                    w.u(1);
                    w.i(*ty);
                    w.u(*t);
                }
            }
        }
    }
    w.i(p.objs.len() as i32);
    for o in &p.objs {
        w.u(o.tmpl);
        w.u(o.owner);
        for v in o.pos {
            w.f(v);
        }
        w.f(o.orient);
        w.u(o.status as u32);
        w.u((o.status >> 32) as u32);
        w.u(o.disabled);
        w.u(o.dead);
        w.u(o.script_bits);
        w.u(o.ai_busy);
        w.u(o.cmd_set);
        w.u(o.quick_path);
        w.f(o.cpct);
        w.u(o.producer);
        w.i(o.sp_cons);
        w.u(o.can_queue);
        w.u(o.geom_override);
        w.f(o.geom_h);
        w.f(o.geom_major);
        w.f(o.geom_minor);
        w.u(o.geom_type);
    }
    w.i(n_ops as i32);
    for (op, a) in p.ops.iter().take(n_ops) {
        w.u(*op);
        for x in a {
            w.u(*x);
        }
    }
    w.0
}

// ---------------------------------------------------------------------------------------------------------------------
// the world, and the host over it (the mirror of ba_standins_decl.h and ba_driver.h)
// ---------------------------------------------------------------------------------------------------------------------

fn h3(a: u32, b: u32, c: u32) -> u32 {
    let mut x = a.wrapping_mul(73856093) ^ b.wrapping_mul(19349663) ^ c.wrapping_mul(83492791);
    x ^= x >> 13;
    x = x.wrapping_mul(0x5bd1e995);
    x ^= x >> 15;
    x
}

fn cell_of(v: f32) -> i32 {
    (v / 10.0).floor() as i32
}

fn dist2(a: &Coord3D, b: &Coord3D) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    dx * dx + dy * dy
}

fn list_of(v: &[i32]) -> String {
    v.iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

struct TmplS {
    kinds: Vec<KindOfType>,
    geom: GeometryInfo,
    exit_width: f32,
    extra_bib: f32,
    vision: f32,
    refund: i32,
    cost: i32,
    equiv: u32,
    flags: u32,
}

struct PlayerS {
    index: i32,
    computer: bool,
    skirmish: bool,
    has_team: bool,
    money: u32,
    can_mask: u32,
    more_mask: u32,
}

struct ObjS {
    id: i32,
    tmpl: usize,
    player: Option<usize>,
    pos: Coord3D,
    orient: f32,
    geom: GeometryInfo,
    status: u64,
    disabled: bool,
    dead: bool,
    destroyed: bool,
    removed: bool,
    script_bits: u8,
    ai_busy: bool,
    quick_path: bool,
    cmd_set: i32,
    cpct: f32,
    producer: ObjectId,
    sp_cons: i32,
    can_queue: i32,
    mc: u128,
}

struct Sim {
    globals: BuildGlobals,
    extent: Region3D,
    pf_extent: Region3D,
    seed_h: u32,
    tile_density: u32,
    cell_density: u32,
    shroud_density: u32,
    layer_density: u32,
    frame: u32,
    tmpls: Vec<TmplS>,
    players: Vec<PlayerS>,
    rel: Vec<Vec<u32>>,
    /// per command set: per slot `(command type, template)`
    sets: Vec<Vec<Option<(i32, usize)>>>,
    objs: Vec<ObjS>,
    rng: RandomValues,
    lines: Vec<String>,
    ba: BuildAssistant,
    saved: Option<Vec<u8>>,
}

fn region(a: &[f32; 6]) -> Region3D {
    Region3D {
        lo: Coord3D {
            x: a[0],
            y: a[1],
            z: a[2],
        },
        hi: Coord3D {
            x: a[3],
            y: a[4],
            z: a[5],
        },
    }
}

fn status_list(bits: &[ObjectStatusTypes]) -> Vec<i32> {
    let mut v: Vec<i32> = bits.to_vec();
    v.sort_unstable();
    v.dedup();
    v
}

fn mc_bit(mc: &mut u128, i: i32, on: bool) {
    if on {
        *mc |= 1u128 << i;
    } else {
        *mc &= !(1u128 << i);
    }
}

impl Sim {
    fn new(p: &Prog) -> Sim {
        let mut rng = RandomValues::new(0);
        rng.init_game_logic_random(p.seed);
        let n_t = p.tmpls.len();
        let tmpls = p
            .tmpls
            .iter()
            .map(|t| TmplS {
                kinds: (0..11)
                    .filter(|b| (t.kind_mask >> b) & 1 != 0)
                    .map(|b| KIND_TABLE[b])
                    .collect(),
                geom: GeometryInfo::new(
                    geom_type(t.geom_type),
                    t.small != 0,
                    t.height,
                    t.major,
                    t.minor,
                ),
                exit_width: t.exit_width,
                extra_bib: t.extra_bib,
                vision: t.vision,
                refund: t.refund,
                cost: t.cost,
                equiv: t.equiv,
                flags: t.flags,
            })
            .collect();
        let n_p = p.players.len();
        let players = p
            .players
            .iter()
            .enumerate()
            .map(|(k, q)| PlayerS {
                index: k as i32,
                computer: q.computer != 0,
                skirmish: q.skirmish != 0,
                has_team: q.has_team != 0,
                money: q.money,
                can_mask: q.can_mask,
                more_mask: q.more_mask,
            })
            .collect();
        let rel = (0..n_p)
            .map(|a| (0..n_p).map(|b| p.rel[a * n_p + b] % 3).collect())
            .collect();
        let sets = p
            .sets
            .iter()
            .map(|s| {
                s.iter()
                    .map(|slot| slot.map(|(ty, t)| (ty, (t as usize) % n_t)))
                    .collect()
            })
            .collect();
        let mut sim = Sim {
            globals: BuildGlobals {
                max_line_build_objects: p.max_line,
                min_dist_from_edge_of_map_for_build: p.min_edge,
                allowed_height_variation_for_building: p.height_var,
                supply_build_border: p.supply_border,
                sell_percentage: p.sell_pct,
            },
            extent: region(&p.extent),
            pf_extent: region(&p.pf_extent),
            seed_h: p.seed_h,
            tile_density: p.tile_density,
            cell_density: p.cell_density,
            shroud_density: p.shroud_density,
            layer_density: p.layer_density,
            frame: p.frame0,
            tmpls,
            players,
            rel,
            sets,
            objs: Vec::new(),
            rng,
            lines: Vec::new(),
            ba: BuildAssistant::new(),
            saved: None,
        };
        let n_cs = p.sets.len() as u32;
        for o in &p.objs {
            let owner = if o.owner >= n_p as u32 {
                None
            } else {
                Some(o.owner as usize)
            };
            let id = sim.make_object(o.tmpl as usize % n_t, owner);
            let ob = &mut sim.objs[id as usize - 1];
            ob.pos = Coord3D {
                x: o.pos[0],
                y: o.pos[1],
                z: o.pos[2],
            };
            ob.orient = o.orient;
            ob.status = o.status;
            ob.disabled = o.disabled != 0;
            ob.dead = o.dead != 0;
            ob.script_bits = o.script_bits as u8;
            ob.ai_busy = o.ai_busy != 0;
            ob.cmd_set = if o.cmd_set >= n_cs {
                -1
            } else {
                o.cmd_set as i32
            };
            ob.quick_path = o.quick_path != 0;
            ob.cpct = o.cpct;
            ob.producer = o.producer as ObjectId;
            ob.sp_cons = if o.sp_cons < 0 || o.sp_cons as usize >= n_t {
                -1
            } else {
                o.sp_cons
            };
            ob.can_queue = (o.can_queue % 7) as i32;
            if o.geom_override != 0 {
                ob.geom = GeometryInfo::new(
                    geom_type(o.geom_type % 3),
                    false,
                    o.geom_h,
                    o.geom_major,
                    o.geom_minor,
                );
            }
        }
        sim
    }

    fn make_object(&mut self, tmpl: usize, player: Option<usize>) -> i32 {
        let id = self.objs.len() as i32 + 1;
        let geom = self.tmpls[tmpl].geom.clone();
        self.objs.push(ObjS {
            id,
            tmpl,
            player,
            pos: Coord3D::default(),
            orient: 0.0,
            geom,
            status: 0,
            disabled: false,
            dead: false,
            destroyed: false,
            removed: false,
            script_bits: 0,
            ai_busy: false,
            quick_path: false,
            cmd_set: -1,
            cpct: -1.0,
            producer: INVALID_ID,
            sp_cons: -1,
            can_queue: 0,
            mc: 0,
        });
        id
    }

    fn rec(&mut self, s: String) {
        self.lines.push(s);
    }

    fn o(&self, id: ObjectId) -> &ObjS {
        &self.objs[id as usize - 1]
    }

    fn om(&mut self, id: ObjectId) -> &mut ObjS {
        &mut self.objs[id as usize - 1]
    }

    fn live(&self) -> impl Iterator<Item = &ObjS> {
        self.objs.iter().filter(|o| !o.removed)
    }

    fn structure_filter(&self, o: &ObjS, kind: KindOfType) -> bool {
        self.tmpls[o.tmpl].kinds.contains(&kind)
    }

    fn dump(&mut self) {
        let mut out: Vec<String> = Vec::new();
        out.push(format!(
            "W frame={} next={}",
            self.frame,
            self.objs.len() + 1
        ));
        for o in self.objs.iter().filter(|o| !o.removed) {
            out.push(format!(
                "O {} t={} p={} pos={:08x},{:08x},{:08x} o={:08x} cp={:08x} st={:x} d{} dead{} dst{} sb{} pr={} mc={:x},{:x}",
                o.id,
                o.tmpl,
                o.player.map_or(-1, |p| p as i32),
                o.pos.x.to_bits(),
                o.pos.y.to_bits(),
                o.pos.z.to_bits(),
                o.orient.to_bits(),
                o.cpct.to_bits(),
                o.status,
                o.disabled as i32,
                o.dead as i32,
                o.destroyed as i32,
                o.script_bits,
                o.producer as u32,
                o.mc as u64,
                (o.mc >> 64) as u64
            ));
        }
        let pos: Vec<String> = self
            .ba
            .build_positions
            .iter()
            .take((self.ba.build_position_size.max(0) as usize).min(4))
            .map(|c| {
                format!(
                    "{:08x},{:08x},{:08x}",
                    c.x.to_bits(),
                    c.y.to_bits(),
                    c.z.to_bits()
                )
            })
            .collect();
        let sells: Vec<String> = self
            .ba
            .sell_list
            .iter()
            .map(|s| format!("{}:{}", s.id, s.sell_frame))
            .collect();
        out.push(format!(
            "B size={} alloc={} pos=[{}] sells=[{}]",
            self.ba.build_position_size,
            (!self.ba.build_positions.is_empty() && self.ba.build_position_size > 0) as i32,
            pos.join(" "),
            sells.join(",")
        ));
        for p in &self.players {
            out.push(format!("P {} money={}", p.index, p.money));
        }
        self.lines.extend(out);
    }
}

fn geom_type(v: u32) -> GeometryType {
    match v {
        0 => GeometryType::Sphere,
        1 => GeometryType::Cylinder,
        _ => GeometryType::Box,
    }
}

fn rel_of(v: u32) -> Relationship {
    match v % 3 {
        0 => Relationship::Enemies,
        1 => Relationship::Neutral,
        _ => Relationship::Allies,
    }
}

impl BuildAssistantHost for Sim {
    fn build_globals(&self) -> BuildGlobals {
        self.globals
    }
    fn frame(&self) -> u32 {
        self.frame
    }
    fn random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.rng.game_logic_random_value_real(lo, hi)
    }
    fn find_object_by_id(&self, id: ObjectId) -> Option<ObjectId> {
        if id >= 1 && (id as usize) <= self.objs.len() && !self.objs[id as usize - 1].removed {
            Some(id)
        } else {
            None
        }
    }
    fn destroy_object(&mut self, o: ObjectId) {
        if !self.o(o).destroyed {
            self.om(o).destroyed = true;
            self.rec(format!("destroy {o}"));
        }
    }
    fn deselect_object(&mut self, o: ObjectId) {
        self.rec(format!("deselect {o}"));
    }
    fn all_objects(&self) -> Vec<ObjectId> {
        self.live().map(|o| o.id).collect()
    }
    fn obj_template(&self, o: ObjectId) -> TemplateRef {
        self.o(o).tmpl as TemplateRef
    }
    fn obj_is_kind_of(&self, o: ObjectId, k: KindOfType) -> bool {
        self.tmpls[self.o(o).tmpl].kinds.contains(&k)
    }
    fn obj_test_status(&self, o: ObjectId, s: ObjectStatusTypes) -> bool {
        (self.o(o).status >> s) & 1 != 0
    }
    fn obj_test_script_status_bit(&self, o: ObjectId, bit: u8) -> bool {
        self.o(o).script_bits & bit != 0
    }
    fn obj_is_disabled(&self, o: ObjectId) -> bool {
        self.o(o).disabled
    }
    fn obj_is_effectively_dead(&self, o: ObjectId) -> bool {
        self.o(o).dead
    }
    fn obj_geometry(&self, o: ObjectId) -> GeometryInfo {
        self.o(o).geom.clone()
    }
    fn obj_position(&self, o: ObjectId) -> Coord3D {
        self.o(o).pos
    }
    fn obj_orientation(&self, o: ObjectId) -> f32 {
        self.o(o).orient
    }
    fn obj_controlling_player(&self, o: ObjectId) -> Option<PlayerIndex> {
        self.o(o).player.map(|p| p as PlayerIndex)
    }
    fn obj_relationship(&self, a: ObjectId, b: ObjectId) -> Relationship {
        match (self.o(a).player, self.o(b).player) {
            (Some(x), Some(y)) => rel_of(self.rel[x][y]),
            _ => Relationship::Neutral,
        }
    }
    fn obj_has_ai(&self, o: ObjectId) -> bool {
        self.tmpls[self.o(o).tmpl].flags & 1 != 0
    }
    fn ai_is_busy(&self, o: ObjectId) -> bool {
        self.o(o).ai_busy
    }
    fn ai_idle(&mut self, o: ObjectId) {
        self.rec(format!("ai.idle {o}"));
    }
    fn ai_construct(
        &mut self,
        o: ObjectId,
        what: TemplateRef,
        pos: Coord3D,
        angle: f32,
        player: PlayerIndex,
    ) -> Option<ObjectId> {
        self.rec(format!(
            "ai.construct {o} tmpl={what} pos={:08x},{:08x},{:08x} angle={:08x} player={player}",
            pos.x.to_bits(),
            pos.y.to_bits(),
            pos.z.to_bits(),
            angle.to_bits()
        ));
        if !self.o(o).quick_path {
            return None;
        }
        let n = self.thing_factory_new_object(what, player, &[OBJECT_STATUS_UNDER_CONSTRUCTION]);
        let ob = self.om(n);
        ob.pos = pos;
        ob.orient = angle;
        self.rec(format!("ai.constructed {n}"));
        Some(n)
    }
    fn ai_is_quick_path_available(&mut self, o: ObjectId, _pos: Coord3D) -> bool {
        self.o(o).quick_path
    }
    fn ai_move_to_position_even_if_sleeping(&mut self, o: ObjectId, pos: Coord3D) {
        self.rec(format!(
            "ai.move {o} {:08x} {:08x} {:08x}",
            pos.x.to_bits(),
            pos.y.to_bits(),
            pos.z.to_bits()
        ));
    }
    fn obj_producer_id(&self, o: ObjectId) -> ObjectId {
        self.o(o).producer
    }
    fn obj_construction_percent(&self, o: ObjectId) -> f32 {
        self.o(o).cpct
    }
    fn obj_set_construction_percent(&mut self, o: ObjectId, p: f32) {
        self.rec(format!("cpct {o} {:08x}", p.to_bits()));
        self.om(o).cpct = p;
    }
    fn obj_clear_and_set_model_condition(
        &mut self,
        o: ObjectId,
        clear: &[ModelConditionFlagType],
        set: &[ModelConditionFlagType],
    ) {
        let c = status_list(clear);
        let s = status_list(set);
        self.rec(format!(
            "mc.clearset {o} clear={} set={}",
            list_of(&c),
            list_of(&s)
        ));
        let ob = self.om(o);
        for i in c {
            mc_bit(&mut ob.mc, i, false);
        }
        for i in s {
            mc_bit(&mut ob.mc, i, true);
        }
    }
    fn obj_set_model_condition(&mut self, o: ObjectId, set: &[ModelConditionFlagType]) {
        let s = status_list(set);
        self.rec(format!("mc.set {o} set={}", list_of(&s)));
        let ob = self.om(o);
        for i in s {
            mc_bit(&mut ob.mc, i, true);
        }
    }
    fn obj_set_status(&mut self, o: ObjectId, bits: &[ObjectStatusTypes]) {
        let v = status_list(bits);
        self.rec(format!("status {o} {} set=1", list_of(&v)));
        let ob = self.om(o);
        for i in v {
            ob.status |= 1u64 << i;
        }
    }
    fn obj_has_drawable(&self, o: ObjectId) -> bool {
        self.tmpls[self.o(o).tmpl].flags & 2 != 0
    }
    fn drawable_set_animation_loop_duration(&mut self, o: ObjectId, frames: u32) {
        self.rec(format!("draw.anim {o} {frames}"));
    }
    fn obj_has_contain(&self, o: ObjectId) -> bool {
        self.tmpls[self.o(o).tmpl].flags & 8 != 0
    }
    fn contain_on_selling(&mut self, o: ObjectId) {
        self.rec(format!("contain.sell {o}"));
    }
    fn obj_has_production(&self, o: ObjectId) -> bool {
        self.tmpls[self.o(o).tmpl].flags & 4 != 0
    }
    fn production_special_power_construction_template(&self, o: ObjectId) -> Option<TemplateRef> {
        let s = self.o(o).sp_cons;
        if s < 0 {
            None
        } else {
            Some(s as TemplateRef)
        }
    }
    fn production_can_queue_create_unit(&mut self, o: ObjectId, _what: TemplateRef) -> CanMakeType {
        match self.o(o).can_queue {
            0 => CanMakeType::Ok,
            1 => CanMakeType::NoPrereq,
            2 => CanMakeType::NoMoney,
            3 => CanMakeType::FactoryIsDisabled,
            4 => CanMakeType::QueueFull,
            5 => CanMakeType::ParkingPlacesFull,
            _ => CanMakeType::MaxedOutForPlayer,
        }
    }
    fn production_cancel_and_refund_all(&mut self, o: ObjectId) {
        self.rec(format!("prod.cancel {o}"));
    }
    fn obj_first_parking_place(&self, o: ObjectId) -> Option<usize> {
        if (self.tmpls[self.o(o).tmpl].flags >> 4) & 1 != 0 {
            Some(0)
        } else {
            None
        }
    }
    fn parking_kill_all_parked_units(&mut self, o: ObjectId, module: usize) {
        self.rec(format!("parking.kill {o} {module}"));
    }
    fn obj_create_module_count(&self, o: ObjectId) -> usize {
        ((self.tmpls[self.o(o).tmpl].flags >> 5) & 7) as usize
    }
    fn create_module_on_build_complete(&mut self, o: ObjectId, n: usize) {
        self.rec(format!("oncreate.done {o} {n}"));
    }
    fn obj_handle_partition_cell_maintenance(&mut self, o: ObjectId) {
        self.rec(format!("partition.maint {o}"));
    }
    fn obj_set_producer(&mut self, o: ObjectId, producer: Option<ObjectId>) {
        let p = producer.unwrap_or(0);
        self.om(o).producer = if producer.is_some() { p } else { INVALID_ID };
        self.rec(format!("producer {o} {p}"));
    }
    fn obj_set_position(&mut self, o: ObjectId, pos: Coord3D) {
        self.rec(format!(
            "pos {o} {:08x} {:08x} {:08x}",
            pos.x.to_bits(),
            pos.y.to_bits(),
            pos.z.to_bits()
        ));
        self.om(o).pos = pos;
    }
    fn obj_set_orientation(&mut self, o: ObjectId, angle: f32) {
        self.rec(format!("orient {o} {:08x}", angle.to_bits()));
        self.om(o).orient = angle;
    }
    fn thing_factory_new_object(
        &mut self,
        what: TemplateRef,
        owner: PlayerIndex,
        status: &[ObjectStatusTypes],
    ) -> ObjectId {
        let id = self.make_object(what as usize, Some(owner as usize));
        let v = status_list(status);
        for i in &v {
            self.om(id).status |= 1u64 << i;
        }
        self.rec(format!(
            "new tmpl={what} owner={owner} status={} -> {id}",
            list_of(&v)
        ));
        id
    }
    fn pathfinder_add_object(&mut self, o: ObjectId) {
        self.rec(format!("pf.add {o}"));
    }
    fn find_command_set(&self, o: ObjectId) -> Option<u32> {
        let c = self.o(o).cmd_set;
        if c < 0 || c as usize >= self.sets.len() {
            None
        } else {
            Some(c as u32)
        }
    }
    fn command_set_button(&self, set: u32, i: i32) -> Option<u32> {
        // a button is named by (set, slot): set * 32 + slot
        if self.sets[set as usize][i as usize].is_some() {
            Some(set * 32 + i as u32)
        } else {
            None
        }
    }
    fn button_command_type(&self, button: u32) -> i32 {
        self.sets[(button / 32) as usize][(button % 32) as usize]
            .unwrap()
            .0
    }
    fn button_template(&self, button: u32) -> TemplateRef {
        self.sets[(button / 32) as usize][(button % 32) as usize]
            .unwrap()
            .1 as TemplateRef
    }
    fn tmpl_geometry(&self, t: TemplateRef) -> GeometryInfo {
        self.tmpls[t as usize].geom.clone()
    }
    fn tmpl_is_kind_of(&self, t: TemplateRef, k: KindOfType) -> bool {
        self.tmpls[t as usize].kinds.contains(&k)
    }
    fn tmpl_factory_exit_width(&self, t: TemplateRef) -> f32 {
        self.tmpls[t as usize].exit_width
    }
    fn tmpl_factory_extra_bib_width(&self, t: TemplateRef) -> f32 {
        self.tmpls[t as usize].extra_bib
    }
    fn tmpl_vision_range(&self, t: TemplateRef) -> f32 {
        self.tmpls[t as usize].vision
    }
    fn tmpl_refund_value(&self, t: TemplateRef) -> i32 {
        self.tmpls[t as usize].refund
    }
    fn tmpl_calc_cost_to_build(&self, t: TemplateRef, _player: PlayerIndex) -> i32 {
        self.tmpls[t as usize].cost
    }
    fn tmpl_is_equivalent_to(&self, a: TemplateRef, b: TemplateRef) -> bool {
        a == b || self.tmpls[a as usize].equiv == self.tmpls[b as usize].equiv
    }
    fn audio_add_voice_created(&mut self, t: TemplateRef, o: ObjectId) {
        self.rec(format!("audio {t} {o}"));
    }
    fn player_type(&self, p: PlayerIndex) -> PlayerType {
        if self.players[p as usize].computer {
            PlayerType::Computer
        } else {
            PlayerType::Human
        }
    }
    fn player_is_skirmish_ai(&self, p: PlayerIndex) -> bool {
        self.players[p as usize].skirmish
    }
    fn player_has_default_team(&self, p: PlayerIndex) -> bool {
        self.players[p as usize].has_team
    }
    fn player_relationship_to_object(&self, p: PlayerIndex, them: ObjectId) -> Relationship {
        match self.o(them).player {
            None => Relationship::Neutral,
            Some(q) => rel_of(self.rel[p as usize][q]),
        }
    }
    fn player_can_build(&mut self, p: PlayerIndex, t: TemplateRef) -> bool {
        (self.players[p as usize].can_mask >> t) & 1 != 0
    }
    fn player_can_build_more_of_type(&mut self, p: PlayerIndex, t: TemplateRef) -> bool {
        (self.players[p as usize].more_mask >> t) & 1 != 0
    }
    fn player_count_money(&self, p: PlayerIndex) -> u32 {
        self.players[p as usize].money
    }
    fn player_deposit(&mut self, p: PlayerIndex, amount: u32) {
        self.rec(format!("deposit {p} {amount}"));
        let m = &mut self.players[p as usize].money;
        *m = m.wrapping_add(amount);
    }
    fn player_on_structure_created(
        &mut self,
        p: PlayerIndex,
        constructor: Option<ObjectId>,
        o: ObjectId,
    ) {
        self.rec(format!(
            "p.structcreated {p} {} {o}",
            constructor.unwrap_or(0)
        ));
    }
    fn player_on_structure_construction_complete(
        &mut self,
        p: PlayerIndex,
        constructor: Option<ObjectId>,
        o: ObjectId,
    ) {
        self.rec(format!("p.structdone {p} {} {o}", constructor.unwrap_or(0)));
    }
    fn player_on_unit_created(
        &mut self,
        p: PlayerIndex,
        constructor: Option<ObjectId>,
        o: ObjectId,
    ) {
        self.rec(format!(
            "p.unitcreated {p} {} {o}",
            constructor.unwrap_or(0)
        ));
    }
    fn terrain_ground_height(&self, x: f32, y: f32) -> f32 {
        (h3(cell_of(x) as u32, cell_of(y) as u32, self.seed_h) & 7) as f32 * 0.75
    }
    fn terrain_extent(&self) -> Region3D {
        self.extent
    }
    fn terrain_maximum_pathfind_extent(&self) -> Region3D {
        self.pf_extent
    }
    fn terrain_layer_for_destination(&self, pos: Coord3D) -> PathfindLayer {
        let seed = self.seed_h ^ 0x4444;
        if self.layer_density > 0
            && h3(cell_of(pos.x) as u32, cell_of(pos.y) as u32, seed)
                .is_multiple_of(self.layer_density)
        {
            15
        } else {
            1
        }
    }
    fn terrain_tile_restricts_construction(&self, x: f32, y: f32) -> Option<bool> {
        let hh = h3(cell_of(x) as u32, cell_of(y) as u32, self.seed_h ^ 0x1111);
        if hh.is_multiple_of(7) {
            return None;
        }
        Some(self.tile_density > 0 && hh.is_multiple_of(self.tile_density))
    }
    fn terrain_visual_add_faction_bib(&mut self, o: ObjectId, highlight: bool, extra: f32) {
        self.rec(format!(
            "bib {o} {} {:08x}",
            highlight as i32,
            extra.to_bits()
        ));
    }
    fn terrain_visual_remove_trees_and_props(
        &mut self,
        pos: Coord3D,
        geom: &GeometryInfo,
        angle: f32,
    ) {
        self.rec(format!(
            "trees {:08x} {:08x} {:08x} geom={},{:08x},{:08x} angle={:08x}",
            pos.x.to_bits(),
            pos.y.to_bits(),
            pos.z.to_bits(),
            geom.get_geom_type() as i32,
            geom.get_major_radius().to_bits(),
            geom.get_minor_radius().to_bits(),
            angle.to_bits()
        ));
    }
    fn partition_iterate_potential_collisions(
        &mut self,
        pos: Coord3D,
        geom: &GeometryInfo,
        _angle: f32,
    ) -> Vec<ObjectId> {
        let mut max_dist = geom.get_bounding_circle_radius();
        max_dist *= 1.1;
        self.objs
            .iter()
            .filter(|o| !o.removed && !o.destroyed)
            .filter(|o| {
                let r = max_dist + o.geom.get_bounding_circle_radius();
                dist2(&pos, &o.pos) <= r * r
            })
            .map(|o| o.id)
            .collect()
    }
    fn partition_structures_in_range(&mut self, pos: Coord3D, range: f32) -> Vec<ObjectId> {
        self.objs
            .iter()
            .filter(|o| !o.removed && !o.destroyed && self.structure_filter(o, KINDOF_STRUCTURE))
            .filter(|o| {
                let r = range + o.geom.get_bounding_circle_radius();
                dist2(&pos, &o.pos) <= r * r
            })
            .map(|o| o.id)
            .collect()
    }
    fn partition_closest_supply_source(&mut self, pos: Coord3D, range: f32) -> Option<ObjectId> {
        let mut best: Option<(ObjectId, f32)> = None;
        for o in self.objs.iter().filter(|o| {
            !o.removed && !o.destroyed && self.structure_filter(o, KINDOF_SUPPLY_SOURCE)
        }) {
            let r = range + o.geom.get_bounding_circle_radius();
            let d = dist2(&pos, &o.pos);
            if d <= r * r && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((o.id, d));
            }
        }
        best.map(|(id, _)| id)
    }
    fn partition_geom_collides_with_geom(
        &self,
        pos1: Coord3D,
        geom1: &GeometryInfo,
        _a1: f32,
        pos2: Coord3D,
        geom2: &GeometryInfo,
        _a2: f32,
    ) -> bool {
        let r = geom1.get_bounding_circle_radius() + geom2.get_bounding_circle_radius();
        dist2(&pos1, &pos2) < r * r
    }
    fn partition_world_to_cell(&self, x: f32, y: f32) -> (i32, i32) {
        (cell_of(x), cell_of(y))
    }
    fn partition_shroud_status_for_player(
        &self,
        player_index: i32,
        x: i32,
        y: i32,
    ) -> CellShroudStatus {
        if self.shroud_density == 0 {
            return CellShroudStatus::Clear;
        }
        let hh = h3(
            x as u32,
            y as u32,
            (self.seed_h ^ 0x3333).wrapping_add(player_index as u32),
        );
        if !hh.is_multiple_of(self.shroud_density) {
            return CellShroudStatus::Clear;
        }
        if (hh >> 9) & 1 != 0 {
            CellShroudStatus::Fogged
        } else {
            CellShroudStatus::Shrouded
        }
    }
    fn pathfinder_cell_type(
        &self,
        _layer: PathfindLayer,
        x: i32,
        y: i32,
    ) -> Option<PathfindCellType> {
        let hh = h3(x as u32, y as u32, self.seed_h ^ 0x2222);
        if hh.is_multiple_of(23) {
            return None;
        }
        if self.cell_density > 0 && hh.is_multiple_of(self.cell_density) {
            Some(match ((hh >> 8) % 6) + 1 {
                1 => PathfindCellType::Water,
                2 => PathfindCellType::Cliff,
                3 => PathfindCellType::Rubble,
                4 => PathfindCellType::Obstacle,
                5 => PathfindCellType::BridgeImpassable,
                _ => PathfindCellType::Impassable,
            })
        } else {
            Some(PathfindCellType::Clear)
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// the operations (the mirror of doOp in ba_driver.h)
// ---------------------------------------------------------------------------------------------------------------------

struct Rd<'a> {
    a: &'a [u32; 16],
    p: usize,
}
impl Rd<'_> {
    fn u(&mut self) -> u32 {
        let v = self.a[self.p];
        self.p += 1;
        v
    }
    fn i(&mut self) -> i32 {
        self.u() as i32
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.u())
    }
    fn c3(&mut self) -> Coord3D {
        let x = self.f();
        let y = self.f();
        let z = self.f();
        Coord3D { x, y, z }
    }
}

fn hash_bytes(v: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for &b in v {
        h = (h ^ u32::from(b)).wrapping_mul(16777619);
    }
    h
}

impl Sim {
    fn sel(&self, s: u32) -> Option<ObjectId> {
        if s == 0 || self.objs.is_empty() {
            return None;
        }
        Some(self.objs[((s - 1) as usize) % self.objs.len()].id)
    }

    fn alive(&self, o: Option<ObjectId>) -> bool {
        o.is_some_and(|o| !self.o(o).removed)
    }

    fn do_op(&mut self, op: u32, a: &[u32; 16]) -> Result<(), BuildError> {
        let mut q = Rd { a, p: 0 };
        let n_t = self.tmpls.len();
        let n_p = self.players.len();
        // `ba` is taken out of the sim while an operation runs (the host is the sim itself)
        let mut ba = std::mem::take(&mut self.ba);
        let r = self.do_op_inner(op, &mut q, n_t, n_p, &mut ba);
        self.ba = ba;
        r
    }

    fn do_op_inner(
        &mut self,
        op: u32,
        q: &mut Rd<'_>,
        n_t: usize,
        n_p: usize,
        ba: &mut BuildAssistant,
    ) -> Result<(), BuildError> {
        match op {
            0 => {
                build_assistant_init(self, ba);
                self.rec("ret init".into());
            }
            1 => {
                build_assistant_reset(ba);
                self.rec("ret reset".into());
            }
            2 => {
                let reps = 1 + q.u() % 160;
                for _ in 0..reps {
                    self.frame = self.frame.wrapping_add(1);
                    build_assistant_update(self, ba);
                }
                self.rec(format!("ret update {reps}"));
            }
            3 => {
                let mut x = XferSave::new();
                x.open(b"ba").unwrap();
                build_assistant_xfer_the_sell_list(ba, &mut x)?;
                x.close().unwrap();
                let bytes = x.take_bytes();
                self.rec(format!(
                    "ret save len={} hash={:08x}",
                    bytes.len(),
                    hash_bytes(&bytes)
                ));
                self.saved = Some(bytes);
            }
            4 => match self.saved.clone() {
                None => self.rec("skip nosave".into()),
                Some(bytes) => {
                    let mut x = XferLoad::new(bytes);
                    x.open(b"ba").unwrap();
                    build_assistant_xfer_the_sell_list(ba, &mut x)?;
                    let _ = x.close();
                    self.rec("ret load".into());
                }
            },
            5 | 6 => {
                let cs = q.u();
                let ti = q.u();
                let pos = q.c3();
                let angle = q.f();
                let pl = q.u();
                let end = q.c3();
                let c = self.sel(cs);
                if (cs != 0 && !self.alive(c)) || n_t == 0 || n_p == 0 {
                    self.rec("skip".into());
                    return Ok(());
                }
                let owner = (pl as usize % n_p) as PlayerIndex;
                let t = (ti as usize % n_t) as TemplateRef;
                if op == 5 {
                    let o = build_assistant_build_object_now(self, c, t, pos, angle, owner)?;
                    self.rec(format!("ret build {}", o.unwrap_or(0)));
                } else {
                    build_assistant_build_object_line_now(self, ba, c, t, pos, end, angle, owner)?;
                    self.rec("ret buildline".into());
                }
            }
            7 | 8 => {
                let pos = q.c3();
                let ti = q.u();
                let angle = q.f();
                let options = q.u();
                let bs = q.u();
                let ps = q.u();
                let b = self.sel(bs);
                if (bs != 0 && !self.alive(b)) || n_t == 0 || n_p == 0 {
                    self.rec("skip".into());
                    return Ok(());
                }
                let pl = if ps == 0 {
                    None
                } else {
                    Some(((ps - 1) as usize % n_p) as PlayerIndex)
                };
                let t = (ti as usize % n_t) as TemplateRef;
                if op == 7 {
                    let c = build_assistant_is_location_legal_to_build(
                        self, pos, t, angle, options, b, pl,
                    );
                    self.rec(format!("ret legal {}", c as i32));
                } else {
                    let c = build_assistant_is_location_clear_of_objects(
                        self, pos, t, angle, b, options, pl,
                    );
                    self.rec(format!("ret clear {}", c as i32));
                }
            }
            9 => {
                let pos = q.c3();
                let ti = q.u();
                if n_t == 0 {
                    self.rec("skip".into());
                    return Ok(());
                }
                build_assistant_add_bibs(self, pos, (ti as usize % n_t) as TemplateRef);
                self.rec("ret bibs".into());
            }
            10 => {
                let ti = q.u();
                let angle = q.f();
                let s = q.c3();
                let e = q.c3();
                let tiling = q.f();
                let max_tiles = q.i();
                let bs = q.u();
                let b = self.sel(bs);
                if (bs != 0 && !self.alive(b)) || n_t == 0 {
                    self.rec("skip".into());
                    return Ok(());
                }
                let info = build_assistant_build_tiled_locations(
                    self,
                    ba,
                    (ti as usize % n_t) as TemplateRef,
                    angle,
                    s,
                    e,
                    tiling,
                    max_tiles,
                    b,
                )?;
                let ps: Vec<String> = info
                    .positions
                    .iter()
                    .take(info.tiles_used.max(0) as usize)
                    .map(|c| {
                        format!(
                            "{:08x},{:08x},{:08x}",
                            c.x.to_bits(),
                            c.y.to_bits(),
                            c.z.to_bits()
                        )
                    })
                    .collect();
                self.rec(format!(
                    "ret tiled used={} [{}]",
                    info.tiles_used,
                    ps.join(" ")
                ));
            }
            11 => {
                let ti = q.u();
                if n_t == 0 {
                    self.rec("skip".into());
                    return Ok(());
                }
                let r = build_assistant_is_line_build_template(
                    self,
                    (ti as usize % n_t) as TemplateRef,
                );
                self.rec(format!("ret isline {}", r as i32));
            }
            12 | 13 => {
                let bs = q.u();
                let ti = q.u();
                let b = self.sel(bs);
                if !self.alive(b) || n_t == 0 {
                    self.rec("skip".into());
                    return Ok(());
                }
                let b = b.unwrap();
                let t = (ti as usize % n_t) as TemplateRef;
                if op == 12 {
                    let r = build_assistant_can_make_unit(self, b, t)?;
                    self.rec(format!("ret canmake {}", r as i32));
                } else {
                    let r = build_assistant_is_possible_to_make_unit(self, b, t)?;
                    self.rec(format!("ret possible {}", r as i32));
                }
            }
            14 => {
                let o = self.sel(q.u());
                if !self.alive(o) {
                    self.rec("skip".into());
                    return Ok(());
                }
                build_assistant_sell_object(self, ba, o.unwrap());
                self.rec("ret sell".into());
            }
            15 => {
                for i in 0..self.objs.len() {
                    if !self.objs[i].removed && self.objs[i].destroyed {
                        self.objs[i].removed = true;
                        let id = self.objs[i].id;
                        self.rec(format!("removed {id}"));
                    }
                }
            }
            16 => {
                let o = self.sel(q.u());
                let which = q.u() % 6;
                if !self.alive(o) {
                    self.rec("skip".into());
                    return Ok(());
                }
                let id = o.unwrap();
                match which {
                    0 => {
                        let p = q.c3();
                        self.om(id).pos = p;
                    }
                    1 => {
                        let lo = q.u();
                        let hi = q.u();
                        self.om(id).status = u64::from(lo) | (u64::from(hi) << 32);
                    }
                    2 => {
                        let d = q.u() != 0;
                        let dead = q.u() != 0;
                        let sb = q.u() as u8;
                        let ob = self.om(id);
                        ob.disabled = d;
                        ob.dead = dead;
                        ob.script_bits = sb;
                    }
                    3 => {
                        let v = q.f();
                        self.om(id).cpct = v;
                    }
                    4 => {
                        let b = q.u() != 0;
                        let qp = q.u() != 0;
                        let ob = self.om(id);
                        ob.ai_busy = b;
                        ob.quick_path = qp;
                    }
                    _ => {
                        if n_p > 0 {
                            let p = q.u() as usize;
                            let owner = if p % (n_p + 1) == n_p {
                                None
                            } else {
                                Some(p % (n_p + 1))
                            };
                            self.om(id).player = owner;
                        }
                    }
                }
                self.rec(format!("ret set {which}"));
            }
            17 => {
                if n_p == 0 {
                    self.rec("skip".into());
                    return Ok(());
                }
                let p = q.u() as usize % n_p;
                let which = q.u() % 4;
                let v = q.u();
                let pl = &mut self.players[p];
                match which {
                    0 => pl.money = v,
                    1 => pl.can_mask = v,
                    2 => pl.more_mask = v,
                    _ => pl.has_team = v & 1 != 0,
                }
                self.rec(format!("ret setplayer {which}"));
            }
            18 => {
                let v = q.u() % 500;
                self.frame = self.frame.wrapping_add(v);
                self.rec("ret jump".into());
            }
            _ => self.rec("ret nop".into()),
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// the generator
// ---------------------------------------------------------------------------------------------------------------------

fn chance(g: &mut Gen, percent: u32) -> bool {
    g.below(100) < percent
}

fn gen_status(g: &mut Gen) -> u64 {
    if chance(g, 35) {
        (if chance(g, 50) {
            1u64 << OBJECT_STATUS_STEALTHED
        } else {
            0
        }) | (if chance(g, 40) {
            1u64 << OBJECT_STATUS_DETECTED
        } else {
            0
        }) | (if chance(g, 20) {
            1u64 << OBJECT_STATUS_DISGUISED
        } else {
            0
        }) | (if chance(g, 25) {
            1u64 << OBJECT_STATUS_IS_USING_ABILITY
        } else {
            0
        })
    } else {
        0
    }
}

/// A third of the placement queries ask for a template with a factory exit, for a skirmish AI player when there is one (the padding of
/// `isLocationClearOfObjects`), with the overlap flags that reach the exit rectangles.
fn directed_exit_case(
    g: &mut Gen,
    a: &mut [u32; 16],
    tmpls: &[TmplCfg],
    players: &[PlayerCfg],
    objs: &[ObjCfg],
) {
    if !chance(g, 35) {
        return;
    }
    let with_exit: Vec<u32> = (0..tmpls.len() as u32)
        .filter(|k| tmpls[*k as usize].exit_width > 0.0)
        .collect();
    let skirmish: Vec<u32> = (0..players.len() as u32)
        .filter(|k| players[*k as usize].skirmish != 0)
        .collect();
    if !with_exit.is_empty() {
        a[3] = with_exit[g.below(with_exit.len() as u32) as usize];
    }
    if !skirmish.is_empty() {
        a[7] = 1 + skirmish[g.below(skirmish.len() as u32) as usize];
    }
    a[5] = NO_OBJECT_OVERLAP | (a[5] & IGNORE_STEALTHED);
    // ... placed at a random distance and bearing from an immobile structure, where its exit rectangle may reach the structure
    // without the padded footprint doing so
    let immobile: Vec<&ObjCfg> = objs
        .iter()
        .filter(|o| {
            let m = tmpls[o.tmpl as usize % tmpls.len()].kind_mask;
            m & (1 << 7) != 0 && m & (1 << 1) != 0
        })
        .collect();
    if !immobile.is_empty() {
        let o = immobile[g.below(immobile.len() as u32) as usize];
        let d = g.uniform(5.0, 140.0);
        let th = g.uniform(0.0, std::f32::consts::TAU);
        a[0] = (o.pos[0] + d * th.cos()).to_bits();
        a[1] = (o.pos[1] + d * th.sin()).to_bits();
    }
}

fn gen_prog(seed: u64) -> Prog {
    let mut g = Gen::new(seed);
    let g = &mut g;
    let n_t = 2 + g.below(5) as usize;
    let n_p = 1 + g.below(4) as usize;
    let tmpls: Vec<TmplCfg> = (0..n_t)
        .map(|_| {
            let mut mask = 0u32;
            for b in 0..11 {
                // dozer, structure, inert, shrubbery, cleared_by_build, mine, always_selectable, immobile, linebuild, cannot_build_near_supplies, supply_source
                let p = [18, 55, 8, 10, 10, 8, 8, 35, 20, 15, 15][b as usize];
                if chance(g, p) {
                    mask |= 1 << b;
                }
            }
            let geom_type = g.below(3);
            TmplCfg {
                kind_mask: mask,
                geom_type,
                small: g.below(2),
                height: g.uniform(2.0, 30.0),
                major: g.uniform(4.0, 40.0),
                minor: g.uniform(4.0, 30.0),
                exit_width: if chance(g, 40) {
                    0.0
                } else {
                    g.uniform(5.0, 100.0)
                },
                extra_bib: if chance(g, 45) {
                    0.0
                } else {
                    g.uniform(1.0, 45.0)
                },
                vision: g.uniform(30.0, 300.0),
                refund: if chance(g, 65) {
                    0
                } else {
                    g.below(2000) as i32
                },
                cost: if chance(g, 4) {
                    -(g.below(500) as i32)
                } else {
                    g.below(3000) as i32
                },
                equiv: g.below(4),
                flags: (if chance(g, 55) { 1 } else { 0 })
                    | (if chance(g, 55) { 2 } else { 0 })
                    | (if chance(g, 40) { 4 } else { 0 })
                    | (if chance(g, 30) { 8 } else { 0 })
                    | (if chance(g, 20) { 16 } else { 0 })
                    | (g.below(4) << 5),
            }
        })
        .collect();
    let players: Vec<PlayerCfg> = (0..n_p)
        .map(|_| PlayerCfg {
            computer: g.below(2),
            skirmish: if chance(g, 50) { 1 } else { 0 },
            has_team: if chance(g, 94) { 1 } else { 0 },
            money: if chance(g, 30) {
                g.below(300)
            } else {
                g.below(10000)
            },
            can_mask: if chance(g, 15) { 0 } else { g.u32() },
            more_mask: if chance(g, 15) { 0 } else { g.u32() },
        })
        .collect();
    let rel: Vec<u32> = (0..n_p * n_p).map(|_| g.below(3)).collect();
    let n_cs = g.below(4) as usize;
    let sets: Vec<Vec<Option<(i32, u32)>>> = (0..n_cs)
        .map(|_| {
            (0..18)
                .map(|_| {
                    if chance(g, 35) {
                        Some(([1, 3, 3, 5, 2][g.below(5) as usize], g.below(n_t as u32)))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();
    let n_o = g.below(15) as usize;
    let size = if chance(g, 30) { 120.0 } else { 300.0 };
    let objs: Vec<ObjCfg> = (0..n_o)
        .map(|_| ObjCfg {
            tmpl: g.below(n_t as u32),
            owner: {
                let extra = if chance(g, 6) { 1 } else { 0 };
                g.below(n_p as u32 + extra)
            },
            pos: [
                g.uniform(0.0, size),
                g.uniform(0.0, size),
                g.uniform(0.0, 6.0),
            ],
            orient: g.uniform(0.0, std::f32::consts::TAU),
            status: gen_status(g),
            disabled: if chance(g, 12) { 1 } else { 0 },
            dead: if chance(g, 12) { 1 } else { 0 },
            script_bits: if chance(g, 15) { g.below(4) } else { 0 },
            ai_busy: if chance(g, 20) { 1 } else { 0 },
            cmd_set: g.below(n_cs as u32 + 1),
            quick_path: if chance(g, 65) { 1 } else { 0 },
            cpct: if chance(g, 70) {
                -1.0
            } else {
                g.uniform(0.0, 100.0)
            },
            producer: if chance(g, 70) { 0 } else { 1 + g.below(14) },
            sp_cons: if chance(g, 75) {
                -1
            } else {
                g.below(n_t as u32) as i32
            },
            can_queue: if chance(g, 55) { 0 } else { g.below(7) },
            geom_override: if chance(g, 30) { 1 } else { 0 },
            geom_h: g.uniform(2.0, 30.0),
            geom_major: g.uniform(3.0, 40.0),
            geom_minor: g.uniform(3.0, 30.0),
            geom_type: g.below(3),
        })
        .collect();
    let ext_hi = if chance(g, 30) { 200.0 } else { 400.0 };
    let n_ops = 25 + g.below(40) as usize;
    let mut ops: Vec<(u32, [u32; 16])> = Vec::new();
    // most programs start with the assistant initialised, as the game does
    if chance(g, 94) {
        ops.push((0, [0; 16]));
    }
    while ops.len() < n_ops {
        let kind = g.below(100);
        let mut a = [0u32; 16];
        for x in a.iter_mut() {
            *x = g.u32();
        }
        let near_obj = |g: &mut Gen, objs: &[ObjCfg]| -> [f32; 3] {
            if objs.is_empty() || chance(g, 40) {
                [
                    g.uniform(-20.0, ext_hi + 20.0),
                    g.uniform(-20.0, ext_hi + 20.0),
                    g.uniform(0.0, 6.0),
                ]
            } else {
                let o = &objs[g.below(objs.len() as u32) as usize];
                [
                    o.pos[0] + g.uniform(-40.0, 40.0),
                    o.pos[1] + g.uniform(-40.0, 40.0),
                    o.pos[2],
                ]
            }
        };
        let put3 = |a: &mut [u32; 16], at: usize, v: [f32; 3]| {
            for k in 0..3 {
                a[at + k] = v[k].to_bits();
            }
        };
        let op = match kind {
            0..=19 => {
                // legal to build: pos(3) tmpl angle options builder player
                put3(&mut a, 0, near_obj(g, &objs));
                a[4] = g.uniform(0.0, std::f32::consts::TAU).to_bits();
                let mut opt = 0u32;
                for bit in 0..8 {
                    if chance(g, if bit == 5 { 20 } else { 45 }) {
                        opt |= 1 << bit;
                    }
                }
                if chance(g, 15) {
                    opt = NO_ENEMY_OBJECT_OVERLAP;
                }
                a[5] = opt;
                if chance(g, 25) {
                    a[6] = 0;
                }
                if chance(g, 40) {
                    a[7] = 0;
                }
                directed_exit_case(g, &mut a, &tmpls, &players, &objs);
                7
            }
            20..=27 => {
                put3(&mut a, 0, near_obj(g, &objs));
                a[4] = g.uniform(0.0, std::f32::consts::TAU).to_bits();
                a[5] = if chance(g, 30) {
                    NO_ENEMY_OBJECT_OVERLAP
                } else {
                    g.u32() & 0xff
                };
                if chance(g, 25) {
                    a[6] = 0;
                }
                if chance(g, 40) {
                    a[7] = 0;
                }
                directed_exit_case(g, &mut a, &tmpls, &players, &objs);
                8
            }
            28..=39 => {
                // build now: constructor tmpl pos(3) angle player end(3)
                if chance(g, 25) {
                    a[0] = 0;
                }
                put3(&mut a, 2, near_obj(g, &objs));
                a[5] = g.uniform(0.0, std::f32::consts::TAU).to_bits();
                put3(&mut a, 7, near_obj(g, &objs));
                5
            }
            40..=45 => {
                if chance(g, 25) {
                    a[0] = 0;
                }
                let s = near_obj(g, &objs);
                put3(&mut a, 2, s);
                a[5] = g.uniform(0.0, std::f32::consts::TAU).to_bits();
                put3(
                    &mut a,
                    7,
                    [
                        s[0] + g.uniform(-80.0, 80.0),
                        s[1] + g.uniform(-80.0, 80.0),
                        if chance(g, 50) {
                            s[2]
                        } else {
                            s[2] + g.uniform(-30.0, 30.0)
                        },
                    ],
                );
                6
            }
            46..=53 => {
                // tiled: tmpl angle start(3) end(3) tiling maxTiles builder
                a[1] = g.uniform(0.0, std::f32::consts::TAU).to_bits();
                let s = near_obj(g, &objs);
                put3(&mut a, 2, s);
                put3(
                    &mut a,
                    5,
                    [
                        s[0] + g.uniform(-120.0, 120.0),
                        s[1] + g.uniform(-120.0, 120.0),
                        if chance(g, 50) {
                            s[2]
                        } else {
                            s[2] + g.uniform(-30.0, 30.0)
                        },
                    ],
                );
                a[8] = g.uniform(2.0, 40.0).to_bits();
                a[9] = g.below(14);
                if chance(g, 25) {
                    a[10] = 0;
                }
                10
            }
            54..=61 => {
                if chance(g, 8) {
                    a[0] = 0;
                }
                12
            }
            62..=65 => 13,
            66..=73 => 14,
            74..=81 => {
                a[0] = if chance(g, 55) {
                    100 + g.below(70)
                } else {
                    g.below(40)
                };
                2
            }
            82..=84 => {
                put3(&mut a, 0, near_obj(g, &objs));
                9
            }
            85 => 11,
            86 | 87 => 3,
            88 | 89 => 4,
            90..=93 => {
                a[1] = g.below(6);
                match a[1] {
                    0 => put3(&mut a, 2, near_obj(g, &objs)),
                    3 => {
                        a[2] = if chance(g, 35) {
                            0.0f32.to_bits()
                        } else {
                            g.uniform(-60.0, 100.0).to_bits()
                        }
                    }
                    1 => {
                        let s = gen_status(g);
                        a[2] = s as u32;
                        a[3] = (s >> 32) as u32;
                    }
                    2 => {
                        a[2] = g.below(2);
                        a[3] = g.below(2);
                        a[4] = g.below(4);
                    }
                    4 => {
                        a[2] = g.below(2);
                        a[3] = g.below(2);
                    }
                    _ => {}
                }
                16
            }
            94 | 95 => {
                if a[1] % 4 == 0 {
                    a[2] = g.below(8000);
                }
                17
            }
            96 | 97 => 15,
            98 => 18,
            _ => {
                if chance(g, 50) {
                    1
                } else {
                    0
                }
            }
        };
        ops.push((op, a));
    }
    Prog {
        seed: g.u32(),
        max_line: if chance(g, 3) {
            0
        } else {
            1 + g.below(14) as i32
        },
        min_edge: if chance(g, 50) {
            0.0
        } else {
            g.uniform(2.0, 30.0)
        },
        height_var: g.uniform(0.5, 8.0),
        supply_border: if chance(g, 40) {
            0.0
        } else {
            g.uniform(3.0, 40.0)
        },
        sell_pct: g.uniform(0.2, 1.0),
        extent: [0.0, 0.0, 0.0, ext_hi, ext_hi, 20.0],
        pf_extent: [
            if chance(g, 50) { 0.0 } else { 10.0 },
            5.0,
            -3.0,
            ext_hi - 5.0,
            ext_hi - 10.0,
            25.0,
        ],
        seed_h: g.u32(),
        tile_density: if chance(g, 45) { 0 } else { 6 + g.below(60) },
        cell_density: if chance(g, 45) { 0 } else { 8 + g.below(80) },
        shroud_density: if chance(g, 55) { 0 } else { 2 + g.below(12) },
        layer_density: if chance(g, 60) { 0 } else { 5 + g.below(60) },
        frame0: g.below(5000),
        tmpls,
        players,
        rel,
        sets,
        objs,
        ops,
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// the run
// ---------------------------------------------------------------------------------------------------------------------

/// The Rust side of a program: the lines, and when the program reaches an input upstream leaves undefined, the number of
/// operations before it.
fn run_rust(p: &Prog) -> (Vec<String>, Option<usize>) {
    let mut sim = Sim::new(p);
    for (k, (op, a)) in p.ops.iter().enumerate() {
        let before = sim.lines.len();
        if std::env::var_os("BA_OPS").is_some() {
            eprintln!("OP {k} code {op}");
        }
        if let Err(e) = sim.do_op(*op, a) {
            if std::env::var_os("BA_OPS").is_some() {
                eprintln!("  -> {e:?}");
            }
            assert!(
                e.is_undefined(),
                "program failed with a defined error: {e:?}"
            );
            // the C++ crashes where upstream is undefined: the lines of the operation are dropped and the program ends
            sim.lines.truncate(before);
            sim.lines.push(format!("UNDEF op={k}"));
            return (sim.lines, Some(k));
        }
        sim.dump();
    }
    (sim.lines, None)
}

fn cpp_run(w: &[u32]) -> Vec<String> {
    let path = CString::new("unused").unwrap();
    let mut cap = 1 << 20;
    loop {
        let mut buf = vec![0u8; cap];
        let n = unsafe {
            cpp::ref_ba_run(
                w.as_ptr(),
                w.len() as i32,
                path.as_ptr(),
                buf.as_mut_ptr() as *mut std::os::raw::c_char,
                cap as i32,
            )
        } as usize;
        if n <= cap {
            buf.truncate(n);
            return String::from_utf8_lossy(&buf)
                .lines()
                .map(str::to_owned)
                .collect();
        }
        cap = n + 1;
    }
}

fn cases() -> u64 {
    std::env::var("BA_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000)
}

#[test]
fn build_assistant_matches_upstream() {
    let _guard = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("build assistant, trace lines");
    let (mut programs, mut excluded, mut lines) = (0u64, 0u64, 0u64);
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let from: u64 = std::env::var("BA_FROM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let markers = [
        "ret legal 0",
        "ret legal 1",
        "ret legal 2",
        "ret legal 3",
        "ret legal 4",
        "ret legal 5",
        "ret legal 6",
        "ret clear 0",
        "ret clear 3",
        "ret clear 7",
        "ret build ",
        "ret buildline",
        "ret tiled",
        "ret canmake 0",
        "ret canmake 1",
        "ret canmake 2",
        "ret canmake 3",
        "ret canmake 4",
        "ret canmake 5",
        "ret canmake 6",
        "ret possible",
        "ret sell",
        "ret update",
        "ret save",
        "ret load",
        "ret isline",
        "ret bibs",
        "bib ",
        "trees ",
        "ai.move",
        "ai.idle",
        "ai.construct",
        "new ",
        "destroy",
        "deselect",
        "deposit",
        "cpct",
        "mc.clearset",
        "mc.set",
        "status ",
        "contain.sell",
        "prod.cancel",
        "parking.kill",
        "oncreate.done",
        "partition.maint",
        "pf.add",
        "audio",
        "p.structcreated",
        "p.structdone",
        "p.unitcreated",
        "removed",
        "skip",
        "draw.anim",
        "producer",
        "pos ",
        "orient ",
    ];
    for case in from..from + cases() {
        let prog = gen_prog(0xba5e_0000 + case);
        programs += 1;
        let (rust, undefined_at) = run_rust(&prog);
        if undefined_at.is_some() {
            excluded += 1;
        }
        let cpp_lines = cpp_run(&words(&prog, prog.ops.len()));
        lines += rust.len() as u64;
        for l in &rust {
            for m in markers {
                if l.starts_with(m) {
                    *counts.entry(m.to_string()).or_insert(0) += 1;
                }
            }
        }
        check.eq(rust.len(), cpp_lines.len(), || {
            format!("program {case}: line count")
        });
        for (i, (a, b)) in rust.iter().zip(cpp_lines.iter()).enumerate() {
            check.eq(a.as_str(), b.as_str(), || {
                format!("program {case} line {i}")
            });
        }
        if std::env::var_os("BA_SHOW").is_some() {
            if let Some(i) = rust.iter().zip(cpp_lines.iter()).position(|(a, b)| a != b) {
                eprintln!("--- program {case}, first difference at line {i}");
                for k in i.saturating_sub(25)..=i {
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
    println!("RESULT programs {programs}, programs ending in UNDEF (upstream undefined) {excluded}, lines compared {lines}");
    for (m, c) in &counts {
        println!("RESULT   {m:<24} {c}");
    }
    check.finish();
}
