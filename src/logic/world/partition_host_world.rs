// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code):
// the world's answers to what `PartitionManager.cpp` asks of the rest of the
// game.

//! [`PartitionHost`] and [`PartitionFilterHost`] for the [`World`]: the
//! partition manager (`world.partition`) run against the world's own objects,
//! players and terrain, instead of the table-driven
//! [`ScenarioHost`](crate::logic::world::partition::scenario::ScenarioHost)
//! its unit and differential tests use.
//!
//! # How the manager is reached
//!
//! The manager is a field of the world and its methods take the host as an
//! argument, so a call checks the manager out ([`World::with_partition`]):
//! `std::mem::take` leaves an empty manager in the field for the duration of
//! the call. The two callbacks upstream makes from inside the manager
//! (`onPartitionCellChange`, `onCollide`) reach back into
//! `ThePartitionManager`; [`PartitionHost::on_partition_cell_change`] and
//! [`PartitionHost::on_collide`] therefore swap the checked-out manager back
//! into the field while the world's own code runs, and out again after.
//!
//! # Live and recorded
//!
//! Every place the world called the partition manager before this batch is a
//! cut point ([`CutCall`]`::Partition*`) answered from the tables of
//! [`ObjectCuts`](crate::logic::world::object_cuts::ObjectCuts), which is
//! how the differential tests of the object and module code see the calls.
//! With `cuts.live_partition` set, [`World::cut`] also runs each of those
//! calls on the real manager ([`World::live_partition_call`]): registration,
//! dirtying, the shroud, the threat and value maps, and the range scans,
//! whose answer replaces `cuts.range_scan` before the caller reads it. It is
//! off by default because the existing tests answer from their tables; the
//! game loop (layer 12) turns it on.
//!
//! # What still answers from a table
//!
//! Upstream asks three systems the world does not hold yet; each such answer
//! names the table that stands in for it.
//!
//! - **The pathfinder** (`TheAI->pathfinder()`, layer 11): the cell types and
//!   the quick path check of `findPositionAround`, and
//!   `isViewBlockedByObstacle`. `cuts.pathfind_cells`, `cuts.partition_quick_path`
//!   and `cuts.view_blocked`; the wall height is the existing ~~`cuts.wall_height`~~ (Corrected 2026-11-03 (batch I48): [`World::wall_height`])
//!   and no point is on a wall. **Corrected 2026-11-02 (batch I42):** the world
//!   holds the pathfinder since batch I9a; with `live_pathfinder` and a map the
//!   cell types (since I9a), the quick path check and `isViewBlockedByObstacle`
//!   (since I42) are its answers, and the tables answer only without it.
//! - **The terrain render object** (`TheTerrainRenderObject`), which answers
//!   `isClearLineOfSight` and `isCliffCell` for a height-map terrain: the
//!   existing `cuts.blocked_lines_of_sight` and `cuts.terrain_cliff`.
//! - **Hot-key squads by number**: `PartitionFilterAcceptOnSquad` holds a
//!   `Squad*`; here a squad is `player * NUM_HOTKEY_SQUADS + hotkey` into the
//!   players' own `squads` (no translated caller builds the filter yet).
//!
//! Ghost objects: `TheGhostObjectManager` is the client's; the logic-only
//! (headless) build uses the base `GhostObjectManager`, whose
//! `addGhostObject` returns `0` (GhostObject.cpp), so no ghost is made.

use crate::common::base_type::{Coord3D, ICoord3D, Region3D};
use crate::common::disabled_types::DISABLED_UNMANNED;
use crate::common::game_common::{AbleToAttackType, CommandSourceType, PlayerMaskType, PlayerType, Relationship};
use crate::common::kind_of::{KindOfMaskType, KindOfType, KINDOF_CLEANUP_HAZARD, KINDOF_STRUCTURE, KINDOF_VEHICLE};
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_DETECTED};
use crate::logic::action_manager::{self, CanEnterType};
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::{ObjectId, PlayerIndex, TeamId};
use crate::logic::world::module_call::{ModuleCall, ModuleReply};
use crate::logic::world::object_cuts::{terrain_cell, CutCall};
use crate::logic::world::object_transform::layer_height;
use crate::logic::world::object_weapons;
use crate::logic::world::partition::{
    CanAttackResult, ContainFacts, GhostGeometry, ObjectGeometry, PartitionFilter, PartitionFilterHost, PartitionHost, PartitionManager, PathfindCellType,
    PathfindLayer, StealthFacts,
};
use crate::logic::world::partition::filters::ALLOW_ALLIES;
use crate::logic::world::player_squads::squad_is_on_squad;
use crate::logic::weapon_host::CanAttackResult as WeaponCanAttackResult;
use crate::logic::object_iter::{IterOrderType, SimpleObjectIterator};

/// `NUM_HOTKEY_SQUADS` (`Player.h`): the squads each player has, for the squad numbering of the module docs.
pub const NUM_HOTKEY_SQUADS: i32 = 10;

impl World {
    /// Runs `f` with the partition manager checked out of the world (see the module docs).
    pub fn with_partition<R>(&mut self, f: impl FnOnce(&mut PartitionManager, &mut World) -> R) -> R {
        let mut pm = std::mem::take(&mut self.partition);
        let r = f(&mut pm, self);
        self.partition = pm;
        r
    }

