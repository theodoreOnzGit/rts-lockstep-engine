#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (Pathfinder: the map, the footprints, the wall, the units' cells)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The Pathfinder's map, footprints, wall and units' cells (`logic::ai::pathfinder`) against the unchanged C++, `p1w13-sonnet`,
//! layer 10 batch P4.
//!
//! # Method
//!
//! The C++ side is two verbatim cuts of AIPathfind.cpp (build.rs, `pathfinder_p4.inc`: `Pathfinder::reset` .. `classifyMap` and
//! `forceMapRecalculation`; `changeBridgeState` .. `removeUnitFromPathfindMap`) with the header's inlines (`getCell`, `worldToCell`,
//! `getClippedCell`), on the real cells, zones and layers, in the pathfinder build (`differential/wrap_pathfind.cpp`,
//! `ref_pf_p4_run`). The Pathfinder is zeroed memory with its members constructed (its constructor would need the vtable of the
//! searches); its world is stand-ins: objects with the facts the pathfinder reads (kinds, fence, geometry, height, layers, AI cells),
//! the terrain's cliffs, water and bridge interactions answering from a hash this test computes too, the bridges as in P3. The Rust
//! side is a [`Pathfinder`] on a [`Host`] holding the same objects.
//!
//! Each case is a terrain of 4 to 18 by 4 to 16 cells, 0 to 2 bridges, 1 to 12 objects (structures of every shape, fences, mines,
//! blast craters, wall pieces, units with and without AI), and a program of 1 to 40 operations, most after a `newMap` (bridges and
//! wall pieces added before it): objects added to and removed from the map, the map recalculated and reset, bridges destroyed and
//! repaired, units' goals (ground and aircraft) and positions updated and removed, objects' layers updated and set, ground movement
//! toggled, cells asked for by index and position, points on the wall. Compared, bit for bit, after every operation: its results, the
//! pathfinder's extent, readiness, wall pieces and height, the zone manager and its blocks, every cell of the map and the layers
//! (type, flags, zone, aircraft goal, pinch, layers, its info and the info's units, obstacle, fence and see-through), the info pool's
//! free list, every object's layers and cells, and the splats.
//!
//! # Results (2026-10-26)
//!
//! `PF_P4_CASES=4000` (the default): 4,000 cases, 81,416 operations (14,054 footprints added or removed, 22,362 units' cells):
//! **0 mismatches**. `PF_P4_CASES=20000`: 412,549 operations (72,091 footprints, 114,278 units' cells), 0 mismatches. The first
//! runs found two defects in the TEST's driver, none in the port: the C++ read 26 words per object of a 22-word layout, and kept the
//! game logic's frame from the previous run (so `markZonesDirty` started from another frame); both fixed in the driver. Nothing is
//! excluded but the calls upstream leaves undefined (a cell asked for before the map exists, a wall layer with cells destroyed
//! without a bridge).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 4,000 cases: 13 of 13 caught. A first 3056 (a unit's goal removed over radius 0 when
//! its radius is 0) survived because it changes nothing: a unit's goal area is only ever set over that same radius, so the cells the
//! wider removal adds never hold the unit. It was replaced.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3050 | a round footprint 0.5 cells wider, not 0.4 | 15,568 |
//! | 3051 | a clear cell pinched with fewer than 3 clear neighbours, not 4 | 36,616 |
//! | 3052 | the clear cells next to a cliff not made cliffs | 62,230 |
//! | 3053 | water from the top left corner only | 62,570 |
//! | 3054 | a removed footprint's impassable cells left impassable | 4,822 |
//! | 3055 | a centred unit's goal one cell short | 6,525 |
//! | 3056 | a goal not removed from its destination layer | 181 |
//! | 3057 | a unit's radius rounded with 0.5, not 0.3 | 4,946 |
//! | 3058 | a unit on a bridge leaves its ground position cells | 73 |
//! | 3059 | footprints stepped in 0.6 cells, not 0.5 | 21,363 |
//! | 3060 | a fence's zones not updated | 2,664 |
//! | 3061 | nothing splatted off a removed wall | 658 |
//! | 3062 | a position beyond the map's right edge not flagged | 583 |

mod support;

