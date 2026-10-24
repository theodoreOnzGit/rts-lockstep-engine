// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::terrain_logic` (`TerrainLogic`,
//! `Waypoint`, `Bridge`) and `logic::terrain_visual` (the logic-owned water
//! grid and height map) against upstream
//! `GameLogic/Map/TerrainLogic.cpp` (commit 0a05454d), compiled UNCHANGED in
//! `differential/wrap_terrain_logic.cpp` on the stand-ins of
//! `differential/shim_terrain_logic` (see build.rs). Phase 1 layer 6.
//!
//! # Methodology
//!
//! A **case** is a random *scenario* (the map: waypoint objects and the chunk
//! file with their links, polygon water areas, a height map, boundaries,
//! `GlobalData`'s water-grid settings; and the world: objects with their
//! geometry, layers and damage states, the bridge and tower templates, the
//! pathfinder's wall) and a random *program* of 3 to 24 operations on one
//! `TerrainLogic`. The same case runs in the Rust and in the C++; both write,
//! per operation,
//!
//! 1. the operation's results (floats as their bits),
//! 2. the **world log**: every call that crosses the cut to a later layer, in
//!    order, with its arguments (`findObjectByID`, `getDamageState`,
//!    `attemptDamage`, the pathfinder's `addBridge` / `changeBridgeState` /
//!    `isPointOnWall`, `newObject` / `setPosition` / `setOrientation` for
//!    bridges and towers, the partition manager and ghost-object calls of
//!    `setActiveBoundary`, ...); the C++ stand-ins log as they are called,
//!    and the Rust `TerrainWorld` test implementation writes the same words,
//! 3. the **visual log**: every write to `TheTerrainVisual` in the C++, and the
//!    client events the Rust queued (`take_events`), as the same words.
//!
//! and the two word streams are compared one word at a time. So a case checks
//! that the Rust computes the same answers, makes the same calls to the world
//! with the same arguments in the same order, and emits the same client
//! notifications.
//!
//! What is real in the C++, what is a stand-in, and which of the stand-ins is
//! a second transcription of upstream code rather than upstream code, is in
//! the header comment of `differential/wrap_terrain_logic.cpp`. In short:
//! `TerrainLogic.cpp`, the chunk reader, Xfer, Matrix3D, the plane and
//! point-in-triangle code, the W3D terrain logic's overrides
//! (`getExtent`..`getLayerHeight`), `getHeightMapHeight` and the polygon
//! trigger's point test are upstream's own text; the object world, the
//! pathfinder and the like answer from the scenario; the water render object
//! and the terrain visual's height map are transcribed from their source.
//!
//! ## Operations
//!
//! `[code, params...]`; floats are bit patterns; results as in the C++
//! wrapper. A program is `[n, op...]`.
//!
//! | code | operation | upstream |
//! |---|---|---|
//! | 0 | set the frame | (world) |
//! | 1 | set object `i`: damage state, scaffold, exists, pos, orientation, layer | (world) |
//! | 2 | set a water mesh height | `setGridVertexHeight` |
//! | 10 | load the map | `loadMap` |
//! | 11 | new map | `newMap` |
//! | 12, 13 | ground / layer height (+ normal) | `getGroundHeight`, `getLayerHeight` |
//! | 14 | the three extents | `getExtent`, `getMaximumPathfindExtent`, `getExtentIncludingBorder` |
//! | 15, 16 | edge points | `findClosestEdgePoint`, `findFarthestEdgePoint` |
//! | 17 | align on terrain | `alignOnTerrain` |
//! | 20-25 | waypoint list dump, by name, by id, closest on path, purpose of path, trigger by name | |
//! | 30, 31 | add a map bridge, a landmark bridge | `addBridgeToLogic`, `addLandmarkBridgeToLogic` |
//! | 32 | bridge list dump | |
//! | 33-38 | `findBridgeAt`, `findBridgeLayerAt`, `getLayerForDestination`, `getHighestLayerForDestination`, `objectInteractsWithBridgeLayer`, `...End` | |
//! | 39 | update bridge damage states | `updateBridgeDamageStates` |
//! | 41, 43, 44 | repaired / broken, attack points, pick | |
//! | 45, 46 | delete a bridge, load post process | |
//! | 47, 48 | one bridge: on-bridge / deck height / normal; the three cell tests | |
//! | 50-53 | water handle, `isUnderwater`, handle by name, water height | |
//! | 54-58 | `setWaterHeight`, `changeWaterHeightOverTime`, `update`, the change list, the bounding box | |
//! | 59, 60 | `enableWaterGrid`, `setActiveBoundary` | |
//! | 61, 62 | `flattenTerrain`, `createCraterInTerrain` (+ a digest of the height map) | |
//! | 63, 64 | `xfer` save, `xfer` load of that save | |
//! | 65, 66, 70, 72, 73 | reset, flags, raw height, grid height, trigger dump | |
//!
//! **Undefined-upstream inputs** are not sent to the C++. The Rust runs every
//! case first; a case in which any operation returns an `Undefined` error
//! is dropped from the comparison and counted, per kind (see
//! `TerrainUndefined`). `undefined_inputs_are_errors` then drives each kind
//! on purpose and checks the error, and `TerrainLogic::undefined_inputs`.
//!
//! **Planted bugs.** Each comparison was also run against one deliberately
//! wrong translation, to show that the cases can tell a right answer from a
//! plausible wrong one. The bugs were planted by hand, the run was observed
//! to fail, and the bug was removed; the record is below, under Results.
//!
//! # Results
//!
//! Run 2026-10-18, upstream `0a05454d`, `cargo test --release --features
//! differential --test differential_terrain_logic -- --nocapture` (the `DIFF`
//! and `CASES` lines). Six random groups, 24,000 programs generated, 22,139
//! compared (346,279 operations), **0 mismatches** in results, in the world
//! log (every call from terrain into objects, pathfinder, partition manager)
//! and in the visual log (every `TheTerrainVisual` write against the Rust
//! `TerrainVisualEvent`), floats compared bitwise. 1,861 programs were dropped
//! as undefined upstream: `PickPosition` 1,323 (an uninitialised out-parameter
//! in `pickBridge`), `StaleWaterHandle` 221, `WaterTransformUnavailable` 200,
//! `ChunkUndefined` 117. About 20% of bridge programs are lost to
//! `PickPosition`; the other operations of those programs are not compared.
//!
//! **Planted bugs** (one at a time in `src/logic/terrain_logic.rs`, the whole
//! file `cargo test`-ed, then removed; 2026-10-18). Caught means at least one
//! comparison failed or the C++ reference crashed on a case the Rust did not
//! flag as undefined:
//!
//! | planted | result |
//! |---|---|
//! | `findBridgeAt` returns the last match, not the first | caught (op 33, 8 mismatches) |
//! | bridge layer search tests `layer !=` | caught (op 13, 191) |
//! | final water transition `>` instead of `>=` | caught only after the water story added a change landing exactly on its target (world log, 226) |
//! | water change-over-time rate uses 31 frames per second | caught (op 53) |
//! | finished water entry copied the wrong way | caught |
//! | crater `j` loop starts at `i_min.y` instead of 0 (the upstream quirk) | caught (op 16, 12) |
//! | crater `distance <= radius` | caught only after objects were placed on grid nodes with whole-cell radii (visual log, 12,582) |
//! | flatten bounding box `minX` test inverted | caught (op 12, 112) |
//! | wall-height threshold `/ 3` in `getLayerForDestination` | caught (op 17, 8) |
//! | `xfer` water list skipped (`version >= 3`) | caught, as a SIGSEGV of the C++ reference on a load the Rust no longer rejects |
//! | `getLayerForDestination` height compared with +0.001 | **NOT caught**: the bias is below the spacing of random heights and a tie is not generated |
//!
//! The two "caught only after" rows were first missed, the generator was
//! strengthened, and the plant re-run; the +0.001 row stays a known weakness.

#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D, ICoord3D, Region2D, Region3D};
use rts_lockstep_engine::common::dict::Dict;
use rts_lockstep_engine::common::game_type::{DrawableId, ObjectId};
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::well_known_keys::{self as wk, WellKnownKeys};
use rts_lockstep_engine::common::xfer::{Xfer, XferMode, XferResult, XferState, XferStatus};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::data_chunk::CachedFileInputStream;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::module::body_module::BodyDamageType;
use rts_lockstep_engine::logic::terrain_logic::{
    Bridge, BridgeInfo, BridgeTowerType, GroundSource, LandmarkBridgeObject, ObjectView, TerrainLogic, TerrainLogicError, TerrainWorld,
    TriggerArea, WaterHandle, WaypointHandle, WaypointKeys, WaypointMapObject,
};
use rts_lockstep_engine::logic::terrain_visual::{LogicHeightMap, TerrainUndefined, TerrainVisualEvent, WaterGrid, WaterGridSample};
use rts_lockstep_engine::ww::vector3::Vector3;
use std::cell::RefCell;
use support::{Check, Gen};

/// Cases per run of the main comparison.
const CASES: usize = 4000;

// ---------------------------------------------------------------------------
// Words
// ---------------------------------------------------------------------------

fn fb(x: f32) -> u32 {
    x.to_bits()
}

/// FNV-1a over the bytes of a string, as `hashOf` in the C++ wrapper.
fn hash_of(s: &[u8]) -> u32 {
    let mut h: u32 = 2_166_136_261;
    for &b in s {
        h ^= u32::from(b);
        h = h.wrapping_mul(16_777_619);
    }
    h
}

// ---------------------------------------------------------------------------
// The scenario
// ---------------------------------------------------------------------------

/// The strings of the scenario: every name a case uses is an index into this.
const VOCAB: [&str; 34] = [
    "", "WP0", "WP1", "WP2", "WP3", "WP4", "WP5", "WaveGuide1", "PathA", "pathA", "PathB", "pathb", "Lake", "River", "Pond", "Water Grid",
    "Nope", "BridgeA", "BridgeB", "Truck", "GenericBridge", "BridgeTower0", "BridgeTower1", "BridgeTower2", "BridgeTower3",
    "Maps\\Dam\\Dam.map", "Save\\Dam.map", "maps\\dam\\DAM.MAP", "Maps\\Other\\Other.map", "Maps\\Alpine\\Alpine.map", "Maps/Slash/Slash.map",
    "Slash.map", "level.map", "Maps\\x.map",
];

fn vocab(name: &str) -> usize {
    VOCAB.iter().position(|&v| v == name).unwrap_or_else(|| panic!("{name} is not in the vocabulary"))
}

fn ascii(i: usize) -> AsciiString {
    AsciiString::try_from_bytes(VOCAB[i].as_bytes()).unwrap()
}

#[derive(Clone, Debug)]
struct Obj {
    id: i32,
    pos: [f32; 3],
    orient: f32,
    layer: i32,
    /// (type, small, height, major, minor)
    geom: (i32, bool, f32, f32, f32),
    name: usize,
    body: i32,
    indestructible: bool,
    drawable: i32,
    scaffold: i32,
    exists: bool,
}

impl Obj {
    /// What `TLThingFactory::newObject` makes.
    fn fresh(id: i32, name: usize) -> Obj {
        Obj {
            id,
            pos: [0.0; 3],
            orient: 0.0,
            layer: 1,
            geom: (0, false, 1.0, 1.0, 1.0),
            name,
            body: 0,
            indestructible: false,
            drawable: 0,
            scaffold: 1,
            exists: true,
        }
    }

    fn view(&self) -> ObjectView {
        ObjectView {
            id: self.id,
            position: Coord3D { x: self.pos[0], y: self.pos[1], z: self.pos[2] },
            orientation: self.orient,
            layer: self.layer,
            geometry: GeometryInfo::new(GeometryType::from_index(self.geom.0).unwrap(), self.geom.1, self.geom.2, self.geom.3, self.geom.4),
        }
    }
}

#[derive(Clone, Debug)]
struct Trig {
    name: usize,
    id: i32,
    water: bool,
    pts: Vec<[i32; 3]>,
}

#[derive(Clone, Debug)]
struct MapObj {
    is_wp: bool,
    loc: [f32; 3],
    id: i32,
    name: usize,
    /// vocabulary index of each label, or -1 for no such property
    labels: [i32; 3],
    /// 0: no property, 1: false, 2: true
    bidir: i32,
}

#[derive(Clone, Debug)]
struct Setting {
    map: usize,
    f: [f32; 6],
    cells: (i32, i32),
    size: f32,
    att: [f32; 4],
}

#[derive(Clone, Debug)]
struct Hm {
    w: i32,
    h: i32,
    border: i32,
    bounds: Vec<(i32, i32)>,
    data: Vec<u8>,
}

#[derive(Clone, Debug)]
struct Scenario {
    w3d: bool,
    hm: Option<Hm>,
    render: bool,
    bounds: Vec<(i32, i32)>,
    active: i32,
    map_dx: i32,
    map_dy: i32,
    min_z: f32,
    max_z: f32,
    map_name: usize,
    settings: Vec<Setting>,
    generic: bool,
    road: bool,
    tower_mask: u32,
    tower_major: [f32; 4],
    wall_height: f32,
    wall_on: bool,
    wall: [f32; 4],
    frame: u32,
    objs: Vec<Obj>,
    trigs: Vec<Trig>,
    maps: Vec<MapObj>,
    file_opens: bool,
    file: Vec<u8>,
}