    /// Runs `f` with `pm` put back into the world's field (a callback from inside a checked-out call), then checks it out again.
    fn with_partition_back(&mut self, pm: &mut PartitionManager, f: impl FnOnce(&mut World)) {
        std::mem::swap(&mut self.partition, pm);
        f(self);
        std::mem::swap(&mut self.partition, pm);
    }

    /// `ThePartitionManager->init()`: the grid over the terrain's extent, with the global data's cell size.
    pub fn partition_init(&mut self) {
        self.with_partition(|pm, w| pm.init(w));
    }

    /// `ThePartitionManager->update()`: the dirty objects' cells, the collisions, the queued shroud undos.
    pub fn partition_update(&mut self) {
        self.with_partition(|pm, w| pm.update(w));
    }

    /// The range scans of [`CutCall`]`::Partition*` as the real manager answers them: the objects, in the order the manager
    /// iterates them; each one's numeric (its squared distance) goes to `cuts.range_scan_numeric`, for the callers that walk
    /// with `firstWithNumeric`.
    pub(crate) fn live_scan(&mut self, f: impl FnOnce(&mut PartitionManager, &mut World) -> SimpleObjectIterator<ObjectId>) -> Vec<ObjectId> {
        let mut it = self.with_partition(f);
        let (mut out, mut numerics) = (Vec::new(), Vec::new());
        let (mut o, mut n) = it.first_with_numeric();
        while let Some(id) = o {
            out.push(id);
            numerics.push(n);
            (o, n) = it.next_with_numeric();
        }
        self.cuts.range_scan_numeric = numerics;
        out
    }

    /// `MAKE_KINDOF_MASK(k)` (integration batch I52).
    fn kinds(k: KindOfType) -> KindOfMaskType {
        let mut m = KindOfMaskType::new();
        m.set(k, 1);
        m
    }

    /// The filter `PartitionFilterSameMapStatus(obj)`, `PartitionFilterAlive`.
    fn alive_same_map(object: ObjectId) -> [PartitionFilter; 2] {
        [PartitionFilter::Alive, PartitionFilter::SameMapStatus { obj: object }]
    }

