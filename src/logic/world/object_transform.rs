// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp
//   (`reactToTransformChange`, `isPosDifferent`, `isAngleDifferent`,
//   `calculateHeightAboveTerrain`, the trigger-area functions,
//   `friend_prepareForMapBoundaryAdjust`, `friend_notifyOfNewMapBoundary`,
//   `calcNaturalRallyPoint`, `setLayer`, `getCarrierDeckHeight`)
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/Thing.cpp (the setters,
//   through `logic::thing::Thing`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Where an `Object` is: the `Thing` setters with `Object`'s reaction to a
//! change of transform, the height above the terrain (which accounts for the
//! bridge layer), the trigger areas the object is in, and the map boundary.

use crate::common::base_type::{fast_float2long_round, fast_float_trunc, Coord2D, Coord3D, ICoord3D, Region3D};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID, LAYER_GROUND};
use crate::common::kind_of::{
    KINDOF_IMMOBILE, KINDOF_INERT, KINDOF_INFANTRY, KINDOF_PROJECTILE, KINDOF_STICK_TO_TERRAIN_SLOPE, KINDOF_VEHICLE,
};
use crate::logic::terrain_logic::{make_align_to_normal_matrix, TerrainLogic};
use crate::logic::thing::{ThingContext, TransformChange};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object::{TriggerInfo, MAX_TRIGGER_AREA_INFOS};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `THRESH` of `isPosDifferent` and `isAngleDifferent`: tiny changes from
/// roundoff while "standing still" must not invalidate the partition data.
const POS_THRESH: f32 = 0.01;

/// `REAL_TO_INT(x)` (`BaseType.h`).
pub fn real_to_int(x: f32) -> i32 {
    fast_float2long_round(fast_float_trunc(x))
}

/// `isPosDifferent(a, b)`.
pub fn is_pos_different(a: &Coord3D, b: &Coord3D) -> bool {
    if (a.x - b.x).abs() > POS_THRESH {
        return true;
    }
    if (a.y - b.y).abs() > POS_THRESH {
        return true;
    }
    if (a.z - b.z).abs() > POS_THRESH {
        return true;
    }
    false
}

/// `isAngleDifferent(a, b)`: in radians, 0.01 is approximately half a degree.
pub fn is_angle_different(a: f32, b: f32) -> bool {
    (a - b).abs() > POS_THRESH
}

/// The `ThingContext` the setters of an `Object` get: the one value they ask
/// for that is not in the thing, computed before the call. The setters only
/// ask `alignOnTerrain` (for a `KINDOF_STICK_TO_TERRAIN_SLOPE` thing); the
/// height queries of `Thing` are not used (`Object` overrides them).
struct SetterContext {
    gravity: f32,
    aligned: Option<Matrix3D>,
}

impl ThingContext for SetterContext {
    fn ground_height(&self, _x: f32, _y: f32) -> f32 {
        0.0 // not used by the setters
    }
    fn underwater_level(&self, _x: f32, _y: f32) -> Option<f32> {
        None // not used by the setters
    }
    fn align_on_terrain(&self, _orientation: f32, _pos: &Coord3D, _stick_to_ground: bool) -> Matrix3D {
        self.aligned.unwrap_or(Matrix3D::IDENTITY)
    }
    fn gravity(&self) -> f32 {
        self.gravity
    }
}

/// `TheTerrainLogic->getLayerHeight(x, y, layer)` (clipped, no normal): the
/// ground height, or on a bridge layer the bridge's height where it is above
/// the ground. The wall layer's height is the pathfinder's (a cut point,
/// `cuts.wall_height`).
pub fn layer_height(terrain: &TerrainLogic, wall_height: f32, x: f32, y: f32, layer: PathfindLayer) -> f32 {
    let (height, _) = terrain.ground_height(x, y, false);
    if layer == LAYER_GROUND {
        return height;
    }
    let loc = Coord3D { x, y, z: height };
    if layer == crate::common::game_type::LAYER_WALL {
        return wall_height;
    }
    if let Some(bi) = terrain.find_bridge_layer_at(&loc, layer, true) {
        if let Ok((bridge_height, _)) = terrain.bridges()[bi].bridge_height(&loc) {
            if bridge_height > height {
                return bridge_height; // Don't return bridge height if it's in the ground.
            }
        }
    }
    height
}