impl Scenario {
    /// The words `ref_terrain_logic_run` reads.
    fn encode(&self) -> Vec<u32> {
        let mut w: Vec<u32> = Vec::new();
        let i = |w: &mut Vec<u32>, v: i32| w.push(v as u32);
        i(&mut w, VOCAB.len() as i32);
        for s in VOCAB {
            i(&mut w, s.len() as i32);
            for b in s.bytes() {
                w.push(u32::from(b));
            }
        }
        w.push(u32::from(self.w3d));
        w.push(u32::from(self.hm.is_some()));
        if let Some(hm) = &self.hm {
            i(&mut w, hm.w);
            i(&mut w, hm.h);
            i(&mut w, hm.border);
            i(&mut w, hm.bounds.len() as i32);
            for &(x, y) in &hm.bounds {
                i(&mut w, x);
                i(&mut w, y);
            }
            for &b in &hm.data {
                w.push(u32::from(b));
            }
        }
        w.push(u32::from(self.render));
        i(&mut w, self.bounds.len() as i32);
        for &(x, y) in &self.bounds {
            i(&mut w, x);
            i(&mut w, y);
        }
        i(&mut w, self.active);
        i(&mut w, self.map_dx);
        i(&mut w, self.map_dy);
        w.push(fb(self.min_z));
        w.push(fb(self.max_z));
        i(&mut w, self.map_name as i32);
        for s in &self.settings {
            i(&mut w, s.map as i32);
            for f in s.f {
                w.push(fb(f));
            }
            i(&mut w, s.cells.0);
            i(&mut w, s.cells.1);
            w.push(fb(s.size));
            for a in s.att {
                w.push(fb(a));
            }
        }
        w.push(u32::from(self.generic));
        w.push(u32::from(self.road));
        w.push(self.tower_mask);
        for m in self.tower_major {
            w.push(fb(m));
        }
        w.push(fb(self.wall_height));
        w.push(u32::from(self.wall_on));
        for v in self.wall {
            w.push(fb(v));
        }
        w.push(self.frame);
        i(&mut w, self.objs.len() as i32);
        for o in &self.objs {
            i(&mut w, o.id);
            for p in o.pos {
                w.push(fb(p));
            }
            w.push(fb(o.orient));
            i(&mut w, o.layer);
            i(&mut w, o.geom.0);
            i(&mut w, i32::from(o.geom.1));
            w.push(fb(o.geom.2));
            w.push(fb(o.geom.3));
            w.push(fb(o.geom.4));
            i(&mut w, o.name as i32);
            i(&mut w, o.body);
            w.push(u32::from(o.indestructible));
            i(&mut w, o.drawable);
            i(&mut w, o.scaffold);
            w.push(u32::from(o.exists));
        }
        i(&mut w, self.trigs.len() as i32);
        for t in &self.trigs {
            i(&mut w, t.name as i32);
            i(&mut w, t.id);
            w.push(u32::from(t.water));
            i(&mut w, t.pts.len() as i32);
            for p in &t.pts {
                for &c in p {
                    i(&mut w, c);
                }
            }
        }
        i(&mut w, self.maps.len() as i32);
        for m in &self.maps {
            w.push(u32::from(m.is_wp));
            for l in m.loc {
                w.push(fb(l));
            }
            i(&mut w, m.id);
            i(&mut w, m.name as i32);
            for l in m.labels {
                i(&mut w, l);
            }
            i(&mut w, m.bidir);
        }
        w.push(u32::from(self.file_opens));
        i(&mut w, self.file.len() as i32);
        for &b in &self.file {
            w.push(u32::from(b));
        }
        w
    }
}

// ---------------------------------------------------------------------------
// The program
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Op {
    code: u32,
    args: Vec<u32>,
}

fn op(code: u32, args: &[u32]) -> Op {
    Op { code, args: args.to_vec() }
}

fn encode_program(ops: &[Op]) -> Vec<u32> {
    let mut w = vec![ops.len() as u32];
    for o in ops {
        w.push(o.code);
        w.extend_from_slice(&o.args);
    }
    w
}

fn op_name(code: u32) -> &'static str {
    match code {
        0 => "setFrame",
        1 => "setObject",
        2 => "setMeshHeight",
        3 => "setMapName",
        10 => "loadMap",
        11 => "newMap",
        12 => "getGroundHeight",
        13 => "getLayerHeight",
        14 => "getExtent*",
        15 => "findClosestEdgePoint",
        16 => "findFarthestEdgePoint",
        17 => "alignOnTerrain",
        20 => "waypoints",
        21 => "getWaypointByName",
        22 => "getWaypointByID",
        23 => "getClosestWaypointOnPath",
        24 => "isPurposeOfPath",
        25 => "getTriggerAreaByName",
        30 => "addBridgeToLogic",
        31 => "addLandmarkBridgeToLogic",
        32 => "bridges",
        33 => "findBridgeAt",
        34 => "findBridgeLayerAt",
        35 => "getLayerForDestination",
        36 => "getHighestLayerForDestination",
        37 => "objectInteractsWithBridgeLayer",
        38 => "objectInteractsWithBridgeEnd",
        39 => "updateBridgeDamageStates",
        41 => "isBridgeRepaired/Broken",
        43 => "getBridgeAttackPoints",
        44 => "pickBridge",
        45 => "deleteBridge",
        46 => "loadPostProcess",
        47 => "bridge height",
        48 => "bridge cells",
        50 => "getWaterHandle",
        51 => "isUnderwater",
        52 => "getWaterHandleByName",
        53 => "getWaterHeight",
        54 => "setWaterHeight",
        55 => "changeWaterHeightOverTime",
        56 => "update",
        57 => "water changes",
        58 => "findAxisAlignedBoundingRect",
        59 => "enableWaterGrid",
        60 => "setActiveBoundary",
        61 => "flattenTerrain",
        62 => "createCraterInTerrain",
        63 => "xfer save",
        64 => "xfer load",
        65 => "reset",
        66 => "state",
        70 => "getRawMapHeight",
        72 => "getWaterGridHeight",
        73 => "triggers",
        _ => "?",
    }
}

// ---------------------------------------------------------------------------
// The world, on the Rust side: the TerrainWorld the C++ stand-ins are
// ---------------------------------------------------------------------------

struct TestWorld {
    log: RefCell<Vec<u32>>,
    objs: RefCell<Vec<Obj>>,
    generic: bool,
    road: bool,
    tower_mask: u32,
    tower_major: [f32; 4],
    wall_height: f32,
    wall_on: bool,
    wall: [f32; 4],
    frame: u32,
    next_id: i32,
    next_layer: i32,
}

impl TestWorld {
    fn new(sc: &Scenario) -> Self {
        TestWorld {
            log: RefCell::new(Vec::new()),
            objs: RefCell::new(sc.objs.clone()),
            generic: sc.generic,
            road: sc.road,
            tower_mask: sc.tower_mask,
            tower_major: sc.tower_major,
            wall_height: sc.wall_height,
            wall_on: sc.wall_on,
            wall: sc.wall,
            frame: sc.frame,
            next_id: 1000,
            next_layer: 2,
        }
    }

    fn rec(&self, code: u32, words: &[u32]) {
        let mut l = self.log.borrow_mut();
        l.push(code);
        l.push(words.len() as u32);
        l.extend_from_slice(words);
    }

    fn existing(&self) -> Vec<usize> {
        self.objs.borrow().iter().enumerate().filter(|(_, o)| o.exists).map(|(i, _)| i).collect()
    }

    fn find(&self, id: i32) -> Option<usize> {
        if id == 0 {
            return None;
        }
        self.objs.borrow().iter().position(|o| o.exists && o.id == id)
    }

    /// The slot of an object the C++ holds a pointer to, whether or not
    /// `findObjectByID` would find it.
    fn slot(&self, id: i32) -> usize {
        self.objs.borrow().iter().position(|o| o.id == id).expect("object")
    }

    fn view(&self, k: usize) -> ObjectView {
        self.objs.borrow()[k].view()
    }

    /// `newObject`: a slot with the defaults, and the log of the call.
    fn new_object(&mut self, name: usize) -> i32 {
        let id = self.next_id;
        self.next_id += 1;
        self.objs.get_mut().push(Obj::fresh(id, name));
        self.rec(41, &[hash_of(VOCAB[name].as_bytes()), id as u32]);
        id
    }

    fn set_pos(&mut self, id: i32, p: [f32; 3]) {
        self.rec(42, &[id as u32, fb(p[0]), fb(p[1]), fb(p[2])]);
        let k = self.find(id).expect("object");
        self.objs.get_mut()[k].pos = p;
    }

    fn set_orient(&mut self, id: i32, a: f32) {
        self.rec(43, &[id as u32, fb(a)]);
        let k = self.find(id).expect("object");
        self.objs.get_mut()[k].orient = a;
    }
}

impl TerrainWorld for TestWorld {
    fn frame(&self) -> u32 {
        self.rec(18, &[self.frame]);
        self.frame
    }

    fn objects(&self) -> Vec<ObjectView> {
        self.rec(17, &[]);
        self.existing().into_iter().map(|k| self.view(k)).collect()
    }

    fn objects_in_range_2d(&self, center: &Coord3D, range: f32) -> Vec<ObjectView> {
        self.rec(16, &[fb(center.x), fb(center.y), fb(center.z), fb(range), 0]);
        let mut v = Vec::new();
        for k in self.existing() {
            let o = self.view(k);
            let dx = o.position.x - center.x;
            let dy = o.position.y - center.y;
            if dx * dx + dy * dy <= range * range {
                v.push(o);
            }
        }
        v
    }

    fn object_exists(&self, id: ObjectId) -> bool {
        let found = self.find(id).is_some();
        self.rec(10, &[id as u32, u32::from(found)]);
        found
    }

    fn object_damage_state(&self, id: ObjectId) -> BodyDamageType {
        self.rec(11, &[id as u32]);
        self.objs.borrow()[self.find(id).expect("object")].body
    }

    fn bridge_scaffold_present(&self, id: ObjectId) -> Option<bool> {
        let code = self.objs.borrow()[self.find(id).expect("object")].scaffold;
        self.rec(12, &[id as u32, code as u32]);
        match code {
            0 => None,
            c => Some(c == 2),
        }
    }

    fn object_drawable(&self, id: ObjectId) -> Option<DrawableId> {
        let found = self.find(id);
        self.rec(10, &[id as u32, u32::from(found.is_some())]);
        let k = found?;
        let d = self.objs.borrow()[k].drawable;
        self.rec(13, &[id as u32, d as u32]);
        (d != 0).then_some(d)
    }

    fn attempt_damage(&mut self, id: ObjectId, info: &DamageInfo) {
        self.rec(14, &[id as u32, info.input.damage_type as u32, info.input.death_type as u32, info.input.source_id as u32, fb(info.input.amount)]);
    }

    fn destroy_object(&mut self, id: ObjectId) {
        self.rec(15, &[id as u32]);
        let k = self.find(id).expect("object");
        self.objs.get_mut()[k].exists = false;
    }

    fn pathfinder_add_bridge(&mut self, bridge: &Bridge) -> i32 {
        let layer = self.next_layer;
        self.next_layer += 1;
        let b = bridge.bounds();
        self.rec(5, &[fb(b.lo.x), fb(b.lo.y), fb(b.hi.x), fb(b.hi.y), bridge.info().bridge_object_id as u32, layer as u32]);
        layer
    }

    fn pathfinder_change_bridge_state(&mut self, layer: i32, enable: bool) {
        self.rec(6, &[layer as u32, u32::from(enable)]);
    }

    fn pathfinder_force_map_recalculation(&mut self) {
        self.rec(7, &[]);
    }

    fn pathfinder_wall_height(&self) -> f32 {
        self.rec(8, &[]);
        self.wall_height
    }

    fn pathfinder_is_point_on_wall(&self, pos: &Coord3D) -> bool {
        let on = self.wall_on && pos.x >= self.wall[0] && pos.x <= self.wall[2] && pos.y >= self.wall[1] && pos.y <= self.wall[3];
        self.rec(9, &[fb(pos.x), fb(pos.y), fb(pos.z), u32::from(on)]);
        on
    }

    fn create_generic_bridge_object(&mut self, center: &Coord3D, orientation: f32, _props: &Dict) -> Option<ObjectId> {
        self.rec(40, &[hash_of(b"GenericBridge"), u32::from(self.generic)]);
        if !self.generic {
            return None;
        }
        let id = self.new_object(vocab("GenericBridge"));
        self.set_pos(id, [center.x, center.y, center.z]);
        self.rec(44, &[id as u32]); // updateObjValuesFromMapProperties
        self.set_orient(id, orientation);
        Some(id)
    }

    fn find_bridge_road_template(&self, name: &AsciiString) -> bool {
        self.rec(50, &[hash_of(name.str()), u32::from(self.road)]);
        self.road
    }

    fn find_bridge_tower_template(&self, _bridge_template: &AsciiString, tower: BridgeTowerType) -> Option<f32> {
        let t = tower as usize;
        let found = (self.tower_mask >> t) & 1 == 1;
        self.rec(40, &[hash_of(VOCAB[vocab(&format!("BridgeTower{t}"))].as_bytes()), u32::from(found)]);
        found.then_some(self.tower_major[t])
    }