    /// With `cuts.live_partition`, runs a `Partition*` cut call on the real manager (see the module docs). Calls of the
    /// other systems, and the partition calls whose answer the caller computes from its own table (the distances, the
    /// closest-object answers of `cuts.closest_object_at`), are left to their tables.
    pub(crate) fn live_partition_call(&mut self, call: &CutCall) {
        use crate::logic::world::partition::DistanceCalculationType as Dc;
        let dc_of = |dc: i32| Dc::from_index(dc).unwrap_or(Dc::FromCenter2D);
        let scan: Option<Vec<ObjectId>> = match *call {
            CutCall::PartitionRegisterObject { object } => {
                self.with_partition(|pm, w| pm.register_object(w, object));
                None
            }
            CutCall::PartitionUnregisterObject { object } => {
                self.partition.unregister_object(object);
                None
            }
            CutCall::PartitionMakeDirty { object } => {
                self.partition.make_object_dirty(object, true);
                None
            }
            CutCall::PartitionDoShroudReveal { x, y, range, mask } => {
                self.with_partition(|pm, w| pm.do_shroud_reveal(w, x, y, range, mask)).ok();
                None
            }
            CutCall::PartitionUndoShroudReveal { x, y, range, mask } => {
                self.with_partition(|pm, w| pm.undo_shroud_reveal(w, x, y, range, mask)).ok();
                None
            }
            CutCall::PartitionQueueUndoShroudReveal { x, y, range, mask } => {
                self.with_partition(|pm, w| pm.queue_undo_shroud_reveal(w, x, y, range, mask));
                None
            }
            CutCall::PartitionDoShroudCover { x, y, range, mask } => {
                self.with_partition(|pm, w| pm.do_shroud_cover(w, x, y, range, mask)).ok();
                None
            }
            CutCall::PartitionUndoShroudCover { x, y, range, mask } => {
                self.with_partition(|pm, w| pm.undo_shroud_cover(w, x, y, range, mask)).ok();
                None
            }
            CutCall::PartitionDoValueAffect { x, y, how_far, data, for_whom } => {
                self.with_partition(|pm, w| pm.do_value_affect(w, x, y, how_far, data, for_whom)).ok();
                None
            }
            CutCall::PartitionUndoValueAffect { x, y, how_far, data, for_whom } => {
                self.with_partition(|pm, w| pm.undo_value_affect(w, x, y, how_far, data, for_whom)).ok();
                None
            }
            CutCall::PartitionDoThreatAffect { x, y, how_far, data, for_whom } => {
                self.with_partition(|pm, w| pm.do_threat_affect(w, x, y, how_far, data, for_whom)).ok();
                None
            }
            CutCall::PartitionUndoThreatAffect { x, y, how_far, data, for_whom } => {
                self.with_partition(|pm, w| pm.undo_threat_affect(w, x, y, how_far, data, for_whom)).ok();
                None
            }
            CutCall::PartitionRevealMapForPlayer { player } => {
                self.with_partition(|pm, w| pm.reveal_map_for_player(w, player));
                None
            }
            CutCall::PartitionIterateObjectsInRange { object, range } => {
                Some(self.live_scan(|pm, w| pm.iterate_objects_in_range(w, object, range, Dc::FromCenter2D, &[], IterOrderType::Fastest)))
            }
            CutCall::PartitionIterateObjectsInRangeOf { object, range, dc } => {
                Some(self.live_scan(|pm, w| pm.iterate_objects_in_range(w, object, range, dc_of(dc), &[], IterOrderType::Fastest)))
            }
            CutCall::PartitionIterateObjectsInRangeAt { position, range, dc } => {
                Some(self.live_scan(|pm, w| pm.iterate_objects_in_range_at(w, &position, range, dc_of(dc), &[], IterOrderType::Fastest)))
            }
            CutCall::PartitionIterateStructuresInRange { object, range } => {
                let mut kinds = KindOfMaskType::new();
                kinds.set(KINDOF_STRUCTURE, 1);
                let f = [PartitionFilter::AcceptByKindOf { must_be_set: kinds, must_be_clear: KindOfMaskType::new() }];
                Some(self.live_scan(|pm, w| pm.iterate_objects_in_range(w, object, range, Dc::FromBoundingSphere3D, &f, IterOrderType::Fastest)))
            }
            CutCall::PartitionIterateSamePlayerInRange { object, range } => {
                let f = [PartitionFilter::SamePlayer { player: self.get_controlling_player(object) }, PartitionFilter::SameMapStatus { obj: object }];
                Some(self.live_scan(|pm, w| pm.iterate_objects_in_range(w, object, range, Dc::FromCenter2D, &f, IterOrderType::Fastest)))
            }
            CutCall::PartitionIterateAlliesInRange { object, range } => {
                let position = self.objects.find(object).map(|o| o.get_position()).unwrap_or_default();
                let f = [
                    PartitionFilter::Relationship { obj: object, flags: ALLOW_ALLIES },
                    PartitionFilter::Alive,
                    PartitionFilter::SameMapStatus { obj: object },
                ];
                Some(self.live_scan(|pm, w| pm.iterate_objects_in_range_at(w, &position, range, Dc::FromCenter2D, &f, IterOrderType::Fastest)))
            }
            CutCall::PartitionIterateAliveSameMapInRangeAt { object, position, range, dc } => {
                let f = Self::alive_same_map(object);
                Some(self.live_scan(|pm, w| pm.iterate_objects_in_range_at(w, &position, range, dc_of(dc), &f, IterOrderType::Fastest)))
            }
            CutCall::PartitionIteratePotentialCollisions { position, geom_type, major_radius, minor_radius, angle } => {
                let Some(t) = crate::common::geometry::GeometryType::from_index(geom_type) else { return };
                let geom = crate::common::geometry::GeometryInfo::new(t, false, 0.0, major_radius, minor_radius);
                Some(self.live_scan(|pm, w| pm.iterate_potential_collisions(w, &position, &geom, angle, false)))
            }
            CutCall::PartitionGetClosestObjectAt { position, range, dc } => {
                let v = self.live_scan(|pm, w| pm.iterate_objects_in_range_at(w, &position, range, dc_of(dc), &[], IterOrderType::SortedNearToFar));
                let closest = self.with_partition(|pm, w| pm.get_closest_object_at(w, &position, range, dc_of(dc), &[]).object);
                self.cuts.closest_object_at = closest.unwrap_or(crate::logic::world::ids::INVALID_ID);
                Some(v)
            }
            CutCall::PartitionGetClosestObject { object, range } => {
                Some(self.live_scan(|pm, w| pm.iterate_objects_in_range(w, object, range, Dc::FromCenter3D, &[], IterOrderType::SortedNearToFar)))
            }
            // --- p1w13-sonnet --- (integration batch I52: the scans that had no live answer)
            CutCall::PartitionIterateHuntTargets { object, range, relationship, team_filter } => {
                // `CommandButtonHuntUpdate::scanClosestTarget`: alive, same map status, stealthed and undetected (FALSE), the relationship
                // unless the building capture clears it; from the object's position, FROM_CENTER_2D, ITER_SORTED_NEAR_TO_FAR.
                let position = self.objects.find(object).map(|o| o.get_position()).unwrap_or_default();
                let mut f = vec![
                    PartitionFilter::Alive,
                    PartitionFilter::SameMapStatus { obj: object },
                    PartitionFilter::StealthedAndUndetected { obj: object, allow: false },
                ];
                if team_filter {
                    f.push(PartitionFilter::Relationship { obj: object, flags: relationship });
                }
                Some(self.live_scan(|pm, w| pm.iterate_objects_in_range_at(w, &position, range, Dc::FromCenter2D, &f, IterOrderType::SortedNearToFar)))
            }
            CutCall::PartitionIterateVehiclesNearToFar { object, range } => {
                // `PilotFindVehicleUpdate::scanClosestTarget`: VEHICLE, alive, `PartitionFilterPlayer(me's player, true)`, same map status.
                let position = self.objects.find(object).map(|o| o.get_position()).unwrap_or_default();
                let f = [
                    PartitionFilter::AcceptByKindOf { must_be_set: Self::kinds(KINDOF_VEHICLE), must_be_clear: KindOfMaskType::new() },
                    PartitionFilter::Alive,
                    PartitionFilter::Player { player: self.get_controlling_player(object), match_: true },
                    PartitionFilter::SameMapStatus { obj: object },
                ];
                Some(self.live_scan(|pm, w| pm.iterate_objects_in_range_at(w, &position, range, Dc::FromCenter2D, &f, IterOrderType::SortedNearToFar)))
            }
            CutCall::PartitionIterateLineOfSightNearToFar { object, range } => {
                // `SpecialAbilityUpdate::isWithinStartAbilityRange`: `PartitionFilterLineOfSight(self)`, FROM_BOUNDINGSPHERE_2D.
                let f = [PartitionFilter::LineOfSight { obj: object }];
                Some(self.live_scan(|pm, w| pm.iterate_objects_in_range(w, object, range, Dc::FromBoundingSphere2D, &f, IterOrderType::SortedNearToFar)))
            }
            CutCall::PartitionIterateAlliesAliveSameMapNotStructuresAt { object, position, range } => {
                // `PropagandaTowerBehavior::doScan`: allies, alive, same map status, not structures; FROM_CENTER_2D, the fastest order.
                let f = [
                    PartitionFilter::Relationship { obj: object, flags: ALLOW_ALLIES },
                    PartitionFilter::Alive,
                    PartitionFilter::SameMapStatus { obj: object },
                    PartitionFilter::AcceptByKindOf { must_be_set: KindOfMaskType::new(), must_be_clear: Self::kinds(KINDOF_STRUCTURE) },
                ];
                Some(self.live_scan(|pm, w| pm.iterate_objects_in_range_at(w, &position, range, Dc::FromCenter2D, &f, IterOrderType::Fastest)))
            }
            CutCall::PartitionIterateSpectreTargets { object, position, range, player_index } => {
                // `SpectreGunshipUpdate::update`: stealthed and undetected (false), possible to attack (ATTACK_NEW_TARGET, CMD_FROM_AI), free of
                // fog to the gunship's player; FROM_BOUNDINGSPHERE_2D, near to far. The first filter, the live map enemies, is the caller's
                // (`live_map_enemy`); the filters only accept or reject, so applying it after the others changes nothing.
                let f = [
                    PartitionFilter::StealthedAndUndetected { obj: object, allow: false },
                    PartitionFilter::PossibleToAttack { attack_type: crate::common::game_common::ATTACK_NEW_TARGET, obj: Some(object), command_source: CommandSourceType::FromAi },
                    PartitionFilter::FreeOfFog { comparison_index: player_index },
                ];
                Some(self.live_scan(|pm, w| pm.iterate_objects_in_range_at(w, &position, range, Dc::FromBoundingSphere2D, &f, IterOrderType::SortedNearToFar)))
            }
            CutCall::PartitionGetClosestCleanupHazardAt { object, position, range } => {
                // `CleanupHazardUpdate::scanClosestTarget`: CLEANUP_HAZARD, same map status; FROM_CENTER_2D.
                let f = [
                    PartitionFilter::AcceptByKindOf { must_be_set: Self::kinds(KINDOF_CLEANUP_HAZARD), must_be_clear: KindOfMaskType::new() },
                    PartitionFilter::SameMapStatus { obj: object },
                ];
                let closest = self.with_partition(|pm, w| pm.get_closest_object_at(w, &position, range, Dc::FromCenter2D, &f).object);
                self.cuts.closest_object_at = closest.unwrap_or(crate::logic::world::ids::INVALID_ID);
                None
            }
            // --- end p1w13-sonnet ---
            _ => None,
        };
        if let Some(found) = scan {
            self.cuts.range_scan = found;
        }
    }

