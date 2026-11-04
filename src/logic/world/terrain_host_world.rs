//! Integration batch I2: the world as the terrain logic's host.
//!
//! `TerrainLogic.cpp` asks the rest of the game (the objects, the game-logic
//! clock, the partition manager, the pathfinder, the thing factory) through
//! [`TerrainWorld`]. Here the [`World`] answers, from its own objects and
//! modules, so that `world.terrain` runs as upstream's `TheTerrainLogic`
//! does, with the objects of the game it is part of:
//!
//! - **The objects**: `getFirstObject`/`getNextObject` is the object list,
//!   newest first; a range scan is the partition manager's
//!   ([`World::partition_iterate_objects_in_range_at`], live or from the table
//!   by `cuts.live_partition`); damage, destruction and the body's damage
//!   state are the world's; the bridge's scaffold is its
//!   `BridgeBehaviorInterface`.
//! - **The thing factory**: the `GenericBridge` object of a bridge drawn in
//!   the map is `findTemplate`, `newObject` (the neutral team), `setPosition`,
//!   `updateObjValuesFromMapProperties` (batch O1) and `setOrientation`; a
//!   landmark bridge's towers are `Bridge::createTower`, with the bridge's and
//!   the tower's behaviour interfaces tied as upstream ties them.
//! - **The terrain roads** ~~(the client's INI, read by no layer yet) are the
//!   `cuts.bridge_templates` table, which batch I2 extends with the tower
//!   object names.~~ **Corrected 2026-11-03 (batch I50):** with `live_terrain`
//!   they are the world's `TheTerrainRoads` (`statics.terrain_roads`, read from
//!   `Roads.ini` by the game data loader); the table is the tests' without it.
//! - **The pathfinder** ~~is not in the world yet~~ is the world's since batch
//!   I9a (`pathfinder_host_world.rs`): with `live_pathfinder`, `addBridge` is
//!   its own and `changeBridgeState` and `forceMapRecalculation` run once the
//!   terrain is back in the world; without it they are cut calls and the layer
//!   `addBridge` assigns is [`World::pathfinder_add_bridge_stand_in`]'s. Its
//!   wall height is ~~`cuts.wall_height`~~ (Corrected 2026-11-03 (batch I48): [`World::wall_height`]); ~~no point is on a wall~~ a point is on
//!   a wall as its `isPointOnWall` answers (`World::pf_wall`).
//! - **`setActiveBoundary`** stores and restores the fog through the world's
//!   partition manager, takes every object out of it and back in
//!   (`friend_prepareForMapBoundaryAdjust`, `friend_notifyOfNewMapBoundary`),
//!   and resets and inits the manager on the new boundary. The ghost object
//!   manager is the base class' (it holds nothing); the radar's `newMap` and
//!   the tactical view's camera are the client's
//!   ([`ObjectClientEvent::RadarNewMap`],
//!   [`ObjectClientEvent::TacticalViewForceCameraConstraintRecalc`]).
//!
//! # Lending the terrain
//!
//! A terrain call runs with `world.terrain` checked out
//! ([`World::with_terrain`]), since the terrain logic and its host are both
//! the world. Most host calls only read objects, but four run object code
//! that may read `TheTerrainLogic` in turn: damage, destruction and the two
//! object creations. For those the terrain call passes the terrain as it
//! stands, and the world puts a copy of it back in its field for the object
//! code ([`World::lend_terrain`]): the object code sees the terrain upstream
//! would show it. Changes the object code makes to that copy are dropped,
//! and counted in `cuts.terrain_lost_writes`; upstream has one terrain, so a
//! non-zero count is a divergence (no translated module makes one).
//!
//! # The object code's own terrain calls
//!
//! With `cuts.live_terrain` set, the `terrain_*` and `bridge_set_*` calls
//! the object and module code makes (`getGroundHeight`, `getLayerHeight`,
//! `getHighestLayerForDestination`, `isUnderwater`, `isCliffCell`,
//! `findBridgeAt`, `deleteBridge`, `setBridgeObjectID`, `setTowerObjectID`,
//! `updateBridgeDamageStates`, the edge points and the extent with the
//! border) answer from `world.terrain` instead of the tables. They are still
//! logged as cut calls. Off by default: the object and module tests answer
//! from the tables.

use crate::common::ascii_string::AsciiString;
use crate::common::base_type::Coord3D;
use crate::common::dict::Dict;
use crate::common::game_type::{DrawableId, PathfindLayer, INVALID_DRAWABLE_ID, LAYER_GROUND, LAYER_WALL};
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::logic::damage::DamageInfo;
use crate::logic::module::body_module::{BodyDamageType, BODY_PRISTINE};
use crate::logic::terrain_logic::{Bridge, BridgeInfo, BridgeTowerType, LandmarkBridgeObject, ObjectView, TerrainLogic, TerrainWorld, WaterHandle};
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::{ObjectId, INVALID_ID};
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object_cuts::{BridgeDetails, CutCall};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::partition::DistanceCalculationType;

/// What a terrain call answers: upstream's value, or the undefined input it met.
pub type TerrainResult<T> = crate::logic::terrain_logic::TlResult<T>;

impl World {
    /// Runs `f` on the world's terrain logic, checked out of the field, with the world as its host (see the module
    /// docs).
    pub fn with_terrain<R>(&mut self, f: impl FnOnce(&mut TerrainLogic, &mut World) -> R) -> R {
        let mut terrain = std::mem::take(&mut self.terrain);
        let r = f(&mut terrain, self);
        self.terrain = terrain;
        // (batch I9a: the pathfinder's bridge calls the terrain made, now that it can be asked about its bridges)
        self.pathfinder_run_deferred_bridge_calls();
        r
    }

