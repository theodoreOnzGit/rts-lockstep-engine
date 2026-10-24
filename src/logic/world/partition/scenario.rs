// SPDX-License-Identifier: GPL-3.0-only
// Translation-owned file (this repository's own design, not upstream code).

//! A table-driven [`PartitionHost`] / [`PartitionFilterHost`]: a world of
//! plain records the partition manager can be run against, with no `Object`,
//! `Player` or pathfinder behind it. It is test support: the unit tests of
//! this module and `tests/differential_partition.rs` use it, and the C++ side
//! of the differential test implements the same answers (differential/
//! wrap_partition.cpp, which says so object by object).
//!
//! Everything the manager asks that upstream answers from another system is a
//! deterministic function of the record and the arguments (a small integer
//! hash for the questions with no natural state, e.g. whether an action is
//! possible), so both languages compute the same answer without sharing code.
//!
//! The two callbacks react the way the real ones do in outline:
//! `onPartitionCellChange` unlooks the old position (queued, as
//! `Object::unlook` does) and looks at the new one; `onCollide` records the
//! collision and, for an object flagged to die on impact, destroys it and
//! unregisters it (the re-entrant case the contact list guards against).

use super::filters::allow_player;
use super::host::{
    CanAttackResult, ContainFacts, GhostGeometry, ObjectGeometry, PartitionFilterHost, PartitionHost, PathfindCellType, PathfindLayer, StealthFacts,
    LAYER_GROUND,
};
use super::manager::PartitionManager;
use crate::common::base_type::{Coord3D, Region3D};
use crate::common::game_common::{AbleToAttackType, CommandSourceType, PlayerMaskType, Relationship};
use crate::common::geometry::GeometryInfo;
use crate::common::kind_of::{KindOfMaskType, KindOfType};
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_DESTROYED};
use crate::logic::world::ids::{ObjectId, TeamId};
use std::collections::BTreeMap;

/// Flag bits of [`ScenarioObject::flags`].
pub mod sflag {
    /// `isEffectivelyDead()`.
    pub const DEAD: u32 = 1 << 0;
    /// `isOffMap()`.
    pub const OFF_MAP: u32 = 1 << 1;
    /// `isUsingAirborneLocomotor()`.
    pub const AIRBORNE: u32 = 1 << 2;
    /// `isStructure()`.
    pub const STRUCTURE: u32 = 1 << 3;
    /// `isNonFactionStructure()`.
    pub const NON_FACTION: u32 = 1 << 4;
    /// `isAbleToAttack()`.
    pub const CAN_ATTACK: u32 = 1 << 5;
    /// `isDisabledByType(DISABLED_UNMANNED)`.
    pub const UNMANNED: u32 = 1 << 6;
    /// The ghost manager makes a ghost for this object when it is immobile.
    pub const WANTS_GHOST: u32 = 1 << 7;
    /// `onCollide` destroys and unregisters this object.
    pub const KILL_ON_COLLIDE: u32 = 1 << 8;
    /// Member of squad 1.
    pub const SQUAD1: u32 = 1 << 9;
}

/// One object of the scenario.
#[derive(Clone, Debug)]
pub struct ScenarioObject {
    /// `getID()`.
    pub id: ObjectId,
    /// `getPosition()`.
    pub position: Coord3D,
    /// `getOrientation()`.
    pub orientation: f32,
    /// `getGeometryInfo()`.
    pub geometry: GeometryInfo,
    /// The template's kind mask.
    pub kind_of: KindOfMaskType,
    /// `getStatusBits()`.
    pub status: ObjectStatusMaskType,
    /// `getTeam()`.
    pub team: TeamId,
    /// `getControllingPlayer()->getPlayerIndex()`, `None` for no player.
    pub player: Option<i32>,
    /// [`sflag`] bits.
    pub flags: u32,
    /// `friend_getBuildCost()`.
    pub build_cost: i32,
    /// An opaque template key.
    pub template: i32,
    /// The container, if any.
    pub contain: Option<ContainFacts>,
    /// `getApparentControllingPlayer()` of the container.
    pub apparent_controller: Option<i32>,
    /// The `StealthUpdate`, if any.
    pub stealth: Option<StealthFacts>,
    /// `getUnitDirectionVector2D()` (x, y).
    pub dir: (f32, f32),
    /// How far the object sees (0 = does not look).
    pub vision: f32,
    /// Where it last looked (`Object::m_shroudClearingRange` bookkeeping).
    pub looking_at: Option<(f32, f32)>,
}