    /// The player a squad number names, and the hot key (see the module docs).
    fn squad_slot(squad: i32) -> (PlayerIndex, usize) {
        (squad.div_euclid(NUM_HOTKEY_SQUADS), squad.rem_euclid(NUM_HOTKEY_SQUADS) as usize)
    }
}

impl PartitionHost for World {
    fn frame(&self) -> u32 {
        World::frame(self)
    }
    fn partition_cell_size(&self) -> f32 {
        self.statics.global_data.the_global_data().map_or(0.0, |g| g.partition_cell_size)
    }
    fn unlook_persist_duration(&self) -> u32 {
        self.statics.global_data.the_global_data().map_or(0, |g| g.unlook_persist_duration)
    }

    fn terrain_extent(&self) -> Option<Region3D> {
        self.terrain.extent().ok()
    }
    fn ground_height(&self, x: f32, y: f32) -> f32 {
        self.terrain.ground_height(x, y, false).0
    }
    fn is_clear_line_of_sight(&self, pos: &Coord3D, pos_other: &Coord3D) -> bool {
        match self.terrain.is_clear_line_of_sight(pos, pos_other) {
            Ok(clear) => clear,
            // the render object's answer (see the module docs)
            Err(_) => {
                let b = |c: &Coord3D| [c.x.to_bits(), c.y.to_bits(), c.z.to_bits()];
                !self.cuts.blocked_lines_of_sight.contains(&(b(pos), b(pos_other)))
            }
        }
    }

    fn player_count(&self) -> i32 {
        self.players.player_count()
    }
    fn player_mask(&self, index: i32) -> Option<PlayerMaskType> {
        self.players.get_nth(index).map(|_| self.tq().player_mask(index))
    }
    fn local_player_index(&self) -> Option<i32> {
        self.players.local_index()
    }
    fn players_with_relationship(&self, player_index: i32, which_player_types: u32) -> PlayerMaskType {
        // (batch I42: the player list's with `live_teams`; it was always the table)
        self.players_with_relationship_live(player_index, which_player_types)
    }
    fn player_relationship_to_object_team(&self, player_index: i32, object: ObjectId) -> Relationship {
        self.tq().player_relationship_to_team(player_index, self.get_team(object))
    }