use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D, ICoord2D, Region2D, Region3D};
use rts_lockstep_engine::common::game_type::{ObjectId, PathfindLayer, LAYER_GROUND, LAYER_LAST, LAYER_WALL};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::ai::cell::{CellId, PathfindCell};
use rts_lockstep_engine::logic::ai::layer::{BridgeHandle, LayerHost, WallPieceFacts};
use rts_lockstep_engine::logic::ai::pathfinder::{PathfinderHost, Pathfinder, PfAiFacts, PfObjectFacts, MAX_WALL_PIECES};
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

#[derive(Clone, Debug)]
struct Scene {
    seed: u32,
    /// cliff, water, bridge layer, bridge end
    rates: [u32; 4],
    extent: Region3D,
    wall_height: f32,
    bridge_rates: [u32; 4],
    bridges: Vec<Bridge>,
    objects: Vec<PfObjectFacts>,
}

/// The kinds in the cfg's bit order.
fn kinds_mask(o: &PfObjectFacts) -> u32 {
    [o.mine, o.projectile, o.bridge_tower, o.defensive_wall, o.blast_crater, o.walk_on_top_of_wall, o.structure, o.immobile, o.produced_at_helipad, o.can_see_through_structure]
        .iter()
        .enumerate()
        .map(|(b, &k)| u32::from(k) << b)
        .sum()
}

impl Scene {
    fn cfg(&self) -> Vec<u32> {
        let mut c = vec![self.seed, self.rates[0], self.rates[1], self.rates[2], self.rates[3]];
        c.extend([self.extent.lo.x, self.extent.lo.y, self.extent.hi.x, self.extent.hi.y, self.wall_height].map(f32::to_bits));
        c.extend(self.bridge_rates);
        c.push(self.bridges.len() as u32);
        for b in &self.bridges {
            c.extend([b.bounds.lo.x, b.bounds.lo.y, b.bounds.hi.x, b.bounds.hi.y, b.from.x, b.from.y, b.from.z, b.to.x, b.to.y, b.to.z].map(f32::to_bits));
        }
        c.push(self.objects.len() as u32);
        for o in &self.objects {
            let ai = o.ai.unwrap_or(PfAiFacts { doing_ground_movement: false, aircraft_adjusts_destination: false, goal_cell: ICoord2D { x: -1, y: -1 }, cur_cell: ICoord2D { x: -1, y: -1 } });
            c.extend([o.id as u32, o.pos.x.to_bits(), o.pos.y.to_bits(), o.pos.z.to_bits(), o.orientation.to_bits(), kinds_mask(o), u32::from(o.rubble)]);
            c.extend([o.fence_width.to_bits(), o.fence_x_offset.to_bits(), u32::from(o.mobile), u32::from(o.geom_type), o.major_radius.to_bits(), o.minor_radius.to_bits()]);
            c.extend([u32::from(o.small), o.bounding_circle_radius.to_bits(), o.height_above_terrain.to_bits(), o.layer as u32, o.destination_layer as u32]);
            c.extend([u32::from(o.ai.is_some()), u32::from(ai.doing_ground_movement), u32::from(ai.aircraft_adjusts_destination), u32::from(o.unmanned)]);
        }
        c
    }
}

/// The stand-ins' world.
struct Host {
    s: Scene,
    objects: Vec<PfObjectFacts>,
    splats: Vec<u32>,
}

impl Host {
    fn obj_mut(&mut self, id: ObjectId) -> &mut PfObjectFacts {
        self.objects.iter_mut().find(|o| o.id == id).expect("a known object")
    }
}

