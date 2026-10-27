#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (Pathfinder: the A* search)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Line2D.cpp (ClipLine2D)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The Pathfinder's A* search (`logic::ai::search`) and `ClipLine2D` against the unchanged C++, `p1w13-sonnet`, layer 10 batch P6.
//!
//! # Method
//!
//! The C++ side is five verbatim cuts of AIPathfind.cpp (build.rs, `search_p6.inc`: `debugShowSearch`; `checkChangeLayers` ..
//! `examineNeighboringCells`; `internalFindPath`, with one edit, its `return false` for a `Path *` made `NULL`; `clip`;
//! `buildActualPath` .. `setDebugPath`) on everything of P1-P5, in the pathfinder build (`ref_pf_p6_run`), with the real
//! `ClipLine2D` (Line2D.cpp, compiled by the partition build). `internalFindPath` is called non-virtually (the Pathfinder is zeroed
//! memory without a vtable). The paths a search builds are optimized by P1's `Path::optimize` with the REAL line tests (the driver's
//! `g_p1RealLines`). The world is P5's stand-ins, with each object's player (human, computer or none), `canPathThroughUnits`, dozer and
//! vehicle kinds, and the terrain's ground height hashed. The Rust side is a [`Pathfinder`] on a [`Host`].
//!
//! Each case is P5's kind of scene, its bridges added, a `newMap`, about half its objects' footprints, some units' positions (half of them fixed, their goals on their place), a logical extent
//! (usually the map), then 1 to 30 operations: searches between points on and off the map, from and to obstacles, for every kind of object (a
//! search without one is undefined upstream: a neighbour the surfaces refuse dereferences it), every surface set and downhill-only sets; `clip`; `ClipLine2D` on random segments and regions; the zone blocks' passable
//! flags cleared and set; the ignored obstacle. Compared, bit for bit, after every operation: its results (a path: every node's
//! position, layer, can-optimize and next optimized node, and blocked-by-ally), tunneling, the lists' heads, the cumulative cell count,
//! the info pool's first free info, the zone blocks' passable flags.
//!
//! # Results (2026-10-27, `PF_P6_CASES=4000`)
//!
//! 4,000 cases, 96,650 operations (41,191 searches, 5,099 paths found): **0 mismatches**, on the first run. Nothing is excluded but
//! the calls upstream leaves undefined (anything on the map before `newMap`; a search without an object).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 4,000 cases: 12 of 12 caught. 3092 and 3094 first survived for want of coverage (no
//! fixed units, no bridges); the scenes now add their bridges and fix some units.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3090 | `clip` moves the end 0.5 in y, not 0.05 | 555 |
//! | 3091 | a shallow cliff costs 6 diagonals, not 7 | 2,960 |
//! | 3092 | a cell next to fixed allies not blocked by them | 34 |
//! | 3093 | the zones' check skipped | 18,111 |
//! | 3094 | no change of layers | 5,171 |
//! | 3095 | the straight line's step costs 6, not 5 | 6,378 |
//! | 3096 | a diagonal opened past two closed orthogonals | 19,605 |
//! | 3097 | the path's cells' zone blocks not marked passable | 1,202 |
//! | 3098 | a pinched goal kept | 672 |
//! | 3099 | a closed zone block costs 99 cells, not 100 | 104 |
//! | 3100 | no surcharge for moving allies | 1,054 |
//! | 3101 | `ClipLine2D`'s second point clipped from the first's y | 3,105 |

mod support;

use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D, ICoord2D, Region2D, Region3D};
use rts_lockstep_engine::common::game_type::{ObjectId, PathfindLayer, LAYER_GROUND, LAYER_WALL};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::common::base_type::IRegion2D;
use rts_lockstep_engine::logic::ai::cell::CellId;
use rts_lockstep_engine::logic::ai::layer::{BridgeHandle, LayerHost, WallPieceFacts};
use rts_lockstep_engine::logic::ai::movement::MovementHost;
use rts_lockstep_engine::logic::ai::search::{PfLocomotorSet, SearchHost};
use rts_lockstep_engine::logic::world::partition::filters::clip_line_2d;
use rts_lockstep_engine::logic::ai::pathfinder::{PathfinderHost, Pathfinder, PfAiFacts, PfObjectFacts};
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
    /// (P5) attack uses line of sight; the allies, crush and weapon-line rates; each object's extras
    attack_los: bool,
    p5_rates: [u32; 3],
    extras: Vec<Extra>,
}