impl ScenarioObject {
    /// A plain object: no kind bits, no player, no flags.
    pub fn new(id: ObjectId, position: Coord3D, geometry: GeometryInfo) -> ScenarioObject {
        ScenarioObject {
            id,
            position,
            orientation: 0.0,
            geometry,
            kind_of: KindOfMaskType::new(),
            status: ObjectStatusMaskType::new(),
            team: 0,
            player: None,
            flags: 0,
            build_cost: 0,
            template: 0,
            contain: None,
            apparent_controller: None,
            stealth: None,
            dir: (1.0, 0.0),
            vision: 0.0,
            looking_at: None,
        }
    }

    fn flag(&self, f: u32) -> bool {
        self.flags & f != 0
    }
}

/// What the callbacks recorded, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum ScenarioLog {
    /// `onPartitionCellChange()` of the object.
    CellChange(ObjectId),
    /// `onCollide(other, loc, normal)` of `id`.
    Collide { id: ObjectId, other: ObjectId, loc: Coord3D, normal: Coord3D },
    /// A ghost was made for the object.
    Ghost(ObjectId),
}

/// The small integer hash behind the questions with no natural state.
pub fn mix(a: u32, b: u32, c: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B) ^ c.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h
}

/// The scenario: the globals, the players, the terrain functions and the objects.
#[derive(Clone, Debug)]
pub struct ScenarioHost {
    /// `getFrame()`.
    pub frame: u32,
    /// `m_partitionCellSize`.
    pub cell_size: f32,
    /// `m_unlookPersistDuration`.
    pub unlook_persist: u32,
    /// The terrain extent; `None` for no terrain.
    pub extent: Option<Region3D>,
    /// `getPlayerCount()`.
    pub player_count: i32,
    /// The local player.
    pub local_player: Option<i32>,
    /// Player to player relationship: `rel[a][b]` as `a` sees `b`.
    pub rel: [[Relationship; 16]; 16],
    /// The objects by id.
    pub objects: BTreeMap<ObjectId, ScenarioObject>,
    /// The callbacks' record.
    pub log: Vec<ScenarioLog>,
    /// The state of the random generator (`GameLogicRandomValueReal`).
    pub rng: u32,
    /// The handle the next ghost object gets (handles are 1, 2, ... in creation order).
    pub next_ghost: u32,
}

impl Default for ScenarioHost {
    fn default() -> Self {
        ScenarioHost {
            frame: 0,
            cell_size: 40.0,
            unlook_persist: 30,
            extent: Some(Region3D { lo: Coord3D { x: 0.0, y: 0.0, z: 0.0 }, hi: Coord3D { x: 400.0, y: 400.0, z: 100.0 } }),
            player_count: 4,
            local_player: Some(1),
            rel: [[Relationship::Neutral; 16]; 16],
            objects: BTreeMap::new(),
            log: Vec::new(),
            rng: 12345,
            next_ghost: 0,
        }
    }
}

impl ScenarioHost {
    /// `getGroundHeight(x, y)` of the scenario terrain: a ramp and a bump, in plain `float` operations.
    pub fn terrain_height(x: f32, y: f32) -> f32 {
        let a = x * 0.5;
        let b = y * 0.25;
        (a * a + b * b) * 0.001 - 3.0
    }

    fn obj(&self, id: ObjectId) -> Option<&ScenarioObject> {
        self.objects.get(&id)
    }

    fn player_of(&self, id: ObjectId) -> Option<i32> {
        self.obj(id).and_then(|o| o.player)
    }

    fn rel_players(&self, a: Option<i32>, b: Option<i32>) -> Relationship {
        match (a, b) {
            (Some(a), Some(b)) if (0..16).contains(&a) && (0..16).contains(&b) => self.rel[a as usize][b as usize],
            _ => Relationship::Neutral,
        }
    }

    /// Adds an object.
    pub fn add(&mut self, o: ScenarioObject) {
        self.objects.insert(o.id, o);
    }
}

impl PartitionHost for ScenarioHost {
    fn frame(&self) -> u32 {
        self.frame
    }
    fn partition_cell_size(&self) -> f32 {
        self.cell_size
    }
    fn unlook_persist_duration(&self) -> u32 {
        self.unlook_persist
    }

