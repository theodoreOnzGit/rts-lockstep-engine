// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::world::partition` (`PartitionManager`)
//! against upstream's own text (commit 0a05454d).
//!
//! # What is compared against what
//!
//! `PartitionManager.cpp` and `PartitionManager.h` are compiled **whole**
//! (`differential/wrap_partition.cpp`, on `differential/shim_partition`):
//! the grid and the ring table, the cell and object records and their
//! intrusive lists, the dirty list, the contact list and the collision pass of
//! `update()`, the three rasterisers, the shroud and the unlook queue, the
//! threat and value maps, `getClosestObjects` and everything built on it, the
//! 33 filters, `findPositionAround`, the cell walks, `crc`. With them, also
//! unmodified, `SimpleObjectIterator.cpp` (its sort asks the object for a build
//! cost) and `Line2D.cpp` (`PointInsideArea2D`), and the real DiscreteCircle,
//! GeometryInfo, BitFlags (kind and status masks), XferCRC and Vector3. The
//! edits to the two PartitionManager files are mechanical (three, asserted
//! once each, listed in `build.rs`).
//!
//! What is a hand-written stand-in on the C++ side, and equally on the Rust
//! side (`ScenarioHost`, `src/logic/world/partition/scenario.rs`): everything
//! the partition manager reaches outside itself, i.e. `Object`, `Player`,
//! `Team`, `PlayerList`, the modules, `Squad`, `ActionManager`, the terrain
//! and pathfinder, the client, the ghost objects, the clock, and the random
//! generator. They answer by the same rules on both sides (each rule is
//! written once in `wrap_partition.cpp` and once in `scenario.rs`), and they
//! LOG every call that crosses the cut; the logs are compared too. So this
//! test compares the partition manager's own logic and its use of the
//! callbacks, not the stand-ins.
//!
//! # Method
//!
//! A **case** is a random world (a map of 160..520 units with cells of
//! 20..50, up to 4 players with a random relationship matrix, 8..45 objects of
//! all three shapes with random kinds, status, flags, containers, stealth) and
//! a **program** of 30..90 operations on one manager: register, unregister,
//! move, dirty, update, set the clock, every query (closest object and range,
//! with 0..3 random filters out of the 33 and every sort order; all objects;
//! potential collisions; the distance and vector functions; relative angle;
//! `isColliding`; `geomCollidesWithGeom`; ground-or-structure height; line of
//! sight; terrain extremes along a line; find-position-around), every shroud
//! operation (reveal, unreveal, cover, uncover, the unlook queue and its
//! rush, the whole-map operations, store/restore of fog), the threat and
//! value maps with their locators, `crc`, and a **full state dump** (the
//! module, dirty and per-cell intersection lists in order, every object's
//! shroudedness, previous and ever-seen, every cell's shroud/threat/cash hash,
//! the pending unlooks). The same program runs on both sides. Compared, word
//! for word (floats as their bits): the result of every operation, the **log
//! of object callbacks** (cell changes in dirty-list order, collisions with
//! their location and normal, ghost creation), and the **log of client calls**
//! (`TheDisplay`/`TheRadar` shroud writes, ghost snapshot calls).
//!
//! Operations that upstream leaves undefined are not sent to the C++: the
//! Rust runs the program first and reports which operations it knows reach an
//! out-of-bounds read of upstream (`getNearestGroupWithValue`'s walk past the
//! last row) or a dangling pointer (`unRegisterGhostObject` of a ghost whose
//! object is alive); those become no-ops on both sides.
//!
//! The inputs reach the places where a different implementation would differ:
//! ties in the visit order (several objects at the same distance, in the same
//! cell, with the same sort key), objects touching several cells and queried
//! once, objects off the map, small and large footprints, rotated boxes,
//! collisions that destroy and unregister an object in the middle of the
//! contact list, shroud counters driven below and above their resting values.
//!
//! **Planted bugs.** Each comparison was also run against one deliberately
//! wrong translation, planted by hand in `src/logic/world/partition/`,
//! observed to fail, and removed; the table is in the results.
//!
//! # Results
//!
//! See the `DIFF` lines of `cargo test --release --features differential
//! --test differential_partition -- --nocapture`, and the summary in
//! `docs/world-design.md`.