impl LayerHost for Host {
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
        pick(p1_hash(self.s.seed, 10, &[b, p.x.to_bits(), p.y.to_bits()]), self.s.bridge_rates[0])
    }
    fn is_cell_on_end(&self, b: BridgeHandle, c: &Region2D) -> bool {
        pick(p1_hash(self.s.seed, 11, &[b, c.lo.x.to_bits(), c.lo.y.to_bits(), c.hi.x.to_bits(), c.hi.y.to_bits()]), self.s.bridge_rates[1])
    }
    fn is_cell_on_side(&self, b: BridgeHandle, c: &Region2D) -> bool {
        pick(p1_hash(self.s.seed, 12, &[b, c.lo.x.to_bits(), c.lo.y.to_bits(), c.hi.x.to_bits(), c.hi.y.to_bits()]), self.s.bridge_rates[2])
    }
    fn is_cell_entry_point(&self, b: BridgeHandle, c: &Region2D) -> bool {
        pick(p1_hash(self.s.seed, 13, &[b, c.lo.x.to_bits(), c.lo.y.to_bits(), c.hi.x.to_bits(), c.hi.y.to_bits()]), self.s.bridge_rates[3])
    }
    fn bridge_height(&self, b: BridgeHandle, p: &Coord3D) -> f32 {
        ((p1_hash(self.s.seed, 14, &[b, p.x.to_bits(), p.y.to_bits()]) >> 8) % 40) as f32
    }
    fn ground_height(&self, x: f32, y: f32) -> f32 {
        ((p1_hash(self.s.seed, 15, &[x.to_bits(), y.to_bits(), 1]) >> 8) % 40) as f32
    }
    fn wall_piece(&self, id: ObjectId) -> Option<WallPieceFacts> {
        let o = self.objects.iter().find(|o| o.id == id)?;
        let m = o.major_radius;
        let bounds_2d = Region2D { lo: Coord2D { x: o.pos.x - m, y: o.pos.y - m }, hi: Coord2D { x: o.pos.x + m, y: o.pos.y + m } };
        Some(WallPieceFacts { pos: o.pos, orientation: o.orientation, bounds_2d, major_radius: m, minor_radius: o.minor_radius, is_sphere: o.geom_type == 0 })
    }
}

impl PathfinderHost for Host {
    fn object(&self, id: ObjectId) -> Option<PfObjectFacts> {
        self.objects.iter().find(|o| o.id == id).copied()
    }
    fn objects(&self) -> Vec<ObjectId> {
        self.objects.iter().map(|o| o.id).collect()
    }
    fn set_layer(&mut self, id: ObjectId, layer: PathfindLayer) {
        self.obj_mut(id).layer = layer;
    }
    fn set_destination_layer(&mut self, id: ObjectId, layer: PathfindLayer) {
        self.obj_mut(id).destination_layer = layer;
    }
    fn set_pathfind_goal_cell(&mut self, id: ObjectId, cell: ICoord2D) {
        self.obj_mut(id).ai.as_mut().expect("an AI").goal_cell = cell;
    }
    fn set_cur_pathfind_cell(&mut self, id: ObjectId, cell: ICoord2D) {
        self.obj_mut(id).ai.as_mut().expect("an AI").cur_cell = cell;
    }
    fn splat(&mut self, id: ObjectId) {
        self.splats.extend([id as u32, 18, 11, id as u32, 999_999.0f32.to_bits()]);
    }
    fn is_cliff_cell(&self, x: f32, y: f32) -> bool {
        pick(p1_hash(self.s.seed, 20, &[x.to_bits(), y.to_bits()]), self.s.rates[0])
    }
    fn is_underwater(&self, x: f32, y: f32) -> bool {
        pick(p1_hash(self.s.seed, 21, &[x.to_bits(), y.to_bits()]), self.s.rates[1])
    }
    fn maximum_pathfind_extent(&self) -> Region3D {
        self.s.extent
    }
    fn object_interacts_with_bridge_layer(&self, id: ObjectId, layer: PathfindLayer) -> bool {
        pick(p1_hash(self.s.seed, 22, &[id as u32, layer as u32]), self.s.rates[2])
    }
    fn object_interacts_with_bridge_end(&self, id: ObjectId, layer: PathfindLayer) -> bool {
        pick(p1_hash(self.s.seed, 23, &[id as u32, layer as u32]), self.s.rates[3])
    }
    fn wall_height(&self) -> f32 {
        self.s.wall_height
    }
}

struct Run {
    pf: Pathfinder,
    host: Host,
    frame: u32,
    high_water: u32,
}

impl Run {
    fn new(scene: &Scene) -> Self {
        let host = Host { s: scene.clone(), objects: scene.objects.clone(), splats: Vec::new() };
        Run { pf: Pathfinder::new(Some(scene.wall_height)), host, frame: 0, high_water: 0 }
    }
    fn obj(&self, k: u32) -> ObjectId {
        self.host.objects[(k as usize) % self.host.objects.len()].id
    }