impl World {
    /// The ground-layer part of `TheTerrainLogic->alignOnTerrain(angle, pos,
    /// stickToGround, mtx)`: the matrix that stands an object on the terrain
    /// at `pos`. (The bridge layers need the pathfinder-aware terrain adapter
    /// of layer 12; an object on a bridge is aligned on the ground below.)
    fn align_on_ground(&self, angle: f32, pos: &Coord3D, stick_to_ground: bool) -> Matrix3D {
        let (terrain_at_pos, normal) = self.terrain.ground_height(pos.x, pos.y, true);
        let terrain_normal = normal.unwrap_or_default();
        let mut mtx = make_align_to_normal_matrix(angle, pos, &terrain_normal);
        if stick_to_ground {
            mtx.set_z_translation(terrain_at_pos);
        }
        mtx
    }

    fn setter_context(&self, id: ObjectId, align: Option<(f32, Coord3D)>) -> Option<SetterContext> {
        let o = self.objects.find(id)?;
        let aligned = if o.is_kind_of(KINDOF_STICK_TO_TERRAIN_SLOPE) {
            align.map(|(angle, pos)| self.align_on_ground(angle, &pos, true))
        } else {
            None
        };
        Some(SetterContext { gravity: self.cuts.globals.gravity, aligned })
    }

    fn after_thing_change(&mut self, id: ObjectId, change: TransformChange) {
        if let Some(o) = self.objects.find_mut(id) {
            o.position = *o.thing.position();
        }
        self.react_to_transform_change(id, &change);
    }

    /// `setPosition(pos)`: a thing that sticks to the terrain slope is
    /// aligned on the terrain at its orientation instead. Then
    /// `reactToTransformChange`.
    pub fn set_position(&mut self, id: ObjectId, pos: &Coord3D) {
        let angle = match self.objects.find(id) {
            Some(o) => o.thing.orientation(),
            None => return,
        };
        let Some(ctx) = self.setter_context(id, Some((angle, *pos))) else { return };
        let o = self.objects.find_mut(id).expect("setter_context found it");
        if let Ok(change) = o.thing.set_position(&ctx, Some(pos)) {
            self.after_thing_change(id, change);
        }
    }

    /// `setPositionZ(z)`.
    pub fn set_position_z(&mut self, id: ObjectId, z: f32) {
        let Some(o) = self.objects.find(id) else { return };
        let mut pos = *o.thing.position();
        let angle = o.thing.orientation();
        pos.z = z;
        let Some(ctx) = self.setter_context(id, Some((angle, pos))) else { return };
        let o = self.objects.find_mut(id).expect("setter_context found it");
        let change = o.thing.set_position_z(&ctx, z);
        self.after_thing_change(id, change);
    }

    /// `setOrientation(angle)`.
    pub fn set_orientation(&mut self, id: ObjectId, angle: f32) {
        let Some(o) = self.objects.find(id) else { return };
        let t = o.thing.transform_matrix();
        let pos = Coord3D { x: t.get_x_translation(), y: t.get_y_translation(), z: t.get_z_translation() };
        let Some(ctx) = self.setter_context(id, Some((angle, pos))) else { return };
        let o = self.objects.find_mut(id).expect("setter_context found it");
        if let Ok(change) = o.thing.set_orientation(&ctx, angle) {
            self.after_thing_change(id, change);
        }
    }

    /// `setTransformMatrix(mx)`.
    pub fn set_transform_matrix(&mut self, id: ObjectId, mx: &Matrix3D) {
        let Some(o) = self.objects.find_mut(id) else { return };
        let change = o.thing.set_transform_matrix(mx);
        self.after_thing_change(id, change);
    }

