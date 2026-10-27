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

//! The Pathfinder's `findPath`, hierarchical search, quick checks and `findBrokenBridge` (`logic::ai::hierarchical`) against the
//! unchanged C++, `p1w13-sonnet`, layer 10 batch P7.
//!
//! # Method
//!
//! The C++ side is P6's build and driver (`ref_pf_p6_run`, its ops 12-18) with `hierarchical_p7.inc`: `findPath` (the class's key
//! function, so the vtable is emitted and the Pathfinder is built by its real constructor; the virtual searches of later batches are
//! stubs), `buildHierachicalPath`, `processHierarchicalCell` .. `clientSafeQuickDoesPathExistForUI` and the Snapshot members, verbatim.
//! The world is P6's (see `differential_search_p6.rs`).
//!
//! Each case is P6's kind of scene and setup, then 1 to 30 operations: `findPath` and both hierarchical searches (human or not,
//! crusher or not) between points on and off the map, the quick checks (game and UI) and `findBrokenBridge` for every surface set, and
//! bridges destroyed and repaired (`changeBridgeState` only marks the zones dirty; `forceMapRecalculation` recomputes them). Compared, bit for bit, after every operation as in P6.
//!
//! The scenes differ from P6's where the hierarchical search needs it: maps up to 40 by 36 cells (several zone blocks each way), a
//! river across a third of them (the driver's water word's columns), bridges 15.5 cells long in x with bounds around their span,
//! their ends well inside the map; a destroyed bridge is followed by `forceMapRecalculation` and `findBrokenBridge` between its ends.
//!
//! # Results (2026-10-27, `PF_P7_CASES=20000`, the default)
//!
//! 20,000 cases, 478,777 operations (151,980 searches, 31,302 paths found, 58 broken bridges found): **0 mismatches**. Excluded,
//! because upstream hangs or crashes on them (found by this test; the port does the same, see `logic::ai::hierarchical`): a search
//! whose two ends are within two cells (from a bridge's cell to itself the parents loop), a bridge whose two ends lie in one zone
//! block, and a bridge end beyond the map's last cell but in its last zone block (the search reads the map array past its end).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 20,000 cases: 9 of 9 caught. 3113, 3116 and 3117 were not caught at 4,000 cases, and
//! 3116 not until the river and the destroy-recalculate-ask sequence were added.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3110 | `findPath` opens every zone block | 12,125 |
//! | 3111 | a side scan does not stop at a searched cell | 707 |
//! | 3112 | a goal on a cliff passes the quick check | 64,905 |
//! | 3113 | a pinched crossing costs 3 cells, not 2 | 1 |
//! | 3114 | the zones a side examined not recorded | 1,302 |
//! | 3115 | the hierarchical path's blocks not opened | 34,194 |
//! | 3116 | `findBrokenBridge` without the terrain zones | 3 |
//! | 3117 | the closest cell taken on a tie | 1 |
//! | 3118 | the UI check refuses an unzoned end | 207 |

mod support;