    /// Runs `f`, object code called from inside a terrain call, with a copy of `terrain` (the checked-out terrain as it
    /// stands) in the world's field; then takes the copy out again, dropping what `f` changed in it (see the module docs).
    pub fn lend_terrain<R>(&mut self, terrain: &TerrainLogic, f: impl FnOnce(&mut World) -> R) -> R {
        let held = std::mem::replace(&mut self.terrain, terrain.clone());
        let was_lent = std::mem::replace(&mut self.cuts.terrain_lent, true);
        let r = f(self);
        self.cuts.terrain_lent = was_lent;
        self.terrain = held;
        r
    }

    /// Counts a change to the terrain logic made while it is lent (see the module docs).
    fn note_terrain_write(&mut self) {
        if self.cuts.terrain_lent {
            self.cuts.terrain_lost_writes += 1;
        }
    }

    /// `TheTerrainLogic->update()`.
    pub fn terrain_update(&mut self) -> TerrainResult<()> {
        self.note_terrain_write();
        self.with_terrain(|t, w| t.update(w))
    }

    /// `TheTerrainLogic->addBridgeToLogic(pInfo, props, bridgeTemplateName)` (the client's
    /// `loadRoadsAndBridges`).
    pub fn terrain_add_bridge_to_logic(&mut self, info: &BridgeInfo, props: &Dict, bridge_template_name: &AsciiString) {
        self.note_terrain_write();
        self.with_terrain(|t, w| t.add_bridge_to_logic(w, info, props, bridge_template_name));
    }

    /// `TheTerrainLogic->addLandmarkBridgeToLogic(obj)`: what `Bridge::Bridge(Object*)` reads of the object, then the
    /// call. Nothing for no such object.
    pub fn terrain_add_landmark_bridge_to_logic(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find(id) else { return };
        let Some(template_name) = AsciiString::try_from_bytes(o.template().get_name()).ok() else { return };
        let landmark = LandmarkBridgeObject {
            id,
            template_name,
            position: o.get_position(),
            orientation: o.thing.orientation(),
            geometry: o.geometry_info.clone(),
        };
        self.note_terrain_write();
        self.with_terrain(|t, w| t.add_landmark_bridge_to_logic(w, &landmark));
    }

    /// `TheTerrainLogic->setWaterHeight(water, height, damageAmount, forcePathfindUpdate)`.
    pub fn terrain_set_water_height(&mut self, water: Option<WaterHandle>, height: f32, damage_amount: f32, force_pathfind_update: bool) -> TerrainResult<()> {
        self.note_terrain_write();
        self.with_terrain(|t, w| t.set_water_height(w, water, height, damage_amount, force_pathfind_update))
    }

    /// `TheTerrainLogic->changeWaterHeightOverTime(water, finalHeight, transitionTimeInSeconds, damageAmount)`.
    pub fn terrain_change_water_height_over_time(&mut self, water: Option<WaterHandle>, final_height: f32, seconds: f32, damage_amount: f32) -> TerrainResult<()> {
        self.note_terrain_write();
        self.terrain.change_water_height_over_time(water, final_height, seconds, damage_amount)
    }

    /// `TheTerrainLogic->setActiveBoundary(newActiveBoundary)`.
    pub fn terrain_set_active_boundary(&mut self, new_active_boundary: i32) {
        self.note_terrain_write();
        self.with_terrain(|t, w| t.set_active_boundary(w, new_active_boundary));
    }

    /// `TheAI->pathfinder()->addBridge(bridge)`'s layer ~~until the pathfinder is in the world~~ without `live_pathfinder`
    /// (batch I9a): the next layer above the
    /// ground, up to and including the wall layer as upstream's loop runs, then the ground ("Ran out of bridge
    /// layers."). Upstream also answers the ground when the layer's `init` fails (a bridge off the map), which this
    /// stand-in does not see.
    pub fn pathfinder_add_bridge_stand_in(&mut self, bridge_object: ObjectId) -> PathfindLayer {
        self.cut(CutCall::PathfinderAddBridge { bridge_object });
        let layer = LAYER_GROUND + 1 + self.cuts.pathfinder_bridges_added;
        if layer > LAYER_WALL {
            return LAYER_GROUND;
        }
        self.cuts.pathfinder_bridges_added += 1;
        layer
    }

    // ---- the object code's terrain calls, live (see the module docs) ----

    /// `findBridgeAt(position)`'s bridge, by its position in the list.
    fn live_bridge_at(&self, position: &Coord3D) -> Option<usize> {
        self.terrain.find_bridge_at(position)
    }

    /// `getHighestLayerForDestination(pos)` on the world's terrain; the ground for an undefined input.
    pub(crate) fn live_terrain_highest_layer(&mut self, pos: &Coord3D) -> PathfindLayer {
        self.with_terrain(|t, w| t.highest_layer_for_destination(w, pos, false)).unwrap_or(LAYER_GROUND)
    }

    /// `getLayerHeight(x, y, layer)` on the world's terrain; the ground height for an undefined input.
    pub(crate) fn live_terrain_layer_height(&mut self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        match self.with_terrain(|t, w| t.layer_height(w, x, y, layer, false, true)) {
            Ok((height, _)) => height,
            Err(_) => self.terrain.ground_height(x, y, false).0,
        }
    }