    fn terrain_extent(&self) -> Option<Region3D> {
        self.extent
    }
    fn ground_height(&self, x: f32, y: f32) -> f32 {
        ScenarioHost::terrain_height(x, y)
    }
    fn is_clear_line_of_sight(&self, pos: &Coord3D, pos_other: &Coord3D) -> bool {
        // sample the ground at 16 steps along the segment
        let mut i = 0;
        while i <= 16 {
            let t = i as f32 / 16.0;
            let x = pos.x + t * (pos_other.x - pos.x);
            let y = pos.y + t * (pos_other.y - pos.y);
            let z = pos.z + t * (pos_other.z - pos.z);
            if ScenarioHost::terrain_height(x, y) > z + 0.5 {
                return false;
            }
            i += 1;
        }
        true
    }

    fn player_count(&self) -> i32 {
        self.player_count
    }
    fn player_mask(&self, index: i32) -> Option<PlayerMaskType> {
        if (0..16).contains(&index) {
            Some(1u16 << index)
        } else {
            None
        }
    }
    fn local_player_index(&self) -> Option<i32> {
        self.local_player
    }
    fn players_with_relationship(&self, player_index: i32, which_player_types: u32) -> PlayerMaskType {
        let mut mask: PlayerMaskType = 0;
        if which_player_types == 0 || !(0..16).contains(&player_index) {
            return mask;
        }
        // PlayerList::getPlayersWithRelationship
        if which_player_types & allow_player::SAME_PLAYER != 0 {
            mask |= 1u16 << player_index;
        }
        for p in 0..self.player_count.clamp(0, 16) {
            if p == player_index {
                continue;
            }
            let flag = match self.rel[player_index as usize][p as usize] {
                Relationship::Enemies => allow_player::ENEMIES,
                Relationship::Allies => allow_player::ALLIES,
                Relationship::Neutral => allow_player::NEUTRAL,
            };
            if which_player_types & flag != 0 {
                mask |= 1u16 << p;
            }
        }
        mask
    }
    fn player_relationship_to_object_team(&self, player_index: i32, object: ObjectId) -> Relationship {
        self.rel_players(Some(player_index), self.player_of(object))
    }

    fn object_geometry(&self, id: ObjectId) -> Option<ObjectGeometry> {
        self.obj(id).map(|o| ObjectGeometry { position: o.position, orientation: o.orientation, geometry: o.geometry.clone() })
    }
    fn is_kind_of(&self, id: ObjectId, t: KindOfType) -> bool {
        self.obj(id).is_some_and(|o| o.kind_of.test(t))
    }
    fn status_bits(&self, id: ObjectId) -> ObjectStatusMaskType {
        self.obj(id).map_or_else(ObjectStatusMaskType::new, |o| o.status)
    }
    fn is_destroyed(&self, id: ObjectId) -> bool {
        self.obj(id).is_none_or(|o| o.status.test(OBJECT_STATUS_DESTROYED))
    }
    fn unit_direction_vector_2d(&self, id: ObjectId) -> Coord3D {
        let d = self.obj(id).map_or((1.0, 0.0), |o| o.dir);
        Coord3D { x: d.0, y: d.1, z: 0.0 }
    }

    fn add_ghost_object(&mut self, id: ObjectId) -> Option<GhostGeometry> {
        let o = self.objects.get(&id)?;
        if !o.flag(sflag::WANTS_GHOST) {
            return None;
        }
        let (position, angle, geometry) = (o.position, o.orientation, o.geometry.clone());
        self.next_ghost += 1;
        self.log.push(ScenarioLog::Ghost(id));
        Some(GhostGeometry { handle: self.next_ghost, position, angle, geometry })
    }

    fn on_partition_cell_change(&mut self, pm: &mut PartitionManager, id: ObjectId) {
        self.log.push(ScenarioLog::CellChange(id));
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let (vision, pos, old, player) = (o.vision, o.position, o.looking_at, o.player);
        if vision > 0.0 {
            if let Some(mask) = player.and_then(|p| self.player_mask(p)) {
                if let Some((ox, oy)) = old {
                    // Object::unlook(): the undo is queued
                    pm.queue_undo_shroud_reveal(&*self, ox, oy, vision, mask);
                }
                // Object::look()
                let _ = pm.do_shroud_reveal(&*self, pos.x, pos.y, vision, mask);
                if let Some(o) = self.objects.get_mut(&id) {
                    o.looking_at = Some((pos.x, pos.y));
                }
            }
        }
    }