/// What P5 adds to an object.
#[derive(Clone, Copy, Debug, Default)]
struct Extra {
    ignored: ObjectId,
    crusher: i32,
    crushable: i32,
    above: bool,
    weapon: bool,
    slaver: ObjectId,
    container: Option<usize>,
    /// (P6) word 30's bits: a player, a computer player, can path through units, dozer, vehicle
    w30: u32,
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
        c.push(u32::from(self.attack_los));
        c.extend(self.bridge_rates);
        c.extend(self.p5_rates);
        c.push(self.bridges.len() as u32);
        for b in &self.bridges {
            c.extend([b.bounds.lo.x, b.bounds.lo.y, b.bounds.hi.x, b.bounds.hi.y, b.from.x, b.from.y, b.from.z, b.to.x, b.to.y, b.to.z].map(f32::to_bits));
        }
        c.push(self.objects.len() as u32);
        for (o, e) in self.objects.iter().zip(&self.extras) {
            let ai = o.ai.unwrap_or(PfAiFacts { doing_ground_movement: false, aircraft_adjusts_destination: false, goal_cell: ICoord2D { x: -1, y: -1 }, cur_cell: ICoord2D { x: -1, y: -1 } });
            c.extend([o.id as u32, o.pos.x.to_bits(), o.pos.y.to_bits(), o.pos.z.to_bits(), o.orientation.to_bits(), kinds_mask(o), u32::from(o.rubble)]);
            c.extend([o.fence_width.to_bits(), o.fence_x_offset.to_bits(), u32::from(o.mobile), u32::from(o.geom_type), o.major_radius.to_bits(), o.minor_radius.to_bits()]);
            c.extend([u32::from(o.small), o.bounding_circle_radius.to_bits(), o.height_above_terrain.to_bits(), o.layer as u32, o.destination_layer as u32]);
            c.extend([u32::from(o.ai.is_some()), u32::from(ai.doing_ground_movement), u32::from(ai.aircraft_adjusts_destination), u32::from(o.unmanned)]);
            let kinds2 = u32::from(o.attack_needs_line_of_sight) | (u32::from(o.aircraft_path_around) << 1);
            c.extend([e.ignored as u32, e.crusher as u32, e.crushable as u32, u32::from(e.above), kinds2, u32::from(e.weapon), e.slaver as u32]);
            c.extend([e.container.map_or(0, |k| k as u32 + 1), e.w30, 0]);
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
                aircraft_path_around: p(g, 25),
                attack_needs_line_of_sight: p(g, 60),
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
    let extras = (0..n)
        .map(|_| Extra {
            ignored: if g.below(3) == 0 { (1 + g.below(n)) as ObjectId } else { 0 },
            crusher: g.below(3) as i32,
            crushable: g.below(3) as i32,
            above: g.below(5) == 0,
            weapon: g.below(3) != 0,
            slaver: if g.below(4) == 0 { (1 + g.below(n)) as ObjectId } else { 0 },
            container: (g.below(4) == 0).then(|| g.below(n) as usize),
            w30: g.below(32),
        })
        .collect();
    Scene {
        attack_los: g.below(5) != 0,
        p5_rates: [g.below(101), g.below(101), g.below(101)],
        extras,
        seed: g.u32(),
        rates: [g.below(20), g.below(15), g.below(100), g.below(100)],
        extent: Region3D { lo, hi },
        wall_height: g.uniform(0.0, 30.0),
        bridge_rates: [40 + g.below(56), g.below(30), g.below(30), g.below(30)],
        bridges,
        objects,
    }
}


impl Host {
    fn extra(&self, id: ObjectId) -> Extra {
        self.objects.iter().position(|o| o.id == id).map_or_else(Extra::default, |k| self.s.extras[k])
    }
}

impl MovementHost for Host {
    fn ignored_obstacle_id(&self, id: ObjectId) -> ObjectId {
        self.extra(id).ignored
    }
    fn are_allies(&self, obj: ObjectId, unit: ObjectId) -> bool {
        pick(p1_hash(self.s.seed, 30, &[obj as u32, unit as u32]), self.s.p5_rates[0])
    }
    fn can_crush_or_squish(&self, obj: ObjectId, unit: ObjectId) -> bool {
        pick(p1_hash(self.s.seed, 31, &[obj as u32, unit as u32, 2]), self.s.p5_rates[1])
    }
    fn crusher_level(&self, id: ObjectId) -> i32 {
        self.extra(id).crusher
    }
    fn crushable_level(&self, id: ObjectId) -> i32 {
        self.extra(id).crushable
    }
    fn layer_height(&self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        ((p1_hash(self.s.seed, 15, &[x.to_bits(), y.to_bits(), layer as u32]) >> 8) % 40) as f32
    }
    fn layer_for_destination(&self, pos: &Coord3D) -> PathfindLayer {
        let h = p1_hash(self.s.seed, 1, &[pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()]);
        (1 + (h >> 8) % 3) as PathfindLayer
    }
    fn is_significantly_above_terrain(&self, id: ObjectId) -> bool {
        self.extra(id).above
    }
    fn attack_uses_line_of_sight(&self) -> bool {
        self.s.attack_los
    }
    fn weapon_terrain_line_clear(&self, attacker: ObjectId, p: &Coord3D, victim: Option<ObjectId>, vp: &Coord3D) -> Option<bool> {
        if !self.extra(attacker).weapon {
            return None;
        }
        let h = match victim {
            Some(v) => p1_hash(self.s.seed, 32, &[attacker as u32, p.x.to_bits(), p.y.to_bits(), v as u32]),
            None => p1_hash(self.s.seed, 33, &[attacker as u32, p.x.to_bits(), p.y.to_bits(), vp.x.to_bits(), vp.y.to_bits()]),
        };
        Some(pick(h, self.s.p5_rates[2]))
    }
    fn slaver_id(&self, id: ObjectId) -> ObjectId {
        self.extra(id).slaver
    }
    fn container_id(&self, id: ObjectId) -> ObjectId {
        self.extra(id).container.map_or(0, |k| self.objects[k % self.objects.len()].id)
    }
    fn closest_tall_building(&self, pos: &Coord3D, radius: f32) -> Option<ObjectId> {
        let mut best: Option<(ObjectId, f32)> = None;
        for o in self.objects.iter().filter(|o| o.aircraft_path_around) {
            let (dx, dy) = (o.pos.x - pos.x, o.pos.y - pos.y);
            let d2 = dx * dx + dy * dy;
            let r = radius + o.bounding_circle_radius;
            if d2 <= r * r && best.is_none_or(|(_, b)| d2 < b) {
                best = Some((o.id, d2));
            }
        }
        best.map(|(id, _)| id)
    }
}


impl SearchHost for Host {
    fn is_human(&self, id: ObjectId) -> bool {
        let w = self.extra(id).w30;
        !(w & 1 != 0 && w & 2 != 0)
    }
    fn can_path_through_units(&self, id: ObjectId) -> bool {
        self.extra(id).w30 & 4 != 0
    }
    fn is_dozer(&self, id: ObjectId) -> bool {
        self.extra(id).w30 & 8 != 0
    }
    fn is_vehicle(&self, id: ObjectId) -> bool {
        self.extra(id).w30 & 16 != 0
    }
    fn are_enemies(&self, obj: ObjectId, other: ObjectId) -> bool {
        !self.are_allies(obj, other)
    }
    fn ground_height_at(&self, x: f32, y: f32) -> f32 {
        ((p1_hash(self.s.seed, 16, &[x.to_bits(), y.to_bits()]) >> 8) % 40) as f32
    }
}

struct Run {
    pf: Pathfinder,
    host: Host,
}

impl Run {
    fn new(scene: &Scene) -> Self {
        let host = Host { s: scene.clone(), objects: scene.objects.clone(), splats: Vec::new() };
        Run { pf: Pathfinder::new(Some(scene.wall_height)), host }
    }
    fn obj(&self, k: u32) -> Option<PfObjectFacts> {
        (k != NONE).then(|| self.host.objects[(k as usize) % self.host.objects.len()])
    }
    fn defined(&self, o: &[u32; 8]) -> bool {
        matches!(o[0], 0 | 4 | 7 | 10 | 11) || self.pf.map_allocated
    }
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