    /// `findBridgeAt(position)` and `getBridgeInfo(&info)`: the info's ends.
    pub(crate) fn live_terrain_find_bridge_at(&self, position: &Coord3D) -> Option<(Coord3D, Coord3D)> {
        self.live_bridge_at(position).map(|i| {
            let info = self.terrain.bridges()[i].info();
            (info.from, info.to)
        })
    }

    /// `findBridgeAt(position)` as `BridgeBehavior` reads it.
    pub(crate) fn live_terrain_find_bridge_details(&self, position: &Coord3D) -> Option<BridgeDetails> {
        self.live_bridge_at(position).map(|i| {
            let b = &self.terrain.bridges()[i];
            let info = b.info();
            BridgeDetails {
                from_left: info.from_left,
                from_right: info.from_right,
                to_left: info.to_left,
                to_right: info.to_right,
                template_name: b.template_name().str().to_vec(),
                layer: b.layer(),
            }
        })
    }

    /// `deleteBridge(bridge)`, the bridge by its info's ends: the first in the list with them (`findBridgeAt` found
    /// the caller's).
    pub(crate) fn live_terrain_delete_bridge(&mut self, bridge: Option<(Coord3D, Coord3D)>) {
        let Some((from, to)) = bridge else { return };
        let Some(i) = self.terrain.bridges().iter().position(|b| b.info().from == from && b.info().to == to) else { return };
        self.note_terrain_write();
        self.with_terrain(|t, w| t.delete_bridge(w, i));
    }

    /// `findBridgeAt(position)->setBridgeObjectID(id)`; nothing when there is no bridge there (upstream's caller
    /// checks).
    pub(crate) fn live_bridge_set_bridge_object_id(&mut self, position: &Coord3D, id: ObjectId) {
        let Some(i) = self.live_bridge_at(position) else { return };
        self.note_terrain_write();
        if let Some(b) = self.terrain.bridge_mut(i) {
            b.set_bridge_object_id(id);
        }
    }

    /// `findBridgeAt(position)->setTowerObjectID(id, tower)`; nothing when there is no bridge there or the tower type is
    /// out of range.
    pub(crate) fn live_bridge_set_tower_object_id(&mut self, position: &Coord3D, id: ObjectId, tower: i32) {
        let Some(which) = usize::try_from(tower).ok().and_then(|t| BridgeTowerType::ALL.get(t).copied()) else { return };
        let Some(i) = self.live_bridge_at(position) else { return };
        self.note_terrain_write();
        if let Some(b) = self.terrain.bridge_mut(i) {
            b.set_tower_object_id(id, which);
        }
    }

    /// `updateBridgeDamageStates()` on the world's terrain.
    pub(crate) fn live_terrain_update_bridge_damage_states(&mut self) {
        self.note_terrain_write();
        // an undefined input (none is reachable from a bridge's damage state) leaves the states as far as they got
        let _ = self.with_terrain(|t, w| t.update_bridge_damage_states(w));
    }

    /// The tower template `TheTerrainRoads->findBridge(bridge)->getTowerObjectName(tower)` names, by id.
    fn bridge_tower_template_id(&self, bridge_template: &AsciiString, tower: BridgeTowerType) -> Option<u16> {
        // (integration batch I50: through `terrain_roads_find_bridge`, the world's `TheTerrainRoads` with `live_terrain`)
        let road = self.terrain_roads_find_bridge(bridge_template.str())?;
        self.thing_factory_find_template(&road.tower_object_names[tower as usize])
    }
}

impl TerrainWorld for World {
    fn frame(&self) -> u32 {
        World::frame(self)
    }

    fn objects(&self) -> Vec<ObjectView> {
        self.objects.ids_in_order().into_iter().filter_map(|id| object_view(self, id)).collect()
    }

    fn objects_in_range_2d(&mut self, center: &Coord3D, range: f32) -> Vec<ObjectView> {
        let ids = self.partition_iterate_objects_in_range_at(*center, range, DistanceCalculationType::FromCenter2D as i32);
        ids.into_iter().filter_map(|id| object_view(self, id)).collect()
    }

    fn object_exists(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some()
    }

    /// `getBodyModule()->getDamageState()`. Every bridge object has a body; upstream dereferences `NULL` for one
    /// without, which answers `BODY_PRISTINE` here.
    fn object_damage_state(&mut self, id: ObjectId) -> BodyDamageType {
        match self.get_body_module(id) {
            Some(body) => self.module_call(id, body, ModuleCall::GetDamageState).as_int(BODY_PRISTINE),
            None => BODY_PRISTINE,
        }
    }

    fn bridge_scaffold_present(&mut self, id: ObjectId) -> Option<bool> {
        let bbi = self.bridge_behavior_interface(id)?;
        Some(self.module_call(id, bbi, ModuleCall::BridgeIsScaffoldPresent).as_bool())
    }

    fn object_drawable(&self, id: ObjectId) -> Option<DrawableId> {
        self.objects.find(id).map(|o| o.drawable).filter(|&d| d != INVALID_DRAWABLE_ID)
    }

    fn attempt_damage(&mut self, terrain: &TerrainLogic, id: ObjectId, info: &DamageInfo) {
        let mut info = info.clone();
        self.lend_terrain(terrain, |w| World::attempt_damage(w, id, &mut info));
    }