    fn on_collide(&mut self, pm: &mut PartitionManager, id: ObjectId, other: ObjectId, loc: &Coord3D, normal: &Coord3D) {
        self.log.push(ScenarioLog::Collide { id, other, loc: *loc, normal: *normal });
        if self.obj(id).is_some_and(|o| o.flag(sflag::KILL_ON_COLLIDE)) {
            if let Some(o) = self.objects.get_mut(&id) {
                o.status.set(OBJECT_STATUS_DESTROYED, 1);
            }
            pm.unregister_object(id);
        }
    }

    fn game_logic_random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        lo + (hi - lo) * ((self.rng >> 8) as f32 / 16_777_216.0)
    }

    fn maximum_pathfind_extent(&self) -> Region3D {
        match self.extent {
            Some(e) => Region3D { lo: Coord3D { x: e.lo.x + 10.0, y: e.lo.y + 10.0, z: e.lo.z }, hi: Coord3D { x: e.hi.x - 10.0, y: e.hi.y - 10.0, z: e.hi.z } },
            None => Region3D::default(),
        }
    }
    fn highest_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayer {
        1 + (mix(pos.x as i32 as u32, pos.y as i32 as u32, 11) % 3) as i32
    }
    fn layer_height(&self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        ScenarioHost::terrain_height(x, y) + layer as f32 * 0.5
    }
    fn is_cliff_cell(&self, x: f32, y: f32) -> bool {
        mix(x as i32 as u32, y as i32 as u32, 12).is_multiple_of(11)
    }
    fn is_underwater(&self, x: f32, y: f32) -> bool {
        ScenarioHost::terrain_height(x, y) < -2.0
    }
    fn pathfind_cell_type(&self, layer: PathfindLayer, cell_x: i32, cell_y: i32) -> Option<PathfindCellType> {
        if cell_x < 0 || cell_y < 0 {
            return None;
        }
        match mix(cell_x as u32, cell_y as u32, 13 + layer as u32) % 9 {
            0 => Some(PathfindCellType::Impassable),
            1 | 2 => Some(PathfindCellType::Other),
            _ => Some(PathfindCellType::Clear),
        }
    }
    fn quick_does_path_exist(&self, source: ObjectId, to: &Coord3D) -> Option<bool> {
        if source % 5 == 0 {
            return None; // no AI update
        }
        Some(!mix(source as u32, to.x as i32 as u32, to.y as i32 as u32).is_multiple_of(4))
    }
    fn relationship(&self, obj: ObjectId, other: ObjectId) -> Relationship {
        self.rel_players(self.player_of(obj), self.player_of(other))
    }
}