    fn object_geometry(&self, id: ObjectId) -> Option<ObjectGeometry> {
        self.objects.find(id).map(|o| ObjectGeometry { position: o.get_position(), orientation: o.thing.orientation(), geometry: o.geometry_info.clone() })
    }
    fn is_kind_of(&self, id: ObjectId, t: KindOfType) -> bool {
        self.objects.find(id).is_some_and(|o| o.is_kind_of(t))
    }
    fn status_bits(&self, id: ObjectId) -> ObjectStatusMaskType {
        self.get_status_bits(id)
    }
    fn is_destroyed(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_none_or(|o| o.is_destroyed())
    }
    fn unit_direction_vector_2d(&self, id: ObjectId) -> Coord3D {
        // `Thing::getUnitDirectionVector2D`'s value (its cache is filled from the same expression)
        let angle = self.objects.find(id).map_or(0.0, |o| o.thing.orientation());
        Coord3D { x: crate::common::trig::cos(angle), y: crate::common::trig::sin(angle), z: 0.0 }
    }

    fn add_ghost_object(&mut self, _id: ObjectId) -> Option<GhostGeometry> {
        None // the base `GhostObjectManager::addGhostObject` (see the module docs)
    }

    fn on_partition_cell_change(&mut self, pm: &mut PartitionManager, id: ObjectId) {
        self.with_partition_back(pm, |w| w.on_partition_cell_change(id));
    }
    fn on_collide(&mut self, pm: &mut PartitionManager, id: ObjectId, other: ObjectId, loc: &Coord3D, normal: &Coord3D) {
        let (loc, normal) = (*loc, *normal);
        self.with_partition_back(pm, |w| w.on_collide(id, other, Some(loc), Some(normal)));
    }

    fn game_logic_random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.random.game_logic_random_value_real(lo, hi)
    }

    fn maximum_pathfind_extent(&self) -> Region3D {
        self.terrain.maximum_pathfind_extent().unwrap_or_default()
    }
    fn highest_layer_for_destination(&mut self, pos: &Coord3D) -> PathfindLayer {
        // the pathfinder's walls: its height from the table, and no point on a wall (see the module docs)
        let wall_height = self.wall_height();
        self.terrain.highest_layer_for_destination_with(|| wall_height, |_| false, pos, false).unwrap_or(crate::logic::world::partition::LAYER_GROUND)
    }
    fn layer_height(&mut self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        layer_height(&self.terrain, self.wall_height(), &self.pf_wall, x, y, layer)
    }
    fn is_cliff_cell(&mut self, x: f32, y: f32) -> bool {
        match self.terrain.is_cliff_cell(x, y) {
            Ok(cliff) => cliff,
            Err(_) => self.cuts.terrain_cliff[terrain_cell(x, y)],
        }
    }
    fn is_underwater(&mut self, x: f32, y: f32) -> bool {
        self.terrain.is_underwater(x, y).is_ok_and(|u| u.underwater)
    }
    fn pathfind_cell_type(&mut self, layer: PathfindLayer, cell_x: i32, cell_y: i32) -> Option<PathfindCellType> {
        // (batch I9a: the world's pathfinder with `live_pathfinder`)
        if let Some(cell) = World::pathfinder_cell_type(self, layer, cell_x, cell_y) {
            return cell.map(|t| match t as u8 {
                crate::logic::ai::cell::CELL_CLEAR => PathfindCellType::Clear,
                crate::logic::ai::cell::CELL_IMPASSABLE => PathfindCellType::Impassable,
                _ => PathfindCellType::Other,
            });
        }
        if cell_x < 0 || cell_y < 0 {
            return None;
        }
        Some(self.cuts.pathfind_cells.get(&(layer, cell_x, cell_y)).copied().unwrap_or(PathfindCellType::Clear))
    }
    fn quick_does_path_exist(&mut self, source: ObjectId, to: &Coord3D) -> Option<bool> {
        // (batch I42: the world's pathfinder with `live_pathfinder` and a map, with the source's AI's locomotor set; it was always
        // the table, which nothing in a game sets, so a computer player's `quickDoesPathExist` was always false)
        if self.pf_ready().is_some() {
            self.ai_peek(source)?;
            let loco = self.ai_locomotor_set(source);
            let from = self.objects.find(source)?.get_position();
            return self.with_pathfinder(|pf, w| pf.client_safe_quick_does_path_exist(w, loco.valid_surfaces, &from, to));
        }
        self.cuts.partition_quick_path
    }
    fn relationship(&self, obj: ObjectId, other: ObjectId) -> Relationship {
        self.get_relationship(obj, other)
    }
}