    fn destroy_object(&mut self, terrain: &TerrainLogic, id: ObjectId) {
        self.lend_terrain(terrain, |w| World::destroy_object(w, id));
    }

    /// Batch I9a: with `live_pathfinder`, the world pathfinder's `addBridge` (it records the bridge only; the handle is the
    /// bridge object's id, see `pathfinder_host_world.rs`).
    fn pathfinder_add_bridge(&mut self, bridge: &Bridge) -> PathfindLayer {
        let id = bridge.info().bridge_object_id;
        if self.cuts.live_pathfinder {
            if let Some(pf) = self.pathfinder.as_mut() {
                return pf.add_bridge(id as crate::logic::ai::layer::BridgeHandle);
            }
            self.cuts.team_live_errors.push("Pathfinder::addBridge: the pathfinder is out".into());
        }
        self.pathfinder_add_bridge_stand_in(id)
    }

    /// Batch I9a: with `live_pathfinder`, deferred until the terrain is back (it asks about the bridges).
    fn pathfinder_change_bridge_state(&mut self, layer: PathfindLayer, enable: bool) {
        if self.pathfinder_bridge_op(crate::logic::world::pathfinder_host_world::PfOp::ChangeBridgeState(layer, enable)) {
            return;
        }
        World::pathfinder_change_bridge_state(self, layer, enable);
    }

    fn pathfinder_force_map_recalculation(&mut self) {
        if self.pathfinder_bridge_op(crate::logic::world::pathfinder_host_world::PfOp::ForceMapRecalculation) {
            return;
        }
        self.cut(CutCall::PathfinderForceMapRecalculation);
    }

    fn pathfinder_wall_height(&self) -> f32 {
        self.wall_height()
    }

    /// ~~No point is on a wall until the pathfinder is in the world (see the module docs).~~ **Batch I9a:** the world
    /// pathfinder's `isPointOnWall`, through its [`WallView`](crate::logic::ai::pathfinder::WallView) (answered while the
    /// pathfinder is checked out too).
    fn pathfinder_is_point_on_wall(&self, pos: &Coord3D) -> bool {
        self.pf_wall.is_point_on_wall(pos)
    }

    /// Upstream caches the template in a function-local `static`, looked up on the first call; the templates do not
    /// change after the INI is read, so looking it up on every call answers alike.
    fn create_generic_bridge_object(&mut self, terrain: &TerrainLogic, center: &Coord3D, orientation: f32, props: &Dict) -> Option<ObjectId> {
        let template = self.thing_factory_find_template(b"GenericBridge")?;
        self.lend_terrain(terrain, |w| {
            let id = w.thing_factory_new_object(template, None, ObjectStatusMaskType::default()).ok()?;
            w.set_position(id, center);
            // a veterancy level out of range is undefined upstream (batch O1); the properties before it are applied
            let _ = w.update_obj_values_from_map_properties(id, props);
            w.set_orientation(id, orientation);
            Some(id)
        })
    }

    fn find_bridge_road_template(&self, name: &AsciiString) -> bool {
        // (integration batch I50: `TheTerrainRoads->findBridge(name) != NULL` on the world's roads with `live_terrain`)
        self.terrain_roads_find_bridge(name.str()).is_some()
    }

    fn find_bridge_tower_template(&self, bridge_template: &AsciiString, tower: BridgeTowerType) -> Option<f32> {
        let id = self.bridge_tower_template_id(bridge_template, tower)?;
        let t = self.statics.thing_factory.find_by_template_id(id)?;
        Some(t.data.get_template_geometry_info().get_major_radius())
    }

    /// `Bridge::createTower`: `newObject(towerTemplate, bridge->getTeam())`, the position and angle, `setTower` on the
    /// bridge's interface, `setBridge` and `setTowerType` on the tower's, and the indestructible flag. `INVALID_ID` when
    /// the object cannot be made (upstream's `newObject` always makes one).
    fn create_bridge_tower(&mut self, terrain: &TerrainLogic, bridge: ObjectId, tower: BridgeTowerType, pos: &Coord3D, angle: f32) -> ObjectId {
        let Some(bridge_name) = self.objects.find(bridge).and_then(|o| AsciiString::try_from_bytes(o.template().get_name()).ok()) else {
            return INVALID_ID;
        };
        let Some(template) = self.bridge_tower_template_id(&bridge_name, tower) else { return INVALID_ID };
        let team = self.objects.find(bridge).map(|o| o.team);
        self.lend_terrain(terrain, |w| {
            let Ok(id) = w.thing_factory_new_object(template, team, ObjectStatusMaskType::default()) else { return INVALID_ID };
            // set the position and angle
            w.set_position(id, pos);
            w.set_orientation(id, angle);
            // tie it to the bridge
            if let Some(bbi) = w.bridge_behavior_interface(bridge) {
                w.module_call(bridge, bbi, ModuleCall::BridgeSetTower { tower: tower as i32, object: id });
            }
            // tie the bridge to us
            if let Some(bti) = w.bridge_tower_interface(id) {
                w.module_call(id, bti, ModuleCall::BridgeTowerSetBridge(bridge));
                w.module_call(id, bti, ModuleCall::BridgeTowerSetTowerType(tower as i32));
            }
            // if the bridge is indestructible, so is this tower
            let indestructible = match w.get_body_module(bridge) {
                Some(body) => w.module_call(bridge, body, ModuleCall::IsIndestructible).as_bool(),
                None => false,
            };
            if indestructible {
                if let Some(body) = w.get_body_module(id) {
                    w.module_call(id, body, ModuleCall::SetIndestructible(true));
                }
            }
            id
        })
    }