    fn defined(&self, o: &[u32; 6]) -> bool {
        let need_map = !matches!(o[0], 0 | 1 | 2 | 3 | 4 | 6 | 19 | 20 | 21 | 22);
        if need_map && !self.pf.map_allocated {
            return false;
        }
        let wall = &self.pf.layers[LAYER_WALL as usize];
        match o[0] {
            2 => !self.host.s.bridges.is_empty(),
            10 => !((o[1] % 16) as i32 == LAYER_WALL && wall.bridge.is_none() && wall.width > 0),
            _ => true,
        }
    }

    /// The driver's code for a cell: the map's index, or the layer's offset under `0x100000 | layer << 16`.
    fn cell_code(&self, c: Option<CellId>) -> u32 {
        let Some(c) = c else { return NONE };
        let e = self.pf.extent;
        let count = ((e.hi.x + 1) * (e.hi.y + 1)) as u32;
        if self.pf.map_allocated && c < count {
            return c;
        }
        for (l, ly) in self.pf.layers.iter().enumerate() {
            if let Some(b) = ly.block {
                if c >= b && c < b + (ly.width * ly.height) as u32 {
                    return 0x10_0000 | ((l as u32) << 16) | (c - b);
                }
            }
        }
        0xffff_fffe
    }

    fn put_cell(&mut self, c: &PathfindCell, d: &mut Vec<u32>) {
        d.extend([u32::from(c.get_type()), u32::from(c.get_flags()), u32::from(c.get_zone()), u32::from(c.is_aircraft_goal())]);
        d.extend([u32::from(c.get_pinched()), c.get_layer() as u32, c.get_connect_layer() as u32, c.info.unwrap_or(NONE)]);
        if let Some(i) = c.info {
            let f = self.pf.store.info(i);
            d.extend([f.goal_unit_id as u32, f.pos_unit_id as u32, f.goal_aircraft_id as u32, f.obstacle_id as u32]);
            d.extend([u32::from(f.obstacle_is_fence), u32::from(f.obstacle_is_transparent), f.pos.x as u32, f.pos.y as u32]);
            self.high_water = self.high_water.max(i + 1);
        }
    }