impl PartitionFilterHost for World {
    fn parking_place_reserve_space(&mut self, airfield: ObjectId, id: ObjectId) -> bool {
        use crate::logic::world::module_call::{ModuleCall, ModuleReply};
        let Some(pp) = self.module_with_interface(airfield, ModuleCall::HasParkingPlaceInterface) else { return false };
        matches!(self.module_call(airfield, pp, ModuleCall::ParkingPlaceReserveSpace { id, parking_offset: 0.0, want_info: false }), ModuleReply::PPInfo { ok: true, .. })
    }
    fn is_using_airborne_locomotor(&self, id: ObjectId) -> bool {
        World::is_using_airborne_locomotor(self, id)
    }
    fn controlling_player(&self, id: ObjectId) -> Option<i32> {
        self.get_controlling_player(id)
    }
    fn team(&self, id: ObjectId) -> TeamId {
        self.objects.find(id).map_or(0, |o| o.team)
    }
    fn is_on_squad(&self, squad: i32, id: ObjectId) -> bool {
        let (player, hotkey) = World::squad_slot(squad);
        self.players.get_nth(player).and_then(|p| p.squads.get(hotkey)).is_some_and(|s| squad_is_on_squad(s, id))
    }
    fn is_effectively_dead(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.is_effectively_dead())
    }
    fn is_off_map(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.is_off_map())
    }
    fn is_view_blocked_by_obstacle(&mut self, obj: ObjectId, other: ObjectId) -> bool {
        // (batch I42: the world's pathfinder with `live_pathfinder` and a map; it was always the table, so nothing blocked a line of
        // sight filter's view)
        if self.pf_ready().is_some() {
            let (Some(a), Some(b)) = (self.pf_object_facts(obj), self.pf_object_facts(other)) else { return false };
            return self.with_pathfinder(|pf, w| pf.is_view_blocked_by_obstacle(w, &a, &b)).unwrap_or(false);
        }
        self.cuts.view_blocked.contains(&(obj, other))
    }
    fn able_to_attack_specific_object(&mut self, obj: ObjectId, attack_type: AbleToAttackType, other: ObjectId, source: CommandSourceType) -> CanAttackResult {
        match object_weapons::get_able_to_attack_specific_object(self, obj, attack_type, other, source, -1) {
            Ok(WeaponCanAttackResult::Possible) => CanAttackResult::Possible,
            Ok(WeaponCanAttackResult::PossibleAfterMoving) => CanAttackResult::PossibleAfterMoving,
            Ok(WeaponCanAttackResult::NotPossible) => CanAttackResult::NotPossible,
            Ok(WeaponCanAttackResult::InvalidShot) | Err(_) => CanAttackResult::OtherRefusal,
        }
    }
    fn can_enter_object(&mut self, obj: ObjectId, other: ObjectId, source: CommandSourceType) -> bool {
        action_manager::can_enter_object(self, Some(obj), Some(other), source, CanEnterType::DontCheckCapacity).unwrap_or(false)
    }
    fn can_hijack_vehicle(&mut self, obj: ObjectId, other: ObjectId, source: CommandSourceType) -> bool {
        action_manager::can_hijack_vehicle(self, Some(obj), Some(other), source)
    }
    fn last_damage_source(&mut self, id: ObjectId) -> Option<ObjectId> {
        let body = self.get_body_module(id)?;
        Some(self.module_call(id, body, ModuleCall::GetLastDamageSource).as_id())
    }
    fn kind_of_mask(&self, id: ObjectId) -> KindOfMaskType {
        self.objects.find(id).map_or_else(KindOfMaskType::new, |o| *o.template().kindof())
    }
    fn transform_x_vector(&self, id: ObjectId) -> Coord3D {
        let v = self.objects.find(id).map(|o| o.thing.transform_matrix().get_x_vector()).unwrap_or_default();
        Coord3D { x: v.x, y: v.y, z: v.z }
    }
    fn is_structure(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.is_kind_of(KINDOF_STRUCTURE))
    }
    fn is_non_faction_structure(&self, id: ObjectId) -> bool {
        World::is_non_faction_structure(self, id)
    }
    fn is_able_to_attack(&mut self, id: ObjectId) -> bool {
        World::is_able_to_attack(self, id)
    }
    fn contain_facts(&mut self, id: ObjectId) -> Option<ContainFacts> {
        let c = self.get_contain(id)?;
        let is_garrisonable = self.module_call(id, c, ModuleCall::IsGarrisonable).as_bool();
        let contain_count = self.module_call(id, c, ModuleCall::GetContainCount).as_int(0);
        let stealth_units_contained = self.module_call(id, c, ModuleCall::GetStealthUnitsContained).as_int(0);
        let first = match self.module_call(id, c, ModuleCall::GetContainedItems) {
            ModuleReply::Ids(v) => v.first().copied(),
            _ => None,
        };
        let first_contained_detected = first.and_then(|f| self.objects.find(f)).map(|o| o.test_status(OBJECT_STATUS_DETECTED));
        Some(ContainFacts { is_garrisonable, contain_count, stealth_units_contained, first_contained_detected })
    }
    fn apparent_controlling_player(&mut self, id: ObjectId, observer_player: Option<i32>) -> Option<i32> {
        let c = self.get_contain(id)?;
        match self.module_call(id, c, ModuleCall::GetApparentControllingPlayer { observer: observer_player.unwrap_or(-1) }) {
            ModuleReply::Int(p) if p >= 0 => Some(p),
            _ => None,
        }
    }
    fn player_relationship_to_player(&self, a: i32, b: i32) -> Relationship {
        self.tq().player_relationship_to_player_default_team(a, b)
    }
    fn controlling_player_is_computer(&self, id: ObjectId) -> bool {
        self.get_controlling_player(id).and_then(|p| self.players.get_nth(p)).is_some_and(|p| p.player_type == PlayerType::Computer)
    }
    fn stealth_facts(&mut self, id: ObjectId) -> Option<StealthFacts> {
        let s = self.objects.find(id)?.stealth?;
        Some(match self.module_call(id, s, ModuleCall::StealthGetDisguisedPlayerIndex) {
            ModuleReply::Int(p) => StealthFacts { is_disguised: true, disguised_player_index: p },
            _ => StealthFacts { is_disguised: false, disguised_player_index: -1 },
        })
    }
    fn team_relationship_to_player(&self, obj: ObjectId, player: i32) -> Relationship {
        match self.get_team(obj) {
            Some(team) => self.tq().team_relationship(team, Some(self.tq().player_default_team(player))),
            None => Relationship::Neutral,
        }
    }
    fn template_is_equivalent_to(&self, template_key: i32, other: ObjectId) -> bool {
        // ~~the template's `getTemplateID()` (the filter has no translated caller yet)~~ **integration batch I26:** upstream's
        // `m_tThing->isEquivalentTo(other->getTemplate())` (a reskin or build variation is the same thing), now that the scripts use it
        u16::try_from(template_key).is_ok_and(|t| self.template_is_equivalent_to_object(t, other))
    }
    fn can_player_garrison(&mut self, player: Option<i32>, other: ObjectId, source: CommandSourceType) -> bool {
        action_manager::can_player_garrison(self, player, Some(other), source).unwrap_or(false)
    }
    fn is_disabled_unmanned(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.is_disabled_by_type(DISABLED_UNMANNED))
    }
    fn command_button_valid_target(&mut self, button: i32, source: ObjectId, other: ObjectId, command_source: CommandSourceType) -> bool {
        // ~~`CommandButton::isValidToUseOn` is the control bar's (layer 13); no translated caller builds this filter yet~~ **integration
        // batch I27:** `m_commandButton->isValidToUseOn(m_source, other, NULL, m_commandSource)`, the button its index in the world's table
        usize::try_from(button).is_ok_and(|b| self.command_button_is_valid_to_use_on(b, source, Some(other), None, command_source))
    }
    fn point_in_trigger(&self, trigger: i32, x: i32, y: i32) -> bool {
        usize::try_from(trigger).ok().and_then(|t| self.terrain.triggers().get(t)).is_some_and(|t| t.point_in_trigger(&ICoord3D { x, y, z: 0 }))
    }
    fn build_cost(&self, id: ObjectId) -> i32 {
        self.objects.find(id).map_or(0, |o| o.template().friend_get_build_cost())
    }
    fn weapon_within_attack_range(&mut self, obj: ObjectId, slot: i32, other: ObjectId) -> Option<bool> {
        let set = crate::logic::world::object_weapons::ObjectWeaponHost::weapon_set(self, obj).clone();
        let weapon = set.get_weapon_in_weapon_slot(slot)?.clone();
        Some(weapon.is_within_attack_range(self, obj, other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::base_type::ICoord2D;
    use crate::common::geometry::{GeometryInfo, GeometryType};
    use crate::common::thing_template::ThingTemplate;
    use crate::logic::terrain_logic::{GroundSource, TerrainLogic};
    use crate::logic::terrain_visual::LogicHeightMap;
    use crate::logic::world::object_create::NewObject;
    use crate::logic::world::partition::scenario::{ScenarioHost, ScenarioObject};
    use crate::logic::world::partition::DistanceCalculationType as Dc;
    use crate::logic::world::statics::GameStatics;
    use std::sync::Arc;

    /// A world over a flat 40 x 40 height map (a 400 x 400 extent, as the scenario host's), 40-unit partition cells,
    /// four players with player 1 local, and the partition manager initialised.
    fn world() -> World {
        let mut w = World::new();
        let mut t = TerrainLogic::new(GroundSource::LogicHeightMap);
        t.set_logic_height_map(Some(LogicHeightMap::new(40, 40, 0, vec![ICoord2D { x: 40, y: 40 }], vec![0; 1600]).unwrap()));
        t.set_boundaries(vec![ICoord2D { x: 40, y: 40 }], 0);
        w.terrain = t;
        let mut st = GameStatics::default();
        let g = st.global_data.create_original();
        g.partition_cell_size = 40.0;
        g.unlook_persist_duration = 30;
        w.statics = Arc::new(st);
        w.players.set_player_count(4);
        w.players.friend_set_local_index(Some(1));
        w.cuts.log = Some(Vec::new());
        w.partition_init();
        w
    }

    /// Small deterministic generator for the layout.
    fn lcg(s: &mut u32) -> u32 {
        *s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        *s >> 8
    }

    /// The same `n` objects in the world (made with the partition recorded only, then registered live) and in a
    /// scenario host: spheres, cylinders and boxes, some turned.
    fn twin(n: i32, seed: u32) -> (World, ScenarioHost, PartitionManager) {
        let mut w = world();
        let mut host = ScenarioHost::default();
        host.objects.clear();
        let mut pm = PartitionManager::new();
        pm.init(&host);
        let mut s = seed;
        let mut ids = Vec::new();
        for _ in 0..n {
            let id = w.new_object(NewObject { template: Arc::new(ThingTemplate::new(0)), status: Default::default(), team: None, modules: vec![] }).expect("new object");
            let kind = lcg(&mut s) % 3;
            let (major, minor) = (2.0 + (lcg(&mut s) % 60) as f32, 2.0 + (lcg(&mut s) % 30) as f32);
            let geom = match kind {
                0 => GeometryInfo::new(GeometryType::Sphere, false, major, major, major),
                1 => GeometryInfo::new(GeometryType::Cylinder, false, 10.0, major, major),
                _ => GeometryInfo::new(GeometryType::Box, false, 10.0, major, minor),
            };
            let pos = Coord3D { x: (lcg(&mut s) % 400) as f32, y: (lcg(&mut s) % 400) as f32, z: 0.0 };
            let angle = (lcg(&mut s) % 628) as f32 / 100.0;
            w.objects.find_mut(id).unwrap().geometry_info = geom.clone();
            w.set_position(id, &pos);
            w.set_orientation(id, angle);
            let o = w.objects.find(id).unwrap();
            let mut so = ScenarioObject::new(id, o.get_position(), geom);
            so.orientation = o.thing.orientation();
            so.status = w.get_status_bits(id);
            host.add(so);
            ids.push(id);
        }
        w.cuts.live_partition = true;
        for &id in &ids {
            w.partition_register_object(id);
            pm.register_object(&mut host, id);
        }
        w.partition_update();
        pm.update(&mut host);
        (w, host, pm)
    }

    fn walk(mut it: SimpleObjectIterator<ObjectId>) -> Vec<ObjectId> {
        let mut v = Vec::new();
        let mut o = it.first();
        while let Some(id) = o {
            v.push(id);
            o = it.next();
        }
        v
    }

    /// The manager run against the world gives the grid and the answers it gives run against the scenario host (which
    /// `tests/differential_partition.rs` checks against the compiled PartitionManager.cpp) for the same objects: every
    /// object's cells, every cell's objects, and range scans and closest objects at random points with every distance kind.
    #[test]
    fn the_world_host_answers_as_the_scenario_host() {
        for seed in [1u32, 7, 99] {
            let (mut w, mut host, mut pm) = twin(30, seed);
            for id in host.objects.keys().copied().collect::<Vec<_>>() {
                assert_eq!(w.partition.cells_touched_by(id), pm.cells_touched_by(id), "cells of {id}");
            }
            for x in 0..pm.get_cell_count_x() {
                for y in 0..pm.get_cell_count_y() {
                    assert_eq!(w.partition.objects_in_cell(x, y), pm.objects_in_cell(x, y));
                }
            }
            let mut s = seed ^ 0x5a5a;
            for _ in 0..200 {
                let p = Coord3D { x: (lcg(&mut s) % 420) as f32 - 10.0, y: (lcg(&mut s) % 420) as f32 - 10.0, z: 0.0 };
                let range = (lcg(&mut s) % 150) as f32;
                let dc = Dc::from_index((lcg(&mut s) % 4) as i32).unwrap();
                let order = if lcg(&mut s).is_multiple_of(2) { IterOrderType::Fastest } else { IterOrderType::SortedNearToFar };
                let mine = walk(w.with_partition(|m, h| m.iterate_objects_in_range_at(h, &p, range, dc, &[], order)));
                let theirs = walk(pm.iterate_objects_in_range_at(&mut host, &p, range, dc, &[], order));
                assert_eq!(mine, theirs, "scan at {p:?} r {range} {dc:?}");
                let mine = w.with_partition(|m, h| m.get_closest_object_at(h, &p, range, dc, &[]).object);
                let theirs = pm.get_closest_object_at(&mut host, &p, range, dc, &[]).object;
                assert_eq!(mine, theirs);
                // the same scan through the world's cut point, live
                let via_cut = w.partition_iterate_objects_in_range_at(p, range, dc as i32);
                if order == IterOrderType::Fastest {
                    assert_eq!(via_cut, walk(pm.iterate_objects_in_range_at(&mut host, &p, range, dc, &[], order)));
                }
            }
        }
    }

    /// With the switch off (the default), the cut points answer from their tables and the manager is not touched.
    #[test]
    fn off_by_default_the_tables_answer() {
        let mut w = world();
        assert!(!w.cuts.live_partition);
        let id = w.new_object(NewObject { template: Arc::new(ThingTemplate::new(0)), status: Default::default(), team: None, modules: vec![] }).unwrap();
        assert!(!w.partition.is_registered(id));
        w.cuts.range_scan = vec![77];
        assert_eq!(w.partition_iterate_objects_in_range_at(Coord3D::default(), 50.0, 0), vec![77]);
    }

    /// A moved object is re-celled by the update, and the cell-change callback runs with the manager back in the world
    /// (it reaches the world's own `onPartitionCellChange`, which needs `world.partition` in place for its shroud calls).
    #[test]
    fn moving_an_object_recells_it_through_the_update() {
        let (mut w, _, _) = twin(3, 5);
        let id = w.objects.first_id();
        w.set_position(id, &Coord3D { x: 10.0, y: 10.0, z: 0.0 });
        w.partition.make_object_dirty(id, true);
        w.partition_update();
        assert!(w.partition.cells_touched_by(id).contains(&(0, 0)));
        assert!(w.partition.get_cell_count_x() > 0, "the manager came back from the update");
        // a shroud reveal through the cut point lands in the real manager
        w.cut(CutCall::PartitionDoShroudReveal { x: 100.0, y: 100.0, range: 50.0, mask: 1 << 1 });
        let st = w.partition.get_shroud_status_for_player(1, &Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        assert_eq!(st, crate::common::game_common::CellShroudStatus::Clear);
    }
}