impl PartitionFilterHost for ScenarioHost {
    fn is_using_airborne_locomotor(&self, id: ObjectId) -> bool {
        self.obj(id).is_some_and(|o| o.flag(sflag::AIRBORNE))
    }
    fn controlling_player(&self, id: ObjectId) -> Option<i32> {
        self.player_of(id)
    }
    fn team(&self, id: ObjectId) -> TeamId {
        self.obj(id).map_or(0, |o| o.team)
    }
    fn is_on_squad(&self, squad: i32, id: ObjectId) -> bool {
        squad == 1 && self.obj(id).is_some_and(|o| o.flag(sflag::SQUAD1))
    }
    fn is_effectively_dead(&self, id: ObjectId) -> bool {
        self.obj(id).is_some_and(|o| o.flag(sflag::DEAD))
    }
    fn is_off_map(&self, id: ObjectId) -> bool {
        self.obj(id).is_some_and(|o| o.flag(sflag::OFF_MAP))
    }
    fn is_view_blocked_by_obstacle(&self, obj: ObjectId, other: ObjectId) -> bool {
        mix(obj as u32, other as u32, 21).is_multiple_of(5)
    }
    fn able_to_attack_specific_object(&self, obj: ObjectId, attack_type: AbleToAttackType, other: ObjectId, source: CommandSourceType) -> CanAttackResult {
        match mix(obj as u32, other as u32, 22 + attack_type as u32 * 7 + source as u32) % 4 {
            0 => CanAttackResult::Possible,
            1 => CanAttackResult::PossibleAfterMoving,
            2 => CanAttackResult::NotPossible,
            _ => CanAttackResult::OtherRefusal,
        }
    }
    fn can_enter_object(&self, obj: ObjectId, other: ObjectId, source: CommandSourceType) -> bool {
        mix(obj as u32, other as u32, 23 + source as u32).is_multiple_of(3)
    }
    fn can_hijack_vehicle(&self, obj: ObjectId, other: ObjectId, source: CommandSourceType) -> bool {
        mix(obj as u32, other as u32, 24 + source as u32).is_multiple_of(3)
    }
    fn last_damage_source(&self, id: ObjectId) -> Option<ObjectId> {
        if id % 4 == 0 {
            None
        } else {
            Some(1 + (mix(id as u32, 0, 25) % 40) as i32)
        }
    }
    fn kind_of_mask(&self, id: ObjectId) -> KindOfMaskType {
        self.obj(id).map_or_else(KindOfMaskType::new, |o| o.kind_of)
    }
    fn transform_x_vector(&self, id: ObjectId) -> Coord3D {
        // the matrix' X vector: (cos a, sin a, 0.25) for the object's direction
        let d = self.obj(id).map_or((1.0, 0.0), |o| o.dir);
        Coord3D { x: d.0, y: d.1, z: 0.25 }
    }
    fn is_structure(&self, id: ObjectId) -> bool {
        self.obj(id).is_some_and(|o| o.flag(sflag::STRUCTURE))
    }
    fn is_non_faction_structure(&self, id: ObjectId) -> bool {
        self.obj(id).is_some_and(|o| o.flag(sflag::NON_FACTION))
    }
    fn is_able_to_attack(&self, id: ObjectId) -> bool {
        self.obj(id).is_some_and(|o| o.flag(sflag::CAN_ATTACK))
    }
    fn contain_facts(&self, id: ObjectId) -> Option<ContainFacts> {
        self.obj(id).and_then(|o| o.contain)
    }
    fn apparent_controlling_player(&self, id: ObjectId, observer_player: Option<i32>) -> Option<i32> {
        let o = self.obj(id)?;
        o.contain?;
        // depends on the observer a little, as the real one does for disguised containers
        match (o.apparent_controller, observer_player) {
            (Some(p), Some(obs)) if p == obs => Some(p),
            (Some(p), _) => Some(p),
            (None, _) => None,
        }
    }
    fn player_relationship_to_player(&self, a: i32, b: i32) -> Relationship {
        self.rel_players(Some(a), Some(b))
    }
    fn controlling_player_is_computer(&self, id: ObjectId) -> bool {
        // the odd-numbered players are computer players
        self.player_of(id).is_some_and(|p| p & 1 == 1)
    }
    fn stealth_facts(&self, id: ObjectId) -> Option<StealthFacts> {
        self.obj(id).and_then(|o| o.stealth)
    }
    fn team_relationship_to_player(&self, obj: ObjectId, player: i32) -> Relationship {
        self.rel_players(self.player_of(obj), Some(player))
    }
    fn template_is_equivalent_to(&self, template_key: i32, other: ObjectId) -> bool {
        self.obj(other).is_some_and(|o| o.template == template_key)
    }
    fn can_player_garrison(&self, player: Option<i32>, other: ObjectId, source: CommandSourceType) -> bool {
        mix(player.unwrap_or(-1) as u32, other as u32, 26 + source as u32).is_multiple_of(2)
    }
    fn is_disabled_unmanned(&self, id: ObjectId) -> bool {
        self.obj(id).is_some_and(|o| o.flag(sflag::UNMANNED))
    }
    fn command_button_valid_target(&self, button: i32, source: ObjectId, other: ObjectId, command_source: CommandSourceType) -> bool {
        mix(button as u32, source as u32 ^ (other as u32).rotate_left(9), 27 + command_source as u32).is_multiple_of(2)
    }
    fn point_in_trigger(&self, trigger: i32, x: i32, y: i32) -> bool {
        // the trigger is the disc of radius 40 + 10*trigger around (150, 150)
        let dx = i64::from(x) - 150;
        let dy = i64::from(y) - 150;
        let r = 40 + 10 * i64::from(trigger);
        dx * dx + dy * dy <= r * r
    }
    fn build_cost(&self, id: ObjectId) -> i32 {
        self.obj(id).map_or(0, |o| o.build_cost)
    }
}

/// The `LAYER_GROUND` the scenario's pathfinder numbers its ground layer with.
pub const SCENARIO_GROUND_LAYER: PathfindLayer = LAYER_GROUND;
