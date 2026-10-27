#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (Pathfinder: getMoveAwayFromPath, patchPath, findAttackPath,
//     findSafePath)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The Pathfinder's purposeful searches (`logic::ai::attack`) against the unchanged C++, `p1w13-sonnet`, layer 10 batch P10.
//!
//! # Method
//!
//! The C++ side is P6's build and driver (`ref_pf_p6_run`, its ops 28 to 31) with `attack_p10.inc` (`getMoveAwayFromPath` ..
//! `findSafePath`, verbatim but for its `return false` as `return NULL`); the scenes are P9's, each unit's AI also idle or not and
//! with its own locomotors (word 30's bits 5 and 6-10), and the weapon's attack distance a stand-in answered from the hash (tag 35,
//! 0 to 149). Each case is a scene and its setup, then 1 to 30 operations: `findAttackPath` against a unit or a position, from its
//! place or elsewhere; `findSafePath` from two repulsors of radius 0 to 120; `patchPath` of the path `findPath` finds, blocked or
//! not; `getMoveAwayFromPath` from one or two paths another unit's `findPath` finds; a few of P9's adjustments; footprints, units'
//! positions and goals, the ignored obstacle, bridges destroyed and the map recalculated, the zone blocks' passable flags. (Never a
//! search whose hierarchical start and goal are one bridge cell after `clip`, P7's upstream hang.) Compared, bit for bit, after every
//! operation as in P6.
//!
//! # Results (2026-10-27, `PF_P10_CASES=10000`, the default)
//!
//! 10,000 cases, 239,613 operations (101,741 searches: 30,055 `findAttackPath`, 22,557 `findSafePath`, 24,489 `patchPath`, 24,640
//! `getMoveAwayFromPath`; paths found 8,627, 6,667, 1,305 and 1,327): **0 mismatches**, on the first run.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 10,000 cases: 9 of 9 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3140 | the straight steps toward the victim stop at 4, not 9 | 3,522 |
//! | 3141 | the view to the victim not checked | 6,262 |
//! | 3142 | a vehicle does not back up from blocked cells | 23 |
//! | 3143 | `findSafePath` ignores the second repulsor | 12,295 |
//! | 3144 | `patchPath` keeps the path's end, not the node nearest the unit | 8,625 |
//! | 3145 | `getMoveAwayFromPath`'s box ignores the other unit's centring | 590 |
//! | 3146 | `getMoveAwayFromPath` may stay in the start cell | 5,866 |
//! | 3147 | `patchPath` considers transient units 3 cells out, not 2 | 7 |
//! | 3148 | the attack distance's margin 2 cells, not 3 | 3,057 |

mod support;

use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D, ICoord2D, Region2D, Region3D};
use rts_lockstep_engine::common::game_type::{ObjectId, PathfindLayer, LAYER_GROUND, LAYER_WALL};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::common::base_type::IRegion2D;
use rts_lockstep_engine::logic::ai::adjust::AdjustHost;
use rts_lockstep_engine::logic::ai::attack::AttackHost;
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
            let kinds2 = u32::from(o.attack_needs_line_of_sight) | (u32::from(o.aircraft_path_around) << 1) | (u32::from(o.aircraft) << 2) | (u32::from(o.fs_airfield) << 3);
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
                aircraft: p(g, 15),
                fs_airfield: p(g, 10),
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
            w30: g.below(2048),
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
impl AttackHost for Host {
    fn attack_distance(&self, obj: ObjectId, victim: Option<ObjectId>, vp: &Coord3D) -> f32 {
        ((p1_hash(self.s.seed, 35, &[obj as u32, victim.map_or(NONE, |v| v as u32), vp.x.to_bits(), vp.y.to_bits()]) >> 8) % 150) as f32
    }
    fn is_idle(&self, id: ObjectId) -> bool {
        self.object(id).is_some_and(|o| o.ai.is_some()) && self.extra(id).w30 & 32 != 0
    }
    fn ai_locomotor_set(&self, id: ObjectId) -> PfLocomotorSet {
        PfLocomotorSet { valid_surfaces: ((self.extra(id).w30 >> 6) & 31) as i32, downhill_only: false }
    }
}

