#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (the zone helpers, ZoneBlock, PathfindZoneManager, PathfindLayer)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The zones and the layers (`logic::ai::zone`, `logic::ai::layer`) against the unchanged C++, `p1w13-sonnet`, layer 10 batch P3.
//!
//! # Method
//!
//! The C++ side is the real `AIPathfind.h` and AIPathfind.cpp from `inline Bool typesMatch(` to the `Pathfinder` constructor, verbatim
//! (build.rs, `zone_p3.inc`), in the pathfinder build (`differential/wrap_pathfind.cpp`, `ref_pf_p3_run`): a map of real cells and the
//! real info pool, a real `PathfindZoneManager`, the 16 real `PathfindLayer`s. Their world is stand-ins (`shim_pathfind/
//! pathfind_world.h`): the bridges' and the ground's answers (which points and cells are on a bridge, its height, the ground's) come
//! from a hash of the arguments that this test computes too ([`Host`]); the wall's pieces are objects with a position, an angle and
//! radii. The Rust side is a [`CellStore`], a [`PathfindZoneManager`] and 16 [`PathfindLayerState`]s.
//!
//! Each case is a map of 3 to 24 by 3 to 18 cells (several zone blocks) of random types, 0 to 3 bridges over and around it, 0 to 4
//! wall pieces, and a program of 1 to 40 operations: zones calculated and updated for a modification, types, zones and connections
//! changed, fences set and removed, the effective, terrain and block zones asked for every surface set, the passable and bridge flags,
//! the dirty frame, the blocks allocated and the manager reset, a modification's cells unset and their zones updated; layers given bridges (or the wall), allocated, classified, destroyed
//! and repaired, reset, asked for cells, zone connections and bridges, zoned. The program keeps to the calls upstream defines, judged
//! on the Rust state (the C++ driver ends a program with a marker when a planted defect makes one undefined there). Compared, bit
//! for bit, after every operation: its results, the manager (its tables to the zone count), every block (its tables to its zone
//! count), every map cell (type, zone, connection, layer), every layer (extent, origin, ends, layer, zone, bridge, destroyed, unused,
//! each cell's type, zone, connection, layer and pinch).
//!
//! # Results (2026-10-26, `PF_P3_CASES=20000`)
//!
//! 20,000 cases, 409,944 operations (57,284 zone calculations and updates, 149,271 layer operations): **0 mismatches**, on the first
//! run. Nothing is excluded: the calls upstream leaves undefined (the zones calculated or a block's zone asked for before the blocks
//! are allocated, the tables read before a calculation, a bridge's layer classified or asked its bridge without one, cells allocated
//! over cells) are not run on either side. The bridges' and walls' answers are the stand-ins'; the real `Bridge` (verified in the
//! terrain batches) meets this code when the pathfinder is translated.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 20,000 cases: 13 of 13 caught. A first 3032 (the left neighbour's water, rubble and cliff
//! ladder run when the terrain or crusher table took the pair) survived because it changes nothing: a pair the terrain or crusher table
//! takes is an obstacle and a clear cell, which none of the three matches. It was replaced. 3034 and 3038/3039 were caught only once
//! the test unset a modification's zones before updating them, and asked for the zones of map cells, and with 20,000 cases.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3030 | `waterGround` one way only | 254,693 |
//! | 3031 | a block's ground and cliff not merged | 233,893 |
//! | 3032 | a bridge's connection not merged into the hierarchical zones | 6,136 |
//! | 3033 | the terrain table not flattened | 226,090 |
//! | 3034 | the backward sweep's diagonal reads the cell above | 148 |
//! | 3035 | a layer one cell taller at the map's edge | 11,364 |
//! | 3036 | the bridge's far end stepped back in y | 511 |
//! | 3037 | a wall piece's minor radius ignored | 6,146 |
//! | 3038 | the manager's crusher table not applied | 27 |
//! | 3039 | a block's crusher table not applied | 31 |
//! | 3040 | a wall cell pinched only next to an impassable one | 1,433 |
//! | 3041 | the dirty frame's start at frame 1 | 9,201 |
//! | 3042 | merged zones keep the higher | 306,463 |