    /// `reactToTransformChange(oldMtx, oldPos, oldAngle)`: a position that is
    /// not a number destroys the object (game logic never stores a NaN); a
    /// real change dirties the partition data and tells the container; a
    /// change of position updates the trigger areas and whether the object is
    /// off the map. (The drawable's transform is pulled by the client.)
    pub fn react_to_transform_change(&mut self, id: ObjectId, change: &TransformChange) {
        let Some(o) = self.objects.find(id) else { return };
        let pos = *o.thing.position();
        if pos.x.is_nan() || pos.y.is_nan() || pos.z.is_nan() {
            // "Object pos is nan."
            self.destroy_object(id);
        }
        let Some(o) = self.objects.find(id) else { return };
        let pos = *o.thing.position();
        let angle = o.thing.orientation();
        let pos_diff = is_pos_different(&change.old_position, &pos);
        let ang_diff = is_angle_different(change.old_angle, angle);

        if pos_diff || ang_diff {
            if o.in_partition {
                self.cut(CutCall::PartitionMakeDirty { object: id });
            }
            if let Some(contain) = self.get_contain(id) {
                self.module_call(id, contain, ModuleCall::ContainReactToTransformChange);
            }
        }

        if pos_diff {
            self.set_trigger_area_flags_for_change_in_position(id); // Update for entered/exited
            if let Ok(map_extent) = self.terrain.extent() {
                let off_map = !map_extent.is_in_region_no_z(&pos);
                self.set_off_map(id, off_map);
            }
        }
    }

    /// `calculateHeightAboveTerrain()`: the actual height above the terrain
    /// of the object's layer. Doesn't use the cache.
    pub fn calculate_height_above_terrain(&self, id: ObjectId) -> f32 {
        let Some(o) = self.objects.find(id) else { return 0.0 };
        let pos = o.thing.position();
        let terrain_z = layer_height(&self.terrain, self.cuts.wall_height, pos.x, pos.y, o.layer);
        pos.z - terrain_z
    }

    /// `getHeightAboveTerrain()`: cached, with the layer-aware calculation.
    pub fn get_height_above_terrain(&mut self, id: ObjectId) -> f32 {
        let wall = self.cuts.wall_height;
        let terrain = &self.terrain;
        let Some(o) = self.objects.find_mut(id) else { return 0.0 };
        let layer = o.layer;
        o.thing.height_above_terrain_by(|t| {
            let pos = t.position();
            pos.z - layer_height(terrain, wall, pos.x, pos.y, layer)
        })
    }

    // ---- trigger areas -----------------------------------------------------------------------------

    /// `updateTriggerAreaFlags()`: "clear the previous entered/exited flags":
    /// update the flags, and remove any trigger areas that this object isn't
    /// inside. (Kept as upstream wrote it: the test reads `m_triggerInfo[j]`,
    /// the slot being filled, and not `m_triggerInfo[i]`.)
    fn update_trigger_area_flags(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find_mut(id) else { return };
        let mut j = 0usize;
        for i in 0..o.num_trigger_areas_active.max(0) as usize {
            if !o.trigger_info[j].is_inside {
                continue;
            }
            o.trigger_info[j].entered = false;
            o.trigger_info[j].exited = false;
            o.trigger_info[j].is_inside = o.trigger_info[i].is_inside;
            o.trigger_info[j].trigger = o.trigger_info[i].trigger;
            j += 1;
        }
        o.num_trigger_areas_active = j as i8;
    }