    fn before_map_boundary_change(&mut self) {
        // "Can't have any lingering looks persist over the resize, so flush the queue now"
        self.with_partition(|pm, w| pm.process_entire_pending_undo_shroud_reveal_queue(&*w));
        // Store fogged cells
        let mut store = std::mem::take(&mut self.cuts.boundary_store);
        self.partition.store_fogged_cells(&mut store, true);
        self.cuts.boundary_store = store;
    }

    fn after_map_boundary_change(&mut self, terrain: &TerrainLogic) {
        self.lend_terrain(terrain, |w| {
            // (the ghost object manager's releasePartitionData: the base class holds none)
            // Remove objects from partition manager so that any remaining cleared cells must be permanently revealed.
            for id in w.objects.ids_in_order() {
                w.friend_prepare_for_map_boundary_adjust(id);
            }
            // Store permanently revealed cells.
            let mut store = std::mem::take(&mut w.cuts.boundary_store);
            w.partition.store_fogged_cells(&mut store, false);
            w.partition.reset();
            w.partition_init();
            w.emit(ObjectClientEvent::RadarNewMap);
            w.with_partition(|pm, h| pm.restore_fogged_cells(&*h, &store, false));
            // (lockGhostObjects(TRUE): nothing to lock)
            for id in w.objects.ids_in_order() {
                w.friend_notify_of_new_map_boundary(id);
            }
            w.with_partition(|pm, h| pm.restore_fogged_cells(&*h, &store, true));
            // (restorePartitionData, lockGhostObjects(FALSE): nothing to restore)
            w.emit(ObjectClientEvent::TacticalViewForceCameraConstraintRecalc);
        });
    }
}

/// What the terrain logic reads of object `id`; `None` for no such object.
pub(crate) fn object_view(w: &World, id: ObjectId) -> Option<ObjectView> {
    let o = w.objects.find(id)?;
    Some(ObjectView { id, position: o.get_position(), orientation: o.thing.orientation(), layer: o.layer, geometry: o.geometry_info.clone() })
}


#[cfg(test)]
mod tests {
    //! The world as the terrain logic's host: the terrain's own logic (bridge corners, tower placement, water, layers)
    //! is checked against the compiled TerrainLogic.cpp by `tests/differential_terrain_logic.rs` with a recording host;
    //! these check that the world's answers to that host's questions are the world's objects and modules.
    use super::*;
    use crate::common::base_type::ICoord2D;
    use crate::common::damage_fx::DamageFxStore;
    use crate::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
    use crate::common::module_factory::ModuleFactory;
    use crate::common::name_key_generator::NameKeyGenerator;
    use crate::common::science::{GameTextFetch, ScienceStore};
    use crate::common::thing_factory::ThingFactory;
    use crate::common::thing_template::ThingParseEnv;
    use crate::common::unicode_string::UnicodeString;
    use crate::common::well_known_keys::{self as wk, WellKnownKeys};
    use crate::logic::armor::ArmorStore;
    use crate::logic::module::body_module::BODY_RUBBLE;
    use crate::logic::terrain_logic::{GroundSource, TriggerArea};
    use crate::logic::terrain_visual::LogicHeightMap;
    use crate::logic::weapon_set::WeaponTemplateTable;
    use crate::logic::world::statics::GameStatics;
    use crate::logic::client_event::ClientEvent;
    use crate::logic::world::team::Team;
    use std::sync::Arc;