    fn create_bridge_tower(&mut self, bridge: ObjectId, tower: BridgeTowerType, pos: &Coord3D, angle: f32) -> ObjectId {
        let id = self.new_object(vocab(&format!("BridgeTower{}", tower as usize)));
        self.set_pos(id, [pos.x, pos.y, pos.z]);
        self.set_orient(id, angle);
        // the bridge's behaviour interface, then the tower's
        let code = self.objs.borrow()[self.slot(bridge)].scaffold;
        self.rec(12, &[bridge as u32, code as u32]);
        if code != 0 {
            self.rec(45, &[bridge as u32, tower as u32, id as u32]);
        }
        self.rec(49, &[id as u32]);
        self.rec(46, &[id as u32, bridge as u32]);
        self.rec(47, &[id as u32, tower as u32]);
        if self.objs.borrow()[self.slot(bridge)].indestructible {
            self.rec(48, &[id as u32, 1]);
            let k = self.slot(id);
            self.objs.get_mut()[k].indestructible = true;
        }
        id
    }

    fn before_map_boundary_change(&mut self) {
        self.rec(19, &[]);
        self.rec(20, &[1]);
    }

    fn after_map_boundary_change(&mut self, terrain: &TerrainLogic) {
        self.rec(21, &[]);
        self.rec(17, &[]);
        for k in self.existing() {
            let id = self.objs.borrow()[k].id;
            self.rec(22, &[id as u32]);
        }
        self.rec(20, &[0]);
        self.rec(24, &[]);
        self.rec(25, &[]);
        self.rec(26, &[terrain.active_boundary() as u32]);
        self.rec(27, &[0]);
        self.rec(28, &[1]);
        self.rec(17, &[]);
        for k in self.existing() {
            let id = self.objs.borrow()[k].id;
            self.rec(29, &[id as u32]);
        }
        self.rec(27, &[1]);
        self.rec(31, &[]);
        self.rec(28, &[0]);
        self.rec(33, &[]);
    }
}

// ---------------------------------------------------------------------------
// Xfer into memory
// ---------------------------------------------------------------------------

struct MemXfer {
    state: XferState,
    bytes: Vec<u8>,
    pos: usize,
}