    fn op(&mut self, o: &[u32; 8], v: &mut Vec<u32>) {
        let mut res: Vec<u32> = Vec::new();
        let pos = |x: u32, y: u32| Coord3D { x: f32::from_bits(x), y: f32::from_bits(y), z: 0.0 };
        let a = self.obj(o[1]);
        let id = a.map_or(0, |a| a.id);
        let (pf, host) = (&mut self.pf, &mut self.host);
        match o[0] {
            0 => pf.new_map(host, 0),
            1 => pf.classify_object_footprint(host, id, true, 0),
            2 => pf.update_goal(host, id, &pos(o[2], o[3]), o[4] as i32),
            3 => pf.update_pos(host, id, &pos(o[2], o[3])),
            4 => pf.ignore_obstacle_id = id,
            5 => {
                let loco = PfLocomotorSet { valid_surfaces: o[2] as i32, downhill_only: o[3] != 0 };
                let facts = a.map(|a| host.object(a.id).expect("an object"));
                let path = pf.internal_find_path(&*host, facts.as_ref(), loco, &pos(o[4], o[5]), &pos(o[6], o[7]));
                res.push(u32::from(path.is_some()));
                if let Some(path) = path {
                    let list = path.list();
                    res.push(list.len() as u32);
                    for &k in &list {
                        let nd = path.node(k);
                        let p = nd.position();
                        res.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), nd.layer() as u32, u32::from(nd.can_optimize())]);
                        let next = nd.next_optimized().0;
                        res.push(next.and_then(|n| list.iter().position(|&m| m == n)).map_or(0, |i| i as u32 + 1));
                    }
                    res.push(u32::from(path.blocked_by_ally()));
                }
            }
            6 => {
                let (mut f, mut t) = (pos(o[1], o[2]), pos(o[3], o[4]));
                pf.clip(&mut f, &mut t);
                res.extend([f.x.to_bits(), f.y.to_bits(), t.x.to_bits(), t.y.to_bits()]);
            }
            7 => {
                let p1 = ICoord2D { x: o[1] as i32, y: o[2] as i32 };
                let p2 = ICoord2D { x: o[3] as i32, y: o[4] as i32 };
                let r = IRegion2D { lo: ICoord2D { x: o[5] as i32, y: o[6] as i32 }, hi: ICoord2D { x: (o[7] & 0xffff) as i32, y: (o[7] >> 16) as i32 } };
                let unset = ICoord2D { x: 0x7777, y: 0x7777 };
                let (vis, c1, c2) = clip_line_2d(p1, p2, &r).unwrap_or((false, unset, unset));
                res.extend([u32::from(vis), c1.x as u32, c1.y as u32, c2.x as u32, c2.y as u32]);
            }
            8 => pf.zone_manager.clear_passable_flags(),
            9 => pf.zone_manager.set_all_passable(),
            10 => {
                pf.logical_extent = IRegion2D { lo: ICoord2D { x: o[1] as i32, y: o[2] as i32 }, hi: ICoord2D { x: o[3] as i32, y: o[4] as i32 } };
            }
            11 => res.push(pf.add_bridge(o[1] % host.s.bridges.len() as u32) as u32),
            _ => unreachable!(),
        }
        let mut d = vec![res.len() as u32];
        d.extend(&res);
        d.push(u32::from(self.pf.is_tunneling));
        d.extend([self.cell_code(self.pf.open_list), self.cell_code(self.pf.closed_list), self.pf.cumulative_cells_allocated as u32]);
        d.push(self.pf.store.first_free().unwrap_or(NONE));
        if let Some(blocks) = &self.pf.zone_manager.blocks {
            d.extend(blocks.iter().map(|b| u32::from(b.marked_passable)));
        }
        v.push(d.len() as u32);
        v.extend(d);
    }
}