    /// `setTriggerAreaFlagsForChangeInPosition()`: checks the object against
    /// the trigger areas when its integer position changes. Not for
    /// projectiles and inert objects.
    ///
    /// Upstream tests an area for "exited" with `m_iPos`, the position from
    /// the *previous* call, and only then stores the new one (kept).
    pub(crate) fn set_trigger_area_flags_for_change_in_position(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find(id) else { return };
        // projectiles cannot trigger areas. neither can inert objects, like the radar ping, etc.
        if o.is_kind_of(KINDOF_PROJECTILE) || o.is_kind_of(KINDOF_INERT) {
            return;
        }
        let pos = *o.thing.position();
        // REAL_TO_INT: `fast_float2long_round(fast_float_trunc(x))`
        let i_pos = ICoord3D { x: real_to_int(pos.x), y: real_to_int(pos.y), z: 0 }; // Trigger areas compare on xy only.
        if o.i_pos.x == i_pos.x && o.i_pos.y == i_pos.y {
            return; // didn't move enough to change integer position.
        }
        let moved_terrain_object = !o.is_kind_of(KINDOF_IMMOBILE) && (o.is_kind_of(KINDOF_INFANTRY) || o.is_kind_of(KINDOF_VEHICLE));
        let has_ai = o.ai.is_some();
        if moved_terrain_object {
            self.emit(ObjectClientEvent::TerrainObjectMoved { object_id: id });
        }
        if has_ai {
            self.cut(CutCall::PathfinderUpdatePos { object: id });
        }
        let now = self.frame();
        let o = self.objects.find(id).expect("checked above");
        if o.entered_or_exited_frame != 0 && o.entered_or_exited_frame != now {
            self.update_trigger_area_flags(id);
        }

        // Check for exited.
        let team = self.get_team(id);
        let active = self.objects.find(id).expect("checked above").num_trigger_areas_active.max(0) as usize;
        for i in 0..active {
            let o = self.objects.find(id).expect("checked above");
            let Some(trigger_id) = o.trigger_info[i].trigger else { continue };
            let old_i_pos = o.i_pos;
            let inside = self.terrain.triggers().iter().find(|t| t.id() == trigger_id).is_some_and(|t| t.point_in_trigger(&old_i_pos));
            if !inside {
                let o = self.objects.find_mut(id).expect("checked above");
                o.trigger_info[i].is_inside = false;
                o.trigger_info[i].exited = true;
                o.entered_or_exited_frame = now;
                if let Some(team) = team {
                    self.cut(CutCall::TeamSetEnteredExited { team });
                }
                self.cut(CutCall::GameLogicUpdateObjectsChangedTriggerAreas);
            }
        }

        if let Some(o) = self.objects.find_mut(id) {
            o.i_pos = i_pos;
        }

        let trigger_ids: Vec<(i32, bool)> = self.terrain.triggers().iter().map(|t| (t.id(), t.point_in_trigger(&i_pos))).collect();
        for (trig_id, point_in) in trigger_ids {
            let o = self.objects.find(id).expect("checked above");
            let mut skip = false;
            for i in 0..o.num_trigger_areas_active.max(0) as usize {
                if o.trigger_info[i].trigger == Some(trig_id) {
                    // Already handled this one in the check for exited above.
                    skip = true;
                    break;
                }
            }
            if skip {
                continue;
            }
            if point_in {
                let n = o.num_trigger_areas_active.max(0) as usize;
                if n < MAX_TRIGGER_AREA_INFOS {
                    let o = self.objects.find_mut(id).expect("checked above");
                    o.trigger_info[n] = TriggerInfo { trigger: Some(trig_id), entered: true, exited: false, is_inside: true };
                    o.entered_or_exited_frame = now;
                    if let Some(team) = team {
                        self.cut(CutCall::TeamSetEnteredExited { team });
                    }
                    self.cut(CutCall::GameLogicUpdateObjectsChangedTriggerAreas);
                    let o = self.objects.find_mut(id).expect("checked above");
                    o.num_trigger_areas_active += 1;
                } else {
                    // Shouldn't happen: "Too many nested trigger areas."
                    self.cut(CutCall::ScriptEngineAppendDebugMessage { too_many_trigger_areas: true });
                }
            }
        }
    }

    /// `didEnterOrExit()`: an object that is not inert entered or exited on
    /// this frame or the last ("the current execution order is ScriptEngine,
    /// then ObjectUpdates, enter/exits detected in ObjectUpdate on frame N
    /// won't be noticed by the ScriptEngine till frame N+1").
    fn did_enter_or_exit(&self, id: ObjectId) -> bool {
        let Some(o) = self.objects.find(id) else { return false };
        if o.is_kind_of(KINDOF_INERT) {
            return false;
        }
        let now = self.frame();
        o.entered_or_exited_frame == now || o.entered_or_exited_frame == now.wrapping_sub(1)
    }

    /// `didEnter(trigger)`: the trigger area by id.
    pub fn did_enter(&self, id: ObjectId, trigger: Option<i32>) -> bool {
        if !self.did_enter_or_exit(id) {
            return false;
        }
        let o = self.objects.find(id).expect("did_enter_or_exit found it");
        (0..o.num_trigger_areas_active.max(0) as usize).any(|i| o.trigger_info[i].entered && o.trigger_info[i].trigger == trigger)
    }

