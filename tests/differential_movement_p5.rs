#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (Pathfinder: the movement checks and the lines)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The Pathfinder's movement checks and lines (`logic::ai::movement`) against the unchanged C++, `p1w13-sonnet`, layer 10 batch P5.
//!
//! # Method
//!
//! The C++ side is AIPathfind.cpp's `IS_IMPASSABLE` and `TCheckMovementInfo` and five verbatim cuts (build.rs, `movement_p5.inc`:
//! `chooseBestLocomotorForPosition` .. `goalPosition`; `checkForLanding` and `adjustToLandingDestination`; `clearCellForDiameter`;
//! `adjustCoordToCell`; `iterateCellsAlongLine` .. `isGroundPathPassable`), on the map, cells, zones and layers of P2-P4, in the
//! pathfinder build (`ref_pf_p5_run`). Batch P1's paths now call the driver's hashed line tests through a one-name edit of their
//! cut, so that the real ones compile here. The world is P4's stand-ins and more: the objects' relations, crush tests and weapons'
//! terrain lines answer from a hash this test computes too; ignored obstacles, crush levels, slavers, containers, heights and kinds
//! are each object's; the closest tall building is by squared 2-D distance. The Rust side is a [`Pathfinder`] on a [`Host`].
//!
//! Each case is P4's kind of scene, a `newMap` and about half its objects' footprints, then 1 to 40 operations: units' goals and
//! positions, the ignored obstacle, and every movement check and line with objects present, absent and on every layer, cells at and
//! beyond the map's edges, the open and closed lists filled and cleaned. Compared, bit for bit, after every operation: its results
//! (positions as bits), the lists' heads, the cumulative cell count and the info pool's first free info.
//!
//! # Results (2026-10-27, `PF_P5_CASES=10000`)
//!
//! 10,000 cases, 216,464 operations (59,339 lines, 63,159 movement checks): **0 mismatches**, on the first run. Nothing is excluded
//! but the calls upstream leaves undefined (anything on the map before `newMap`, the object-dereferencing calls without an object, a
//! cell put on a list it is already linked into).
//!
//! **Gap found 2026-10-27 (batch P11):** no scene here made a unit infantry, so `checkForMovement`'s pass of infantry through
//! infantry (`INFANTRY_MOVES_THROUGH_INFANTRY`, which AIPathfind.h defines) was missing from the port and unseen. P11's scenes set
//! the kind, found it, and the port now has it (`differential_queue_p11.rs`, plant 3157).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 10,000 cases: 12 of 12 caught. A first 3072 (`validMovementTerrain`'s layer-equals-
//! `CELL_CLEAR` test dropped) survived because no reachable cell has layer 0, so upstream's typo is dead code; it was replaced.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3070 | an uncentred cell 0.06 in, not 0.05 | 10,541 |
//! | 3071 | the line never steps its minor axis | 2,009 |
//! | 3072 | `validMovementTerrain` refuses an obstacle | 123 |
//! | 3073 | a crusher not let over a fence | 13 |
//! | 3074 | an ally's goal usurped | 34 |
//! | 3075 | a fixed ally counted on every cell | 4 |
//! | 3076 | corners cut at radius 1 | 5,096 |
//! | 3077 | 2 cells skipped from a bridge, not 3 | 13 |
//! | 3078 | a pinched cell passable | 760 |
//! | 3079 | a landing above the map's top counted inside | 113 |
//! | 3080 | the attacker's container blocks its view | 18 |
//! | 3081 | a tall building's circle 0.9 of its radius | 17 |

mod support;

use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D, ICoord2D, Region2D, Region3D};
use rts_lockstep_engine::common::game_type::{ObjectId, PathfindLayer, LAYER_GROUND, LAYER_WALL};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::ai::cell::CellId;
use rts_lockstep_engine::logic::ai::layer::{BridgeHandle, LayerHost, WallPieceFacts};
use rts_lockstep_engine::logic::ai::movement::{valid_locomotor_surfaces_for_cell_type, CheckMovementInfo, LineProc, MovementHost};
use rts_lockstep_engine::logic::ai::path::Path;
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
            c.extend([e.container.map_or(0, |k| k as u32 + 1), 0, 0]);
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
    /// (P11's word 30 bit 11: P5's word 30 is 0)
    fn is_infantry(&self, _id: ObjectId) -> bool {
        false
    }
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