#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::{Coord3D, Region3D};
use rts_lockstep_engine::common::game_common::{CommandSourceType, Relationship};
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::kind_of::{
    KindOfMaskType, KINDOF_ALWAYS_VISIBLE, KINDOF_DISGUISER, KINDOF_FS_BASE_DEFENSE, KINDOF_IMMOBILE, KINDOF_INERT, KINDOF_INFANTRY, KINDOF_MINE,
    KINDOF_NO_COLLIDE, KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use rts_lockstep_engine::common::object_status_types::{
    ObjectStatusMaskType, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_NO_COLLISIONS, OBJECT_STATUS_REPULSOR, OBJECT_STATUS_STEALTHED,
};
use rts_lockstep_engine::common::xfer::Snapshot;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::object_iter::IterOrderType;
use rts_lockstep_engine::logic::world::partition::scenario::{sflag, ScenarioHost, ScenarioLog, ScenarioObject};
use rts_lockstep_engine::logic::world::partition::{
    fpf, ContainFacts, DistanceCalculationType, FindPositionOptions, PartitionClientEvent, PartitionFilter, PartitionManager, ShroudStatusStoreRestore,
    StealthFacts, ValueOrThreat,
};
use support::Gen;

const IO: usize = 24;
const FO: usize = 10;

// ---------------------------------------------------------------------------
// The program
// ---------------------------------------------------------------------------

/// One filter, in the encoding both sides read: a type code, eight integer
/// arguments and eight floats (see `wrap_partition.cpp` `buildFilters`).
#[derive(Clone, Copy, Debug)]
struct FilterSpec {
    ty: i32,
    a: [i32; 8],
    f: [f32; 8],
}

#[derive(Clone, Debug)]
enum Op {
    Register(i32),
    Unregister(i32),
    Move { id: i32, x: f32, y: f32, z: f32, orient: f32 },
    Dirty { id: i32, need: i32 },
    Update,
    SetFrame(u32),
    ClosestObj { origin: i32, dc: i32, filters: Vec<FilterSpec>, max: f32 },
    ClosestPos { dc: i32, filters: Vec<FilterSpec>, pos: [f32; 3], max: f32 },
    RangeObj { origin: i32, dc: i32, order: i32, filters: Vec<FilterSpec>, max: f32 },
    RangePos { dc: i32, order: i32, filters: Vec<FilterSpec>, pos: [f32; 3], max: f32 },
    AllObjects { filters: Vec<FilterSpec> },
    PotentialColl { gtype: i32, small: i32, use2d: i32, f: [f32; 7] },
    Dist { kind: i32, a: i32, b: i32, dc: i32, f: [f32; 6] },
    RelAngle { a: i32, b: i32, pos: [f32; 3] },
    IsColliding(i32, i32),
    GeomCollides { g1: i32, g2: i32, f: [f32; 14] },
    GroundStruct(f32, f32),
    Los { a: i32, b: i32, f: [f32; 6] },
    Extremes([f32; 6]),
    Shroud { kind: i32, mask: i32, value: i32, x: f32, y: f32, r: f32 },
    ProcessQueue,
    CellStatus { kind: i32, player: i32, x: f32, y: f32 },
    ObjShroud { id: i32, player: i32 },
    MostValuable { player: i32, types: i32, vt: i32 },
    NearestGroup { player: i32, types: i32, vt: i32, required: i32, greater: i32, pos: [f32; 3] },
    MapOps { kind: i32, player: i32 },
    StoreRestore(i32),
    Crc,
    Dump,
    FindPosition { flags: i32, ignore: i32, src: i32, rel: i32, f: [f32; 7] },
    UnregisterGhost(i32),
    Nop,
}

fn push_filters(ints: &mut Vec<i32>, floats: &mut Vec<f32>, filters: &[FilterSpec]) {
    ints.push(filters.len() as i32);
    for f in filters {
        ints.push(f.ty);
        ints.extend_from_slice(&f.a);
        floats.extend_from_slice(&f.f);
    }
}

impl Op {
    /// The op in the encoding `ref_partition_run` reads: integers and floats in the order the
    /// C++ interpreter consumes them.
    fn encode(&self, ints: &mut Vec<i32>, floats: &mut Vec<f32>) {
        match self {
            Op::Register(id) => ints.extend([0, *id]),
            Op::Unregister(id) => ints.extend([1, *id]),
            Op::Move { id, x, y, z, orient } => {
                ints.extend([2, *id]);
                floats.extend([*x, *y, *z, *orient]);
            }
            Op::Dirty { id, need } => ints.extend([3, *id, *need]),
            Op::Update => ints.push(4),
            Op::SetFrame(f) => ints.extend([5, *f as i32]),
            Op::ClosestObj { origin, dc, filters, max } => {
                ints.extend([6, *origin, *dc]);
                push_filters(ints, floats, filters);
                floats.push(*max);
            }
            Op::ClosestPos { dc, filters, pos, max } => {
                ints.extend([7, *dc]);
                push_filters(ints, floats, filters);
                floats.extend([pos[0], pos[1], pos[2], *max]);
            }
            Op::RangeObj { origin, dc, order, filters, max } => {
                ints.extend([8, *origin, *dc, *order]);
                push_filters(ints, floats, filters);
                floats.push(*max);
            }
            Op::RangePos { dc, order, filters, pos, max } => {
                ints.extend([9, *dc, *order]);
                push_filters(ints, floats, filters);
                floats.extend([pos[0], pos[1], pos[2], *max]);
            }
            Op::AllObjects { filters } => {
                ints.push(10);
                push_filters(ints, floats, filters);
            }
            Op::PotentialColl { gtype, small, use2d, f } => {
                ints.extend([11, *gtype, *small, *use2d]);
                floats.extend_from_slice(f);
            }
            Op::Dist { kind, a, b, dc, f } => {
                ints.extend([12, *kind, *a, *b, *dc]);
                floats.extend_from_slice(f);
            }
            Op::RelAngle { a, b, pos } => {
                ints.extend([13, *a, *b]);
                floats.extend_from_slice(pos);
            }
            Op::IsColliding(a, b) => ints.extend([14, *a, *b]),
            Op::GeomCollides { g1, g2, f } => {
                ints.extend([15, *g1, *g2]);
                floats.extend_from_slice(f);
            }
            Op::GroundStruct(x, y) => {
                ints.push(16);
                floats.extend([*x, *y]);
            }
            Op::Los { a, b, f } => {
                ints.extend([17, *a, *b]);
                floats.extend_from_slice(f);
            }
            Op::Extremes(f) => {
                ints.push(18);
                floats.extend_from_slice(f);
            }
            Op::Shroud { kind, mask, value, x, y, r } => {
                ints.extend([19, *kind, *mask, *value]);
                floats.extend([*x, *y, *r]);
            }
            Op::ProcessQueue => ints.push(20),
            Op::CellStatus { kind, player, x, y } => {
                ints.extend([21, *kind, *player]);
                floats.extend([*x, *y]);
            }
            Op::ObjShroud { id, player } => ints.extend([22, *id, *player]),
            Op::MostValuable { player, types, vt } => ints.extend([23, *player, *types, *vt]),
            Op::NearestGroup { player, types, vt, required, greater, pos } => {
                ints.extend([24, *player, *types, *vt, *required, *greater]);
                floats.extend_from_slice(pos);
            }
            Op::MapOps { kind, player } => ints.extend([25, *kind, *player]),
            Op::StoreRestore(k) => ints.extend([26, *k]),
            Op::Crc => ints.push(27),
            Op::Dump => ints.push(28),
            Op::FindPosition { flags, ignore, src, rel, f } => {
                ints.extend([29, *flags, *ignore, *src, *rel]);
                floats.extend_from_slice(f);
            }
            Op::UnregisterGhost(h) => ints.extend([30, *h]),
            Op::Nop => ints.push(31),
        }
    }
}

// ---------------------------------------------------------------------------
// The scenario, on both sides
// ---------------------------------------------------------------------------

fn kind_words(k: &KindOfMaskType) -> [i32; 4] {
    let w = k.words();
    [w[0] as u32 as i32, (w[0] >> 32) as u32 as i32, w[1] as u32 as i32, (w[1] >> 32) as u32 as i32]
}

fn status_words(s: &ObjectStatusMaskType) -> [i32; 2] {
    let w = s.words();
    [w[0] as u32 as i32, (w[0] >> 32) as u32 as i32]
}

/// The scenario in the encoding of `ref_partition_run`: globals, extent, relationships, objects.
struct EncodedScenario {
    ig: Vec<i32>,
    fg: Vec<f32>,
    rel: Vec<u8>,
    io: Vec<i32>,
    fo: Vec<f32>,
}

fn encode_scenario(h: &ScenarioHost) -> EncodedScenario {
    let ext = h.extent.unwrap_or_default();
    let ig = vec![
        h.frame as i32,
        h.unlook_persist as i32,
        h.player_count,
        h.local_player.unwrap_or(-1),
        i32::from(h.extent.is_some()),
        h.rng as i32,
        h.objects.len() as i32,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    let fg = vec![h.cell_size, ext.lo.x, ext.lo.y, ext.lo.z, ext.hi.x, ext.hi.y, ext.hi.z, 0.0];
    let mut rel = vec![0u8; 256];
    for a in 0..16 {
        for b in 0..16 {
            rel[a * 16 + b] = h.rel[a][b] as u8;
        }
    }
    let mut io = Vec::new();
    let mut fo = Vec::new();
    for o in h.objects.values() {
        let kw = kind_words(&o.kind_of);
        let sw = status_words(&o.status);
        let c = o.contain;
        io.extend([
            o.id,
            o.geometry.get_geom_type() as i32,
            i32::from(o.geometry.get_is_small()),
            o.player.unwrap_or(-1),
            o.team as i32,
            o.flags as i32,
            o.build_cost,
            o.template,
            kw[0],
            kw[1],
            kw[2],
            kw[3],
            sw[0],
            sw[1],
            i32::from(c.is_some()),
            c.map_or(0, |c| i32::from(c.is_garrisonable)),
            c.map_or(0, |c| c.contain_count),
            c.map_or(0, |c| c.stealth_units_contained),
            c.map_or(0, |c| match c.first_contained_detected {
                None => 0,
                Some(false) => 1,
                Some(true) => 2,
            }),
            o.apparent_controller.unwrap_or(-1),
            o.stealth.map_or(0, |s| if s.is_disguised { 2 } else { 1 }),
            o.stealth.map_or(0, |s| s.disguised_player_index),
            0,
            0,
        ]);
        fo.extend([
            o.position.x,
            o.position.y,
            o.position.z,
            o.orientation,
            o.geometry.get_height(),
            o.geometry.get_major_radius(),
            o.geometry.get_minor_radius(),
            o.dir.0,
            o.dir.1,
            o.vision,
        ]);
    }
    assert_eq!(io.len(), IO * h.objects.len());
    assert_eq!(fo.len(), FO * h.objects.len());
    EncodedScenario { ig, fg, rel, io, fo }
}

fn command_source(i: i32) -> CommandSourceType {
    match i {
        0 => CommandSourceType::FromPlayer,
        1 => CommandSourceType::FromScript,
        2 => CommandSourceType::FromAi,
        3 => CommandSourceType::FromDozer,
        _ => CommandSourceType::DefaultSwitchWeapon,
    }
}

fn opt_obj(i: i32) -> Option<i32> {
    if i < 0 {
        None
    } else {
        Some(i)
    }
}

fn opt_player(i: i32) -> Option<i32> {
    if (0..16).contains(&i) {
        Some(i)
    } else {
        None
    }
}

fn kind_mask(a: &[i32]) -> KindOfMaskType {
    let mut m = KindOfMaskType::new();
    for &i in a.iter().take(3) {
        if i >= 0 {
            m.set(i, 1);
        }
    }
    m
}

fn status_mask(a: &[i32]) -> ObjectStatusMaskType {
    let mut m = ObjectStatusMaskType::new();
    for &i in a.iter().take(3) {
        if i >= 0 {
            m.set(i, 1);
        }
    }
    m
}

/// The Rust filter a spec stands for: the same construction `buildFilters` makes in C++.
fn build_filter(spec: &FilterSpec, host: &ScenarioHost) -> PartitionFilter {
    let a = &spec.a;
    let f = &spec.f;
    match spec.ty {
        0 => PartitionFilter::IsFlying,
        1 => PartitionFilter::WouldCollide {
            position: Coord3D { x: f[0], y: f[1], z: f[2] },
            geom: GeometryInfo::new(GeometryType::from_index(a[0]).expect("geometry"), a[1] != 0, f[4], f[5], f[6]),
            angle: f[3],
            desired: a[2] != 0,
        },
        2 => PartitionFilter::SamePlayer { player: opt_player(a[0]) },
        3 => PartitionFilter::Relationship { obj: a[0], flags: a[1] },
        4 => PartitionFilter::AcceptOnTeam { team: a[0] as u32 },
        5 => PartitionFilter::AcceptOnSquad { squad: if a[0] == 1 || a[0] == 2 { Some(a[0]) } else { None } },
        6 => PartitionFilter::LineOfSight { obj: a[0] },
        7 => PartitionFilter::PossibleToAttack { attack_type: a[0], obj: opt_obj(a[1]), command_source: command_source(a[2]) },
        8 => PartitionFilter::PossibleToEnter { obj: opt_obj(a[0]), command_source: command_source(a[1]) },
        9 => PartitionFilter::PossibleToHijack { obj: opt_obj(a[0]), command_source: command_source(a[1]) },
        10 => PartitionFilter::last_attacked_by(host, opt_obj(a[0])),
        11 => PartitionFilter::AcceptByObjectStatus { must_be_set: status_mask(&a[0..3]), must_be_clear: status_mask(&a[3..6]) },
        12 => PartitionFilter::RejectByObjectStatus { must_be_set: status_mask(&a[0..3]), must_be_clear: status_mask(&a[3..6]) },
        13 => PartitionFilter::StealthedAndUndetected { obj: a[0], allow: a[1] != 0 },
        14 => PartitionFilter::AcceptByKindOf { must_be_set: kind_mask(&a[0..3]), must_be_clear: kind_mask(&a[3..6]) },
        15 => PartitionFilter::RejectByKindOf { must_be_set: kind_mask(&a[0..3]), must_be_clear: kind_mask(&a[3..6]) },
        16 => PartitionFilter::RejectBehind { obj: a[0] },
        17 => PartitionFilter::Alive,
        18 => PartitionFilter::SameMapStatus { obj: a[0] },
        19 => PartitionFilter::OnMap,
        20 => PartitionFilter::reject_buildings(host, a[0]),
        21 => PartitionFilter::InsignificantBuildings { allow_non_buildings: a[0] != 0, allow_insignificant: a[1] != 0 },
        22 => PartitionFilter::FreeOfFog { comparison_index: a[0] },
        23 => PartitionFilter::Repulsor { self_obj: a[0] },
        24 => PartitionFilter::IrregularArea { area: (0..4).take(a[0] as usize).map(|i| Coord3D { x: f[2 * i], y: f[2 * i + 1], z: 0.0 }).collect() },
        25 => PartitionFilter::PolygonTrigger { trigger: a[0] },
        26 => PartitionFilter::Player { player: opt_player(a[0]), match_: a[1] != 0 },
        27 => PartitionFilter::PlayerAffiliation { player: a[0], affiliation: a[1] as u32, match_: a[2] != 0 },
        28 => PartitionFilter::Thing { template_key: a[0], match_: a[1] != 0 },
        29 => PartitionFilter::Garrisonable { match_: a[0] != 0 },
        30 => PartitionFilter::GarrisonableByPlayer { player: opt_player(a[0]), match_: a[1] != 0, command_source: command_source(a[2]) },
        31 => PartitionFilter::UnmannedObject { match_: a[0] != 0 },
        32 => PartitionFilter::ValidCommandButtonTarget { source: a[0], command_button: a[1], match_: a[2] != 0, command_source: command_source(a[3]) },
        t => panic!("filter type {t}"),
    }
}

fn dc_of(i: i32) -> DistanceCalculationType {
    DistanceCalculationType::from_index(i).expect("dc")
}

fn order_of(i: i32) -> IterOrderType {
    match i {
        0 => IterOrderType::Fastest,
        1 => IterOrderType::SortedNearToFar,
        2 => IterOrderType::SortedFarToNear,
        3 => IterOrderType::SortedCheapToExpensive,
        _ => IterOrderType::SortedExpensiveToCheap,
    }
}

// ---------------------------------------------------------------------------
// The Rust side of the interpreter
// ---------------------------------------------------------------------------

struct RustRun {
    out: Vec<u32>,
    /// Where each operation's words start in `out`.
    starts: Vec<usize>,
    /// Operations the C++ must not be sent (undefined upstream); they were run anyway.
    flagged: Vec<bool>,
    /// How many log records of each kind the program produced: cell changes, collisions, ghosts
    /// made, shroud writes to the client, ghost calls to the client.
    logs: [u64; 5],
}

fn put_bits(out: &mut Vec<u32>, f: f32) {
    out.push(f.to_bits());
}

fn fnv(h: u32, w: u32) -> u32 {
    (h ^ w).wrapping_mul(16_777_619)
}

fn flush_logs(out: &mut Vec<u32>, host: &mut ScenarioHost, pm: &mut PartitionManager, logs: &mut [u64; 5]) {
    let mut h: Vec<u32> = Vec::new();
    for l in host.log.drain(..) {
        match l {
            ScenarioLog::CellChange(id) => {
                logs[0] += 1;
                h.extend([1, id as u32])
            }
            ScenarioLog::Collide { id, other, loc, normal } => {
                logs[1] += 1;
                h.extend([2, id as u32, other as u32, loc.x.to_bits(), loc.y.to_bits(), loc.z.to_bits(), normal.x.to_bits(), normal.y.to_bits(), normal.z.to_bits()])
            }
            ScenarioLog::Ghost(id) => {
                logs[2] += 1;
                h.extend([3, id as u32])
            }
        }
    }
    out.push(h.len() as u32);
    out.extend(h);
    let mut c: Vec<u32> = Vec::new();
    for e in pm.take_client_events() {
        match e {
            PartitionClientEvent::SetShroudLevel { x, y, status } => {
                logs[3] += 1;
                c.extend([4, x as u32, y as u32, status as u32])
            }
            PartitionClientEvent::ClearShroud => {
                logs[3] += 1;
                c.push(5)
            }
            PartitionClientEvent::GhostSnapShot { ghost, player } => {
                logs[4] += 1;
                c.extend([6, ghost, player as u32])
            }
            PartitionClientEvent::GhostFreeSnapShot { ghost, player } => {
                logs[4] += 1;
                c.extend([7, ghost, player as u32])
            }
            PartitionClientEvent::GhostRemoved { ghost } => {
                logs[4] += 1;
                c.extend([8, ghost])
            }
            PartitionClientEvent::GhostParentGone { ghost } => {
                logs[4] += 1;
                c.extend([9, ghost])
            }
        }
    }
    out.push(c.len() as u32);
    out.extend(c);
}

/// Runs the program on the Rust side, in the encoding of the words the C++ writes.
fn run_rust(scn: &ScenarioHost, ops: &[Op]) -> RustRun {
    let mut host = scn.clone();
    let mut pm = PartitionManager::new();
    pm.init(&host);
    let mut out: Vec<u32> = Vec::new();
    let mut starts = Vec::new();
    let mut flagged = vec![false; ops.len()];
    let mut store = ShroudStatusStoreRestore::default();
    let mut logs = [0u64; 5];
    for (n, op) in ops.iter().enumerate() {
        starts.push(out.len());
        match op {
            Op::Register(id) => pm.register_object(&mut host, *id),
            Op::Unregister(id) => pm.unregister_object(*id),
            Op::Move { id, x, y, z, orient } => {
                let o = host.objects.get_mut(id).expect("object");
                o.position = Coord3D { x: *x, y: *y, z: *z };
                o.orientation = *orient;
                pm.make_object_dirty(*id, true);
            }
            Op::Dirty { id, need } => pm.make_object_dirty(*id, *need != 0),
            Op::Update => pm.update(&mut host),
            Op::SetFrame(f) => host.frame = *f,
            Op::ClosestObj { origin, dc, filters, max } => {
                let fl: Vec<PartitionFilter> = filters.iter().map(|f| build_filter(f, &host)).collect();
                let r = pm.get_closest_object(&host, *origin, *max, dc_of(*dc), &fl);
                out.push(r.object.map_or(0, |i| i as u32));
                put_bits(&mut out, r.dist);
                let v = if r.object.is_some() { r.vec } else { Coord3D::default() };
                out.extend([v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]);
            }
            Op::ClosestPos { dc, filters, pos, max } => {
                let fl: Vec<PartitionFilter> = filters.iter().map(|f| build_filter(f, &host)).collect();
                let p = Coord3D { x: pos[0], y: pos[1], z: pos[2] };
                let r = pm.get_closest_object_at(&host, &p, *max, dc_of(*dc), &fl);
                out.push(r.object.map_or(0, |i| i as u32));
                put_bits(&mut out, r.dist);
                let v = if r.object.is_some() { r.vec } else { Coord3D::default() };
                out.extend([v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]);
            }
            Op::RangeObj { origin, dc, order, filters, max } => {
                let fl: Vec<PartitionFilter> = filters.iter().map(|f| build_filter(f, &host)).collect();
                let mut it = pm.iterate_objects_in_range(&host, *origin, *max, dc_of(*dc), &fl, order_of(*order));
                out.push(it.count() as u32);
                let (mut o, mut num) = it.first_with_numeric();
                while let Some(id) = o {
                    out.extend([id as u32, num.to_bits()]);
                    (o, num) = it.next_with_numeric();
                }
            }
            Op::RangePos { dc, order, filters, pos, max } => {
                let fl: Vec<PartitionFilter> = filters.iter().map(|f| build_filter(f, &host)).collect();
                let p = Coord3D { x: pos[0], y: pos[1], z: pos[2] };
                let mut it = pm.iterate_objects_in_range_at(&host, &p, *max, dc_of(*dc), &fl, order_of(*order));
                out.push(it.count() as u32);
                let (mut o, mut num) = it.first_with_numeric();
                while let Some(id) = o {
                    out.extend([id as u32, num.to_bits()]);
                    (o, num) = it.next_with_numeric();
                }
            }
            Op::AllObjects { filters } => {
                let fl: Vec<PartitionFilter> = filters.iter().map(|f| build_filter(f, &host)).collect();
                let mut it = pm.iterate_all_objects(&host, &fl);
                out.push(it.count() as u32);
                let mut o = it.first();
                while let Some(id) = o {
                    out.push(id as u32);
                    o = it.next();
                }
            }
            Op::PotentialColl { gtype, small, use2d, f } => {
                let p = Coord3D { x: f[0], y: f[1], z: f[2] };
                let g = GeometryInfo::new(GeometryType::from_index(*gtype).expect("geometry"), *small != 0, f[3], f[4], f[5]);
                let mut it = pm.iterate_potential_collisions(&host, &p, &g, f[6], *use2d != 0);
                out.push(it.count() as u32);
                let mut o = it.first();
                while let Some(id) = o {
                    out.push(id as u32);
                    o = it.next();
                }
            }
            Op::Dist { kind, a, b, dc, f } => {
                let pos = Coord3D { x: f[0], y: f[1], z: f[2] };
                let goal = Coord3D { x: f[3], y: f[4], z: f[5] };
                let dc = dc_of(*dc);
                let (d, v) = match kind {
                    0 => (0.0, pm.get_vector_to(&host, *a, *b, dc).unwrap_or_default()),
                    1 => (0.0, pm.get_vector_to_pos(&host, *a, &pos, dc).unwrap_or_default()),
                    2 => pm.get_distance_squared(&host, *a, *b, dc).unwrap_or_default(),
                    3 => pm.get_distance_squared_pos(&host, *a, &pos, dc).unwrap_or_default(),
                    4 => pm.get_goal_distance_squared(&host, *a, &goal, *b, dc).unwrap_or_default(),
                    _ => pm.get_goal_distance_squared_pos(&host, *a, &goal, &pos, dc).unwrap_or_default(),
                };
                out.extend([d.to_bits(), v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]);
            }
            Op::RelAngle { a, b, pos } => {
                let r = if *b >= 0 {
                    pm.get_relative_angle_2d(&host, *a, *b)
                } else {
                    pm.get_relative_angle_2d_pos(&host, *a, &Coord3D { x: pos[0], y: pos[1], z: pos[2] })
                };
                put_bits(&mut out, r.unwrap_or(0.0));
            }
            Op::IsColliding(a, b) => out.push(u32::from(pm.is_colliding(&host, *a, *b))),
            Op::GeomCollides { g1, g2, f } => {
                let p1 = Coord3D { x: f[0], y: f[1], z: f[2] };
                let ge1 = GeometryInfo::new(GeometryType::from_index(*g1).expect("geometry"), false, f[3], f[4], f[5]);
                let p2 = Coord3D { x: f[7], y: f[8], z: f[9] };
                let ge2 = GeometryInfo::new(GeometryType::from_index(*g2).expect("geometry"), false, f[10], f[11], f[12]);
                out.push(u32::from(pm.geom_collides_with_geom(&p1, &ge1, f[6], &p2, &ge2, f[13])));
            }
            Op::GroundStruct(x, y) => {
                let h = pm.get_ground_or_structure_height(&host, *x, *y);
                put_bits(&mut out, h);
            }
            Op::Los { a, b, f } => {
                let pa = Coord3D { x: f[0], y: f[1], z: f[2] };
                let pb = Coord3D { x: f[3], y: f[4], z: f[5] };
                out.push(u32::from(pm.is_clear_line_of_sight_terrain(&host, opt_obj(*a), &pa, opt_obj(*b), &pb)));
            }
            Op::Extremes(f) => {
                let e = pm.estimate_terrain_extremes_along_line(&Coord3D { x: f[0], y: f[1], z: f[2] }, &Coord3D { x: f[3], y: f[4], z: f[5] });
                out.push(u32::from(e.is_valid));
                put_bits(&mut out, e.min_z);
                put_bits(&mut out, e.max_z);
                let mp = e.min_z_pos.unwrap_or(rts_lockstep_engine::common::base_type::Coord2D { x: 0.0, y: 0.0 });
                let xp = e.max_z_pos.unwrap_or(rts_lockstep_engine::common::base_type::Coord2D { x: 0.0, y: 0.0 });
                out.extend([mp.x.to_bits(), mp.y.to_bits(), xp.x.to_bits(), xp.y.to_bits()]);
            }
            Op::Shroud { kind, mask, value, x, y, r } => {
                let m = *mask as u16;
                let v = *value as u32;
                let _ = match kind {
                    0 => pm.do_shroud_reveal(&host, *x, *y, *r, m),
                    1 => pm.undo_shroud_reveal(&host, *x, *y, *r, m),
                    2 => pm.do_shroud_cover(&host, *x, *y, *r, m),
                    3 => pm.undo_shroud_cover(&host, *x, *y, *r, m),
                    4 => {
                        pm.queue_undo_shroud_reveal(&host, *x, *y, *r, m);
                        Ok(())
                    }
                    5 => pm.do_threat_affect(&host, *x, *y, *r, v, m),
                    6 => pm.undo_threat_affect(&host, *x, *y, *r, v, m),
                    7 => pm.do_value_affect(&host, *x, *y, *r, v, m),
                    _ => pm.undo_value_affect(&host, *x, *y, *r, v, m),
                };
            }
            Op::ProcessQueue => pm.process_entire_pending_undo_shroud_reveal_queue(&host),
            Op::CellStatus { kind, player, x, y } => {
                let loc = Coord3D { x: *x, y: *y, z: 0.0 };
                out.push(if *kind == 0 { pm.get_shroud_status_for_player(*player, &loc) as u32 } else { pm.get_prop_shroud_status_for_player(*player, &loc) as u32 });
            }
            Op::ObjShroud { id, player } => out.push(pm.get_object_shrouded_status(&host, *id, *player) as u32),
            Op::MostValuable { player, types, vt } => {
                let vt = if *vt == 1 { ValueOrThreat::CashValue } else { ValueOrThreat::ThreatValue };
                let r = pm.get_most_valuable_location(&host, *player, *types as u32, vt);
                out.push(u32::from(r.is_some()));
                let l = r.unwrap_or_default();
                out.extend([l.x.to_bits(), l.y.to_bits(), l.z.to_bits()]);
            }
            Op::NearestGroup { player, types, vt, required, greater, pos } => {
                let vt = if *vt == 1 { ValueOrThreat::CashValue } else { ValueOrThreat::ThreatValue };
                let (r, undefined) = pm.get_nearest_group_with_value(&host, *player, *types as u32, vt, &Coord3D { x: pos[0], y: pos[1], z: pos[2] }, *required, *greater != 0);
                if undefined {
                    flagged[n] = true;
                }
                out.push(u32::from(r.is_some()));
                let l = r.unwrap_or_default();
                out.extend([l.x.to_bits(), l.y.to_bits(), l.z.to_bits()]);
            }
            Op::MapOps { kind, player } => match kind {
                0 => pm.reveal_map_for_player(&host, *player),
                1 => pm.reveal_map_for_player_permanently(&host, *player),
                2 => pm.undo_reveal_map_for_player_permanently(&host, *player),
                3 => pm.shroud_map_for_player(&host, *player),
                _ => pm.refresh_shroud_for_local_player(&host),
            },
            Op::StoreRestore(kind) => {
                // upstream divides by `m_cellsWide`, which is 0 until a store has been made
                if *kind >= 2 && store.cells_wide <= 0 {
                    flagged[n] = true;
                }
                match kind {
                    0 => pm.store_fogged_cells(&mut store, true),
                    1 => pm.store_fogged_cells(&mut store, false),
                    2 => pm.restore_fogged_cells(&host, &store, true),
                    _ => pm.restore_fogged_cells(&host, &store, false),
                }
                let mut h = 2_166_136_261u32;
                h = fnv(h, store.cells_wide as u32);
                for p in 0..16 {
                    for &b in &store.fogged_or_revealed[p] {
                        h = fnv(h, u32::from(b));
                    }
                }
                out.push(h);
            }
            Op::Crc => {
                let mut x = XferCrc::new_without_nan_guard();
                Snapshot::crc(&mut pm, &mut x).expect("crc");
                out.push(x.get_crc());
            }
            Op::Dump => out.extend(pm.state_words()),
            Op::FindPosition { flags, ignore, src, rel, f } => {
                let opts = FindPositionOptions {
                    flags: *flags as u32,
                    min_radius: f[3],
                    max_radius: f[4],
                    start_angle: f[5],
                    max_z_delta: f[6],
                    ignore_object: opt_obj(*ignore),
                    source_to_path_to_dest: opt_obj(*src),
                    relationship_object: opt_obj(*rel),
                };
                let r = pm.find_position_around(&mut host, &Coord3D { x: f[0], y: f[1], z: f[2] }, &opts);
                out.push(u32::from(r.is_some()));
                let l = r.unwrap_or_default();
                out.extend([l.x.to_bits(), l.y.to_bits(), l.z.to_bits()]);
            }
            Op::UnregisterGhost(h) => {
                if pm.has_objectless_ghost_module(*h as u32) {
                    pm.unregister_ghost_object(*h as u32);
                } else {
                    flagged[n] = true;
                }
            }
            Op::Nop => {}
        }
        flush_logs(&mut out, &mut host, &mut pm, &mut logs);
    }
    RustRun { out, starts, flagged, logs }
}

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

struct Case {
    scn: ScenarioHost,
    ops: Vec<Op>,
}

fn pick<T: Copy>(g: &mut Gen, v: &[T]) -> T {
    v[g.below(v.len() as u32) as usize]
}

fn chance(g: &mut Gen, percent: u32) -> bool {
    g.below(100) < percent
}

fn gen_scenario(g: &mut Gen) -> ScenarioHost {
    let mut h = ScenarioHost::default();
    let cell = pick(g, &[20.0f32, 24.0, 30.0, 33.3, 40.0, 50.0]);
    h.cell_size = cell;
    let lo = pick(g, &[(0.0f32, 0.0f32), (-80.0, -50.0), (25.5, 10.25), (0.0, 0.0)]);
    let w = g.uniform(160.0, 520.0);
    let hh = g.uniform(160.0, 520.0);
    h.extent = Some(Region3D { lo: Coord3D { x: lo.0, y: lo.1, z: 0.0 }, hi: Coord3D { x: lo.0 + w, y: lo.1 + hh, z: 100.0 } });
    h.player_count = 2 + g.below(3) as i32;
    h.local_player = Some(g.below(h.player_count as u32) as i32);
    h.frame = g.below(100);
    h.unlook_persist = pick(g, &[0u32, 3, 30]);
    h.rng = g.u32();
    for a in 0..16 {
        for b in 0..16 {
            h.rel[a][b] = pick(g, &[Relationship::Enemies, Relationship::Neutral, Relationship::Allies]);
        }
    }
    let n = 8 + g.below(38) as i32;
    let ext = h.extent.unwrap();
    let id_stride = pick(g, &[1, 1, 3, 7]);
    for k in 0..n {
        let id = 1 + k * id_stride;
        let inside = chance(g, 88);
        let (x, y) = if inside {
            (g.uniform(ext.lo.x, ext.hi.x), g.uniform(ext.lo.y, ext.hi.y))
        } else {
            (g.uniform(ext.lo.x - 60.0, ext.hi.x + 60.0), g.uniform(ext.lo.y - 60.0, ext.hi.y + 60.0))
        };
        // cluster some objects on a few spots so that ties and shared cells are common
        let (x, y) = if chance(g, 25) {
            let spots = [(ext.lo.x + w * 0.3, ext.lo.y + hh * 0.3), (ext.lo.x + w * 0.7, ext.lo.y + hh * 0.5), (ext.lo.x + w * 0.5, ext.lo.y + hh * 0.8)];
            let s = pick(g, &spots);
            (s.0 + pick(g, &[0.0f32, 0.0, 5.0, -5.0, 12.5]), s.1 + pick(g, &[0.0f32, 0.0, 5.0, -5.0, 12.5]))
        } else {
            (x, y)
        };
        let zr = g.uniform(-5.0, 30.0);
        let z = pick(g, &[0.0f32, 0.0, zr]);
        let gtype = g.below(3) as i32;
        let small = chance(g, 18);
        let (major, minor, height) = if small {
            let r = g.uniform(0.5, cell * 0.3);
            (r, g.uniform(0.5, cell * 0.3), g.uniform(1.0, 20.0))
        } else {
            (g.uniform(0.5, cell * 1.7), g.uniform(0.5, cell * 1.3), g.uniform(1.0, 40.0))
        };
        let geometry = GeometryInfo::new(GeometryType::from_index(gtype).unwrap(), small, height, major, minor);
        let mut o = ScenarioObject::new(id, Coord3D { x, y, z }, geometry);
        o.orientation = g.uniform(-3.5, 3.5);
        o.dir = pick(g, &[(1.0f32, 0.0f32), (0.0, 1.0), (-0.6, 0.8), (0.8, -0.6), (-1.0, 0.0)]);
        for (bit, pct) in [
            (KINDOF_IMMOBILE, 30),
            (KINDOF_STRUCTURE, 20),
            (KINDOF_INFANTRY, 20),
            (KINDOF_VEHICLE, 20),
            (KINDOF_NO_COLLIDE, 5),
            (KINDOF_MINE, 8),
            (KINDOF_ALWAYS_VISIBLE, 6),
            (KINDOF_FS_BASE_DEFENSE, 12),
            (KINDOF_DISGUISER, 12),
            (KINDOF_INERT, 10),
        ] {
            if chance(g, pct) {
                o.kind_of.set(bit, 1);
            }
        }
        for (bit, pct) in [(OBJECT_STATUS_NO_COLLISIONS, 5), (OBJECT_STATUS_REPULSOR, 10), (OBJECT_STATUS_STEALTHED, 15), (OBJECT_STATUS_DETECTED, 20), (OBJECT_STATUS_DISGUISED, 12)] {
            if chance(g, pct) {
                o.status.set(bit, 1);
            }
        }
        o.player = if chance(g, 18) { None } else { Some(g.below(h.player_count as u32) as i32) };
        o.team = match o.player {
            None => 0,
            Some(p) => 1 + p as u32 * 3 + g.below(3),
        };
        for (flag, pct) in [
            (sflag::DEAD, 10),
            (sflag::OFF_MAP, 8),
            (sflag::AIRBORNE, 15),
            (sflag::STRUCTURE, 25),
            (sflag::NON_FACTION, 20),
            (sflag::CAN_ATTACK, 40),
            (sflag::UNMANNED, 10),
            (sflag::WANTS_GHOST, 45),
            (sflag::KILL_ON_COLLIDE, 7),
            (sflag::SQUAD1, 30),
        ] {
            if chance(g, pct) {
                o.flags |= flag;
            }
        }
        o.build_cost = pick(g, &[0, 100, 100, 250, 400, 400, 1000]);
        o.template = g.below(5) as i32;
        if chance(g, 22) {
            let count = g.below(5) as i32;
            o.contain = Some(ContainFacts {
                is_garrisonable: chance(g, 60),
                contain_count: count,
                stealth_units_contained: if chance(g, 45) { count } else { g.below(5) as i32 },
                first_contained_detected: Some(chance(g, 50)),
            });
            o.apparent_controller = if chance(g, 30) { None } else { Some(g.below(4) as i32) };
        }
        if chance(g, 15) {
            o.stealth = Some(StealthFacts { is_disguised: chance(g, 60), disguised_player_index: pick(g, &[0i32, 1, 2, 3, 3, 20]) });
        }
        if o.player.is_some() && chance(g, 30) {
            o.vision = g.uniform(15.0, 90.0);
        }
        h.add(o);
    }
    h
}

fn gen_filter(g: &mut Gen, ids: &[i32], owned: &[i32], players: i32) -> FilterSpec {
    let mut s = FilterSpec { ty: 0, a: [-1; 8], f: [0.0; 8] };
    let oid = |g: &mut Gen| ids[g.below(ids.len() as u32) as usize];
    let player = |g: &mut Gen| -> i32 {
        if g.below(10) == 0 {
            -1
        } else {
            g.below(players as u32) as i32
        }
    };
    let kinds = [KINDOF_IMMOBILE, KINDOF_STRUCTURE, KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_MINE, KINDOF_NO_COLLIDE, KINDOF_INERT, KINDOF_FS_BASE_DEFENSE];
    let stats = [OBJECT_STATUS_NO_COLLISIONS, OBJECT_STATUS_REPULSOR, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED];
    s.ty = g.below(33) as i32;
    match s.ty {
        1 => {
            s.a[0] = g.below(3) as i32;
            s.a[1] = 0;
            s.a[2] = i32::from(chance(g, 60));
            s.f = [g.uniform(0.0, 400.0), g.uniform(0.0, 400.0), g.uniform(-3.0, 20.0), g.uniform(-3.0, 3.0), g.uniform(1.0, 20.0), g.uniform(2.0, 45.0), g.uniform(2.0, 30.0), 0.0];
        }
        2 | 26 => {
            s.a[0] = player(g);
            s.a[1] = i32::from(chance(g, 60));
        }
        3 => {
            s.a[0] = oid(g);
            s.a[1] = g.below(8) as i32;
        }
        4 => s.a[0] = pick(g, &[0, 1, 2, 3, 4, 5, 6, 10, 13]),
        5 => s.a[0] = pick(g, &[1, 2, -1]),
        6 | 16 | 18 | 23 => s.a[0] = oid(g),
        7 => {
            s.a[0] = g.below(8) as i32;
            s.a[1] = oid(g); // (a null attacker is dereferenced by upstream)
            s.a[2] = g.below(5) as i32;
        }
        8 | 9 => {
            s.a[0] = if chance(g, 90) { oid(g) } else { -1 };
            s.a[1] = g.below(5) as i32;
        }
        10 => s.a[0] = if chance(g, 90) { oid(g) } else { -1 },
        11 | 12 => {
            s.a[0] = pick(g, &stats);
            s.a[1] = if chance(g, 25) { pick(g, &stats) } else { -1 };
            s.a[3] = if chance(g, 40) { pick(g, &stats) } else { -1 };
        }
        13 => {
            s.a[0] = oid(g);
            s.a[1] = i32::from(chance(g, 50));
        }
        14 | 15 => {
            s.a[0] = if chance(g, 80) { pick(g, &kinds) } else { -1 };
            s.a[1] = if chance(g, 25) { pick(g, &kinds) } else { -1 };
            s.a[3] = if chance(g, 40) { pick(g, &kinds) } else { -1 };
        }
        // (the C++ constructor dereferences the object's controlling player)
        20 => s.a[0] = owned[g.below(owned.len() as u32) as usize],
        21 => {
            s.a[0] = i32::from(chance(g, 50));
            s.a[1] = i32::from(chance(g, 50));
        }
        22 => s.a[0] = g.below(players as u32) as i32,
        24 => {
            s.a[0] = 3 + g.below(2) as i32;
            let cx = g.uniform(100.0, 300.0);
            let cy = g.uniform(100.0, 300.0);
            let r = g.uniform(40.0, 200.0);
            s.f = [cx - r, cy - r, cx + r, cy - r, cx + r, cy + r, cx - r, cy + r];
        }
        25 => s.a[0] = g.below(6) as i32,
        27 => {
            s.a[0] = g.below(players as u32) as i32;
            s.a[1] = pick(g, &[1, 2, 4, 8, 6, 10, 12, 14, 5]);
            s.a[2] = i32::from(chance(g, 60));
        }
        28 => {
            s.a[0] = g.below(5) as i32;
            s.a[1] = i32::from(chance(g, 50));
        }
        29 | 31 => s.a[0] = i32::from(chance(g, 50)),
        30 => {
            s.a[0] = player(g);
            s.a[1] = i32::from(chance(g, 50));
            s.a[2] = g.below(5) as i32;
        }
        32 => {
            s.a[0] = oid(g);
            s.a[1] = g.below(6) as i32;
            s.a[2] = i32::from(chance(g, 50));
            s.a[3] = g.below(5) as i32;
        }
        _ => {}
    }
    s
}

fn gen_filters(g: &mut Gen, ids: &[i32], owned: &[i32], players: i32) -> Vec<FilterSpec> {
    let n = match g.below(10) {
        0..=3 => 0,
        4..=7 => 1,
        8 => 2,
        _ => 3,
    };
    (0..n).map(|_| gen_filter(g, ids, owned, players)).collect()
}

fn gen_case(seed: u64) -> Case {
    let mut g = Gen::new(seed);
    let scn = gen_scenario(&mut g);
    let ids: Vec<i32> = scn.objects.keys().copied().collect();
    let owned: Vec<i32> = scn.objects.values().filter(|o| o.player.is_some()).map(|o| o.id).collect();
    let owned = if owned.is_empty() { ids.clone() } else { owned };
    let ext = scn.extent.unwrap();
    let (w, hh) = (ext.hi.x - ext.lo.x, ext.hi.y - ext.lo.y);
    let players = scn.player_count;
    let g = &mut g;
    let mut ops = Vec::new();
    // register most objects in a random order
    let mut order = ids.clone();
    for i in (1..order.len()).rev() {
        order.swap(i, g.below(i as u32 + 1) as usize);
    }
    let regs = (order.len() * (60 + g.below(41) as usize)) / 100;
    for &id in order.iter().take(regs) {
        ops.push(Op::Register(id));
        if chance(g, 15) {
            ops.push(Op::Update);
        }
    }
    ops.push(Op::Update);
    let n_ops = 30 + g.below(60);
    let pos = |g: &mut Gen| -> [f32; 3] {
        let (px, py, zr) = (g.uniform(ext.lo.x - 30.0, ext.hi.x + 30.0), g.uniform(ext.lo.y - 30.0, ext.hi.y + 30.0), g.uniform(-5.0, 25.0));
        [px, py, pick(g, &[0.0f32, 0.0, zr])]
    };
    let dists = [3.0f32, 25.0, 60.0, 120.0, 250.0, 700.0, 2.0e6];
    for _ in 0..n_ops {
        let id = ids[g.below(ids.len() as u32) as usize];
        let other = ids[g.below(ids.len() as u32) as usize];
        let op = match g.below(100) {
            0..=3 => Op::Register(id),
            4..=6 => Op::Unregister(id),
            7..=13 => {
                let p = pos(g);
                Op::Move { id, x: p[0], y: p[1], z: p[2], orient: g.uniform(-3.5, 3.5) }
            }
            14..=15 => Op::Dirty { id, need: g.below(2) as i32 },
            16..=22 => Op::Update,
            23 => Op::SetFrame(g.below(200)),
            24..=28 => Op::ClosestObj { origin: id, dc: g.below(4) as i32, filters: gen_filters(g, &ids, &owned, players), max: pick(g, &dists) },
            29..=31 => Op::ClosestPos { dc: g.below(4) as i32, filters: gen_filters(g, &ids, &owned, players), pos: pos(g), max: pick(g, &dists) },
            32..=38 => Op::RangeObj { origin: id, dc: g.below(4) as i32, order: g.below(5) as i32, filters: gen_filters(g, &ids, &owned, players), max: pick(g, &dists) },
            39..=43 => Op::RangePos { dc: g.below(4) as i32, order: g.below(5) as i32, filters: gen_filters(g, &ids, &owned, players), pos: pos(g), max: pick(g, &dists) },
            44 => Op::AllObjects { filters: gen_filters(g, &ids, &owned, players) },
            45..=46 => {
                let p = pos(g);
                Op::PotentialColl {
                    gtype: g.below(3) as i32,
                    small: 0,
                    use2d: g.below(2) as i32,
                    f: [p[0], p[1], p[2], g.uniform(1.0, 20.0), g.uniform(1.0, 40.0), g.uniform(1.0, 30.0), g.uniform(-3.0, 3.0)],
                }
            }
            47..=49 => {
                let p = pos(g);
                let q = pos(g);
                Op::Dist { kind: g.below(6) as i32, a: id, b: other, dc: g.below(4) as i32, f: [p[0], p[1], p[2], q[0], q[1], q[2]] }
            }
            50 => Op::RelAngle { a: id, b: if chance(g, 50) { other } else { -1 }, pos: pos(g) },
            51 => Op::IsColliding(id, other),
            52 => {
                let p = pos(g);
                let q = [p[0] + g.uniform(-30.0, 30.0), p[1] + g.uniform(-30.0, 30.0), p[2] + g.uniform(-8.0, 8.0)];
                Op::GeomCollides {
                    g1: g.below(3) as i32,
                    g2: g.below(3) as i32,
                    f: [
                        p[0], p[1], p[2], g.uniform(1.0, 20.0), g.uniform(1.0, 30.0), g.uniform(1.0, 20.0), g.uniform(-3.0, 3.0),
                        q[0], q[1], q[2], g.uniform(1.0, 20.0), g.uniform(1.0, 30.0), g.uniform(1.0, 20.0), g.uniform(-3.0, 3.0),
                    ],
                }
            }
            53 => {
                let p = pos(g);
                Op::GroundStruct(p[0], p[1])
            }
            54 => {
                let p = pos(g);
                let q = pos(g);
                Op::Los { a: if chance(g, 60) { id } else { -1 }, b: if chance(g, 60) { other } else { -1 }, f: [p[0], p[1], p[2], q[0], q[1], q[2]] }
            }
            55 => {
                let p = pos(g);
                let q = pos(g);
                Op::Extremes([p[0], p[1], p[2], q[0], q[1], q[2]])
            }
            56..=66 => {
                let kind = pick(g, &[0, 0, 0, 1, 1, 2, 3, 4, 4, 5, 6, 7, 8]);
                let p = pos(g);
                Op::Shroud {
                    kind,
                    mask: g.below(1 << players) as i32,
                    value: g.below(2000) as i32,
                    x: p[0],
                    y: p[1],
                    r: {
                        let (r1, r2) = (g.uniform(5.0, 150.0), g.uniform(5.0, w.min(hh) * 0.6));
                        pick(g, &[0.0f32, 10.0, 35.0, 60.0, r1, r2])
                    },
                }
            }
            67 => Op::ProcessQueue,
            68..=70 => {
                let p = pos(g);
                Op::CellStatus { kind: g.below(2) as i32, player: g.below(players as u32) as i32 - i32::from(chance(g, 5)), x: p[0], y: p[1] }
            }
            71..=73 => Op::ObjShroud { id, player: g.below(players as u32) as i32 },
            74 => Op::MostValuable { player: g.below(players as u32) as i32, types: g.below(16) as i32, vt: 1 + g.below(2) as i32 },
            75 | 81 | 82 => Op::NearestGroup {
                player: g.below(players as u32) as i32,
                types: g.below(16) as i32,
                vt: 1 + g.below(2) as i32,
                required: pick(g, &[0, 1, 5, 50, -3]),
                greater: g.below(2) as i32,
                pos: {
                    let p = pos(g);
                    [p[0].clamp(ext.lo.x, ext.hi.x), p[1].clamp(ext.lo.y, ext.hi.y), 0.0]
                },
            },
            76 => Op::MapOps { kind: g.below(5) as i32, player: g.below(players as u32) as i32 },
            77 => Op::StoreRestore(g.below(4) as i32),
            78..=80 => Op::Crc,
            83..=85 => Op::Dump,
            86..=92 => {
                let p = pos(g);
                Op::FindPosition {
                    flags: pick(g, &[0, fpf::IGNORE_WATER, fpf::IGNORE_WATER | fpf::IGNORE_ALL_OBJECTS, fpf::WATER_ONLY, fpf::CLEAR_CELLS_ONLY | fpf::IGNORE_WATER, fpf::USE_HIGHEST_LAYER | fpf::IGNORE_WATER, fpf::IGNORE_ALLY_OR_NEUTRAL_UNITS | fpf::IGNORE_WATER, fpf::IGNORE_ENEMY_UNITS | fpf::IGNORE_ENEMY_STRUCTURES | fpf::IGNORE_ALLY_OR_NEUTRAL_STRUCTURES | fpf::IGNORE_WATER]) as i32,
                    ignore: if chance(g, 30) { id } else { -1 },
                    src: if chance(g, 40) { other } else { -1 },
                    rel: if chance(g, 50) { id } else { -1 },
                    f: [
                        p[0].clamp(ext.lo.x + 10.0, ext.hi.x - 10.0),
                        p[1].clamp(ext.lo.y + 10.0, ext.hi.y - 10.0),
                        p[2],
                        pick(g, &[0.0f32, 0.0, 12.0]),
                        pick(g, &[0.0f32, 8.0, 25.0, 60.0]),
                        pick(g, &[support_random_angle(), 0.0, 1.5, -2.0]),
                        pick(g, &[1.0e10f32, 1.0e10, 4.0]),
                    ],
                }
            }
            93..=95 => Op::UnregisterGhost(1 + g.below(14) as i32),
            _ => Op::Nop,
        };
        ops.push(op);
    }
    ops.push(Op::Update);
    ops.push(Op::Dump);
    ops.push(Op::Crc);
    Case { scn, ops }
}

/// `RANDOM_START_ANGLE`.
fn support_random_angle() -> f32 {
    -99999.9
}

// ---------------------------------------------------------------------------
// The comparison
// ---------------------------------------------------------------------------

fn run_cpp(scn: &ScenarioHost, ops: &[Op]) -> Vec<u32> {
    let EncodedScenario { ig, fg, rel, io, fo } = encode_scenario(scn);
    let mut ints = Vec::new();
    let mut floats = Vec::new();
    for op in ops {
        op.encode(&mut ints, &mut floats);
    }
    let mut out = vec![0u32; 4_000_000];
    let n = unsafe { cpp::ref_partition_run(ig.as_ptr(), fg.as_ptr(), rel.as_ptr(), io.as_ptr(), fo.as_ptr(), ops.len() as i32, ints.as_ptr(), floats.as_ptr(), out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "ref_partition_run returned {n}");
    out.truncate(n as usize);
    out
}

/// Statistics of what the cases reached, for the report.
#[derive(Default)]
struct Stats {
    cases: u64,
    ops: u64,
    words: u64,
    closest_found: u64,
    range_lists: u64,
    range_multi: u64,
    collisions: u64,
    cell_changes: u64,
    ghosts_made: u64,
    shroud_events: u64,
    ghost_calls: u64,
    filter_uses: Vec<u64>,
    dumps: u64,
    find_found: u64,
    flagged: u64,
    filtered_queries: u64,
    ties: u64,
}

fn op_name(op: &Op) -> String {
    format!("{op:?}")
}

fn compare(seed: u64, case: &Case, stats: &mut Stats) -> u64 {
    // run the Rust once to find the operations the C++ must not run, replace them, run again
    let first = run_rust(&case.scn, &case.ops);
    let mut ops = case.ops.clone();
    for (i, &f) in first.flagged.iter().enumerate() {
        if f {
            ops[i] = Op::Nop;
            stats.flagged += 1;
        }
    }
    let rust = run_rust(&case.scn, &ops);
    let cpp_out = run_cpp(&case.scn, &ops);
    stats.cases += 1;
    stats.ops += ops.len() as u64;
    stats.words += rust.out.len() as u64;
    stats.cell_changes += rust.logs[0];
    stats.collisions += rust.logs[1];
    stats.ghosts_made += rust.logs[2];
    stats.shroud_events += rust.logs[3];
    stats.ghost_calls += rust.logs[4];
    // statistics of the inputs, from the Rust results
    for (n, op) in ops.iter().enumerate() {
        let end = rust.starts.get(n + 1).copied().unwrap_or(rust.out.len());
        let w = &rust.out[rust.starts[n]..end];
        match op {
            Op::ClosestObj { filters, .. } | Op::ClosestPos { filters, .. } => {
                for f in filters {
                    stats.filter_uses[f.ty as usize] += 1;
                }
                if w[0] != 0 {
                    stats.closest_found += 1;
                }
                if !filters.is_empty() {
                    stats.filtered_queries += 1;
                }
            }
            Op::RangeObj { filters, .. } | Op::RangePos { filters, .. } => {
                for f in filters {
                    stats.filter_uses[f.ty as usize] += 1;
                }
                stats.range_lists += 1;
                if w[0] >= 2 {
                    stats.range_multi += 1;
                    // a tie: two neighbouring results with the same numeric key
                    for k in 0..(w[0] as usize - 1) {
                        if w[1 + 2 * k + 1] == w[1 + 2 * (k + 1) + 1] {
                            stats.ties += 1;
                        }
                    }
                }
                if !filters.is_empty() {
                    stats.filtered_queries += 1;
                }
            }
            Op::AllObjects { filters } => {
                for f in filters {
                    stats.filter_uses[f.ty as usize] += 1;
                }
            }
            Op::Dump => stats.dumps += 1,
            Op::FindPosition { .. } if w[0] == 1 => stats.find_found += 1,
            _ => {}
        }
    }
    let mut mismatches = 0;
    if rust.out != cpp_out {
        let n = rust.out.len().min(cpp_out.len());
        let first_diff = (0..n).find(|&i| rust.out[i] != cpp_out[i]).unwrap_or(n);
        let op_idx = rust.starts.iter().rposition(|&s| s <= first_diff).unwrap_or(0);
        let from = rust.starts[op_idx];
        let lo = first_diff.saturating_sub(12).max(from);
        let hi = first_diff + 20;
        mismatches += 1;
        eprintln!(
            "MISMATCH seed {seed:#x}: first difference at word {first_diff} (offset {} in op #{op_idx} {}; rust {} words, c++ {} words)\n  rust[{lo}..]: {:x?}\n  c++ [{lo}..]: {:x?}",
            first_diff - from,
            op_name(&ops[op_idx]),
            rust.out.len(),
            cpp_out.len(),
            &rust.out[lo..hi.min(rust.out.len())],
            &cpp_out[lo.min(cpp_out.len())..hi.min(cpp_out.len())],
        );
    }
    mismatches
}

const SEED_BASE: u64 = 0x6A21;

#[test]
fn partition_manager_matches_upstream() {
    let cases: u64 = std::env::var("PARTITION_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(3000);
    // PARTITION_SEED=0x6a23 runs that one case; PARTITION_VERBOSE=1 also prints its operations
    let only: Option<u64> = std::env::var("PARTITION_SEED").ok().and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok());
    let mut stats = Stats { filter_uses: vec![0; 33], ..Default::default() };
    let mut mismatches = 0;
    for c in 0..cases {
        let seed = only.unwrap_or(SEED_BASE + c);
        let case = gen_case(seed);
        if std::env::var("PARTITION_VERBOSE").is_ok() {
            for (i, op) in case.ops.iter().enumerate() {
                eprintln!("op #{i}: {op:?}");
            }
        }
        mismatches += compare(seed, &case, &mut stats);
        if mismatches >= 5 || only.is_some() {
            break;
        }
    }
    println!(
        "DIFF partition: {} cases, {} operations, {} words compared; closest queries with a result {}, range lists {} ({} with 2+ elements, {} adjacent equal-key pairs), \
         filtered queries {}, state dumps {}, find-position successes {}, undefined operations withheld {}",
        stats.cases, stats.ops, stats.words, stats.closest_found, stats.range_lists, stats.range_multi, stats.ties, stats.filtered_queries, stats.dumps, stats.find_found, stats.flagged
    );
    println!(
        "DIFF partition logs: {} cell-change callbacks, {} collision callbacks, {} ghosts made, {} shroud writes to the client, {} ghost calls to the client",
        stats.cell_changes, stats.collisions, stats.ghosts_made, stats.shroud_events, stats.ghost_calls
    );
    println!("DIFF partition filter uses by type 0..32: {:?}", stats.filter_uses);
    assert_eq!(mismatches, 0, "{mismatches} cases differ from upstream");
}