impl MemXfer {
    fn new(mode: XferMode) -> Self {
        MemXfer { state: XferState::new(mode), bytes: Vec::new(), pos: 0 }
    }
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
    fn xfer_snapshot(&mut self, _snapshot: &mut dyn rts_lockstep_engine::common::xfer::Snapshot) -> XferResult<()> {
        Ok(())
    }
    fn xfer_implementation(&mut self, data: &mut [u8]) -> XferResult<()> {
        if self.state.mode == XferMode::Save {
            self.bytes.extend_from_slice(data);
        } else {
            if self.pos + data.len() > self.bytes.len() {
                return Err(XferStatus::ReadError);
            }
            data.copy_from_slice(&self.bytes[self.pos..self.pos + data.len()]);
            self.pos += data.len();
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The Rust interpreter
// ---------------------------------------------------------------------------

/// Why a case is not compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Skip {
    Undefined(TerrainUndefined),
    ChunkUndefined,
    /// `pickBridge` left its position uninitialised (the last bridge missed)
    PickPosition,
}

impl From<TerrainLogicError> for Skip {
    fn from(e: TerrainLogicError) -> Skip {
        match e {
            TerrainLogicError::Undefined(u) => Skip::Undefined(u),
            TerrainLogicError::Chunk(_) => Skip::ChunkUndefined,
            other => panic!("unexpected error {other:?}"),
        }
    }
}

impl From<TerrainUndefined> for Skip {
    fn from(u: TerrainUndefined) -> Skip {
        Skip::Undefined(u)
    }
}

fn water_code(h: Option<WaterHandle>) -> i32 {
    match h {
        None => -1,
        Some(WaterHandle::Grid) => -2,
        Some(WaterHandle::Polygon(i)) => i as i32,
    }
}

fn event_words(e: &TerrainVisualEvent) -> (u32, Vec<u32>) {
    match e {
        TerrainVisualEvent::Load { filename } => (60, vec![hash_of(filename.str())]),
        TerrainVisualEvent::EnableWaterGrid { enable } => (61, vec![u32::from(*enable)]),
        TerrainVisualEvent::SetWaterGridHeightClamps { min_z, max_z } => (62, vec![fb(*min_z), fb(*max_z)]),
        TerrainVisualEvent::SetWaterTransform { angle, x, y, z } => (63, vec![fb(*angle), fb(*x), fb(*y), fb(*z)]),
        TerrainVisualEvent::SetWaterTransformMatrix { transform } => (64, transform.to_array().iter().map(|&f| fb(f)).collect()),
        TerrainVisualEvent::SetWaterGridResolution { cells_x, cells_y, cell_size } => (65, vec![fb(*cells_x), fb(*cells_y), fb(*cell_size)]),
        TerrainVisualEvent::SetWaterAttenuationFactors { a, b, c, range } => (66, vec![fb(*a), fb(*b), fb(*c), fb(*range)]),
        TerrainVisualEvent::SetRawMapHeight { x, y, height } => (67, vec![*x as u32, *y as u32, *height as u32]),
    }
}

struct Ctx {
    ts: TerrainLogic,
    world: TestWorld,
    gd: GlobalData,
    keys: WaypointKeys,
    map_objs: Vec<WaypointMapObject>,
    file_opens: bool,
    file: Vec<u8>,
    last_save: Vec<u8>,
    n_initial_objs: usize,
}

fn make_keys() -> WaypointKeys {
    let mut gen = NameKeyGenerator::new();
    gen.init();
    let w = WellKnownKeys::new();
    WaypointKeys {
        id: w.key(wk::WAYPOINT_ID, &mut gen),
        name: w.key(wk::WAYPOINT_NAME, &mut gen),
        label1: w.key(wk::WAYPOINT_PATH_LABEL1, &mut gen),
        label2: w.key(wk::WAYPOINT_PATH_LABEL2, &mut gen),
        label3: w.key(wk::WAYPOINT_PATH_LABEL3, &mut gen),
        bi_directional: w.key(wk::WAYPOINT_PATH_BI_DIRECTIONAL, &mut gen),
    }
}

impl Ctx {
    fn new(sc: &Scenario) -> Ctx {
        let keys = make_keys();
        let visual = rts_lockstep_engine::logic::terrain_visual::TerrainVisualState {
            water_grid: if sc.render { WaterGrid::default() } else { WaterGrid::without_render_object() },
            height_map: sc
                .hm
                .as_ref()
                .map(|h| LogicHeightMap::new(h.w, h.h, h.border, h.bounds.iter().map(|&(x, y)| ICoord2D { x, y }).collect(), h.data.clone()).unwrap()),
        };
        let mut ts = TerrainLogic::with_visual(if sc.w3d { GroundSource::LogicHeightMap } else { GroundSource::Base }, visual);
        ts.set_boundaries(sc.bounds.iter().map(|&(x, y)| ICoord2D { x, y }).collect(), sc.active);
        ts.set_map_fields(sc.map_dx, sc.map_dy, sc.min_z, sc.max_z);
        ts.set_triggers(
            sc.trigs
                .iter()
                .map(|t| {
                    TriggerArea::new(ascii(t.name), t.id, t.water, t.pts.iter().map(|p| ICoord3D { x: p[0], y: p[1], z: p[2] }).collect()).unwrap()
                })
                .collect(),
        );
        let mut gd = GlobalData::new();
        gd.map_name = VOCAB[sc.map_name].as_bytes().to_vec();
        for (i, s) in sc.settings.iter().enumerate() {
            gd.vertex_water_available_maps[i] = VOCAB[s.map].as_bytes().to_vec();
            gd.vertex_water_height_clamp_low[i] = s.f[0];
            gd.vertex_water_height_clamp_hi[i] = s.f[1];
            gd.vertex_water_angle[i] = s.f[2];
            gd.vertex_water_x_position[i] = s.f[3];
            gd.vertex_water_y_position[i] = s.f[4];
            gd.vertex_water_z_position[i] = s.f[5];
            gd.vertex_water_x_grid_cells[i] = s.cells.0;
            gd.vertex_water_y_grid_cells[i] = s.cells.1;
            gd.vertex_water_grid_size[i] = s.size;
            gd.vertex_water_attenuation_a[i] = s.att[0];
            gd.vertex_water_attenuation_b[i] = s.att[1];
            gd.vertex_water_attenuation_c[i] = s.att[2];
            gd.vertex_water_attenuation_range[i] = s.att[3];
        }
        let map_objs = sc
            .maps
            .iter()
            .map(|m| {
                let mut d = Dict::new();
                d.set_int(keys.id, m.id).unwrap();
                d.set_ascii_string(keys.name, VOCAB[m.name].as_bytes()).unwrap();
                for (k, key) in [keys.label1, keys.label2, keys.label3].into_iter().enumerate() {
                    if m.labels[k] >= 0 {
                        d.set_ascii_string(key, VOCAB[m.labels[k] as usize].as_bytes()).unwrap();
                    }
                }
                if m.bidir != 0 {
                    d.set_bool(keys.bi_directional, m.bidir == 2).unwrap();
                }
                WaypointMapObject { is_waypoint: m.is_wp, location: Coord3D { x: m.loc[0], y: m.loc[1], z: m.loc[2] }, properties: d }
            })
            .collect();
        Ctx {
            ts,
            world: TestWorld::new(sc),
            gd,
            keys,
            map_objs,
            file_opens: sc.file_opens,
            file: sc.file.clone(),
            last_save: Vec::new(),
            n_initial_objs: sc.objs.len(),
        }
    }

    fn handle(&self, code: i32) -> Option<WaterHandle> {
        match code {
            -1 => None,
            -2 => Some(WaterHandle::Grid),
            k => {
                let k = k as usize;
                self.ts.triggers()[k].is_water_area().then_some(WaterHandle::Polygon(k))
            }
        }
    }

    fn list_pos(&self, h: Option<WaypointHandle>) -> i32 {
        match h {
            None => -1,
            Some(h) => self.ts.waypoint_handles().iter().position(|&x| x == h).map_or(-1, |p| p as i32),
        }
    }

    fn digest(&self) -> u32 {
        let mut h: u32 = 2_166_136_261;
        if let Some(m) = &self.ts.visual().height_map {
            for &b in m.data() {
                h ^= u32::from(b);
                h = h.wrapping_mul(16_777_619);
            }
        }
        h
    }
}

fn coord(a: &[u32], i: usize) -> Coord3D {
    Coord3D { x: f32::from_bits(a[i]), y: f32::from_bits(a[i + 1]), z: f32::from_bits(a[i + 2]) }
}

fn putc(o: &mut Vec<u32>, c: &Coord3D) {
    o.extend_from_slice(&[fb(c.x), fb(c.y), fb(c.z)]);
}

fn flt(a: &[u32], i: usize) -> f32 {
    f32::from_bits(a[i])
}

/// Run one operation; its results.
fn exec(c: &mut Ctx, op: &Op) -> Result<Vec<u32>, Skip> {
    let a = &op.args[..];
    let mut o: Vec<u32> = Vec::new();
    match op.code {
        0 => c.world.frame = a[0],
        1 => {
            let k = a[0] as usize;
            let ob = &mut c.world.objs.get_mut()[k];
            ob.body = a[1] as i32;
            ob.scaffold = a[2] as i32;
            ob.exists = a[3] != 0;
            ob.pos = [flt(a, 4), flt(a, 5), flt(a, 6)];
            ob.orient = flt(a, 7);
            ob.layer = a[8] as i32;
        }
        2 => c.ts.visual_mut().water_grid.set_mesh_height(a[0] as i32, a[1] as i32, flt(a, 2)),
        3 => c.gd.map_name = VOCAB[a[0] as usize].as_bytes().to_vec(),
        10 => {
            let fname = ascii(a[0] as usize);
            let stream = if c.file_opens { CachedFileInputStream::new(c.file.clone()) } else { None };
            let r = c.ts.load_map(&fname, a[1] != 0, &c.map_objs, &c.keys, stream)?;
            o.push(u32::from(r));
        }
        11 => c.ts.new_map(&c.gd)?,
        12 => {
            let want = a[2] != 0;
            let (h, n) = c.ts.ground_height(flt(a, 0), flt(a, 1), want);
            o.push(fb(h));
            if want {
                putc(&mut o, &n.unwrap());
            }
        }
        13 => {
            let want = a[3] != 0;
            let (h, n) = c.ts.layer_height(&c.world, flt(a, 0), flt(a, 1), a[2] as i32, want, a[4] != 0)?;
            o.push(fb(h));
            if want {
                putc(&mut o, &n.unwrap());
            }
        }
        14 => {
            match a[0] {
                0 | 1 => {
                    let e: Region3D = if a[0] == 0 { c.ts.extent()? } else { c.ts.maximum_pathfind_extent()? };
                    putc(&mut o, &e.lo);
                    putc(&mut o, &e.hi);
                }
                _ => {
                    // the z of this one is never written upstream
                    let e = c.ts.extent_including_border()?;
                    o.extend_from_slice(&[fb(e.lo.x), fb(e.lo.y), fb(e.hi.x), fb(e.hi.y)]);
                }
            }
        }
        15 => putc(&mut o, &c.ts.find_closest_edge_point(&coord(a, 0))?),
        16 => putc(&mut o, &c.ts.find_farthest_edge_point(&coord(a, 0))?),
        17 => {
            let (layer, m) = c.ts.align_on_terrain(&c.world, flt(a, 0), &coord(a, 1), a[4] != 0)?;
            o.push(layer as u32);
            o.extend(m.to_array().iter().map(|&f| fb(f)));
        }
        20 => {
            let hs = c.ts.waypoint_handles();
            o.push(hs.len() as u32);
            for &h in &hs {
                let w = c.ts.waypoint(h).unwrap().clone();
                o.push(w.id() as u32);
                o.push(hash_of(w.name().str()));
                putc(&mut o, w.location());
                o.push(w.num_links() as u32);
                for k in 0..w.num_links() {
                    let l = c.ts.waypoint_link(h, k)?;
                    o.push(c.list_pos(l) as u32);
                }
                o.push(hash_of(w.path_label1().str()));
                o.push(hash_of(w.path_label2().str()));
                o.push(hash_of(w.path_label3().str()));
                o.push(u32::from(w.bi_directional()));
            }
        }
        21 => o.push(c.list_pos(c.ts.waypoint_by_name(&ascii(a[0] as usize))) as u32),
        22 => o.push(c.list_pos(c.ts.waypoint_by_id(a[0])) as u32),
        23 => o.push(c.list_pos(c.ts.closest_waypoint_on_path(&coord(a, 0), &ascii(a[3] as usize))) as u32),
        24 => {
            let wi = a[0] as i32;
            let h = if wi >= 0 { c.ts.waypoint_handles().get(wi as usize).copied() } else { None };
            o.push(u32::from(c.ts.is_purpose_of_path(h, &ascii(a[1] as usize))));
        }
        25 => o.push(c.ts.trigger_area_by_name(&ascii(a[0] as usize)).map_or(-1i32, |i| i as i32) as u32),
        30 => {
            let info = BridgeInfo {
                from: coord(a, 0),
                to: coord(a, 3),
                bridge_width: flt(a, 6),
                from_left: coord(a, 7),
                from_right: coord(a, 10),
                to_left: coord(a, 13),
                to_right: coord(a, 16),
                bridge_index: a[19] as i32,
                bridge_object_id: a[20] as i32,
                ..BridgeInfo::default()
            };
            c.ts.add_bridge_to_logic(&mut c.world, &info, &Dict::new(), &ascii(a[21] as usize));
        }
        31 => {
            let k = a[0] as usize;
            let ob = c.world.objs.borrow()[k].clone();
            let v = ob.view();
            let lo = LandmarkBridgeObject { id: ob.id, template_name: ascii(ob.name), position: v.position, orientation: v.orientation, geometry: v.geometry };
            c.ts.add_landmark_bridge_to_logic(&mut c.world, &lo);
        }
        32 => {
            let bs = c.ts.bridges();
            o.push(bs.len() as u32);
            for b in bs {
                let i = b.info();
                o.push(hash_of(b.template_name().str()));
                o.push(b.layer() as u32);
                putc(&mut o, &i.from);
                putc(&mut o, &i.to);
                o.push(fb(i.bridge_width));
                for p in [&i.from_left, &i.from_right, &i.to_left, &i.to_right] {
                    putc(&mut o, p);
                }
                o.push(i.bridge_index as u32);
                o.push(i.cur_damage_state as u32);
                o.push(i.bridge_object_id as u32);
                for t in i.tower_object_id {
                    o.push(t as u32);
                }
                o.push(u32::from(i.damage_state_changed));
                let r: &Region2D = b.bounds();
                o.extend_from_slice(&[fb(r.lo.x), fb(r.lo.y), fb(r.hi.x), fb(r.hi.y)]);
            }
        }
        33 => o.push(c.ts.find_bridge_at(&coord(a, 0)).map_or(-1i32, |i| i as i32) as u32),
        34 => o.push(c.ts.find_bridge_layer_at(&coord(a, 0), a[3] as i32, a[4] != 0).map_or(-1i32, |i| i as i32) as u32),
        35 => o.push(c.ts.layer_for_destination(&c.world, &coord(a, 0))? as u32),
        36 => o.push(c.ts.highest_layer_for_destination(&c.world, &coord(a, 0), a[3] != 0)? as u32),
        37 => {
            let v = c.world.view(a[0] as usize);
            o.push(u32::from(c.ts.object_interacts_with_bridge_layer(&c.world, &v, a[1] as i32, a[2] != 0)?));
        }
        38 => {
            let v = c.world.view(a[0] as usize);
            o.push(u32::from(c.ts.object_interacts_with_bridge_end(&v, a[1] as i32)?));
        }
        39 => {
            c.ts.update_bridge_damage_states(&mut c.world)?;
            o.push(u32::from(c.ts.any_bridges_damage_states_changed()));
        }
        41 => {
            let k = a[0] as i32;
            let id = if k >= 0 { Some(c.world.objs.borrow()[k as usize].id) } else { None };
            o.push(u32::from(c.ts.is_bridge_repaired(id)));
            o.push(u32::from(c.ts.is_bridge_broken(id)));
        }
        43 => {
            let ob = c.world.objs.borrow()[a[0] as usize].clone();
            let v = ob.view();
            let ai = c.ts.bridge_attack_points(ob.id, &v.position);
            putc(&mut o, &ai.attack_point1);
            putc(&mut o, &ai.attack_point2);
        }
        44 => {
            let from = Vector3::new(flt(a, 0), flt(a, 1), flt(a, 2));
            let to = Vector3::new(flt(a, 3), flt(a, 4), flt(a, 5));
            let p = c.ts.pick_bridge(&c.world, &from, &to)?;
            let pos = p.pos.ok_or(Skip::PickPosition)?;
            o.push(p.drawable.unwrap_or(0) as u32);
            o.extend_from_slice(&[fb(pos.x), fb(pos.y), fb(pos.z)]);
        }
        45 => c.ts.delete_bridge(&mut c.world, a[0] as usize),
        46 => c.ts.load_post_process(&mut c.world),
        47 => {
            let n = c.ts.bridges().len();
            if n > 0 {
                let b = c.ts.bridges()[a[0] as usize % n].clone();
                let p = coord(a, 1);
                o.push(u32::from(b.is_point_on_bridge(&p)));
                let (h, nrm) = c.ts.bridge_height(a[0] as usize % n, &p)?;
                o.push(fb(h));
                putc(&mut o, &nrm);
            }
        }
        48 => {
            let n = c.ts.bridges().len();
            if n > 0 {
                let b = &c.ts.bridges()[a[0] as usize % n];
                let mut rg = Region2D::default();
                rg.lo.x = flt(a, 1);
                rg.lo.y = flt(a, 2);
                rg.hi.x = flt(a, 3);
                rg.hi.y = flt(a, 4);
                o.push(u32::from(b.is_cell_on_end(&rg)));
                o.push(u32::from(b.is_cell_on_side(&rg)));
                o.push(u32::from(b.is_cell_entry_point(&rg)));
            }
        }
        50 => {
            let h = c.ts.water_handle(flt(a, 0), flt(a, 1))?;
            o.push(water_code(h) as u32);
        }
        51 => {
            let u = c.ts.is_underwater(flt(a, 0), flt(a, 1))?;
            o.push(u32::from(u.underwater));
            o.push(u32::from(u.water_z.is_some()));
            o.push(u.water_z.map_or(0x7fc1_2345, fb));
            o.push(fb(u.terrain_z));
        }
        52 => o.push(water_code(c.ts.water_handle_by_name(&ascii(a[0] as usize))) as u32),
        53 => {
            let h = c.handle(a[0] as i32);
            o.push(fb(c.ts.water_height(h)?));
        }
        54 => {
            let h = c.handle(a[0] as i32);
            c.ts.set_water_height(&mut c.world, h, flt(a, 1), flt(a, 2), a[3] != 0)?;
        }
        55 => {
            let h = c.handle(a[0] as i32);
            c.ts.change_water_height_over_time(h, flt(a, 1), flt(a, 2), flt(a, 3))?;
        }
        56 => c.ts.update(&mut c.world)?,
        57 => {
            let ch = c.ts.water_changes();
            o.push(ch.len() as u32);
            for e in ch {
                o.push(water_code(e.water_table) as u32);
                o.extend_from_slice(&[fb(e.change_per_frame), fb(e.target_height), fb(e.damage_amount), fb(e.current_height)]);
            }
        }
        58 => {
            let h = c.handle(a[0] as i32);
            let mut rg = Region3D::default();
            rg.zero();
            c.ts.find_axis_aligned_bounding_rect(h, Some(&mut rg))?;
            putc(&mut o, &rg.lo);
            putc(&mut o, &rg.hi);
        }
        59 => c.ts.enable_water_grid(a[0] != 0, &c.gd)?,
        60 => {
            c.ts.set_active_boundary(&mut c.world, a[0] as i32);
            o.push(c.ts.active_boundary() as u32);
        }
        61 => {
            let v = c.world.view(a[0] as usize);
            c.ts.flatten_terrain(&v)?;
            o.push(c.digest());
        }
        62 => {
            let v = c.world.view(a[0] as usize);
            c.ts.create_crater_in_terrain(&v)?;
            o.push(c.digest());
        }
        63 => {
            let mut x = MemXfer::new(XferMode::Save);
            c.ts.xfer(&mut x, &mut c.world)?;
            c.last_save = x.bytes.clone();
            o.push(c.last_save.len() as u32);
            o.extend(c.last_save.iter().map(|&b| u32::from(b)));
        }
        64 => {
            let mut x = MemXfer::new(XferMode::Load);
            x.bytes = c.last_save.clone();
            let status: i32 = match c.ts.xfer(&mut x, &mut c.world) {
                Ok(()) => 0,
                Err(TerrainLogicError::Xfer(s)) => 100 + s as i32,
                Err(TerrainLogicError::InvalidData) => 206,
                Err(e) => return Err(e.into()),
            };
            o.push(status as u32);
        }
        65 => c.ts.reset(),
        66 => {
            o.push(u32::from(c.ts.water_grid_enabled()));
            o.push(c.ts.active_boundary() as u32);
            o.push(hash_of(c.ts.source_filename().str()));
        }
        70 => o.push(c.ts.visual().raw_map_height(&ICoord2D { x: a[0] as i32, y: a[1] as i32 }) as u32),
        72 => {
            let s = c.ts.visual().water_grid.water_grid_height(flt(a, 0), flt(a, 1))?;
            match s {
                WaterGridSample::NotOnGrid => o.extend_from_slice(&[0, fb(777.0)]),
                WaterGridSample::OnGrid(z) => o.extend_from_slice(&[1, fb(z)]),
                WaterGridSample::OnGridUnset => o.extend_from_slice(&[1, fb(777.0)]),
            }
        }
        73 => {
            let ts = c.ts.triggers();
            o.push(ts.len() as u32);
            for t in ts {
                o.push(t.id() as u32);
                o.push(hash_of(t.name().str()));
                o.push(u32::from(t.is_water_area()));
                o.push(t.num_points() as u32);
                for q in 0..t.num_points() {
                    let p = t.point(q);
                    o.extend_from_slice(&[p.x as u32, p.y as u32, p.z as u32]);
                }
            }
        }
        other => panic!("unknown op {other}"),
    }
    Ok(o)
}

/// Run a case in the Rust: the words the C++ would write, or why it cannot be
/// compared. Also: how many times the terrain counted an undefined input.
fn run_rust(sc: &Scenario, prog: &[Op]) -> (Result<Vec<u32>, Skip>, u32) {
    let mut c = Ctx::new(sc);
    let mut out = Vec::new();
    for o in prog {
        c.world.log.borrow_mut().clear();
        let _ = c.ts.take_events();
        let res = match exec(&mut c, o) {
            Ok(r) => r,
            Err(s) => return (Err(s), c.ts.undefined_inputs()),
        };
        out.push(o.code);
        out.push(res.len() as u32);
        out.extend_from_slice(&res);
        let log = c.world.log.borrow().clone();
        out.push(log.len() as u32);
        out.extend_from_slice(&log);
        let mut vl: Vec<u32> = Vec::new();
        for e in c.ts.take_events() {
            let (code, words) = event_words(&e);
            vl.push(code);
            vl.push(words.len() as u32);
            vl.extend(words);
        }
        out.push(vl.len() as u32);
        out.extend_from_slice(&vl);
    }
    let _ = c.n_initial_objs;
    (Ok(out), c.ts.undefined_inputs())
}

fn run_cpp(sc: &Scenario, prog: &[Op]) -> Vec<u32> {
    let s = sc.encode();
    let p = encode_program(prog);
    let mut out = vec![0u32; 1 << 20];
    let n = unsafe { cpp::ref_terrain_logic_run(s.as_ptr(), s.len() as i32, p.as_ptr(), p.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "the C++ output did not fit");
    out.truncate(n as usize);
    out
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

fn is_nan_bits(w: u32) -> bool {
    (w & 0x7f80_0000) == 0x7f80_0000 && (w & 0x007f_ffff) != 0
}

/// One word, NaN payloads aside.
fn word_eq(a: u32, b: u32) -> Option<bool> {
    if a == b {
        Some(true)
    } else if is_nan_bits(a) && is_nan_bits(b) {
        None
    } else {
        Some(false)
    }
}

struct Checks {
    by_code: std::collections::BTreeMap<u32, Check>,
    world_log: Check,
    visual_log: Check,
    framing: Check,
    /// How often each kind of event or result occurred in the compared cases:
    /// what the comparison actually exercised.
    cover: std::collections::BTreeMap<String, u64>,
}

fn world_name(code: u32) -> &'static str {
    match code {
        5 => "pathfinder.addBridge",
        6 => "pathfinder.changeBridgeState",
        7 => "pathfinder.forceMapRecalculation",
        8 => "pathfinder.getWallHeight",
        9 => "pathfinder.isPointOnWall",
        10 => "findObjectByID",
        11 => "getDamageState",
        12 => "getBridgeBehaviorInterface",
        13 => "getDrawable",
        14 => "attemptDamage",
        15 => "destroyObject",
        16 => "iterateObjectsInRange",
        17 => "getFirstObject",
        18 => "getFrame",
        19 => "boundary: processPendingUndoShroudRevealQueue",
        20 => "boundary: storeFoggedCells",
        21 => "boundary: releasePartitionData",
        22 => "boundary: friend_prepareForMapBoundaryAdjust",
        24 => "boundary: partition reset",
        25 => "boundary: partition init",
        26 => "boundary: radar newMap",
        27 => "boundary: restoreFoggedCells",
        28 => "boundary: lockGhostObjects",
        29 => "boundary: friend_notifyOfNewMapBoundary",
        31 => "boundary: restorePartitionData",
        33 => "boundary: forceCameraConstraintRecalc",
        40 => "findTemplate",
        41 => "newObject",
        42 => "object setPosition",
        43 => "object setOrientation",
        44 => "updateObjValuesFromMapProperties",
        45 => "bridge setTower",
        46 => "tower setBridge",
        47 => "tower setTowerType",
        48 => "setIndestructible",
        49 => "getBridgeTowerBehaviorInterface",
        50 => "findBridge (road template)",
        _ => "?",
    }
}

fn visual_name(code: u32) -> &'static str {
    match code {
        60 => "load",
        61 => "enableWaterGrid",
        62 => "setWaterGridHeightClamps",
        63 => "setWaterTransform(angle, x, y, z)",
        64 => "setWaterTransform(matrix)",
        65 => "setWaterGridResolution",
        66 => "setWaterAttenuationFactors",
        67 => "setRawMapHeight",
        _ => "?",
    }
}

/// The records of a log: `(code, words)`.
fn records(l: &[u32]) -> Vec<(u32, &[u32])> {
    let mut v = Vec::new();
    let mut i = 0;
    while i + 1 < l.len() {
        let n = l[i + 1] as usize;
        v.push((l[i], &l[i + 2..i + 2 + n]));
        i += 2 + n;
    }
    v
}

impl Checks {
    fn new(prefix: &str) -> Self {
        Checks {
            by_code: std::collections::BTreeMap::new(),
            world_log: Check::new(&format!("{prefix}: world log")),
            visual_log: Check::new(&format!("{prefix}: visual (client event) log")),
            framing: Check::new(&format!("{prefix}: output framing")),
            cover: std::collections::BTreeMap::new(),
        }
    }

    fn bump(&mut self, key: String, n: u64) {
        *self.cover.entry(key).or_insert(0) += n;
    }

    /// What one operation's (agreeing) output exercised.
    fn tally(&mut self, code: u32, results: &[u32], world: &[u32], visual: &[u32]) {
        for (c, _) in records(world) {
            self.bump(format!("world  {c:>2} {}", world_name(c)), 1);
        }
        for (c, _) in records(visual) {
            self.bump(format!("visual {c:>2} {}", visual_name(c)), 1);
        }
        let flag = |r: &[u32], k: usize| r.get(k).copied().unwrap_or(0) != 0;
        match code {
            10 if flag(results, 0) => self.bump("result loadMap true".into(), 1),
            20 => {
                // [n, per waypoint: id, name, loc3, nlinks, links.., labels3, bidir]
                let n = results[0] as usize;
                let mut i = 1;
                for _ in 0..n {
                    let nl = results[i + 5] as usize;
                    self.bump("result waypoint links".into(), nl as u64);
                    i += 6 + nl + 4;
                }
                self.bump("result waypoints".into(), n as u64);
            }
            24 if flag(results, 0) => self.bump("result isPurposeOfPath true".into(), 1),
            33 | 34 if results[0] as i32 >= 0 => self.bump("result bridge found".into(), 1),
            37 | 38 if flag(results, 0) => self.bump("result objectInteractsWithBridge true".into(), 1),
            41 => {
                if flag(results, 0) {
                    self.bump("result isBridgeRepaired true".into(), 1);
                }
                if flag(results, 1) {
                    self.bump("result isBridgeBroken true".into(), 1);
                }
            }
            44 if results[0] != 0 => self.bump("result pickBridge hit a drawable".into(), 1),
            47 if flag(results, 0) => self.bump("result isPointOnBridge true".into(), 1),
            48 => {
                for (k, name) in ["isCellOnEnd", "isCellOnSide", "isCellEntryPoint"].iter().enumerate() {
                    if flag(results, k) {
                        self.bump(format!("result {name} true"), 1);
                    }
                }
            }
            50 if results[0] as i32 != -1 => self.bump("result getWaterHandle found water".into(), 1),
            51 if flag(results, 0) => self.bump("result isUnderwater true".into(), 1),
            57 if results[0] > 0 => self.bump("result water changes pending".into(), results[0] as u64),
            63 if results[0] > 0 => {
                self.bump("result xfer saved bytes".into(), results[0] as u64);
                // version (1) + active boundary (4) + count (4) is a save with no water change in it
                if results[0] > 9 {
                    self.bump("result xfer saved a water change".into(), 1);
                }
            }
            64 if results[0] == 0 => self.bump("result xfer load ok".into(), 1),
            72 if flag(results, 0) => self.bump("result getWaterGridHeight on the grid".into(), 1),
            _ => {}
        }
    }

    /// Print what the comparison exercised; each of `must` must have occurred
    /// at least that often.
    fn finish(self, must: &[(&str, u64)]) {
        for (_, c) in self.by_code {
            c.finish();
        }
        self.world_log.finish();
        self.visual_log.finish();
        self.framing.finish();
        for (k, n) in &self.cover {
            println!("COVER {k:<60} {n:>9}");
        }
        for (k, min) in must {
            let n = self.cover.iter().filter(|(key, _)| key.contains(k)).map(|(_, n)| *n).sum::<u64>();
            assert!(n >= *min, "the cases did not exercise {k:?} enough: {n} < {min}");
        }
    }
}

fn compare_words(chk: &mut Check, r: &[u32], c: &[u32], what: impl Fn() -> String) {
    chk.eq(r.len(), c.len(), || format!("{} (word count)", what()));
    for (k, (&a, &b)) in r.iter().zip(c.iter()).enumerate() {
        match word_eq(a, b) {
            Some(true) => chk.eq(0, 0, String::new),
            Some(false) => chk.eq(a, b, || format!("{} [word {k}]", what())),
            None => chk.eq(0, 0, String::new),
        }
    }
}

/// Compare the Rust and C++ streams of one case, op by op.
fn compare_case(checks: &mut Checks, prog: &[Op], r: &[u32], c: &[u32], case: usize) {
    let (mut ri, mut ci) = (0usize, 0usize);
    for (n, o) in prog.iter().enumerate() {
        let what = || format!("case {case} op {n} {}{:?}", op_name(o.code), o.args);
        let take = |s: &[u32], i: &mut usize| -> Option<Vec<u32>> {
            let len = *s.get(*i)? as usize;
            let v = s.get(*i + 1..*i + 1 + len)?.to_vec();
            *i += 1 + len;
            Some(v)
        };
        // [op, results.., log.., vlog..]
        let (rop, cop) = (r.get(ri).copied(), c.get(ci).copied());
        checks.framing.eq(rop, cop, what);
        if rop != cop || rop.is_none() {
            return;
        }
        ri += 1;
        ci += 1;
        let (Some(rr), Some(cr)) = (take(r, &mut ri), take(c, &mut ci)) else {
            checks.framing.eq(false, true, || format!("{} truncated", what()));
            return;
        };
        let (Some(rl), Some(cl)) = (take(r, &mut ri), take(c, &mut ci)) else {
            checks.framing.eq(false, true, || format!("{} truncated", what()));
            return;
        };
        let (Some(rv), Some(cv)) = (take(r, &mut ri), take(c, &mut ci)) else {
            checks.framing.eq(false, true, || format!("{} truncated", what()));
            return;
        };
        let chk = checks.by_code.entry(o.code).or_insert_with(|| Check::new(&format!("op {:>2} {}", o.code, op_name(o.code))));
        compare_words(chk, &rr, &cr, what);
        compare_words(&mut checks.world_log, &rl, &cl, || format!("{} world log", what()));
        compare_words(&mut checks.visual_log, &rv, &cv, || format!("{} visual log", what()));
        checks.tally(o.code, &cr, &cl, &cv);
    }
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

fn gf(g: &mut Gen, lo: f32, hi: f32) -> f32 {
    g.uniform(lo, hi)
}

fn gi(g: &mut Gen, lo: i32, hi: i32) -> i32 {
    lo + g.below((hi - lo + 1) as u32) as i32
}

fn chance(g: &mut Gen, pct: u32) -> bool {
    g.below(100) < pct
}

/// A chunk file: the table of contents, then the chunks.
fn chunk_file(names: &[&str], chunks: &[(usize, u16, Vec<u8>)]) -> Vec<u8> {
    let mut b = b"CkMp".to_vec();
    b.extend_from_slice(&(names.len() as i32).to_le_bytes());
    for (i, n) in names.iter().enumerate() {
        b.push(n.len() as u8);
        b.extend_from_slice(n.as_bytes());
        b.extend_from_slice(&((i + 1) as u32).to_le_bytes());
    }
    for (name_idx, version, data) in chunks {
        b.extend_from_slice(&((name_idx + 1) as u32).to_le_bytes());
        b.extend_from_slice(&version.to_le_bytes());
        b.extend_from_slice(&(data.len() as i32).to_le_bytes());
        b.extend_from_slice(data);
    }
    b
}

fn gen_waypoint_file(g: &mut Gen, ids: &[i32]) -> (bool, Vec<u8>) {
    if chance(g, 6) {
        return (false, Vec::new()); // the file does not open
    }
    if chance(g, 6) {
        // not a chunk file at all ("legacy": no table of contents)
        return (true, (0..gi(g, 12, 64)).map(|_| g.below(256) as u8).collect());
    }
    let mut data = Vec::new();
    let n = gi(g, 0, 12);
    data.extend_from_slice(&n.to_le_bytes());
    for _ in 0..n {
        let pick = |g: &mut Gen| -> i32 {
            if chance(g, 15) || ids.is_empty() {
                gi(g, 0, 9)
            } else {
                ids[g.below(ids.len() as u32) as usize]
            }
        };
        data.extend_from_slice(&pick(g).to_le_bytes());
        data.extend_from_slice(&pick(g).to_le_bytes());
    }
    if chance(g, 20) {
        // data left over in the chunk: skipped
        data.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7]);
    }
    let mut chunks = vec![];
    let names = ["HeightMapData", "WaypointsList", "ObjectsList"];
    if chance(g, 40) {
        chunks.push((0usize, 1u16, vec![1, 2, 3, 4, 5]));
    }
    chunks.push((1, 1, data));
    if chance(g, 25) {
        chunks.push((2, 3, vec![9, 9, 9]));
    }
    let wrong_label = chance(g, 10);
    let mut f = if wrong_label { chunk_file(&["Other", "Waypoints", "ObjectsList"], &chunks) } else { chunk_file(&names, &chunks) };
    if chance(g, 4) {
        // cut off in the middle: the reader's own undefined input
        let cut = gi(g, 5, f.len() as i32 - 1) as usize;
        f.truncate(cut);
        if f.is_empty() {
            f.push(b'C');
        }
    }
    (true, f)
}

fn gen_scenario(g: &mut Gen, w3d: bool) -> Scenario {
    // the height map
    let hm = if w3d || chance(g, 20) {
        let w = gi(g, 8, 20);
        let h = gi(g, 8, 20);
        let border = gi(g, 0, 2);
        let mut data = Vec::with_capacity((w * h) as usize);
        let mut v = gi(g, 0, 90);
        for _ in 0..w * h {
            v = (v + gi(g, -6, 6)).clamp(0, 255);
            if chance(g, 2) {
                v = gi(g, 0, 255);
            }
            data.push(v as u8);
        }
        let nb = gi(g, 1, 3);
        let bounds = (0..nb).map(|_| (gi(g, 2, (w - 2 * border).max(3)), gi(g, 2, (h - 2 * border).max(3)))).collect();
        Some(Hm { w, h, border, bounds, data })
    } else {
        None
    };
    let hm_bounds: Vec<(i32, i32)> = hm.as_ref().map_or_else(|| vec![(gi(g, 4, 30), gi(g, 4, 30))], |h| h.bounds.clone());
    let mut bounds = hm_bounds;
    if chance(g, 8) {
        bounds.clear();
    }
    let active = if bounds.is_empty() { 0 } else { gi(g, 0, bounds.len() as i32 - 1) };
    let (min_z, span) = (gf(g, 0.0, 20.0), gf(g, 0.0, 100.0));

    // water settings
    let maps = [25usize, 28, 29, 30];
    let settings: Vec<Setting> = maps
        .iter()
        .map(|&m| Setting {
            map: m,
            f: [gf(g, -10.0, 0.0), gf(g, 0.0, 50.0), gf(g, -3.2, 3.2), gf(g, 0.0, 100.0), gf(g, 0.0, 100.0), gf(g, -5.0, 30.0)],
            cells: (gi(g, 4, 16), gi(g, 4, 16)),
            size: gf(g, 6.0, 16.0),
            att: [gf(g, 0.0, 2.0), gf(g, 0.0, 2.0), gf(g, 0.0, 2.0), gf(g, 0.0, 80.0)],
        })
        .collect();
    let mut settings = settings;
    if chance(g, 35) {
        // two settings that differ only in the Y cell count (the case upstream's `m_gridCellsY != m_gridCellsY` ignores)
        settings[1].cells.0 = settings[0].cells.0;
        if settings[1].cells.1 == settings[0].cells.1 {
            settings[1].cells.1 += 3;
        }
    }
    let map_name = match g.below(6) {
        0 => 25, // exact
        1 => 26, // stripped match with "Maps\Dam\Dam.map"
        2 => 27, // case-insensitive
        3 => 33, // no match
        4 => 29,
        _ => 31,
    };

    // the world
    let n_obj = gi(g, 4, 10);
    let extent_x = hm.as_ref().map_or(300.0, |h| h.w as f32 * 10.0);
    let extent_y = hm.as_ref().map_or(300.0, |h| h.h as f32 * 10.0);
    let objs: Vec<Obj> = (0..n_obj)
        .map(|k| {
            let gt = gi(g, 0, 2);
            // a quarter of the objects sit on a grid node with a radius that is
            // a whole number of cells, so a crater's `distance < radius` meets
            // `distance == radius` exactly
            let on_node = chance(g, 25);
            let (px, py) = if on_node { ((gi(g, 1, (extent_x / 10.0) as i32 - 1) * 10) as f32, (gi(g, 1, (extent_y / 10.0) as i32 - 1) * 10) as f32) } else { (gf(g, 0.0, extent_x), gf(g, 0.0, extent_y)) };
            let major = if on_node { (gi(g, 1, 4) * 10) as f32 } else { gf(g, 2.0, 40.0) };
            Obj {
                id: 10 + k,
                pos: [px, py, gf(g, 0.0, 40.0)],
                orient: gf(g, -3.2, 3.2),
                layer: *[1, 1, 2, 3, 4, 15][g.below(6) as usize..].first().unwrap(),
                geom: (gt, chance(g, 15), gf(g, 1.0, 20.0), major, gf(g, 2.0, 30.0)),
                name: *[17usize, 18, 19][g.below(3) as usize..].first().unwrap(),
                body: gi(g, 0, 3),
                indestructible: chance(g, 30),
                drawable: if chance(g, 70) { gi(g, 1, 500) } else { 0 },
                scaffold: gi(g, 0, 2),
                exists: chance(g, 92),
            }
        })
        .collect();
    let trigs: Vec<Trig> = (0..gi(g, 0, 3))
        .map(|k| {
            let (x0, y0) = (gi(g, 0, 150), gi(g, 0, 150));
            let (dx, dy) = (gi(g, 20, 200), gi(g, 20, 200));
            let z = gi(g, -5, 90);
            let pts = if chance(g, 30) {
                vec![[x0, y0, z], [x0 + dx, y0, z], [x0, y0 + dy, z]]
            } else {
                vec![[x0, y0, z], [x0 + dx, y0, z], [x0 + dx, y0 + dy, z], [x0, y0 + dy, z]]
            };
            Trig { name: [12usize, 13, 14][g.below(3) as usize], id: 5 + k, water: chance(g, 85), pts }
        })
        .collect();
    let n_map = gi(g, 0, 8);
    let mut wp_ids = Vec::new();
    let maps: Vec<MapObj> = (0..n_map)
        .map(|_| {
            let is_wp = chance(g, 80);
            let id = if chance(g, 15) && !wp_ids.is_empty() { wp_ids[0] } else { gi(g, 1, 9) };
            if is_wp {
                wp_ids.push(id);
            }
            MapObj {
                is_wp,
                loc: [gf(g, 0.0, extent_x), gf(g, 0.0, extent_y), 0.0],
                id,
                name: if chance(g, 15) { 7 } else { gi(g, 1, 6) as usize },
                labels: [0, 1, 2].map(|_| if chance(g, 55) { *[8i32, 9, 10, 11, 0][g.below(5) as usize..].first().unwrap() } else { -1 }),
                bidir: gi(g, 0, 2),
            }
        })
        .collect();
    let (file_opens, file) = gen_waypoint_file(g, &wp_ids);
    Scenario {
        w3d,
        hm,
        render: chance(g, 93),
        bounds,
        active,
        map_dx: gi(g, 4, 40),
        map_dy: gi(g, 4, 40),
        min_z,
        max_z: min_z + span,
        map_name,
        settings,
        generic: chance(g, 85),
        road: chance(g, 85),
        tower_mask: g.below(16),
        tower_major: [gf(g, 2.0, 20.0), gf(g, 2.0, 20.0), gf(g, 2.0, 20.0), gf(g, 2.0, 20.0)],
        wall_height: gf(g, 5.0, 30.0),
        wall_on: chance(g, 40),
        wall: [gf(g, 0.0, 100.0), gf(g, 0.0, 100.0), gf(g, 100.0, 300.0), gf(g, 100.0, 300.0)],
        frame: g.below(120),
        objs,
        trigs,
        maps,
        file_opens,
        file,
    }
}

/// A position near the map, sometimes outside it.
fn gpos(g: &mut Gen, sc: &Scenario) -> [u32; 3] {
    let ex = sc.hm.as_ref().map_or(300.0, |h| h.w as f32 * 10.0);
    let ey = sc.hm.as_ref().map_or(300.0, |h| h.h as f32 * 10.0);
    let wide = chance(g, 10);
    let (x, y) = if wide { (gf(g, -100.0, ex + 100.0), gf(g, -100.0, ey + 100.0)) } else { (gf(g, 0.0, ex), gf(g, 0.0, ey)) };
    [fb(x), fb(y), fb(gf(g, -5.0, 45.0))]
}

fn gen_op(g: &mut Gen, sc: &Scenario, kinds: &[u32]) -> Op {
    let k = kinds[g.below(kinds.len() as u32) as usize];
    let n_obj = sc.objs.len() as u32;
    let obj = |g: &mut Gen| g.below(n_obj);
    let n_trig = sc.trigs.len() as i32;
    let code = |g: &mut Gen| -> u32 { (gi(g, -2, n_trig - 1)) as u32 };
    match k {
        0 => op(0, &[g.below(200)]),
        1 => {
            let p = gpos(g, sc);
            op(1, &[obj(g), gi(g, 0, 3) as u32, gi(g, 0, 2) as u32, u32::from(chance(g, 90)), p[0], p[1], p[2], fb(gf(g, -3.2, 3.2)), *[1u32, 2, 3, 4, 15][g.below(5) as usize..].first().unwrap()])
        }
        2 => op(2, &[g.below(4), g.below(4), fb(gf(g, -3.0, 6.0))]),
        10 => op(10, &[if chance(g, 6) { 0 } else { 32 }, g.below(2)]),
        11 => op(11, &[]),
        12 => {
            let p = gpos(g, sc);
            op(12, &[p[0], p[1], g.below(2)])
        }
        13 => {
            let p = gpos(g, sc);
            op(13, &[p[0], p[1], *[1u32, 1, 2, 3, 4, 15][g.below(6) as usize..].first().unwrap(), g.below(2), g.below(2)])
        }
        14 => op(14, &[if sc.hm.is_some() { g.below(3) } else { g.below(2) }]),
        15 | 16 => {
            let p = gpos(g, sc);
            op(k, &p)
        }
        17 => {
            let p = gpos(g, sc);
            op(17, &[fb(gf(g, -3.2, 3.2)), p[0], p[1], p[2], g.below(2)])
        }
        20 => op(20, &[]),
        21 => op(21, &[*[1u32, 2, 3, 4, 5, 6, 7, 16][g.below(8) as usize..].first().unwrap()]),
        22 => op(22, &[gi(g, 0, 10) as u32]),
        23 => {
            let p = gpos(g, sc);
            op(23, &[p[0], p[1], p[2], *[8u32, 9, 10, 11, 0, 16][g.below(6) as usize..].first().unwrap()])
        }
        24 => op(24, &[gi(g, -1, 5) as u32, *[8u32, 9, 10, 11, 0][g.below(5) as usize..].first().unwrap()]),
        25 => op(25, &[*[12u32, 13, 14, 16][g.below(4) as usize..].first().unwrap()]),
        30 => {
            let c = gpos(g, sc);
            let (cx, cy, cz) = (f32::from_bits(c[0]), f32::from_bits(c[1]), gf(g, 0.0, 30.0));
            let ang = gf(g, -3.2, 3.2);
            let (len, wid) = (gf(g, 20.0, 80.0), gf(g, 6.0, 20.0));
            let (ca, sa) = (ang.cos(), ang.sin());
            let corner = |g: &mut Gen, sx: f32, sy: f32| -> [f32; 3] {
                [cx + sx * len * ca - sy * wid * sa, cy + sx * len * sa + sy * wid * ca, cz + if chance(g, 20) { gf(g, -2.0, 2.0) } else { 0.0 }]
            };
            let fl = corner(g, -1.0, 1.0);
            let fr = corner(g, -1.0, -1.0);
            let tl = corner(g, 1.0, 1.0);
            let tr = corner(g, 1.0, -1.0);
            let mid = |a: [f32; 3], b: [f32; 3]| [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0];
            let (from, to) = (mid(fl, fr), mid(tl, tr));
            let mut args: Vec<u32> = Vec::new();
            for p in [from, to] {
                args.extend(p.iter().map(|&f| fb(f)));
            }
            args.push(fb(2.0 * wid));
            for p in [fl, fr, tl, tr] {
                args.extend(p.iter().map(|&f| fb(f)));
            }
            args.push(g.below(5));
            args.push(if chance(g, 20) { 0 } else { 500 + g.below(50) });
            args.push(*[17u32, 18, 19][g.below(3) as usize..].first().unwrap());
            op(30, &args)
        }
        31 => op(31, &[obj(g)]),
        32 => op(32, &[]),
        33 => {
            let p = gpos(g, sc);
            op(33, &p)
        }
        34 => {
            let p = gpos(g, sc);
            op(34, &[p[0], p[1], p[2], *[1u32, 2, 3, 4, 15][g.below(5) as usize..].first().unwrap(), g.below(2)])
        }
        35 => {
            let p = gpos(g, sc);
            op(35, &p)
        }
        36 => {
            let p = gpos(g, sc);
            op(36, &[p[0], p[1], p[2], g.below(2)])
        }
        37 => op(37, &[obj(g), *[1u32, 2, 3, 4, 15][g.below(5) as usize..].first().unwrap(), g.below(2)]),
        38 => op(38, &[obj(g), *[1u32, 2, 3, 4, 15][g.below(5) as usize..].first().unwrap()]),
        39 => op(39, &[]),
        41 => op(41, &[if chance(g, 8) { u32::MAX } else { obj(g) }]),
        43 => op(43, &[obj(g)]),
        44 => {
            let a = gpos(g, sc);
            let b = gpos(g, sc);
            let mut args = a.to_vec();
            args[2] = fb(gf(g, 30.0, 60.0));
            let mut bb = b.to_vec();
            bb[2] = fb(gf(g, -20.0, 5.0));
            args.extend(bb);
            op(44, &args)
        }
        45 => op(45, &[g.below(4)]),
        46 => op(46, &[]),
        47 => {
            let p = gpos(g, sc);
            op(47, &[g.below(4), p[0], p[1], p[2]])
        }
        48 => {
            let p = gpos(g, sc);
            let (x, y) = (f32::from_bits(p[0]), f32::from_bits(p[1]));
            let s = gf(g, 2.0, 25.0);
            op(48, &[g.below(4), fb(x - s), fb(y - s), fb(x + s), fb(y + s)])
        }
        50 => {
            let p = gpos(g, sc);
            op(50, &[p[0], p[1]])
        }
        51 => {
            let p = gpos(g, sc);
            op(51, &[p[0], p[1]])
        }
        52 => op(52, &[*[12u32, 13, 14, 15, 16][g.below(5) as usize..].first().unwrap()]),
        53 => op(53, &[code(g)]),
        54 => op(54, &[code(g), fb(gf(g, -5.0, 45.0)), fb(if chance(g, 50) { gf(g, 1.0, 50.0) } else { 0.0 }), g.below(2)]),
        55 => op(55, &[code(g), fb(gf(g, -5.0, 45.0)), fb(gf(g, 0.05, 0.6)), fb(if chance(g, 50) { gf(g, 1.0, 50.0) } else { 0.0 })]),
        56 => op(56, &[]),
        57 => op(57, &[]),
        58 => op(58, &[code(g)]),
        59 => op(59, &[g.below(2)]),
        60 => op(60, &[g.below(4)]),
        61 | 62 => op(k, &[obj(g)]),
        63 => op(63, &[]),
        64 => op(64, &[]),
        66 => op(66, &[]),
        70 => op(70, &[gi(g, -2, 22) as u32, gi(g, -2, 22) as u32]),
        72 => {
            let p = gpos(g, sc);
            op(72, &[p[0], p[1]])
        }
        73 => op(73, &[]),
        other => panic!("unknown kind {other}"),
    }
}

/// A bridge that is destroyed and then repaired, with an object standing on
/// it: the sequence random programs rarely produce. Landmark bridge from
/// object 0 (the first bridge, so layer 2), object 1 put on its deck, object
/// 0's damage state set to rubble and back, with an update after each and
/// the repaired/broken queries.
fn bridge_story(g: &mut Gen, sc: &Scenario) -> Vec<Op> {
    let o0 = &sc.objs[0];
    let o1 = &sc.objs[1];
    let set = |k: u32, body: u32, scaffold: u32, exists: bool, pos: [f32; 3], orient: f32, layer: u32| {
        op(1, &[k, body, scaffold, u32::from(exists), fb(pos[0]), fb(pos[1]), fb(pos[2]), fb(orient), layer])
    };
    let mut v = vec![
        set(0, 0, o0.scaffold as u32, true, o0.pos, o0.orient, o0.layer as u32),
        op(31, &[0]),
        set(1, 0, 0, true, [o0.pos[0] + gf(g, -2.0, 2.0), o0.pos[1] + gf(g, -2.0, 2.0), o0.pos[2] + gf(g, -3.0, 3.0)], o1.orient, 2),
        set(0, 3, o0.scaffold as u32, true, o0.pos, o0.orient, o0.layer as u32),
        op(39, &[]),
        op(41, &[0]),
        op(32, &[]),
        set(0, gi(g, 0, 2) as u32, g.below(3), true, o0.pos, o0.orient, o0.layer as u32),
        op(39, &[]),
        op(41, &[0]),
        op(32, &[]),
    ];
    if chance(g, 50) {
        v.push(set(0, 3, 0, false, o0.pos, o0.orient, o0.layer as u32));
        v.push(op(39, &[]));
        v.push(op(46, &[]));
    }
    v
}

/// Three or four water changes running at once and finishing at different
/// updates (upstream's removal of a finished entry copies the last entry over
/// it, which only shows in the order of the list with three entries or more),
/// then the grid switched on for two maps whose settings differ in Y cells
/// only.
fn water_story(g: &mut Gen, sc: &Scenario) -> Vec<Op> {
    let mut v = vec![op(59, &[1])];
    let handles: Vec<u32> = (0..sc.trigs.len() as i32).map(|k| k as u32).chain(std::iter::once((-2i32) as u32)).collect();
    for &h in handles.iter().take(4) {
        v.push(op(55, &[h, fb(gf(g, 1.0, 60.0)), fb(gf(g, 0.04, 0.5)), fb(if chance(g, 50) { gf(g, 1.0, 40.0) } else { 0.0 })]));
    }
    // one change that lands exactly on its target: 1.0 per frame for 30
    // frames from the polygon's own height, so `>=` and `>` part ways
    if let Some((k, t)) = sc.trigs.iter().enumerate().find(|(_, t)| t.water) {
        let z = t.pts[0][2] as f32;
        v.push(op(55, &[k as u32, fb(z + 30.0), fb(1.0), fb(0.0)]));
        for _ in 0..32 {
            v.push(op(56, &[]));
        }
    }
    v.push(op(57, &[]));
    for _ in 0..gi(g, 8, 16) {
        v.push(op(0, &[g.below(200)]));
        v.push(op(56, &[]));
        if chance(g, 60) {
            v.push(op(57, &[]));
        }
    }
    // the grid for the second map, then the first again
    v.push(op(3, &[sc.settings[1].map as u32]));
    v.push(op(59, &[1]));
    v.push(op(72, &[fb(sc.settings[1].f[3] + 5.0), fb(sc.settings[1].f[4] + 5.0)]));
    v.push(op(3, &[sc.settings[0].map as u32]));
    v.push(op(59, &[1]));
    v.push(op(72, &[fb(sc.settings[0].f[3] + 5.0), fb(sc.settings[0].f[4] + 5.0)]));
    v
}

/// Water changes under way when the terrain is saved and loaded back, with
/// the active boundary changing in between (the load calls `setActiveBoundary`
/// itself).
fn snapshot_story(g: &mut Gen, sc: &Scenario) -> Vec<Op> {
    let water: Vec<u32> = sc.trigs.iter().enumerate().filter(|(_, t)| t.water).map(|(i, _)| i as u32).collect();
    let mut v = Vec::new();
    for _ in 0..gi(g, 1, 3) {
        let h = water[g.below(water.len() as u32) as usize];
        v.push(op(55, &[h, fb(gf(g, 1.0, 60.0)), fb(gf(g, 0.1, 0.9)), fb(gf(g, 0.0, 30.0))]));
    }
    v.push(op(63, &[]));
    for _ in 0..gi(g, 0, 4) {
        v.push(op(0, &[g.below(200)]));
        v.push(op(56, &[]));
    }
    v.push(op(60, &[g.below(4)]));
    v.push(op(63, &[]));
    v.push(op(60, &[g.below(4)]));
    v.push(op(64, &[]));
    v.push(op(57, &[]));
    v.push(op(66, &[]));
    v
}

/// The program of a case: 3 to 24 operations drawn from `kinds`.
fn gen_program(g: &mut Gen, sc: &Scenario, kinds: &[u32], prologue: &[u32]) -> Vec<Op> {
    let mut ops: Vec<Op> = prologue.iter().map(|&k| gen_op(g, sc, &[k])).collect();
    if kinds.contains(&31) && kinds.contains(&39) && sc.objs.len() > 1 && ops.is_empty() && chance(g, 25) {
        ops.extend(bridge_story(g, sc));
    }
    if kinds.contains(&63) && kinds.contains(&64) && kinds.contains(&55) && prologue.is_empty() && sc.trigs.iter().any(|t| t.water) && chance(g, 40) {
        ops.extend(snapshot_story(g, sc));
    }
    if kinds.contains(&55) && kinds.contains(&56) && kinds.contains(&59) && sc.trigs.len() >= 3 && sc.trigs.iter().all(|t| t.water) && prologue.contains(&59) && chance(g, 40) {
        ops.extend(water_story(g, sc));
    }
    let n = gi(g, 3, 24);
    for _ in 0..n {
        ops.push(gen_op(g, sc, kinds));
    }
    ops
}

// kinds that need the device subclass' extent (undefined in the base class)
const BASE_KINDS: &[u32] = &[
    0, 1, 10, 11, 12, 13, 17, 20, 21, 22, 23, 24, 25, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 41, 43, 44, 45, 46, 47, 48, 50, 51, 52, 53, 55, 57, 59, 66, 70, 72, 73,
];
const WAYPOINT_KINDS: &[u32] = &[10, 11, 20, 21, 22, 23, 24, 25, 66, 20, 20];
const BRIDGE_KINDS: &[u32] = &[0, 1, 30, 31, 31, 32, 33, 34, 35, 36, 37, 38, 39, 39, 41, 43, 44, 45, 46, 47, 48, 12, 13, 17];
const WATER_KINDS: &[u32] = &[0, 1, 2, 11, 50, 51, 52, 53, 54, 55, 56, 56, 57, 58, 59, 72, 73, 66, 14, 12];
const SHAPE_KINDS: &[u32] = &[12, 13, 14, 15, 16, 17, 61, 62, 70, 70, 61, 62, 12, 13, 30, 31];
const SNAPSHOT_KINDS: &[u32] = &[0, 1, 55, 56, 60, 63, 64, 45, 46, 30, 31, 66, 57, 53, 14, 60];
const ALL_KINDS: &[u32] = &[
    0, 1, 2, 10, 11, 12, 13, 14, 15, 16, 17, 20, 21, 22, 23, 24, 25, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 41, 43, 44, 45, 46, 47, 48, 50, 51, 52, 53, 54, 55, 56, 57,
    58, 59, 60, 61, 62, 63, 64, 66, 70, 72, 73,
];

#[derive(Default)]
struct Tally {
    cases: u64,
    compared: u64,
    skipped: std::collections::BTreeMap<Skip, u64>,
    ops: u64,
    undefined_counted: u64,
}

#[allow(clippy::too_many_arguments)] // test driver: one call per group, the arguments are the group's whole definition
fn run_group(name: &str, seed: u64, cases: usize, w3d_share: u32, kinds_w3d: &[u32], kinds_base: &[u32], prologue: &[u32], must: &[(&str, u64)]) -> Tally {
    let _guard = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(seed);
    let mut checks = Checks::new(name);
    let mut tally = Tally::default();
    for case in 0..cases {
        let w3d = chance(&mut g, w3d_share);
        let sc = gen_scenario(&mut g, w3d);
        let kinds = if w3d { kinds_w3d } else { kinds_base };
        let prog = gen_program(&mut g, &sc, kinds, prologue);
        tally.cases += 1;
        let (res, counted) = run_rust(&sc, &prog);
        tally.undefined_counted += u64::from(counted);
        match res {
            Err(skip) => {
                *tally.skipped.entry(skip).or_insert(0) += 1;
            }
            Ok(r) => {
                let c = run_cpp(&sc, &prog);
                if std::env::var("TL_SHOW").ok().and_then(|v| v.parse::<usize>().ok()) == Some(case) {
                    println!("SCENARIO {sc:#?}");
                    for (n, o) in prog.iter().enumerate() {
                        println!("op {n} {} {:?}", op_name(o.code), o.args);
                    }
                    println!("RUST {r:?}");
                    println!("CPP  {c:?}");
                }
                compare_case(&mut checks, &prog, &r, &c, case);
                tally.compared += 1;
                tally.ops += prog.len() as u64;
            }
        }
    }
    println!(
        "CASES {name}: {} generated, {} compared ({} operations), {} dropped as undefined upstream: {:?}",
        tally.cases, tally.compared, tally.ops, tally.skipped.values().sum::<u64>(), tally.skipped
    );
    assert!(tally.compared * 2 > tally.cases, "{name}: too few cases are defined upstream ({} of {})", tally.compared, tally.cases);
    checks.finish(must);
    tally
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

#[test]
fn waypoints_and_the_waypoint_chunk() {
    run_group("waypoints", 0x7701, CASES / 2, 50, WAYPOINT_KINDS, WAYPOINT_KINDS, &[10], &[("result waypoint links", 2000), ("result isPurposeOfPath true", 100), ("result loadMap true", 1000), ("visual 60 load", 500)]);
}

#[test]
fn bridges_and_their_objects() {
    run_group("bridges", 0x7702, CASES, 70, BRIDGE_KINDS,
        BRIDGE_KINDS,
        &[],
        &[
            ("pathfinder.addBridge", 2000),
            ("pathfinder.changeBridgeState", 300),
            ("attemptDamage", 30),
            ("destroyObject", 100),
            ("bridge setTower", 1000),
            ("setIndestructible", 500),
            ("getDrawable", 40),
            ("isBridgeBroken true", 20),
            ("isBridgeRepaired true", 20),
            ("isPointOnBridge true", 20),
            ("isCellOnEnd true", 20),
            ("isCellOnSide true", 20),
            ("isCellEntryPoint true", 20),
            ("objectInteractsWithBridge true", 50),
        ],
    );
}

#[test]
fn water_areas_the_grid_and_changes_over_time() {
    run_group("water", 0x7703, CASES, 100, WATER_KINDS,
        BASE_KINDS,
        &[59],
        &[
            ("attemptDamage", 40),
            ("forceMapRecalculation", 1000),
            ("iterateObjectsInRange", 300),
            ("isUnderwater true", 100),
            ("getWaterGridHeight on the grid", 50),
            ("setWaterTransform(matrix)", 500),
            ("water changes pending", 300),
            ("setWaterGridResolution", 1000),
        ],
    );
}

#[test]
fn ground_height_extents_flatten_and_crater() {
    run_group("ground", 0x7704, CASES, 100, SHAPE_KINDS, BASE_KINDS, &[], &[("setRawMapHeight", 100_000)]);
}

#[test]
fn snapshots_boundaries_and_post_processing() {
    run_group("snapshot", 0x7705, CASES / 2, 100, SNAPSHOT_KINDS, BASE_KINDS, &[], &[("processPendingUndoShroudRevealQueue", 300), ("xfer load ok", 250), ("water changes pending", 150), ("setWaterTransform(matrix)", 50), ("xfer saved a water change", 100)]);
}

#[test]
fn everything_together() {
    run_group("mixed", 0x7706, CASES * 2, 80, ALL_KINDS,
        BASE_KINDS,
        &[10, 11],
        &[("attemptDamage", 30), ("processPendingUndoShroudRevealQueue", 150), ("xfer load ok", 100), ("getWaterGridHeight on the grid", 15), ("result waypoint links", 1000)],
    );
}

// ---------------------------------------------------------------------------
// Inputs upstream does not define, and the translation-only parts
// ---------------------------------------------------------------------------

fn empty_scenario() -> Scenario {
    Scenario {
        w3d: true,
        hm: Some(Hm { w: 12, h: 12, border: 0, bounds: vec![(12, 12)], data: vec![40; 144] }),
        render: true,
        bounds: vec![(12, 12), (6, 6)],
        active: 0,
        map_dx: 12,
        map_dy: 12,
        min_z: 0.0,
        max_z: 25.0,
        map_name: 25,
        settings: (0..4)
            .map(|k| Setting { map: 25 + k, f: [0.0, 10.0, 0.5, 5.0, 5.0, 3.0], cells: (8, 8), size: 10.0, att: [0.0; 4] })
            .collect(),
        generic: true,
        road: true,
        tower_mask: 15,
        tower_major: [5.0; 4],
        wall_height: 10.0,
        wall_on: false,
        wall: [0.0; 4],
        frame: 0,
        objs: Vec::new(),
        trigs: vec![Trig { name: 12, id: 3, water: true, pts: vec![[0, 0, 5], [100, 0, 5], [100, 100, 5], [0, 100, 5]] }],
        maps: Vec::new(),
        file_opens: false,
        file: Vec::new(),
    }
}

/// Every kind of input the C++ does not define is an `Err` of the matching
/// kind, and each one is counted by `TerrainLogic::undefined_inputs`.
#[test]
fn undefined_inputs_are_errors() {
    use TerrainUndefined as U;
    #[derive(Default)]
    struct Seen {
        kinds: std::collections::BTreeSet<TerrainUndefined>,
        refusals: u32,
    }
    /// The operation was refused as `kind`, and the terrain's counter is the number refused so far.
    fn expect(seen: &mut Seen, ctx: &Ctx, kind: TerrainUndefined, r: Result<Vec<u32>, Skip>) {
        match r {
            Err(Skip::Undefined(u)) => assert_eq!(u, kind),
            other => panic!("{kind:?}: got {other:?}"),
        }
        seen.refusals += 1;
        assert!(ctx.ts.undefined_inputs() >= 1, "{kind:?} was not counted");
        seen.kinds.insert(kind);
    }
    let mut seen = Seen::default();

    // the base class has no extent: the edge points, the bounding box, the extents
    let mut base = empty_scenario();
    base.w3d = false;
    let mut c = Ctx::new(&base);
    let r = exec(&mut c, &op(15, &[fb(1.0), fb(1.0), fb(0.0)]));
    expect(&mut seen, &c, U::ExtentNotImplemented, r);
    assert_eq!(c.ts.undefined_inputs(), 1);
    let r = exec(&mut c, &op(14, &[1]));
    assert_eq!(r, Err(Skip::Undefined(U::ExtentNotImplemented)));
    assert_eq!(c.ts.undefined_inputs(), 1, "extent() itself is a query, not a counted call");

    // a bridge segment parallel to the plane of the deck (a vertical deck)
    let mut c = Ctx::new(&empty_scenario());
    let mut args = vec![0u32; 22];
    let corners = [[0.0f32, 0.0, 0.0], [10.0, 0.0, 5.0], [20.0, 0.0, 0.0], [30.0, 0.0, 5.0]]; // all in the plane y = 0: a vertical deck
    for (k, p) in corners.iter().enumerate() {
        for (i, v) in p.iter().enumerate() {
            args[7 + 3 * k + i] = fb(*v);
        }
    }
    args[21] = 17;
    exec(&mut c, &op(30, &args)).unwrap();
    let r = exec(&mut c, &op(47, &[0, fb(5.0), fb(0.0), fb(0.0)]));
    expect(&mut seen, &c, U::BridgePlaneParallel, r);

    // getWaterTransform with no water render object
    let mut sc = empty_scenario();
    sc.render = false;
    let mut c = Ctx::new(&sc);
    let r = exec(&mut c, &op(54, &[(-2i32) as u32, fb(1.0), fb(0.0), 0]));
    expect(&mut seen, &c, U::WaterTransformUnavailable, r);
    let r = exec(&mut c, &op(58, &[(-2i32) as u32]));
    expect(&mut seen, &c, U::WaterTransformUnavailable, r);

    // a grid mesh that could not be allocated, with the grid on: the height is read unset;
    // and a NaN grid coordinate
    let mut c = Ctx::new(&empty_scenario());
    c.ts.visual_mut().water_grid.set_resolution(100_000.0, 100_000.0, 10.0).unwrap();
    assert_eq!(c.ts.visual_mut().water_grid.enable_water_grid(true), Err(U::WaterGridSize));
    seen.kinds.insert(U::WaterGridSize);
    assert_eq!(c.ts.visual().water_grid.water_grid_height(5.0, 5.0), Ok(WaterGridSample::OnGridUnset));
    let r = exec(&mut c, &op(50, &[fb(5.0), fb(5.0)]));
    expect(&mut seen, &c, U::WaterGridHeightUnset, r);
    let mut c = Ctx::new(&empty_scenario());
    exec(&mut c, &op(59, &[1])).unwrap();
    let r = exec(&mut c, &op(51, &[fb(f32::NAN), fb(5.0)]));
    expect(&mut seen, &c, U::NonFiniteWaterGridCoordinate, r);

    // a grid transform that does not ignore z, then the bounding box of the grid
    let mut c = Ctx::new(&empty_scenario());
    let mut m = rts_lockstep_engine::ww::matrix3d::Matrix3D::IDENTITY;
    m.row[0].z = 1.0;
    c.ts.visual_mut().apply(&TerrainVisualEvent::SetWaterTransformMatrix { transform: m }).unwrap();
    let r = exec(&mut c, &op(58, &[(-2i32) as u32]));
    expect(&mut seen, &c, U::WaterGridCornerZ, r);

    // a handle to a trigger that is not there
    let mut c = Ctx::new(&empty_scenario());
    assert_eq!(c.ts.water_height(Some(WaterHandle::Polygon(7))), Err(TerrainLogicError::Undefined(U::StaleWaterHandle)));
    assert_eq!(c.ts.undefined_inputs(), 1);
    seen.kinds.insert(U::StaleWaterHandle);
    seen.refusals += 1;

    // a polygon height that is not an Int
    let r = exec(&mut c, &op(54, &[0, fb(f32::NAN), fb(0.0), 0]));
    expect(&mut seen, &c, U::ConversionOutOfRange, r);

    // a count of water changes out of range, loaded from a save
    let mut c = Ctx::new(&empty_scenario());
    let mut bytes = vec![2u8]; // version
    bytes.extend_from_slice(&0i32.to_le_bytes()); // active boundary
    bytes.extend_from_slice(&70i32.to_le_bytes()); // m_numWaterToUpdate
    c.last_save = bytes;
    let r = exec(&mut c, &op(64, &[]));
    expect(&mut seen, &c, U::WaterUpdateCount, r);

    // an active boundary out of range
    let mut c = Ctx::new(&empty_scenario());
    c.ts.set_boundaries(vec![(12, 12)].into_iter().map(|(x, y)| ICoord2D { x, y }).collect(), 5);
    let r = exec(&mut c, &op(14, &[0]));
    assert_eq!(r, Err(Skip::Undefined(U::ActiveBoundary)));
    seen.kinds.insert(U::ActiveBoundary);

    // a waypoint link index past the array, and a trigger with no points, a height map with no samples
    let mut sc = empty_scenario();
    sc.maps = vec![MapObj { is_wp: true, loc: [1.0, 2.0, 0.0], id: 4, name: 1, labels: [-1; 3], bidir: 0 }];
    let mut c = Ctx::new(&sc);
    exec(&mut c, &op(10, &[32, 0])).unwrap();
    let w0 = c.ts.first_waypoint().unwrap();
    assert_eq!(c.ts.waypoint_link(w0, 8), Err(TerrainLogicError::Undefined(U::WaypointLinkIndex)));
    assert_eq!(c.ts.undefined_inputs(), 1);
    assert_eq!(c.ts.waypoint_link(w0, -1), Ok(None));
    assert_eq!(TriggerArea::new(ascii(12), 1, true, Vec::new()), Err(U::TriggerWithoutPoints));
    assert_eq!(LogicHeightMap::new(0, 5, 0, Vec::new(), Vec::new()), Err(U::HeightMapShape));
    assert_eq!(LogicHeightMap::new(2, 2, 0, Vec::new(), vec![0; 3]), Err(U::HeightMapShape));
    assert!(LogicHeightMap::new(2, 2, 0, Vec::new(), vec![0; 5]).is_ok(), "longer data is fine (a version 1 map keeps its full data)");
    seen.kinds.extend([U::WaypointLinkIndex, U::TriggerWithoutPoints, U::HeightMapShape]);

    // the chunk reader's own undefined input, through loadMap: a count past the end of the file
    let mut sc = empty_scenario();
    sc.maps = Vec::new();
    sc.file_opens = true;
    let mut data = Vec::new();
    data.extend_from_slice(&5i32.to_le_bytes()); // five links, no data after
    sc.file = chunk_file(&["WaypointsList"], &[(0, 1, data)]);
    let mut c = Ctx::new(&sc);
    let r = exec(&mut c, &op(10, &[32, 0]));
    assert_eq!(r, Err(Skip::ChunkUndefined));
    assert_eq!(c.ts.undefined_inputs(), 1);

    println!("UNDEFINED inputs checked as errors, each counted: {} kinds ({:?}), {} refusals", seen.kinds.len(), seen.kinds, seen.refusals);
    assert_eq!(seen.kinds.len(), 14, "every kind of TerrainUndefined is exercised");
}

/// `HeightMapData` chunks through the Rust readers (`ParseHeightMapData`,
/// `ParseSizeOnly`; WorldHeightMap.cpp is not compiled in the differential
/// build, so these are checked by hand against the source, not against the
/// C++): the fields of versions 1, 3 and 4, the version-1 halving that only
/// `ParseSizeOnly` applies, the corrupt-size error, and `load_map_w3d`'s
/// extent and height range.
#[test]
fn height_map_chunks_and_load_map_w3d() {
    fn chunk(version: u16, width: i32, height: i32, border: Option<i32>, bounds: Option<&[(i32, i32)]>, size: i32, data: &[u8]) -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(&width.to_le_bytes());
        d.extend_from_slice(&height.to_le_bytes());
        if let Some(b) = border {
            d.extend_from_slice(&b.to_le_bytes());
        }
        if let Some(bs) = bounds {
            d.extend_from_slice(&(bs.len() as i32).to_le_bytes());
            for &(x, y) in bs {
                d.extend_from_slice(&x.to_le_bytes());
                d.extend_from_slice(&y.to_le_bytes());
            }
        }
        d.extend_from_slice(&size.to_le_bytes());
        d.extend_from_slice(data);
        chunk_file(&["HeightMapData"], &[(0, version, d)])
    }
    let data: Vec<u8> = (0..24u8).collect();
    let load = |bytes: Vec<u8>, size_only: bool| LogicHeightMap::from_map_file(CachedFileInputStream::new(bytes).unwrap(), size_only);

    // version 4: border, boundaries
    let m = load(chunk(4, 6, 4, Some(1), Some(&[(4, 2), (3, 1)]), 24, &data), false).unwrap().unwrap();
    assert_eq!((m.x_extent(), m.y_extent(), m.border_size()), (6, 4, 1));
    assert_eq!(m.all_boundaries(), &[ICoord2D { x: 4, y: 2 }, ICoord2D { x: 3, y: 1 }]);
    assert_eq!(m.data(), &data[..]);
    assert_eq!(m.height_at(2, 3), 20);
    // version 3: a border, and the boundary is the interior
    let m = load(chunk(3, 6, 4, Some(1), None, 24, &data), true).unwrap().unwrap();
    assert_eq!(m.all_boundaries(), &[ICoord2D { x: 4, y: 2 }]);
    // version 2: no border; the boundary is the whole map
    let m = load(chunk(2, 6, 4, None, None, 24, &data), false).unwrap().unwrap();
    assert_eq!((m.border_size(), m.all_boundaries()), (0, &[ICoord2D { x: 6, y: 4 }][..]));
    // version 1: ParseSizeOnly halves the extents and compacts the data; ParseHeightMapData compacts the data and keeps the extents
    let so = load(chunk(1, 6, 4, None, None, 24, &data), true).unwrap().unwrap();
    assert_eq!((so.x_extent(), so.y_extent()), (3, 2));
    let full = load(chunk(1, 6, 4, None, None, 24, &data), false).unwrap().unwrap();
    assert_eq!((full.x_extent(), full.y_extent()), (6, 4));
    assert_eq!(&full.data()[..6], &[0, 2, 4, 12, 14, 16]);
    // a data size that is not width * height, or not positive: ERROR_CORRUPT_FILE_FORMAT, thrown
    for size in [23, 0, -4] {
        assert!(matches!(load(chunk(4, 6, 4, Some(0), Some(&[(6, 4)]), size, &data), false), Err(rts_lockstep_engine::formats::data_chunk::ChunkError::Thrown(_))));
    }
    // no HeightMapData chunk: no map
    assert_eq!(load(chunk_file(&["Other"], &[(0, 1, vec![1])]), false).unwrap().map(|_| ()), None);

    // W3DTerrainLogic::loadMap: extent fields, boundaries, height range
    let mut t = TerrainLogic::new(GroundSource::LogicHeightMap);
    let keys = make_keys();
    let name = AsciiString::try_from_bytes(b"level.map").unwrap();
    let bytes = chunk(4, 6, 4, Some(1), Some(&[(4, 2), (3, 1)]), 24, &data);
    assert_eq!(t.load_map_w3d(&name, true, Some(&bytes), &[], &keys), Ok(true));
    assert_eq!(t.boundaries(), &[ICoord2D { x: 4, y: 2 }, ICoord2D { x: 3, y: 1 }]);
    assert_eq!(t.active_boundary(), 0);
    let e = t.extent().unwrap();
    assert_eq!((e.hi.x, e.hi.y, e.lo.z, e.hi.z), (40.0, 20.0, 0.0, 23.0 * 0.625));
    assert_eq!(t.take_events(), Vec::<TerrainVisualEvent>::new(), "a query load tells the client nothing");
    assert_eq!(t.load_map_w3d(&name, false, None, &[], &keys), Ok(false), "a file that does not open");
    assert_eq!(t.load_map_w3d(&AsciiString::new(), false, Some(&bytes), &[], &keys), Ok(false), "an empty file name does not open");
}