struct Run {
    pf: Pathfinder,
    host: Host,
}

fn bits(p: &Coord3D) -> [u32; 3] {
    [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
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
        if !matches!(o[0], 0 | 4 | 25) && !self.pf.map_allocated {
            return false;
        }
        match o[0] {
            10 | 11 | 12 | 17 | 18 | 21 => o[1] != NONE && (o[0] != 17 && o[0] != 21 || o[2] != NONE),
            22 | 23 => match self.pf.get_cell(o[3] as i32, o[1] as i32, o[2] as i32) {
                None => false,
                Some(c) => self.pf.store.cells[c as usize].info.is_none_or(|i| {
                    let f = self.pf.store.info(i);
                    !f.open && !f.closed && f.next_open.is_none() && f.prev_open.is_none()
                }),
            },
            _ => true,
        }
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
        let (b2, b4) = (self.obj(o[2]), self.obj(o[4]));
        let (pf, host) = (&mut self.pf, &mut self.host);
        match o[0] {
            0 => pf.new_map(host, 0),
            1 => pf.classify_object_footprint(host, id, true, 0),
            2 => pf.update_goal(host, id, &pos(o[2], o[3]), o[4] as i32),
            3 => pf.update_pos(host, id, &pos(o[2], o[3])),
            4 => pf.ignore_obstacle_id = id,
            5 => res.push(u32::from(pf.valid_movement_terrain(o[1] as i32, o[2] as i32, &pos(o[3], o[4])))),
            6 => {
                let c = pf.get_cell(o[3] as i32, o[4] as i32, o[5] as i32);
                res.push(u32::from(pf.valid_movement_position(o[1] != 0, o[2] as i32, c)));
            }
            7 => res.push(u32::from(pf.check_destination(host, a.as_ref(), o[2] as i32, o[3] as i32, o[4] as i32, o[5] as i32, o[6] != 0))),
            8 => {
                let mut info = CheckMovementInfo {
                    cell: ICoord2D { x: o[2] as i32, y: o[3] as i32 },
                    layer: o[4] as i32,
                    radius: o[5] as i32,
                    center_in_cell: o[6] != 0,
                    consider_transient: o[7] != 0,
                    acceptable_surfaces: 0,
                    ally_fixed_count: 77,
                    enemy_fixed: true,
                    ally_moving: true,
                    ally_goal: true,
                };
                let r = pf.check_for_movement(host, a.as_ref(), &mut info);
                res.extend([u32::from(r), info.ally_fixed_count as u32, u32::from(info.enemy_fixed), u32::from(info.ally_moving), u32::from(info.ally_goal)]);
            }
            9 => {
                let mut p = pos(o[2], o[3]);
                pf.snap_position(host, a.as_ref(), &mut p);
                res.extend(bits(&p));
            }
            10 => {
                let mut p = pos(o[2], o[3]);
                pf.snap_closest_goal_position(host, &a.expect("an object"), &mut p);
                res.extend(bits(&p));
            }
            11 => {
                let mut p = pos(0x3f80_0000, 0x3f80_0000);
                let r = pf.goal_position(host, &host.object(id).expect("an object"), &mut p);
                res.push(u32::from(r));
                res.extend(bits(&p));
            }
            12 => {
                let mut p = pos(o[2], o[3]);
                let r = pf.adjust_to_landing_destination(host, &a.expect("an object"), &mut p);
                res.push(u32::from(r));
                res.extend(bits(&p));
            }
            13 => res.push(pf.clear_cell_for_diameter(host, o[1] != 0, o[2] as i32, o[3] as i32, o[4] as i32, o[5] as i32) as u32),
            14 => {
                let mut p = pos(0, 0);
                Pathfinder::adjust_coord_to_cell(host, o[1] as i32, o[2] as i32, o[3] != 0, &mut p, o[4] as i32);
                res.extend(bits(&p));
            }
            15 => {
                let a = a.map(|a| host.object(a.id).expect("an object"));
                let r = pf.is_line_passable(host, a.as_ref(), o[2] as i32, (o[3] & 255) as i32, &pos(o[4], o[5]), &pos(o[6], o[7]), (o[3] >> 8) & 1 != 0, (o[3] >> 9) & 1 != 0);
                res.push(u32::from(r));
            }
            16 => res.push(u32::from(pf.is_ground_path_passable(host, o[1] != 0, &pos(o[2], o[3]), o[4] as i32, &pos(o[5], o[6]), o[7] as i32))),
            17 => {
                let (x, y) = (host.object(id).expect("an object"), host.object(b2.expect("an object").id).expect("an object"));
                res.push(u32::from(pf.is_view_blocked_by_obstacle(host, &x, &y)));
            }
            18 => {
                let att = host.object(id).expect("an object");
                let victim = b4.map(|v| host.object(v.id).expect("an object"));
                res.push(u32::from(pf.is_attack_view_blocked_by_obstacle(host, &att, &pos(o[2], o[3]), victim.as_ref(), &pos(o[5], o[6]))));
            }
            19 => {
                let mut path = Path::new();
                path.append_node(&pos(o[1], o[2]), LAYER_GROUND);
                path.append_node(&pos(o[3], o[4]), LAYER_GROUND);
                let first = path.first_node().expect("a node");
                let next = path.node(first).next().expect("two nodes");
                let r = pf.segment_intersects_tall_building(host, &mut path, first, next, o[5] as ObjectId);
                res.push(u32::from(r.is_some()));
                res.extend(bits(path.node(next).position()));
                if let Some(ps) = r {
                    for p in ps {
                        res.extend(bits(&p));
                    }
                }
            }
            20 => {
                let r = pf.circle_clips_tall_building(host, &pos(o[1], o[2]), &pos(o[3], o[4]), f32::from_bits(o[5]));
                res.push(u32::from(r.is_some()));
                if let Some(p) = r {
                    res.extend(bits(&p));
                }
            }
            21 => {
                let other = b2.expect("an object").id;
                let mut p = LineProc::LineBlocked { obj: id, other };
                res.push(pf.iterate_cells_along_line_world(host, &pos(o[3], o[4]), &pos(o[5], o[6]), o[7] as i32, &mut p) as u32);
            }
            22 | 23 => {
                let c = pf.get_cell(o[3] as i32, o[1] as i32, o[2] as i32).expect("a cell");
                if !pf.store.cells[c as usize].has_info() {
                    pf.store.allocate_info(c, ICoord2D { x: o[1] as i32, y: o[2] as i32 });
                }
                pf.store.set_cost_so_far(c, o[4]);
                pf.store.set_total_cost(c, o[4]);
                if o[0] == 22 {
                    pf.open_list = pf.store.put_on_sorted_open_list(c, pf.open_list);
                } else {
                    pf.closed_list = pf.store.put_on_closed_list(c, pf.closed_list);
                }
            }
            24 => pf.clean_open_and_closed_lists(),
            25 => res.push(valid_locomotor_surfaces_for_cell_type(o[1] as u8) as u32),
            _ => unreachable!(),
        }
        let mut d = vec![res.len() as u32];
        d.extend(&res);
        d.extend([self.cell_code(self.pf.open_list), self.cell_code(self.pf.closed_list), self.pf.cumulative_cells_allocated as u32]);
        d.push(self.pf.store.first_free().unwrap_or(NONE));
        v.push(d.len() as u32);
        v.extend(d);
    }
}