fn cpp_run(scene: &Scene, ops: &[[u32; 8]]) -> Vec<u32> {
    let cfg = scene.cfg();
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut buf = vec![0u32; 1 << 22];
    let mut len = 0i32;
    let st = unsafe { cpp::ref_pf_p6_run(cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len) };
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

fn gen_op(g: &mut Gen, sc: &Scene) -> [u32; 8] {
    let n = sc.objects.len() as u32;
    let (wf, hf) = (sc.extent.hi.x, sc.extent.hi.y);
    // (mostly within the map; sometimes beyond it, or at x = 0)
    let px = |g: &mut Gen| match g.below(10) {
        0 => 0.0f32.to_bits(),
        1 => g.uniform(-15.0, wf + 15.0).to_bits(),
        _ => g.uniform(5.0, (wf - 5.0).max(6.0)).to_bits(),
    };
    let py = |g: &mut Gen| if g.below(10) == 0 { g.uniform(-15.0, hf + 15.0).to_bits() } else { g.uniform(5.0, (hf - 5.0).max(6.0)).to_bits() };
    let layer = |g: &mut Gen| [1u32, 1, 1, 2, 3, 15][g.below(6) as usize];
    let ob = |g: &mut Gen| g.below(n);
    let maybe = |g: &mut Gen| if g.below(5) == 0 { NONE } else { g.below(n) };
    let surf = |g: &mut Gen| if g.below(4) == 0 { g.below(32) } else { [1u32, 1, 1, 3, 5, 17, 7, 9][g.below(8) as usize] };
    let i = |g: &mut Gen, lo: i32, hi: i32| (lo + g.below((hi - lo + 1) as u32) as i32) as u32;
    match g.below(100) {
        0 => [0, 0, 0, 0, 0, 0, 0, 0],
        1..=4 => [1, ob(g), 0, 0, 0, 0, 0, 0],
        5..=8 => [2, ob(g), px(g), py(g), layer(g), 0, 0, 0],
        9..=12 => [3, ob(g), px(g), py(g), 0, 0, 0, 0],
        13 => [4, maybe(g), 0, 0, 0, 0, 0, 0],
        14..=79 => [5, ob(g), surf(g), u32::from(g.below(10) == 0), px(g), py(g), px(g), py(g)],
        80..=85 => [6, px(g), py(g), px(g), py(g), 0, 0, 0],
        86..=93 => {
            let (lx, ly) = (i(g, -5, 10), i(g, -5, 10));
            let (hx, hy) = (lx as i32 + g.below(20) as i32, ly as i32 + g.below(20) as i32);
            [7, i(g, -30, 40), i(g, -30, 40), i(g, -30, 40), i(g, -30, 40), lx, ly, ((hx as u32) & 0xffff) | ((hy as u32) << 16)]
        }
        94..=95 => [8, 0, 0, 0, 0, 0, 0, 0],
        96..=97 => [9, 0, 0, 0, 0, 0, 0, 0],
        _ => [10, i(g, -1, 3), i(g, -1, 3), i(g, 3, 25), i(g, 3, 25), 0, 0, 0],
    }
}

#[test]
fn search_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("PF_P6_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(4000);
    let mut g = Gen::new(0x0010_C3A6);
    let mut check = support::Check::new("P6 search");
    let (mut ops_run, mut searches, mut found) = (0u64, 0u64, 0u64);
    for case in 0..cases {
        let scene = gen_scene(&mut g);
        let n = 1 + g.below(30) as usize;
        let mut run = Run::new(&scene);
        let mut ops: Vec<[u32; 8]> = Vec::new();
        let mut r = Vec::new();
        let (wc, hc) = ((scene.extent.hi.x / 10.0) as u32, (scene.extent.hi.y / 10.0) as u32);
        // the bridges (before the map), the map, the logical extent, footprints, and units moving or fixed (a goal on their place)
        let mut setup: Vec<[u32; 8]> = (0..scene.bridges.len() as u32).map(|k| [11, k, 0, 0, 0, 0, 0, 0]).collect();
        setup.push([0, 0, 0, 0, 0, 0, 0, 0]);
        if g.below(5) != 0 {
            setup.push([10, 0, 0, wc, hc, 0, 0, 0]);
        }
        for k in 0..scene.objects.len() as u32 {
            match g.below(4) {
                0 | 1 => setup.push([1, k, 0, 0, 0, 0, 0, 0]),
                2 => {
                    let o = scene.objects[k as usize];
                    setup.push([3, k, o.pos.x.to_bits(), o.pos.y.to_bits(), 0, 0, 0, 0]);
                    if g.below(2) == 0 {
                        setup.push([2, k, o.pos.x.to_bits(), o.pos.y.to_bits(), o.layer as u32, 0, 0, 0]);
                    }
                }
                _ => {}
            }
        }
        for o in setup {
            run.op(&o, &mut r);
            ops.push(o);
        }
        let first = ops.len();
        while ops.len() < first + n {
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
                panic!("case {case} op {i} {:?}: first difference at {k}: rust {:?} c++ {:?}; ops {:?}", ops[i], a, b, &ops[..=i]);
            }
            if ops[i][0] == 5 {
                searches += 1;
                found += u64::from(a.get(1) == Some(&1));
            }
            check.eq(a.clone(), b, || format!("case {case} op {i} {:?} ops {:?}", ops[i], &ops[..=i]));
        }
        ops_run += ops.len() as u64;
    }
    println!("P6: {cases} cases, {ops_run} ops ({searches} searches, {found} paths found)");
    check.finish();
}