mod support;

use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D, ICoord2D, IRegion2D, Region2D};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::ai::cell::{CellStore, ObstacleFacts, PathfindCell};
use rts_lockstep_engine::logic::ai::layer::{BridgeHandle, LayerHost, PathfindLayerState, WallPieceFacts};
use rts_lockstep_engine::logic::ai::zone::{GroundMap, PathfindZoneManager};
use support::Gen;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

const NONE: u32 = u32::MAX;
const MARKER: u32 = 0xbadc_0de5;

/// The driver's `p1Hash`.
fn p1_hash(seed: u32, tag: u32, words: &[u32]) -> u32 {
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

#[derive(Clone, Copy, Debug)]
struct Bridge {
    bounds: Region2D,
    from: Coord3D,
    to: Coord3D,
}

#[derive(Clone, Copy, Debug)]
struct Wall {
    id: ObjectId,
    pos: Coord2D,
    angle: f32,
    major: f32,
    minor: f32,
    sphere: bool,
}

#[derive(Clone, Debug)]
struct Scene {
    w: u32,
    h: u32,
    seed: u32,
    rates: [u32; 4],
    bridges: Vec<Bridge>,
    walls: Vec<Wall>,
    types: Vec<u8>,
}

impl Scene {
    fn cfg(&self) -> Vec<u32> {
        let mut c = vec![self.w, self.h, self.seed, self.rates[0], self.rates[1], self.rates[2], self.rates[3], self.bridges.len() as u32];
        for b in &self.bridges {
            c.extend([b.bounds.lo.x, b.bounds.lo.y, b.bounds.hi.x, b.bounds.hi.y, b.from.x, b.from.y, b.from.z, b.to.x, b.to.y, b.to.z].map(f32::to_bits));
        }
        c.push(self.walls.len() as u32);
        for w in &self.walls {
            c.extend([w.id as u32, w.pos.x.to_bits(), w.pos.y.to_bits(), w.angle.to_bits(), w.major.to_bits(), w.minor.to_bits(), u32::from(w.sphere)]);
        }
        c.extend(self.types.iter().map(|&t| u32::from(t)));
        c
    }
}

/// The stand-ins' answers, as the driver gives them.
struct Host<'a> {
    s: &'a Scene,
}

impl LayerHost for Host<'_> {
    fn bridge_bounds(&self, b: BridgeHandle) -> Region2D {
        self.s.bridges[b as usize].bounds
    }
    fn bridge_ends(&self, b: BridgeHandle) -> (Coord3D, Coord3D) {
        (self.s.bridges[b as usize].from, self.s.bridges[b as usize].to)
    }
    fn bridge_object_id(&self, b: BridgeHandle) -> ObjectId {
        (1000 + b) as ObjectId
    }
    fn is_point_on_bridge(&self, b: BridgeHandle, p: &Coord3D) -> bool {
        pick(p1_hash(self.s.seed, 10, &[b, p.x.to_bits(), p.y.to_bits()]), self.s.rates[0])
    }
    fn is_cell_on_end(&self, b: BridgeHandle, c: &Region2D) -> bool {
        pick(p1_hash(self.s.seed, 11, &[b, c.lo.x.to_bits(), c.lo.y.to_bits(), c.hi.x.to_bits(), c.hi.y.to_bits()]), self.s.rates[1])
    }
    fn is_cell_on_side(&self, b: BridgeHandle, c: &Region2D) -> bool {
        pick(p1_hash(self.s.seed, 12, &[b, c.lo.x.to_bits(), c.lo.y.to_bits(), c.hi.x.to_bits(), c.hi.y.to_bits()]), self.s.rates[2])
    }
    fn is_cell_entry_point(&self, b: BridgeHandle, c: &Region2D) -> bool {
        pick(p1_hash(self.s.seed, 13, &[b, c.lo.x.to_bits(), c.lo.y.to_bits(), c.hi.x.to_bits(), c.hi.y.to_bits()]), self.s.rates[3])
    }
    fn bridge_height(&self, b: BridgeHandle, p: &Coord3D) -> f32 {
        ((p1_hash(self.s.seed, 14, &[b, p.x.to_bits(), p.y.to_bits()]) >> 8) % 40) as f32
    }
    fn ground_height(&self, x: f32, y: f32) -> f32 {
        ((p1_hash(self.s.seed, 15, &[x.to_bits(), y.to_bits(), 1]) >> 8) % 40) as f32
    }
    fn wall_piece(&self, id: ObjectId) -> Option<WallPieceFacts> {
        let w = self.s.walls.iter().find(|w| w.id == id)?;
        let pos = Coord3D { x: w.pos.x, y: w.pos.y, z: 0.0 };
        let bounds_2d = Region2D { lo: Coord2D { x: pos.x - w.major, y: pos.y - w.major }, hi: Coord2D { x: pos.x + w.major, y: pos.y + w.major } };
        Some(WallPieceFacts { pos, orientation: w.angle, bounds_2d, major_radius: w.major, minor_radius: w.minor, is_sphere: w.sphere })
    }
}