fn cpp_run(scene: &Scene, ops: &[[u32; 8]]) -> Vec<u32> {
    let cfg = scene.cfg();
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut buf = vec![0u32; 1 << 22];
    let mut len = 0i32;
    let st = unsafe { cpp::ref_pf_p5_run(cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len) };
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
    let (wc, hc) = ((wf / 10.0) as u32 + 2, (hf / 10.0) as u32 + 2);
    let px = |g: &mut Gen| g.uniform(-15.0, wf + 15.0).to_bits();
    let py = |g: &mut Gen| g.uniform(-15.0, hf + 15.0).to_bits();
    let cx = |g: &mut Gen| (g.below(wc + 2) as i32 - 2) as u32;
    let cy = |g: &mut Gen| (g.below(hc + 2) as i32 - 2) as u32;
    let layer = |g: &mut Gen| [1u32, 1, 1, 2, 3, 15, 0][g.below(7) as usize];
    let ob = |g: &mut Gen| g.below(n);
    let maybe = |g: &mut Gen| if g.below(6) == 0 { NONE } else { g.below(n) };
    let surf = |g: &mut Gen| if g.below(3) == 0 { g.below(32) } else { [1u32, 3, 5, 17, 7, 2, 8][g.below(7) as usize] };
    match g.below(100) {
        0..=1 => [0, 0, 0, 0, 0, 0, 0, 0],
        2..=9 => [1, ob(g), 0, 0, 0, 0, 0, 0],
        10..=15 => [2, ob(g), px(g), py(g), layer(g), 0, 0, 0],
        16..=21 => [3, ob(g), px(g), py(g), 0, 0, 0, 0],
        22 => [4, maybe(g), 0, 0, 0, 0, 0, 0],
        23..=25 => [5, layer(g), surf(g), px(g), py(g), 0, 0, 0],
        26..=28 => [6, g.below(2), surf(g), layer(g), cx(g), cy(g), 0, 0],
        29..=33 => [7, maybe(g), cx(g), cy(g), layer(g), g.below(3), g.below(2), 0],
        34..=38 => [8, maybe(g), cx(g), cy(g), layer(g), g.below(3), g.below(2), g.below(2)],
        39..=41 => [9, maybe(g), px(g), py(g), 0, 0, 0, 0],
        42..=45 => [10, ob(g), px(g), py(g), 0, 0, 0, 0],
        46..=47 => [11, ob(g), 0, 0, 0, 0, 0, 0],
        48..=50 => [12, ob(g), px(g), py(g), 0, 0, 0, 0],
        51..=55 => [13, g.below(2), cx(g), cy(g), layer(g), g.below(7), 0, 0],
        56..=57 => [14, cx(g), cy(g), g.below(2), layer(g), 0, 0, 0],
        58..=66 => [15, maybe(g), surf(g), layer(g) | (g.below(4) << 8), px(g), py(g), px(g), py(g)],
        67..=72 => [16, g.below(2), px(g), py(g), layer(g), px(g), py(g), g.below(6)],
        73..=75 => [17, ob(g), ob(g), 0, 0, 0, 0, 0],
        76..=80 => [18, ob(g), px(g), py(g), maybe(g), px(g), py(g), 0],
        81..=84 => [19, px(g), py(g), px(g), py(g), if g.below(3) == 0 { 1 + g.below(n) } else { 0 }, 0, 0],
        85..=87 => [20, px(g), py(g), px(g), py(g), g.uniform(0.0, 40.0).to_bits(), 0, 0],
        88..=90 => [21, ob(g), ob(g), px(g), py(g), px(g), py(g), layer(g)],
        91..=94 => [22, cx(g), cy(g), layer(g), g.below(200), 0, 0, 0],
        95..=96 => [23, cx(g), cy(g), layer(g), g.below(200), 0, 0, 0],
        97..=98 => [24, 0, 0, 0, 0, 0, 0, 0],
        _ => [25, g.below(8), 0, 0, 0, 0, 0, 0],
    }
}