impl AdjustHost for Host {
    fn goal_pos_within_attack_range(&self, obj: ObjectId, goal: &Coord3D, victim: Option<ObjectId>, vp: &Coord3D) -> bool {
        let words = [obj as u32, goal.x.to_bits(), goal.y.to_bits(), victim.map_or(NONE, |v| v as u32), vp.x.to_bits(), vp.y.to_bits()];
        pick(p1_hash(self.s.seed, 34, &words), self.s.p5_rates[2])
    }
}

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
    /// The hierarchical search's start and goal for a search from words `f`, `f + 1` to words `t`, `t + 1` are one bridge cell (an
    /// upstream hang).
    fn same_bridge_cell(&self, o: &[u32; 8], f: usize, t: usize) -> bool {
        let (from, raw_to) = (Coord3D { x: f32::from_bits(o[f]), y: f32::from_bits(o[f + 1]), z: 0.0 }, Coord3D { x: f32::from_bits(o[t]), y: f32::from_bits(o[t + 1]), z: 0.0 });
        let (mut cf, mut to) = (from, raw_to);
        self.pf.clip(&mut cf, &mut to);
        let goal = self.pf.get_cell(self.host.layer_for_destination(&to), self.pf.world_to_cell(&to).0.x, self.pf.world_to_cell(&to).0.y);
        let start = self.pf.get_clipped_cell(self.host.layer_for_destination(&from), &cf);
        matches!((start, goal), (Some(a), Some(b)) if a == b && self.pf.store.cells[a as usize].get_layer() != LAYER_GROUND)
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
            20 => {
                let path = pf.find_ground_path(&*host, &pos(o[4], o[5]), &pos(o[6], o[7]), o[1] as i32, o[2] != 0);
                dump_path(path, &mut res);
            }
            28..=31 => {
                let f = a.map(|a| host.object(a.id).expect("an object")).expect("an object");
                let loco = PfLocomotorSet { valid_surfaces: o[2] as i32, downhill_only: false };
                match o[0] {
                    28 => {
                        let victim = self_obj(&host.objects, o[3]).map(|v| host.object(v.id).expect("an object"));
                        let path = pf.find_attack_path(&*host, &f, loco, &pos(o[6], o[7]), victim.as_ref(), &pos(o[4], o[5]));
                        dump_path(path, &mut res);
                    }
                    29 => {
                        let path = pf.find_safe_path(&*host, &f, loco, &f.pos, &pos(o[3], o[4]), &pos(o[5], o[6]), f32::from_bits(o[7]));
                        dump_path(path, &mut res);
                    }
                    30 => {
                        let original = pf.find_path(&*host, Some(&f), loco, &pos(o[3], o[4]), &pos(o[5], o[6]));
                        let path = pf.patch_path(&*host, &f, loco, original.as_ref(), o[7] != 0);
                        dump_path(path, &mut res);
                    }
                    _ => {
                        let other = self_obj(&host.objects, o[2]).map(|v| host.object(v.id).expect("an object"));
                        let loco = PfLocomotorSet { valid_surfaces: (o[7] & 31) as i32, downhill_only: false };
                        let (pa, pb) = (pos(o[3], o[4]), pos(o[5], o[6]));
                        let avoid = pf.find_path(&*host, other.as_ref(), loco, &pa, &pb);
                        let avoid2 = if o[7] & 32 != 0 { pf.find_path(&*host, other.as_ref(), loco, &pb, &pa) } else { None };
                        res.push(u32::from(avoid.is_some()));
                        if let Some(avoid) = &avoid {
                            let path = pf.get_move_away_from_path(&*host, &f, other.as_ref(), avoid, avoid2.as_ref());
                            dump_path(path, &mut res);
                        }
                    }
                }
            }
            21..=27 => {
                let loco = PfLocomotorSet { valid_surfaces: o[2] as i32, downhill_only: false };
                let facts = a.map(|a| host.object(a.id).expect("an object"));
                let put = |res: &mut Vec<u32>, p: &Coord3D| res.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
                match o[0] {
                    21 | 23 => {
                        let mut dest = pos(o[3], o[4]);
                        let group = (o[5] != NONE).then(|| pos(o[5], o[6]));
                        let f = facts.expect("an object");
                        let r = if o[0] == 21 {
                            pf.adjust_destination(&*host, &f, loco, &mut dest, group.as_ref())
                        } else {
                            pf.adjust_to_possible_destination(&*host, &f, loco, &mut dest)
                        };
                        res.push(u32::from(r));
                        put(&mut res, &dest);
                    }
                    22 => {
                        let target = self_obj(&host.objects, o[2]).map(|t| t.id);
                        let mut dest = pos(o[5], o[6]);
                        let r = pf.adjust_target_destination(&*host, &facts.expect("an object"), target, &pos(o[3], o[4]), &mut dest);
                        res.push(u32::from(r));
                        put(&mut res, &dest);
                    }
                    24 => {
                        let dest = pos(o[3], o[4]);
                        let r = pf.path_destination(&*host, facts.as_ref(), loco, &dest, o[5] as i32, &pos(o[6], o[7]));
                        res.push(u32::from(r));
                        put(&mut res, &dest);
                    }
                    25 => {
                        let mut from = pos(o[3], o[4]);
                        pf.tighten_path(&*host, &facts.expect("an object"), loco, &mut from, &pos(o[5], o[6]));
                        put(&mut res, &from);
                    }
                    26 => res.push(pf.check_path_cost(&*host, facts.as_ref(), loco, &pos(o[3], o[4]), &pos(o[5], o[6])) as u32),
                    _ => {
                        let mut to = pos(o[5], o[6]);
                        let path = pf.find_closest_path(&*host, facts.as_ref(), loco, &pos(o[3], o[4]), &mut to, o[7] & 1 != 0, (o[7] >> 1) as f32 / 4.0);
                        dump_path(path, &mut res);
                        put(&mut res, &to);
                    }
                }
            }
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

/// `Run::obj` on the objects alone (the target of op 22, while the pathfinder is borrowed).
fn self_obj(objects: &[PfObjectFacts], k: u32) -> Option<PfObjectFacts> {
    (k != NONE).then(|| objects[(k as usize) % objects.len()])
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
    let surfaces = |g: &mut Gen| [1u32, 1, 1, 3, 5, 17, 8, 0x1f, 0][g.below(9) as usize];
    let some_ob = |g: &mut Gen| if g.below(8) == 0 { NONE } else { g.below(n) };
    let radius = |g: &mut Gen| g.uniform(0.0, 120.0).to_bits();
    match g.below(100) {
        0..=3 => [1, ob(g), 0, 0, 0, 0, 0, 0],
        4..=7 => [2, ob(g), px(g), py(g), layer(g), 0, 0, 0],
        8..=10 => [3, ob(g), px(g), py(g), 0, 0, 0, 0],
        11..=12 => [0, 0, 0, 0, 0, 0, 0, 0],
        13..=15 => [4, some_ob(g), 0, 0, 0, 0, 0, 0],
        16..=35 => {
            // (from the unit's place, or elsewhere)
            let k = ob(g);
            let o = sc.objects[k as usize];
            let (fx, fy) = if g.below(2) == 0 { (o.pos.x.to_bits(), o.pos.y.to_bits()) } else { (px(g), py(g)) };
            [28, k, surfaces(g), some_ob(g), px(g), py(g), fx, fy]
        }
        36..=50 => [29, ob(g), surfaces(g), px(g), py(g), px(g), py(g), radius(g)],
        51..=66 => {
            let k = ob(g);
            let o = sc.objects[k as usize];
            [30, k, surfaces(g), o.pos.x.to_bits(), o.pos.y.to_bits(), px(g), py(g), g.below(2)]
        }
        67..=82 => [31, ob(g), ob(g), px(g), py(g), px(g), py(g), surfaces(g) | (g.below(2) << 5)],
        83..=86 => [21, ob(g), surfaces(g), px(g), py(g), NONE, 0, 0],
        87..=89 => [27, some_ob(g), surfaces(g), px(g), py(g), px(g), py(g), g.below(32)],
        90..=92 => [18, [2u32, 3, 4][g.below(3) as usize], g.below(2), 0, 0, 0, 0, 0],
        93..=94 => [19, 0, 0, 0, 0, 0, 0, 0],
        95..=97 => [8, 0, 0, 0, 0, 0, 0, 0],
        _ => [9, 0, 0, 0, 0, 0, 0, 0],
    }
}

#[test]
fn purposeful_searches_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (the expected panics of `pathDestination` where upstream crashes are not printed; any other is)
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if !info.to_string().contains("upstream dereferences the cell's info unchecked") {
            hook(info);
        }
    }));
    let cases: usize = std::env::var("PF_P10_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(10_000);
    let mut g = Gen::new(0x0010_C3AA);
    let mut check = support::Check::new("P10 purposeful searches");
    let (mut ops_run, mut searches, mut found, mut broken) = (0u64, 0u64, 0u64, 0u64);
    let mut per_op = [0u64; 4];
    let mut found_per_op = [0u64; 4];
    let mut path_destination_crashes = 0u64;
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
            // (not from a bridge's cell to the same cell, after `clip`, as the hierarchical search finds them: an upstream hang, see
            // logic::ai::hierarchical)
            let hang = match o[0] {
                27 => run.same_bridge_cell(&o, 3, 5),
                28 => run.same_bridge_cell(&o, 6, 4),
                30 | 31 => run.same_bridge_cell(&o, 3, 5) || run.same_bridge_cell(&o, 5, 3),
                _ => false,
            };
            if hang {
                continue;
            }
            // (a Rust panic ends the program there: the ops before it are still compared, to show where the two first differ)
            if cpp_only {
                // (the program as generated, the C++ alone: the setup made the map, so every op is defined)
                ops.push(o);
                continue;
            }
            let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run.op(&o, &mut r))).is_ok();
            if !ok && o[0] == 24 {
                // (`pathDestination` reading a cell whose info the nested `checkPathCost` released: upstream crashes there, see
                // logic::ai::adjust; the case ends before it)
                path_destination_crashes += 1;
                break;
            }
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
            if (28..=31).contains(&ops[i][0]) {
                searches += 1;
                // (op 31: the avoided path's flag first)
                let k = if ops[i][0] == 31 { 2 } else { 1 };
                found += u64::from(a.get(k) == Some(&1));
                per_op[ops[i][0] as usize - 28] += 1;
                found_per_op[ops[i][0] as usize - 28] += u64::from(a.get(k) == Some(&1));
            }
            check.eq(a.clone(), b, || format!("case {case} op {i} {:?} ops {:?}", ops[i], &ops[..=i]));
        }
        assert!(!panicked, "case {case}: the Rust side panicked after op {} (the ops before it matched)", ops.len());
        ops_run += ops.len() as u64;
    }
    println!("P10: {cases} cases, {ops_run} ops ({searches} searches, {per_op:?} of ops 28-31, {found} paths found ({found_per_op:?}), {broken} broken bridges found; {path_destination_crashes} cases ended at an upstream crash in pathDestination)");
    check.finish();
}