struct Run<'a> {
    scene: &'a Scene,
    store: CellStore,
    map: GroundMap,
    zm: PathfindZoneManager,
    layers: Vec<PathfindLayerState>,
}

impl<'a> Run<'a> {
    fn new(scene: &'a Scene) -> Self {
        let mut store = CellStore::new();
        store.allocate_cell_infos();
        let count = (scene.w * scene.h) as usize;
        store.cells = vec![PathfindCell::default(); count];
        for (k, &t) in scene.types.iter().enumerate() {
            store.set_type(k as u32, t);
        }
        let map = GroundMap { extent: IRegion2D { lo: ICoord2D { x: 0, y: 0 }, hi: ICoord2D { x: scene.w as i32 - 1, y: scene.h as i32 - 1 } } };
        Run { scene, store, map, zm: PathfindZoneManager::new(), layers: vec![PathfindLayerState::default(); 16] }
    }
    fn count(&self) -> u32 {
        self.scene.w * self.scene.h
    }
    fn pieces(&self) -> Vec<ObjectId> {
        self.scene.walls.iter().map(|w| w.id).collect()
    }

    /// Whether `o` is a call upstream defines here.
    fn defined(&self, o: &[u32; 6]) -> bool {
        let l = &self.layers[(o[1] % 16) as usize];
        let tables = self.zm.tables.is_some();
        match o[0] {
            0 | 1 | 9 => self.zm.blocks.is_some(),
            7 => tables,
            8 => tables && o[1] < u32::from(self.zm.zones_allocated) && o[1] <= u32::from(self.zm.max_zone),
            22 | 23 => l.width == 0 || l.bridge.is_some(),
            27 => l.bridge.is_some(),
            26 => !l.destroyed || tables,
            21 | 28 => l.block.is_none(),
            _ => true,
        }
    }