    struct Text;
    impl GameTextFetch for Text {
        fn fetch(&self, _label: &[u8]) -> UnicodeString {
            UnicodeString::new()
        }
    }
    struct Ctx {
        factory: ThingFactory,
        names: NameKeyGenerator,
        modules: ModuleFactory,
        armors: ArmorStore,
        dfx: DamageFxStore,
        weapons: WeaponTemplateTable,
        sciences: ScienceStore,
    }
    fn object_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        let mut env = ThingParseEnv {
            names: &mut c.names,
            modules: &mut c.modules,
            armors: &c.armors,
            damage_fx: &c.dfx,
            weapons: &c.weapons,
            sciences: &c.sciences,
            text: &Text,
            locomotors: &crate::logic::locomotor::LocomotorStore::new(),
        };
        c.factory.parse_object_block(ini, &mut env, 0)
    }

    const INI: &str = "Object GenericBridge\n KindOf = BRIDGE\n Body = ActiveBody ModuleTag_Body\n  MaxHealth = 1000\n  InitialHealth = 1000\n End\nEnd\n\
                       Object LandBridge\n KindOf = BRIDGE\n Geometry = BOX\n GeometryMajorRadius = 60\n GeometryMinorRadius = 15\n GeometryHeight = 5\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 1000\n  InitialHealth = 1000\n End\n Behavior = BridgeBehavior ModuleTag_Bridge\n End\nEnd\n\
                       Object BridgeTower\n KindOf = BRIDGE_TOWER\n Geometry = BOX\n GeometryMajorRadius = 12\n GeometryMinorRadius = 12\n GeometryHeight = 10\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n Behavior = BridgeTowerBehavior ModuleTag_Tower\n End\nEnd\n\
                       Object Tank\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 200\n  InitialHealth = 200\n End\nEnd\n";

    /// A world over a flat 40 x 40 height map at raw height 10 (400 x 400; two boundaries, the second 20 x 20), with the
    /// templates of [`INI`], the neutral player (player 0) on team 101 and player 1 on team 100, the cut log on and the
    /// partition manager initialised and live.
    fn world() -> World {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut modules = ModuleFactory::new();
        modules.init(&mut names).unwrap();
        let mut c = Ctx {
            factory: ThingFactory::new(),
            names,
            modules,
            armors: ArmorStore::new(),
            dfx: DamageFxStore::new(),
            weapons: WeaponTemplateTable::new(),
            sciences: ScienceStore::new(),
        };
        static BLOCKS: [BlockParse<Ctx>; 1] = [BlockParse { token: "Object", parse: object_block }];
        Ini::load(b"Data\\INI\\Test.ini", INI.as_bytes(), IniLoadType::Overwrite, None, &BLOCKS, &mut c).expect("load");

        let mut w = World::new();
        w.names = c.names;
        let mut st = GameStatics { thing_factory: c.factory, ..Default::default() };
        let g = st.global_data.create_original();
        g.partition_cell_size = 40.0;
        w.statics = Arc::new(st);
        let mut t = TerrainLogic::new(GroundSource::LogicHeightMap);
        t.set_logic_height_map(Some(LogicHeightMap::new(40, 40, 0, vec![ICoord2D { x: 40, y: 40 }, ICoord2D { x: 20, y: 20 }], vec![10; 1600]).unwrap()));
        t.set_boundaries(vec![ICoord2D { x: 40, y: 40 }, ICoord2D { x: 20, y: 20 }], 0);
        w.terrain = t;
        w.players.set_player_count(2);
        for team in [100u32, 101] {
            w.teams.insert_team(Team::new(team, 0)).unwrap();
        }
        w.players.get_nth_mut(0).unwrap().default_team = Some(101);
        w.players.get_nth_mut(1).unwrap().default_team = Some(100);
        w.cuts.team_players.insert(100, 1);
        w.cuts.team_players.insert(101, 0);
        w.cuts.neutral_default_team = 101;
        w.cuts.log = Some(Vec::new());
        w.partition_init();
        w.cuts.live_partition = true;
        w.cuts.live_terrain = true;
        w
    }

    /// A new object of the template `name` on `team`, at `pos`, registered with the partition manager.
    fn make(w: &mut World, name: &[u8], team: Option<u32>, pos: Coord3D) -> ObjectId {
        let t = w.thing_factory_find_template(name).expect("template");
        let id = w.thing_factory_new_object(t, team, ObjectStatusMaskType::default()).expect("object");
        w.set_position(id, &pos);
        w.partition_register_object(id);
        w.partition_update();
        id
    }

    fn logged(w: &World, f: impl Fn(&CutCall) -> bool) -> usize {
        w.cuts.log.as_ref().unwrap().iter().filter(|c| f(c)).count()
    }

    fn health(w: &mut World, id: ObjectId) -> f32 {
        let body = w.get_body_module(id).unwrap();
        w.module_call(id, body, ModuleCall::GetHealth).as_real(-1.0)
    }

    /// A bridge drawn in the map: its `GenericBridge` object is made on the neutral team at the centre of the deck,
    /// turned from the 'from' side to the 'to' side, with the map properties applied (batch O1: the initial health in
    /// percent); the bridge gets the object's id and the pathfinder's first bridge layer.
    #[test]
    fn a_map_bridge_makes_its_generic_bridge_object() {
        let mut w = world();
        let info = BridgeInfo {
            from_left: Coord3D { x: 100.0, y: 110.0, z: 10.0 },
            from_right: Coord3D { x: 100.0, y: 90.0, z: 10.0 },
            to_left: Coord3D { x: 200.0, y: 110.0, z: 10.0 },
            to_right: Coord3D { x: 200.0, y: 90.0, z: 10.0 },
            from: Coord3D { x: 100.0, y: 100.0, z: 10.0 },
            to: Coord3D { x: 200.0, y: 100.0, z: 10.0 },
            ..Default::default()
        };
        let mut props = Dict::new();
        let key = WellKnownKeys::new().key(wk::OBJECT_INITIAL_HEALTH, &mut w.names);
        props.set_int(key, 40).unwrap();
        let name = AsciiString::try_from_bytes(b"WoodBridge").unwrap();
        w.terrain_add_bridge_to_logic(&info, &props, &name);

        let b = &w.terrain.bridges()[0];
        let id = b.info().bridge_object_id;
        assert_eq!(b.layer(), LAYER_GROUND + 1);
        let o = w.objects.find(id).expect("the bridge object");
        assert_eq!(o.template().get_name(), b"GenericBridge");
        assert_eq!(o.get_position(), Coord3D { x: 150.0, y: 100.0, z: 10.0 });
        assert_eq!(o.thing.orientation(), 0.0);
        assert_eq!(o.team, 101);
        assert_eq!(health(&mut w, id), 400.0);
        assert_eq!(logged(&w, |c| matches!(c, CutCall::PathfinderAddBridge { bridge_object } if *bridge_object == id)), 1);
        assert_eq!(w.cuts.terrain_lost_writes, 0);

        // the live getters find it, and the object code's deleteBridge destroys its object
        let at = Coord3D { x: 150.0, y: 100.0, z: 10.0 };
        assert_eq!(w.terrain_find_bridge_at(at), Some((info.from, info.to)));
        let details = w.terrain_find_bridge_details(at).unwrap();
        assert_eq!((details.from_left, details.template_name.as_slice(), details.layer), (info.from_left, &b"WoodBridge"[..], LAYER_GROUND + 1));
        w.terrain_delete_bridge(Some((info.from, info.to)));
        assert!(w.terrain.bridges().is_empty());
        assert!(w.objects.find(id).unwrap().is_destroyed());
        assert_eq!(logged(&w, |c| matches!(c, CutCall::PathfinderChangeBridgeState { layer, repaired: false } if *layer == LAYER_GROUND + 1)), 1);
    }

    /// A landmark bridge: the four towers are made from the road's tower template on the bridge's team, around the deck,
    /// turned outward at the 'from' end; each tower and the bridge's interface know each other; an indestructible bridge
    /// makes indestructible towers.
    #[test]
    fn a_landmark_bridge_makes_and_ties_its_towers() {
        let mut w = world();
        // (integration batch I50: the road is the world's `TheTerrainRoads`, as the live game reads it, not the tests' table)
        let roads = &mut std::sync::Arc::make_mut(&mut w.statics).terrain_roads;
        let i = roads.new_bridge(b"LandBridge");
        roads.bridges[i].tower_object_name = std::array::from_fn(|_| b"BridgeTower".to_vec());
        let bridge = make(&mut w, b"LandBridge", Some(100), Coord3D { x: 200.0, y: 200.0, z: 10.0 });
        let body = w.get_body_module(bridge).unwrap();
        w.module_call(bridge, body, ModuleCall::SetIndestructible(true));
        w.terrain_add_landmark_bridge_to_logic(bridge);

        let info = *w.terrain.bridges()[0].info();
        assert_eq!(info.bridge_object_id, bridge);
        let bbi = w.bridge_behavior_interface(bridge).unwrap();
        for (i, tower) in BridgeTowerType::ALL.into_iter().enumerate() {
            let id = info.tower_object_id[i];
            let o = w.objects.find(id).expect("a tower");
            assert_eq!(o.template().get_name(), b"BridgeTower");
            assert_eq!(o.team, 100);
            let angle = o.thing.orientation();
            let pos = o.get_position();
            match tower {
                BridgeTowerType::FromLeft | BridgeTowerType::FromRight => assert!((angle.abs() - std::f32::consts::PI).abs() < 1e-5, "{angle}"),
                _ => assert_eq!(angle, 0.0),
            }
            // the corners are 60 out along the deck and 15 across it; the towers sit 12 (their radius) further across
            let corner = [info.from_left, info.from_right, info.to_left, info.to_right][i];
            assert_eq!(pos.x, corner.x);
            assert!((pos.y - corner.y).abs() == 12.0, "{pos:?} {corner:?}");
            assert_eq!(w.module_call(bridge, bbi, ModuleCall::GetBridgeTowerId { tower: i as i32 }).as_id(), id);
            let bti = w.bridge_tower_interface(id).unwrap();
            assert_eq!(w.module_call(id, bti, ModuleCall::BridgeTowerGetBridgeId).as_id(), bridge);
            let tb = w.get_body_module(id).unwrap();
            assert!(w.module_call(id, tb, ModuleCall::IsIndestructible).as_bool());
        }
        assert_eq!(w.cuts.terrain_lost_writes, 0);
    }

    /// A bridge brought to rubble: the pathfinder's layer goes, and an object on the bridge's layer over the deck falls
    /// (huge falling damage), while one on the ground under it does not.
    #[test]
    fn a_bridge_in_rubble_drops_what_is_on_it() {
        let mut w = world();
        std::sync::Arc::make_mut(&mut w.statics).terrain_roads.new_bridge(b"LandBridge");
        let bridge = make(&mut w, b"LandBridge", Some(100), Coord3D { x: 200.0, y: 200.0, z: 10.0 });
        w.terrain_add_landmark_bridge_to_logic(bridge);
        let layer = w.terrain.bridges()[0].layer();
        let on = make(&mut w, b"Tank", Some(100), Coord3D { x: 210.0, y: 200.0, z: 20.0 });
        w.objects.find_mut(on).unwrap().layer = layer;
        let under = make(&mut w, b"Tank", Some(100), Coord3D { x: 190.0, y: 200.0, z: 10.0 });

        w.terrain_update_bridge_damage_states();
        assert_eq!(health(&mut w, on), 200.0, "nothing changed yet");

        let body = w.get_body_module(bridge).unwrap();
        w.module_call(bridge, body, ModuleCall::SetDamageState(BODY_RUBBLE));
        w.terrain_update_bridge_damage_states();
        assert_eq!(w.terrain.bridges()[0].info().cur_damage_state, BODY_RUBBLE);
        assert!(w.terrain.is_bridge_broken(Some(bridge)));
        assert_eq!(logged(&w, |c| matches!(c, CutCall::PathfinderChangeBridgeState { layer: l, repaired: false } if *l == layer)), 1);
        assert_eq!(health(&mut w, on), 0.0, "it fell");
        assert_eq!(health(&mut w, under), 200.0);

        // repaired: the layer comes back (no scaffold)
        w.module_call(bridge, body, ModuleCall::SetDamageState(0));
        w.terrain_update_bridge_damage_states();
        assert!(w.terrain.is_bridge_repaired(Some(bridge)));
        assert_eq!(logged(&w, |c| matches!(c, CutCall::PathfinderChangeBridgeState { layer: l, repaired: true } if *l == layer)), 1);
    }

    /// Water raised with damage over a lake: the objects the partition manager finds in its square and under the new
    /// water are damaged, the one outside the lake is not; the pathfinder is told to recalculate; and the object code's
    /// `isUnderwater` sees the new water.
    #[test]
    fn raised_water_damages_what_it_covers() {
        let mut w = world();
        let pts = vec![
            crate::common::base_type::ICoord3D { x: 0, y: 0, z: 0 },
            crate::common::base_type::ICoord3D { x: 100, y: 0, z: 0 },
            crate::common::base_type::ICoord3D { x: 100, y: 100, z: 0 },
            crate::common::base_type::ICoord3D { x: 0, y: 100, z: 0 },
        ];
        w.terrain.set_triggers(vec![TriggerArea::new(AsciiString::try_from_bytes(b"Lake").unwrap(), 7, true, pts).unwrap()]);
        let wet = make(&mut w, b"Tank", Some(100), Coord3D { x: 50.0, y: 50.0, z: 10.0 });
        let dry = make(&mut w, b"Tank", Some(100), Coord3D { x: 150.0, y: 50.0, z: 10.0 });
        let lake = w.terrain.water_handle(50.0, 50.0).unwrap();
        assert_eq!(w.terrain_is_underwater_water_z(50.0, 50.0), Some(0.0));
        w.terrain_set_water_height(lake, 30.0, 25.0, true).unwrap();
        assert_eq!(health(&mut w, wet), 175.0);
        assert_eq!(health(&mut w, dry), 200.0);
        assert_eq!(w.terrain_is_underwater_water_z(50.0, 50.0), Some(30.0));
        assert_eq!(logged(&w, |c| matches!(c, CutCall::PathfinderForceMapRecalculation)), 1);
        assert_eq!(w.cuts.terrain_lost_writes, 0);
    }

    /// The object code's terrain calls: from the tables while `live_terrain` is off, from the terrain when on.
    #[test]
    fn the_getters_answer_from_the_terrain_when_live() {
        let mut w = world();
        w.cuts.terrain_ground = [-7.0; 8];
        w.cuts.live_terrain = false;
        assert_eq!(w.terrain_get_ground_height(5.0, 5.0), -7.0);
        w.cuts.live_terrain = true;
        let expect = w.terrain.ground_height(5.0, 5.0, false).0;
        assert_eq!(w.terrain_get_ground_height(5.0, 5.0), expect);
        assert_eq!(w.terrain_get_layer_height(5.0, 5.0, LAYER_GROUND), expect);
        // the wall layer away from any wall is the ground (the wall height only on a wall)
        w.cuts.wall_height = 99.0;
        assert_eq!(w.terrain_get_layer_height(5.0, 5.0, LAYER_WALL), expect);
        assert_eq!(crate::logic::world::object_transform::layer_height(&w.terrain, 99.0, &w.pf_wall, 5.0, 5.0, LAYER_WALL), expect);
        assert_eq!(w.terrain_get_highest_layer_for_destination(&Coord3D { x: 5.0, y: 5.0, z: 50.0 }), LAYER_GROUND);
        assert_eq!(w.terrain_get_extent_including_border(), w.terrain.extent_including_border().unwrap());
        let edge = w.terrain_find_closest_edge_point(Coord3D { x: 5.0, y: 200.0, z: 0.0 });
        assert_eq!((edge.x, edge.y), (0.0, 200.0));
    }

    /// `setActiveBoundary`: every object leaves the partition manager and comes back into the one rebuilt on the new
    /// boundary, the radar and the camera are told, and an object outside the new boundary is off the map.
    #[test]
    fn a_new_boundary_rebuilds_the_partition_manager() {
        let mut w = world();
        let inside = make(&mut w, b"Tank", Some(100), Coord3D { x: 50.0, y: 50.0, z: 10.0 });
        let outside = make(&mut w, b"Tank", Some(100), Coord3D { x: 350.0, y: 350.0, z: 10.0 });
        let cells_before = w.partition.get_cell_count_x();
        w.terrain_set_active_boundary(1);
        assert_eq!(w.terrain.active_boundary(), 1);
        // the objects are registered again; the manager finds their cells on its next update, as upstream's
        w.partition_update();
        assert!(w.partition.get_cell_count_x() < cells_before, "{} -> {}", cells_before, w.partition.get_cell_count_x());
        assert!(!w.partition.cells_touched_by(inside).is_empty());
        assert!(!w.objects.find(inside).unwrap().is_off_map());
        assert!(w.objects.find(outside).unwrap().is_off_map());
        let events = w.events.events();
        assert!(events.iter().any(|e| matches!(e, ClientEvent::Object(ObjectClientEvent::RadarNewMap))));
        assert!(events.iter().any(|e| matches!(e, ClientEvent::Object(ObjectClientEvent::TacticalViewForceCameraConstraintRecalc))));
        assert_eq!(w.cuts.terrain_lost_writes, 0);
    }

    /// A terrain change made by object code while the terrain is lent to it is counted (upstream has one terrain).
    #[test]
    fn a_write_to_a_lent_terrain_is_counted() {
        let mut w = world();
        let t = w.terrain.clone();
        w.lend_terrain(&t, |w| w.terrain_update_bridge_damage_states());
        assert_eq!(w.cuts.terrain_lost_writes, 1);
        assert!(!w.cuts.terrain_lent);
    }
}