    fn op(&mut self, o: &[u32; 6], v: &mut Vec<u32>) {
        let mut res = Vec::new();
        let pos = |x: u32, y: u32| Coord3D { x: f32::from_bits(x), y: f32::from_bits(y), z: 0.0 };
        let id = self.obj(o[1]);
        let frame = self.frame;
        let (pf, host) = (&mut self.pf, &mut self.host);
        match o[0] {
            0 => pf.new_map(host, frame),
            1 => pf.reset(Some(host.s.wall_height)),
            2 => res.push(pf.add_bridge(o[1] % host.s.bridges.len() as u32) as u32),
            3 => pf.add_wall_piece(id),
            4 => pf.remove_wall_piece(id),
            5 => res.push(u32::from(pf.is_point_on_wall(&pos(o[1], o[2])))),
            6 => pf.update_layer(host, id, o[2] as i32),
            7 => pf.classify_object_footprint(host, id, true, frame),
            8 => pf.classify_object_footprint(host, id, false, frame),
            9 => pf.force_map_recalculation(host),
            10 => pf.change_bridge_state(host, (o[1] % 16) as i32, o[2] != 0, frame),
            11 => pf.update_goal(host, id, &pos(o[2], o[3]), o[4] as i32),
            12 => pf.update_aircraft_goal(host, id, &pos(o[2], o[3])),
            13 => pf.remove_goal(host, id),
            14 => pf.update_pos(host, id, &pos(o[2], o[3])),
            15 => pf.remove_pos(host, id),
            16 => pf.remove_unit_from_pathfind_map(host, id),
            17 => {
                let c = pf.get_cell(o[1] as i32, o[2] as i32, o[3] as i32);
                res.push(c.map_or(NONE, |c| c | 0x8000_0000));
            }
            18 => {
                let p = pos(o[2], o[3]);
                let a = pf.get_cell_at(o[1] as i32, &p);
                let b = pf.get_clipped_cell(o[1] as i32, &p);
                res.extend([a.map_or(NONE, |c| c | 0x8000_0000), b.map_or(NONE, |c| c | 0x8000_0000)]);
            }
            19 => {
                let (c, overflow) = pf.world_to_cell(&pos(o[1], o[2]));
                res.extend([c.x as u32, c.y as u32, u32::from(overflow)]);
            }
            20 => host.obj_mut(id).layer = o[2] as i32,
            21 => {
                if let Some(ai) = host.obj_mut(id).ai.as_mut() {
                    ai.doing_ground_movement = o[2] != 0;
                }
            }
            22 => self.frame = o[1],
            _ => unreachable!(),
        }
        // (cells as their codes: the store's ids are marked above, encoded here)
        let res: Vec<u32> = res.into_iter().map(|r| if r != NONE && r & 0x8000_0000 != 0 && matches!(o[0], 17 | 18) { self.cell_code(Some(r & 0x7fff_ffff)) } else { r }).collect();
        let mut d = vec![res.len() as u32];
        d.extend(&res);
        let pf = &self.pf;
        let e = pf.extent;
        d.extend([e.lo.x as u32, e.lo.y as u32, e.hi.x as u32, e.hi.y as u32, u32::from(pf.map_allocated), u32::from(pf.is_map_ready), pf.num_wall_pieces as u32]);
        d.extend(pf.wall_pieces[..MAX_WALL_PIECES].iter().map(|&w| w as u32));
        d.push(pf.wall_height.to_bits());
        let zm = &pf.zone_manager;
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
        if self.pf.map_allocated {
            let count = ((e.hi.x + 1) * (e.hi.y + 1)) as usize;
            for k in 0..count {
                let c = self.pf.store.cells[k];
                self.put_cell(&c, &mut d);
            }
        }
        for l in 0..=LAYER_LAST as usize {
            let ly = self.pf.layers[l].clone();
            d.extend([ly.width as u32, ly.height as u32, ly.x_origin as u32, ly.y_origin as u32]);
            d.extend([ly.start_cell.x as u32, ly.start_cell.y as u32, ly.end_cell.x as u32, ly.end_cell.y as u32]);
            d.extend([ly.layer as u32, ly.zone as u32, ly.bridge.unwrap_or(NONE), u32::from(ly.destroyed), u32::from(ly.is_unused()), u32::from(ly.block.is_some())]);
            if ly.block.is_some() {
                for i in 0..ly.width {
                    for j in 0..ly.height {
                        let c = self.pf.store.cells[ly.layer_cell(i, j) as usize];
                        self.put_cell(&c, &mut d);
                    }
                }
            }
        }
        d.push(self.pf.store.first_free().unwrap_or(NONE));
        d.push(self.high_water);
        for k in 0..self.high_water {
            let f = self.pf.store.info(k);
            d.extend([u32::from(f.is_free), f.path_parent.unwrap_or(NONE)]);
        }
        for ob in &self.host.objects {
            let ai = ob.ai.map_or((ICoord2D { x: -1, y: -1 }, ICoord2D { x: -1, y: -1 }), |a| (a.goal_cell, a.cur_cell));
            d.extend([ob.layer as u32, ob.destination_layer as u32, ai.0.x as u32, ai.0.y as u32, ai.1.x as u32, ai.1.y as u32]);
        }
        d.push(self.host.splats.len() as u32);
        d.extend(&self.host.splats);
        v.push(d.len() as u32);
        v.extend(d);
    }
}