use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D, ICoord2D, Region2D, Region3D};
use rts_lockstep_engine::common::game_type::{ObjectId, PathfindLayer, LAYER_GROUND, LAYER_WALL};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::common::base_type::IRegion2D;
use rts_lockstep_engine::logic::ai::cell::CellId;
use rts_lockstep_engine::logic::ai::layer::{BridgeHandle, LayerHost, WallPieceFacts};
use rts_lockstep_engine::logic::ai::movement::MovementHost;
use rts_lockstep_engine::logic::ai::path::Path;
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
        // (the driver's water word: the rate, and a river of the columns lo .. hi when hi is not 0)
        let (lo, hi) = ((self.s.rates[1] >> 8) & 255, (self.s.rates[1] >> 16) & 255);
        if hi != 0 && x >= lo as f32 * 10.0 && x <= hi as f32 * 10.0 {
            return true;
        }
        pick(p1_hash(self.s.seed, 21, &[x.to_bits(), y.to_bits()]), self.s.rates[1] & 255)
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
    let (w, h) = (4 + g.below(37), 4 + g.below(33));
    let lo = Coord3D { x: if g.below(4) == 0 { g.uniform(0.0, 15.0) } else { 0.0 }, y: if g.below(4) == 0 { g.uniform(0.0, 15.0) } else { 0.0 }, z: 0.0 };
    let hi = Coord3D { x: w as f32 * 10.0 + g.uniform(0.0, 9.9), y: h as f32 * 10.0 + g.uniform(0.0, 9.9), z: 0.0 };
    let (wf, hf) = (w as f32 * 10.0, h as f32 * 10.0);
    let bridges = (0..g.below(3))
        .map(|_| {
            let lo = Coord2D { x: g.uniform(-15.0, wf), y: g.uniform(-15.0, hf) };
            let hi = Coord2D { x: lo.x + g.uniform(5.0, 60.0), y: lo.y + g.uniform(5.0, 60.0) };
            // (the ends well inside the map: an end beyond its last cell but in its last zone block is undefined upstream, the
            // hierarchical search reading the map array past its end)
            let inside = |v: f32, max: f32| v.clamp(15.0, (max - 15.0).max(15.0));
            let from = Coord3D { x: inside(g.uniform(lo.x, hi.x), wf), y: inside(g.uniform(lo.y, hi.y), hf), z: g.uniform(0.0, 20.0) };
            let mut to = Coord3D { x: inside(g.uniform(lo.x, hi.x), wf), y: inside(g.uniform(lo.y, hi.y), hf), z: g.uniform(0.0, 20.0) };
            // (and their ends in different zone blocks: with both in one block, the hierarchical search can put the cell it popped
            // back on the open list as the bridge's end, while it also goes on the closed list; the lists then cycle or crash,
            // upstream as here. 15 cells apart in x; a map too narrow gets the bridge's end off the map, which the search skips)
            to.x = if from.x + 155.0 < wf - 15.0 { from.x + 155.0 } else { from.x - 155.0 };
            // (the bounds around the span, as a real bridge's are)
            let lo = Coord2D { x: from.x.min(to.x) - 5.0, y: from.y.min(to.y) - g.uniform(5.0, 15.0) };
            let hi = Coord2D { x: from.x.max(to.x) + 5.0, y: from.y.max(to.y) + g.uniform(5.0, 15.0) };
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
        // (a third of the maps have a river: water columns across the middle, for the bridges over it)
        rates: [g.below(20), g.below(15) | if w >= 12 && g.below(3) == 0 { ((w / 2 - 1) << 8) | ((w / 2 + 1) << 16) } else { 0 }, g.below(100), g.below(100)],
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
    /// (P11's word 30 bit 11: never set here, as the C++ reads it)
    fn is_infantry(&self, id: ObjectId) -> bool {
        self.extra(id).w30 >> 11 & 1 != 0
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

/// A search's result as the driver dumps it.
fn dump_path(path: Option<Path>, res: &mut Vec<u32>) {
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
        if std::env::var("PF_TRACE").is_ok() {
            eprintln!("op {o:?}");
        }
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
                dump_path(path, &mut res);
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
            12..=14 => {
                let loco = PfLocomotorSet { valid_surfaces: o[2] as i32, downhill_only: o[3] != 0 };
                let path = if o[0] == 12 {
                    let facts = a.map(|a| host.object(a.id).expect("an object"));
                    pf.find_path(&*host, facts.as_ref(), loco, &pos(o[4], o[5]), &pos(o[6], o[7]))
                } else if o[0] == 13 {
                    pf.find_hierarchical_path(&*host, o[1] != 0, o[2] as i32, &pos(o[4], o[5]), &pos(o[6], o[7]), o[3] != 0)
                } else {
                    pf.find_closest_hierarchical_path(&*host, o[1] != 0, o[2] as i32, &pos(o[4], o[5]), &pos(o[6], o[7]), o[3] != 0)
                };
                dump_path(path, &mut res);
            }
            15 => res.push(u32::from(pf.client_safe_quick_does_path_exist(&*host, o[1] as i32, &pos(o[2], o[3]), &pos(o[4], o[5])))),
            16 => res.push(u32::from(pf.client_safe_quick_does_path_exist_for_ui(&*host, o[1] as i32, &pos(o[2], o[3]), &pos(o[4], o[5])))),
            17 => {
                let (r, id) = pf.find_broken_bridge(&*host, o[1] as i32, &pos(o[2], o[3]), &pos(o[4], o[5]));
                res.extend([u32::from(r), if r { id as u32 } else { 0 }]);
            }
            18 => pf.change_bridge_state(&*host, (o[1] % 16) as i32, o[2] != 0, 0),
            19 => pf.force_map_recalculation(&*host),
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
    match g.below(100) {
        0..=3 => [1, ob(g), 0, 0, 0, 0, 0, 0],
        4..=7 => [2, ob(g), px(g), py(g), layer(g), 0, 0, 0],
        8..=11 => [3, ob(g), px(g), py(g), 0, 0, 0, 0],
        12 => [4, maybe(g), 0, 0, 0, 0, 0, 0],
        13..=17 => [0, 0, 0, 0, 0, 0, 0, 0],
        18..=42 => [12, ob(g), surf(g), u32::from(g.below(10) == 0), px(g), py(g), px(g), py(g)],
        43..=57 => [13, g.below(2), surf(g), g.below(2), px(g), py(g), px(g), py(g)],
        58..=69 => [14, g.below(2), surf(g), g.below(2), px(g), py(g), px(g), py(g)],
        70..=79 => [15, surf(g), px(g), py(g), px(g), py(g), 0, 0],
        80..=85 => [16, surf(g), px(g), py(g), px(g), py(g), 0, 0],
        86..=91 => {
            // (half of them from one end of a bridge to the other, where a destroyed bridge would join two zones)
            if !sc.bridges.is_empty() && g.below(2) == 0 {
                let b = sc.bridges[g.below(sc.bridges.len() as u32) as usize];
                [17, surf(g), b.from.x.to_bits(), b.from.y.to_bits(), b.to.x.to_bits(), b.to.y.to_bits(), 0, 0]
            } else {
                [17, surf(g), px(g), py(g), px(g), py(g), 0, 0]
            }
        }
        92..=95 => [18, [2u32, 3, 4][g.below(3) as usize], g.below(2), 0, 0, 0, 0, 0],
        96..=97 => [8, 0, 0, 0, 0, 0, 0, 0],
        _ => [9, 0, 0, 0, 0, 0, 0, 0],
    }
}

#[test]
fn hierarchical_search_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("PF_P7_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen::new(0x0010_C3A7);
    let mut check = support::Check::new("P7 findPath and the hierarchical search");
    let (mut ops_run, mut searches, mut found, mut broken) = (0u64, 0u64, 0u64, 0u64);
    for case in 0..cases {
        let scene = gen_scene(&mut g);
        if std::env::var("PF_TRACE").is_ok() {
            eprintln!("case {case}");
        }
        let cpp_only = std::env::var("PF_CPP_ONLY").ok().and_then(|v| v.parse::<usize>().ok()) == Some(case);
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
        let mut panicked = false;
        while ops.len() < first + n {
            let o = gen_op(&mut g, &scene);
            if !run.defined(&o) {
                continue;
            }
            // (a bridge destroyed: the map recalculated, and its ends asked for the broken bridge, as an AI deciding to repair it)
            let combo: Vec<[u32; 8]> = if o[0] == 18 && o[2] == 0 && !scene.bridges.is_empty() && ops.len() + 3 <= first + n {
                let b = scene.bridges[g.below(scene.bridges.len() as u32) as usize];
                vec![[19, 0, 0, 0, 0, 0, 0, 0], [17, 1, b.from.x.to_bits(), b.from.y.to_bits(), b.to.x.to_bits(), b.to.y.to_bits(), 0, 0]]
            } else {
                Vec::new()
            };
            // (a search's two ends at least two cells apart: from a bridge's cell to the same cell is an upstream hang, see
            // logic::ai::hierarchical)
            if matches!(o[0], 12..=14) {
                let (fx, fy, tx, ty) = (f32::from_bits(o[4]), f32::from_bits(o[5]), f32::from_bits(o[6]), f32::from_bits(o[7]));
                if (fx - tx).abs().max((fy - ty).abs()) < 20.0 {
                    continue;
                }
            }
            // (a Rust panic ends the program there: the ops before it are still compared, to show where the two first differ)
            if cpp_only {
                // (the program as generated, the C++ alone: the setup made the map, so every op is defined)
                ops.push(o);
                continue;
            }
            let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run.op(&o, &mut r))).is_ok();
            if !ok {
                panicked = true;
                if std::env::var("PF_CPP_TOO").is_ok() {
                    ops.push(o);
                }
                break;
            }
            ops.push(o);
            for x in combo {
                run.op(&x, &mut r);
                ops.push(x);
            }
        }
        let c = cpp_run(&scene, &ops);
        if cpp_only {
            eprintln!("case {case}: the C++ alone completed {} ops", split_ops(&c).len());
            continue;
        }
        let (rs, cs) = (split_ops(&r), split_ops(&c));
        for (i, a) in rs.iter().enumerate() {
            let b = cs.get(i).cloned().unwrap_or_else(|| vec![MARKER]);
            if std::env::var("PF_DEBUG").is_ok() && *a != b {
                let k = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
                panic!("case {case} op {i} {:?}: first difference at {k}: rust {:?} c++ {:?}; ops {:?}", ops[i], a, b, &ops[..=i]);
            }
            if ops[i][0] == 17 && a.get(1) == Some(&1) {
                broken += 1;
            }
            if matches!(ops[i][0], 12..=14) {
                searches += 1;
                found += u64::from(a.get(1) == Some(&1));
            }
            check.eq(a.clone(), b, || format!("case {case} op {i} {:?} ops {:?}", ops[i], &ops[..=i]));
        }
        assert!(!panicked, "case {case}: the Rust side panicked after op {} (the ops before it matched)", ops.len());
        ops_run += ops.len() as u64;
    }
    println!("P7: {cases} cases, {ops_run} ops ({searches} searches, {found} paths found, {broken} broken bridges found)");
    check.finish();
}