#[test]
fn movement_and_lines_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("PF_P5_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(10_000);
    let mut g = Gen::new(0x0010_C3A5);
    let mut check = support::Check::new("P5 movement and lines");
    let (mut ops_run, mut lines, mut checks) = (0u64, 0u64, 0u64);
    for case in 0..cases {
        let scene = gen_scene(&mut g);
        let n = 1 + g.below(40) as usize;
        let mut run = Run::new(&scene);
        let mut ops: Vec<[u32; 8]> = Vec::new();
        let mut r = Vec::new();
        let mut setup = vec![[0u32, 0, 0, 0, 0, 0, 0, 0]];
        for k in 0..scene.objects.len() as u32 {
            if g.below(2) == 0 {
                setup.push([1, k, 0, 0, 0, 0, 0, 0]);
            }
        }
        for o in setup {
            run.op(&o, &mut r);
            ops.push(o);
        }
        while ops.len() < n + 1 {
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
                panic!("case {case} op {i} {:?}: rust {:?} c++ {:?}; ops {:?}", ops[i], a, b, &ops[..=i]);
            }
            check.eq(a.clone(), b, || format!("case {case} op {i} {:?} ops {:?}", ops[i], &ops[..=i]));
        }
        ops_run += ops.len() as u64;
        lines += ops.iter().filter(|o| (15..=21).contains(&o[0])).count() as u64;
        checks += ops.iter().filter(|o| (5..=14).contains(&o[0])).count() as u64;
    }
    println!("P5: {cases} cases, {ops_run} ops ({lines} lines, {checks} movement checks)");
    check.finish();
}