    fn op(&mut self, o: &[u32; 6], v: &mut Vec<u32>) {
        let host = Host { s: self.scene };
        let h = self.scene.h;
        let a = o[1] % self.count();
        let extent = self.map.extent;
        let pieces = self.pieces();
        let mut res = Vec::new();
        let li = (o[1] % 16) as usize;
        let (s, map, zm) = (&mut self.store, &self.map, &mut self.zm);
        match o[0] {
            0 => zm.calculate_zones(s, map, &mut self.layers, &extent),
            1 => {
                let sb = IRegion2D { lo: ICoord2D { x: o[1] as i32, y: o[2] as i32 }, hi: ICoord2D { x: o[3] as i32, y: o[4] as i32 } };
                zm.update_zones_for_modify(s, map, &sb, &extent);
            }
            2 => s.set_type(a, o[2] as u8),
            3 => s.cells[a as usize].set_zone(o[2] as u16),
            4 => s.cells[a as usize].set_connect_layer(o[2] as i32),
            5 => {
                let pos = ICoord2D { x: (a / h) as i32, y: (a % h) as i32 };
                let ob = ObstacleFacts { id: o[2] as ObjectId, rubble: false, see_through: false };
                res.push(u32::from(s.set_type_as_obstacle(a, &ob, o[3] != 0, pos)));
            }
            6 => res.push(u32::from(s.remove_obstacle(a, o[2] as ObjectId))),
            7 => res.push(u32::from(zm.get_effective_zone(o[1] as i32, o[2] != 0, o[3] as u16))),
            8 => res.push(u32::from(zm.get_effective_terrain_zone(o[1] as u16))),
            9 => res.push(u32::from(zm.get_block_zone(s, map, o[1] as i32, o[2] != 0, o[3] as i32, o[4] as i32))),
            10 => zm.clear_passable_flags(),
            11 => zm.set_all_passable(),
            12 => zm.set_passable(o[1] as i32, o[2] as i32, o[3] != 0),
            13 => res.push(u32::from(zm.is_passable(o[1] as i32, o[2] as i32))),
            14 => res.push(u32::from(zm.clip_is_passable(o[1] as i32, o[2] as i32))),
            15 => zm.set_bridge(o[1] as i32, o[2] as i32, o[3] != 0),
            16 => res.push(u32::from(zm.interacts_with_bridge(o[1] as i32, o[2] as i32))),
            17 => {
                zm.mark_zones_dirty(o[2] != 0, o[1]);
                res.push(u32::from(zm.need_to_calculate_zones(o[1])));
            }
            18 => zm.allocate_blocks(&extent),
            19 => zm.reset(),
            20 => {
                let nb = self.scene.bridges.len() as u32;
                let bridge = (o[2] != NONE).then(|| o[2] % nb);
                res.push(u32::from(self.layers[li].init(bridge, o[3] as i32)));
            }
            21 => self.layers[li].allocate_cells(s, &extent, &host),
            22 => self.layers[li].classify_cells(s, map, &host),
            23 => res.push(u32::from(self.layers[li].set_destroyed(o[2] != 0, s, map, &host))),
            24 => self.layers[li].reset(s),
            25 => {
                let l = &self.layers[li];
                res.push(l.get_cell(s, o[2] as i32, o[3] as i32).map_or(NONE, |c| c - l.block.expect("a cell is in a block")));
            }
            26 => res.push(u32::from(self.layers[li].connects_zones(s, map, zm, o[2] as i32, o[3] as i32, o[4] as i32))),
            27 => res.push(self.layers[li].get_bridge_id(&host) as u32),
            28 => self.layers[li].allocate_cells_for_wall_layer(s, &extent, &pieces, &host),
            29 => self.layers[li].classify_wall_cells(s, map, &pieces, &host),
            30 => {
                let pt = Coord3D { x: f32::from_bits(o[2]), y: f32::from_bits(o[3]), z: 0.0 };
                res.push(u32::from(self.layers[li].is_point_on_wall(&pieces, &pt, &host)));
            }
            31 => {
                self.layers[li].set_zone(o[2] as i32);
                self.layers[li].apply_zone(s);
            }
            32 => {
                for x in o[1] as i32..=o[2] as i32 {
                    for y in o[3] as i32..=o[4] as i32 {
                        if x >= 0 && x < self.scene.w as i32 && y >= 0 && y < h as i32 {
                            let c = map.cell(x, y);
                            s.cells[c as usize].set_zone(0);
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
        let mut d = vec![res.len() as u32];
        d.extend(&res);
        let zm = &self.zm;
        d.extend([u32::from(zm.blocks.is_some()), zm.zone_block_extent.x as u32, zm.zone_block_extent.y as u32, u32::from(zm.max_zone)]);
        d.extend([zm.next_frame_to_calculate_zones, u32::from(zm.zones_allocated), u32::from(zm.tables.is_some())]);
        if let Some(t) = &zm.tables {
            let m = (usize::from(zm.max_zone) + 1).min(usize::from(zm.zones_allocated));
            for tab in [&t.ground_cliff, &t.ground_water, &t.ground_rubble, &t.terrain, &t.crusher, &t.hierarchical] {
                d.extend(tab[..m].iter().map(|&z| u32::from(z)));
            }
        }
        if let Some(blocks) = &zm.blocks {
            for b in blocks {
                d.extend([b.cell_origin.x as u32, b.cell_origin.y as u32, u32::from(b.first_zone), u32::from(b.num_zones)]);
                d.extend([u32::from(b.zones_allocated), u32::from(b.interacts_with_bridge), u32::from(b.marked_passable), u32::from(b.tables.is_some())]);
                if let Some(t) = &b.tables {
                    let m = usize::from(b.num_zones).min(usize::from(b.zones_allocated));
                    for tab in [&t.ground_cliff, &t.ground_water, &t.ground_rubble, &t.crusher] {
                        d.extend(tab[..m].iter().map(|&z| u32::from(z)));
                    }
                }
            }
        }
        let s = &self.store;
        for c in &s.cells[..self.count() as usize] {
            d.extend([u32::from(c.get_type()), u32::from(c.get_zone()), c.get_connect_layer() as u32, c.get_layer() as u32]);
        }
        for l in &self.layers {
            d.extend([l.width as u32, l.height as u32, l.x_origin as u32, l.y_origin as u32]);
            d.extend([l.start_cell.x as u32, l.start_cell.y as u32, l.end_cell.x as u32, l.end_cell.y as u32]);
            d.extend([l.layer as u32, l.zone as u32, l.bridge.unwrap_or(NONE), u32::from(l.destroyed), u32::from(l.is_unused()), u32::from(l.block.is_some())]);
            if l.block.is_some() {
                for i in 0..l.width {
                    for j in 0..l.height {
                        let c = &s.cells[l.layer_cell(i, j) as usize];
                        d.extend([u32::from(c.get_type()), u32::from(c.get_zone()), c.get_connect_layer() as u32, c.get_layer() as u32]);
                        d.push(u32::from(c.get_pinched()));
                    }
                }
            }
        }
        v.push(d.len() as u32);
        v.extend(d);
    }
}

fn cpp_run(scene: &Scene, ops: &[[u32; 6]]) -> Vec<u32> {
    let cfg = scene.cfg();
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut buf = vec![0u32; 1 << 24];
    let mut len = 0i32;
    let st = unsafe { cpp::ref_pf_p3_run(cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len) };
    assert_eq!(st, 0, "the dump fits");
    buf.truncate(len as usize);
    buf
}

/// The ops' parts of a dump (each after its length word; a marker ends it).
fn split_ops(b: &[u32]) -> Vec<Vec<u32>> {
    let mut out = Vec::new();
    let mut p = 0;
    while p < b.len() && b[p] != MARKER {
        let n = b[p] as usize;
        out.push(b[p + 1..p + 1 + n].to_vec());
        p += 1 + n;
    }
    out
}

fn gen_scene(g: &mut Gen) -> Scene {
    let (w, h) = (3 + g.below(22), 3 + g.below(16));
    let (wf, hf) = (w as f32 * 10.0, h as f32 * 10.0);
    let bridges = (0..g.below(4))
        .map(|_| {
            let lo = Coord2D { x: g.uniform(-15.0, wf), y: g.uniform(-15.0, hf) };
            let hi = Coord2D { x: lo.x + g.uniform(5.0, 60.0), y: lo.y + g.uniform(5.0, 60.0) };
            let from = Coord3D { x: g.uniform(lo.x, hi.x), y: g.uniform(lo.y, hi.y), z: g.uniform(0.0, 20.0) };
            let to = if g.below(10) == 0 { from } else { Coord3D { x: g.uniform(lo.x, hi.x), y: g.uniform(lo.y, hi.y), z: g.uniform(0.0, 20.0) } };
            Bridge { bounds: Region2D { lo, hi }, from, to }
        })
        .collect();
    let walls = (0..g.below(5))
        .map(|k| Wall {
            id: (500 + k) as ObjectId,
            pos: Coord2D { x: g.uniform(0.0, wf), y: g.uniform(0.0, hf) },
            angle: g.uniform(-3.2, 3.2),
            major: g.uniform(5.0, 30.0),
            minor: g.uniform(3.0, 15.0),
            sphere: g.below(4) == 0,
        })
        .collect();
    // mostly clear, then water, cliff, rubble, impassable, a bridge's impassable cell; in patches (a cell copies a neighbour)
    let mut types = vec![0u8; (w * h) as usize];
    for k in 0..types.len() {
        types[k] = if k >= h as usize && g.below(3) != 0 {
            types[k - h as usize]
        } else if k % h as usize != 0 && g.below(2) == 0 {
            types[k - 1]
        } else {
            [0, 0, 0, 0, 1, 1, 2, 2, 3, 6, 5][g.below(11) as usize]
        };
    }
    let rates = [40 + g.below(56), g.below(30), g.below(30), g.below(30)];
    Scene { w, h, seed: g.u32(), rates, bridges, walls, types }
}

fn gen_op(g: &mut Gen, sc: &Scene, run: &Run) -> [u32; 6] {
    let count = sc.w * sc.h;
    let max_zone = u32::from(run.zm.max_zone);
    // a zone: often a map cell's (a fence's neighbours' zones are where the crusher tables differ)
    let cell_zone = |g: &mut Gen| u32::from(run.store.cells[g.below(count) as usize].get_zone());
    let (w, h) = (sc.w as i32, sc.h as i32);
    let xi = |g: &mut Gen, n: i32| (g.below((n + 6) as u32) as i32 - 3) as u32;
    let surfaces = |g: &mut Gen| if g.below(3) == 0 { g.below(32) } else { [1u32, 3, 5, 17, 7, 2, 8][g.below(7) as usize] };
    let layer = |g: &mut Gen| if g.below(4) == 0 { 15 } else { 2 + g.below(4) };
    let zone = |g: &mut Gen| g.below(max_zone + 3);
    match g.below(100) {
        0..=11 => [0, 0, 0, 0, 0, 0],
        12..=14 => {
            let (x, y) = (g.below(w as u32) as i32, g.below(h as u32) as i32);
            [1, (x - 1) as u32, (y - 1) as u32, (x + g.below(4) as i32) as u32, (y + g.below(4) as i32) as u32, 0]
        }
        15..=21 => [2, g.below(count), [0, 0, 1, 2, 3, 5, 6][g.below(7) as usize], 0, 0, 0],
        22..=24 => [3, g.below(count), if g.below(2) == 0 { 0 } else { zone(g) }, 0, 0, 0],
        25..=26 => [4, g.below(count), if g.below(2) == 0 { 0 } else { g.below(16) }, 0, 0, 0],
        27..=33 => [5, g.below(count), 1 + g.below(3), u32::from(g.below(3) != 0), 0, 0],
        34 => [6, g.below(count), 1 + g.below(3), 0, 0, 0],
        35..=40 => [7, surfaces(g), g.below(2), if g.below(2) == 0 { cell_zone(g) } else { zone(g) }, 0, 0],
        41..=43 => [8, g.below(max_zone + 1), 0, 0, 0, 0],
        44..=50 => [9, surfaces(g), g.below(2), g.below(w as u32), g.below(h as u32), 0],
        51 => [10, 0, 0, 0, 0, 0],
        52 => [11, 0, 0, 0, 0, 0],
        53..=54 => [12, xi(g, w), xi(g, h), g.below(2), 0, 0],
        55..=56 => [13 + g.below(2), xi(g, w), xi(g, h), 0, 0, 0],
        57 => [15, xi(g, w), xi(g, h), g.below(2), 0, 0],
        58..=59 => [16, xi(g, w), xi(g, h), 0, 0, 0],
        60..=61 => [17, [0u32, 1, 2, 3, 200, 301, 600][g.below(7) as usize], g.below(2), 0, 0, 0],
        62..=63 => [18, 0, 0, 0, 0, 0],
        64 => [19, 0, 0, 0, 0, 0],
        65..=69 => {
            let l = layer(g);
            let bridge = if l == 15 || sc.bridges.is_empty() || g.below(8) == 0 { NONE } else { g.below(4) };
            [20, l, bridge, if g.below(8) == 0 { 2 + g.below(14) } else { l }, 0, 0]
        }
        70..=73 => [21, layer(g), 0, 0, 0, 0],
        74..=77 => [22, layer(g), 0, 0, 0, 0],
        78..=80 => [23, layer(g), g.below(2), 0, 0, 0],
        81..=82 => [24, layer(g), 0, 0, 0, 0],
        83..=85 => [25, layer(g), xi(g, w), xi(g, h), 0, 0],
        86..=87 => [26, layer(g), surfaces(g), zone(g), zone(g), 0],
        88 => [27, layer(g), 0, 0, 0, 0],
        89..=91 => [28, 15, 0, 0, 0, 0],
        92..=94 => [29, if g.below(5) == 0 { layer(g) } else { 15 }, 0, 0, 0, 0],
        95..=97 => {
            let (x, y) = (g.uniform(-10.0, w as f32 * 10.0 + 10.0), g.uniform(-10.0, h as f32 * 10.0 + 10.0));
            [30, 15, x.to_bits(), y.to_bits(), 0, 0]
        }
        _ => [31, layer(g), g.below(300), 0, 0, 0],
    }
}

#[test]
fn zones_and_layers_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("PF_P3_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen::new(0x0010_C3A3);
    let mut check = support::Check::new("P3 zones and layers");
    let (mut ops_run, mut calcs, mut layer_ops) = (0u64, 0u64, 0u64);
    for case in 0..cases {
        let scene = gen_scene(&mut g);
        let n = 1 + g.below(40) as usize;
        let mut run = Run::new(&scene);
        let mut ops: Vec<[u32; 6]> = Vec::new();
        let mut r = Vec::new();
        // most programs start with the blocks and the zones, as a map does
        if g.below(4) != 0 {
            for o in [[18, 0, 0, 0, 0, 0], [0, 0, 0, 0, 0, 0]] {
                run.op(&o, &mut r);
                ops.push(o);
            }
        }
        while ops.len() < n {
            let o = gen_op(&mut g, &scene, &run);
            if !run.defined(&o) {
                continue;
            }
            // a modification's setup: its cells' zones unset (inside the structure's bounds)
            if o[0] == 1 && g.below(3) != 0 && ops.len() + 1 < n {
                let setup = [32, o[1].wrapping_add(1), o[3], o[2].wrapping_add(1), o[4], 0];
                run.op(&setup, &mut r);
                ops.push(setup);
            }
            run.op(&o, &mut r);
            ops.push(o);
        }
        let c = cpp_run(&scene, &ops);
        let (rs, cs) = (split_ops(&r), split_ops(&c));
        for (i, a) in rs.iter().enumerate() {
            let b = cs.get(i).cloned().unwrap_or_else(|| vec![MARKER]);
            check.eq(a.clone(), b, || format!("case {case} op {i} {:?} scene {scene:?} ops {:?}", ops[i], &ops[..=i]));
        }
        ops_run += ops.len() as u64;
        calcs += ops.iter().filter(|o| o[0] == 0 || o[0] == 1).count() as u64;
        layer_ops += ops.iter().filter(|o| o[0] >= 20).count() as u64;
    }
    println!("P3: {cases} cases, {ops_run} ops ({calcs} zone calculations and updates, {layer_ops} layer operations)");
    check.finish();
}