    /// `didExit(trigger)`.
    pub fn did_exit(&self, id: ObjectId, trigger: Option<i32>) -> bool {
        if !self.did_enter_or_exit(id) {
            return false;
        }
        let o = self.objects.find(id).expect("did_enter_or_exit found it");
        (0..o.num_trigger_areas_active.max(0) as usize).any(|i| o.trigger_info[i].exited && o.trigger_info[i].trigger == trigger)
    }

    /// `isInside(trigger)`.
    pub fn is_inside(&self, id: ObjectId, trigger: Option<i32>) -> bool {
        let Some(o) = self.objects.find(id) else { return false };
        (0..o.num_trigger_areas_active.max(0) as usize).any(|i| o.trigger_info[i].is_inside && o.trigger_info[i].trigger == trigger)
    }

    // ---- the map boundary --------------------------------------------------------------------------------

    /// `friend_prepareForMapBoundaryAdjust()`: out of the radar and the
    /// partition manager, and forget everything the object thinks it looked
    /// at, shrouded, or added to the maps ("the whole PartitionManager and
    /// all of the Looker data is about to be blown away").
    pub fn friend_prepare_for_map_boundary_adjust(&mut self, id: ObjectId) {
        self.radar_remove_object(id);
        self.partition_unregister_object(id);
        if let Some(o) = self.objects.find_mut(id) {
            o.partition_last_look.reset();
            o.partition_reveal_all_last_look.reset();
            o.partition_last_shroud.reset();
            o.partition_last_threat.reset();
            o.partition_last_value.reset();
        }
    }

    /// `friend_notifyOfNewMapBoundary()`: back into the partition manager,
    /// the radar and the pathfinder's map; relook; and whether the object is
    /// off the new map.
    pub fn friend_notify_of_new_map_boundary(&mut self, id: ObjectId) {
        self.partition_register_object(id);
        self.radar_add_object(id);
        self.cut(CutCall::PathfinderAddObjectToPathfindMap { object: id });
        // Now that the PartitionManager has finished its reset, we need to relook
        self.handle_partition_cell_maintenance(id);
        let Some(o) = self.objects.find(id) else { return };
        let pos = *o.thing.position();
        if let Ok(map_extent) = self.terrain.extent() {
            let r: Region3D = map_extent;
            self.set_off_map(id, !r.is_in_region_no_z(&pos));
        }
    }

    // ---- layers and odds and ends ---------------------------------------------------------------------------

    /// `setLayer(layer)`: the pathfinder forgets the old layer's position and
    /// learns the new one.
    pub fn set_layer(&mut self, id: ObjectId, layer: PathfindLayer) {
        let Some(o) = self.objects.find(id) else { return };
        if layer != o.layer {
            self.cut(CutCall::PathfinderRemovePos { object: id });
            if let Some(o) = self.objects.find_mut(id) {
                o.layer = layer;
            }
            self.cut(CutCall::PathfinderUpdatePos { object: id });
        }
    }

    /// `setDestinationLayer(layer)`.
    pub fn set_destination_layer(&mut self, id: ObjectId, layer: PathfindLayer) {
        if let Some(o) = self.objects.find_mut(id) {
            if layer != o.destination_layer {
                o.destination_layer = layer;
            }
        }
    }

    /// `calcNaturalRallyPoint(pt)`: the object's origin through its transform
    /// (the template's natural rally point is commented out upstream), in 2D.
    pub fn calc_natural_rally_point(&self, id: ObjectId) -> Option<Coord2D> {
        let o = self.objects.find(id)?;
        let v = Vector3::new(0.0, 0.0, 0.0);
        let out = Matrix3D::transform_vector(o.thing.transform_matrix(), &v);
        Some(Coord2D { x: out.x, y: out.y })
    }

    /// `getCarrierDeckHeight()`: the landing deck height of the producer's
    /// parking place, else 0.
    pub fn get_carrier_deck_height(&mut self, id: ObjectId) -> f32 {
        let producer = self.objects.find(id).map_or(INVALID_ID, |o| o.producer_id);
        if self.objects.find(producer).is_some() {
            // Find a parking place behavior.
            for idx in self.all_modules(producer) {
                if self.module_call(producer, idx, ModuleCall::HasParkingPlaceInterface).as_bool() {
                    return self.module_call(producer, idx, ModuleCall::GetLandingDeckHeightOffset).as_real(0.0);
                }
            }
        }
        0.0
    }
}