fn cpp_run(scene: &Scene, ops: &[[u32; 6]]) -> Vec<u32> {
    let cfg = scene.cfg();
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut buf = vec![0u32; 1 << 25];
    let mut len = 0i32;
    let st = unsafe { cpp::ref_pf_p4_run(cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len) };
    assert_eq!(st, 0, "the dump fits");
    buf.truncate(len as usize);
    buf
}

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
    let (w, h) = (4 + g.below(15), 4 + g.below(13));
    let lo = Coord3D { x: if g.below(4) == 0 { g.uniform(0.0, 15.0) } else { 0.0 }, y: if g.below(4) == 0 { g.uniform(0.0, 15.0) } else { 0.0 }, z: 0.0 };
    let hi = Coord3D { x: w as f32 * 10.0 + g.uniform(0.0, 9.9), y: h as f32 * 10.0 + g.uniform(0.0, 9.9), z: 0.0 };
    let (wf, hf) = (w as f32 * 10.0, h as f32 * 10.0);
    let bridges = (0..g.below(3))
        .map(|_| {
            let lo = Coord2D { x: g.uniform(-15.0, wf), y: g.uniform(-15.0, hf) };
            let hi = Coord2D { x: lo.x + g.uniform(5.0, 60.0), y: lo.y + g.uniform(5.0, 60.0) };
            let from = Coord3D { x: g.uniform(lo.x, hi.x), y: g.uniform(lo.y, hi.y), z: g.uniform(0.0, 20.0) };
            let to = Coord3D { x: g.uniform(lo.x, hi.x), y: g.uniform(lo.y, hi.y), z: g.uniform(0.0, 20.0) };
            Bridge { bounds: Region2D { lo, hi }, from, to }
        })
        .collect();
    let n = 1 + g.below(12);
    let objects = (0..n)
        .map(|k| {
            let p = |g: &mut Gen, pct: u32| g.below(100) < pct;
            let geom_type = [0u8, 1, 2, 2][g.below(4) as usize];
            let has_ai = p(g, 50);
            let layer = if p(g, 75) { LAYER_GROUND } else { [2, 3, LAYER_WALL][g.below(3) as usize] };
            PfObjectFacts {
                id: (1 + k) as ObjectId,
                pos: Coord3D { x: g.uniform(-5.0, wf + 5.0), y: g.uniform(-5.0, hf + 5.0), z: 0.0 },
                orientation: g.uniform(-3.2, 3.2),
                layer,
                destination_layer: if p(g, 70) { layer } else { [1, 2, 3][g.below(3) as usize] },
                mine: p(g, 5),
                projectile: p(g, 5),
                bridge_tower: p(g, 5),
                defensive_wall: p(g, 10),
                blast_crater: p(g, 5),
                walk_on_top_of_wall: p(g, 20),
                structure: p(g, 65),
                immobile: p(g, 10),
                produced_at_helipad: p(g, 15),
                can_see_through_structure: p(g, 20),
                aircraft_path_around: false,
                attack_needs_line_of_sight: false,
                aircraft: false,
                fs_airfield: false,
                unmanned: p(g, 15),
                rubble: p(g, 10),
                fence_width: if p(g, 15) { g.uniform(5.0, 40.0) } else { 0.0 },
                fence_x_offset: if p(g, 50) { g.uniform(-10.0, 10.0) } else { 0.0 },
                mobile: p(g, 20),
                geom_type,
                major_radius: g.uniform(2.0, 35.0),
                minor_radius: g.uniform(2.0, 20.0),
                small: p(g, 10),
                bounding_circle_radius: g.uniform(1.0, 25.0),
                height_above_terrain: if p(g, 85) { 0.0 } else { g.uniform(5.0, 20.0) },
                ai: has_ai.then(|| PfAiFacts {
                    doing_ground_movement: p(g, 70),
                    aircraft_adjusts_destination: p(g, 50),
                    goal_cell: ICoord2D { x: -1, y: -1 },
                    cur_cell: ICoord2D { x: -1, y: -1 },
                }),
            }
        })
        .collect();
    Scene {
        seed: g.u32(),
        rates: [g.below(20), g.below(15), g.below(100), g.below(100)],
        extent: Region3D { lo, hi },
        wall_height: g.uniform(0.0, 30.0),
        bridge_rates: [40 + g.below(56), g.below(30), g.below(30), g.below(30)],
        bridges,
        objects,
    }
}

fn gen_op(g: &mut Gen, sc: &Scene) -> [u32; 6] {
    let n = sc.objects.len() as u32;
    let (wf, hf) = (sc.extent.hi.x, sc.extent.hi.y);
    let px = |g: &mut Gen| g.uniform(-15.0, wf + 15.0).to_bits();
    let py = |g: &mut Gen| g.uniform(-15.0, hf + 15.0).to_bits();
    let layer = |g: &mut Gen| [1u32, 1, 2, 3, 15, 0][g.below(6) as usize];
    let ob = |g: &mut Gen| g.below(n);
    match g.below(100) {
        0..=2 => [0, 0, 0, 0, 0, 0],
        3 => [1, 0, 0, 0, 0, 0],
        4..=5 => [2, g.below(4), 0, 0, 0, 0],
        6..=8 => [3, ob(g), 0, 0, 0, 0],
        9..=10 => [4, ob(g), 0, 0, 0, 0],
        11..=13 => [5, px(g), py(g), 0, 0, 0],
        14..=16 => [6, ob(g), layer(g), 0, 0, 0],
        17..=30 => [7, ob(g), 0, 0, 0, 0],
        31..=38 => [8, ob(g), 0, 0, 0, 0],
        39..=40 => [9, 0, 0, 0, 0, 0],
        41..=43 => [10, [2u32, 3, 15][g.below(3) as usize], g.below(2), 0, 0, 0],
        44..=53 => [11, ob(g), px(g), py(g), layer(g), 0],
        54..=57 => [12, ob(g), px(g), py(g), 0, 0],
        58..=61 => [13, ob(g), 0, 0, 0, 0],
        62..=71 => [14, ob(g), px(g), py(g), 0, 0],
        72..=75 => [15, ob(g), 0, 0, 0, 0],
        76..=78 => [16, ob(g), 0, 0, 0, 0],
        79..=83 => [17, layer(g), (g.below(24) as i32 - 3) as u32, (g.below(22) as i32 - 3) as u32, 0, 0],
        84..=87 => [18, layer(g), px(g), py(g), 0, 0],
        88..=89 => [19, px(g), py(g), 0, 0, 0],
        90..=93 => [20, ob(g), layer(g), 0, 0, 0],
        94..=96 => [21, ob(g), g.below(2), 0, 0, 0],
        _ => [22, [0u32, 1, 2, 5, 400][g.below(5) as usize], 0, 0, 0, 0],
    }
}

#[test]
fn pathfinder_map_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("PF_P4_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(4000);
    let mut g = Gen::new(0x0010_C3A4);
    let mut check = support::Check::new("P4 pathfinder map");
    let (mut ops_run, mut footprints, mut units) = (0u64, 0u64, 0u64);
    for case in 0..cases {
        let scene = gen_scene(&mut g);
        let n = 1 + g.below(40) as usize;
        let mut run = Run::new(&scene);
        let mut ops: Vec<[u32; 6]> = Vec::new();
        let mut r = Vec::new();
        // before the map: bridges and wall pieces, as a map's loading adds them; then the map
        if g.below(8) != 0 {
            let mut setup = Vec::new();
            if !scene.bridges.is_empty() {
                for _ in 0..g.below(3) {
                    setup.push([2, g.below(4), 0, 0, 0, 0]);
                }
            }
            for _ in 0..g.below(4) {
                setup.push([3, g.below(scene.objects.len() as u32), 0, 0, 0, 0]);
            }
            setup.push([0, 0, 0, 0, 0, 0]);
            for o in setup {
                run.op(&o, &mut r);
                ops.push(o);
            }
        }
        while ops.len() < n {
            let o = gen_op(&mut g, &scene);
            if !run.defined(&o) {
                continue;
            }
            run.op(&o, &mut r);
            ops.push(o);
        }
        let c = cpp_run(&scene, &ops);
        let (rs, cs) = (split_ops(&r), split_ops(&c));
        for (i, a) in rs.iter().enumerate() {
            let b = cs.get(i).cloned().unwrap_or_else(|| vec![MARKER]);
            if std::env::var("PF_DEBUG").is_ok() && *a != b {
                let k = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
                let lo = k.saturating_sub(8);
                panic!("case {case} op {i} {:?}: first difference at {k} (lens {} {}): rust {:?} c++ {:?}; ops {:?}", ops[i], a.len(), b.len(), &a[lo..(k + 8).min(a.len())], &b[lo..(k + 8).min(b.len())], &ops[..=i]);
            }
            check.eq(a.clone(), b, || format!("case {case} op {i} {:?} scene {scene:?} ops {:?}", ops[i], &ops[..=i]));
        }
        ops_run += ops.len() as u64;
        footprints += ops.iter().filter(|o| o[0] == 7 || o[0] == 8).count() as u64;
        units += ops.iter().filter(|o| (11..=16).contains(&o[0])).count() as u64;
    }
    println!("P4: {cases} cases, {ops_run} ops ({footprints} footprints added or removed, {units} units' cells)");
    check.finish();
}
