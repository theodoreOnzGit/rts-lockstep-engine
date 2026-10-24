// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/TensileFormationUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/TensileFormationUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TensileFormationUpdate`: a piece of a formation (an avalanche, a glacier) that sits still as a pathfinding wall until it, or a neighbour,
//! is damaged; then it slides down the terrain's slope, held to its four nearest neighbours by "tensors", toppling shrubs it runs over, for
//! 300 frames, and settles as rubble.
//!
//! # Where the world comes in
//!
//! - The links are found once, by [`World::partition_iterate_objects_in_range_of`] (1000, `FROM_BOUNDINGSPHERE_3D`) with the formation
//!   filter (`PartitionFilterTensileFormationMember`: the other object has a `TensileFormationUpdate`), applied here; the dislodgement is the
//!   same scan at 100.
//! - The wall is [`World::pathfinder_create_wall_from_footprint`] / [`World::pathfinder_remove_wall_from_footprint`] (the pathfinder is
//!   layer 11's); the slope is [`World::terrain_get_ground_height_and_normal`]; the shrub is
//!   [`World::partition_get_closest_object_at`]; the crack sound is [`World::audio_is_currently_playing`] and [`World::audio_add_event`].
//! - The drawable's model condition flags are [`ObjectClientEvent::DrawableModelCondition`], one flag a call, as upstream makes them.
//!
//! # Upstream facts kept
//!
//! - `xfer` saves the version, the `UpdateModule` base and `m_enabled` only: the links, the inertia, the life and the lowest elevation are not
//!   saved (after a load the links are found again only if `m_linksInited` was false, which it is not).
//! - The links keep the nearest objects in the order the scan met a nearer one: each nearer object pushes the others down (the first link is
//!   the nearest, the last pushed off is lost); an object no nearer than the nearest so far is not linked at all. The scan can link the
//!   object itself (the filter lets it through).
//! - `m_motionlessCounter` is set and never used; `tensorSum` and `inertiaNormal` are computed and never used (no effect, not translated).
//! - `MIN(m_lowestSlideElevation, getGroundHeight(..))` keeps the formation from climbing; `MIN` is a macro, so the ground is read once
//!   per link for the comparison and a second time when it is the smaller.
//! - The shrub is toppled with `m_inertia`, its length and `TOPPLE_OPTIONS_NO_BOUNCE`.
//!
//! # Where upstream is undefined
//!
//! An update with no body module (`getBodyModule()->getDamageState()`, [`Undefined::NullBodyModule`]); no object closest to the new position
//! (`tree->isKindOf`, [`Undefined::NullObject`]). The case is recorded in `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, batch R34, 2026-10-24.

use std::sync::Arc;

use crate::common::audio_event_name::AudioEventName;
use crate::common::base_type::{Coord3D, PI};
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KINDOF_SHRUBBERY;
use crate::common::model_state::{ModelConditionFlagType, MODELCONDITION_FREEFALL, MODELCONDITION_MOVING, MODELCONDITION_POST_COLLAPSE};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::body_module::{BodyDamageType, BODY_DAMAGED, BODY_RUBBLE};
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `FROM_CENTER_2D` (`DistanceCalculationType`).
const FROM_CENTER_2D: i32 = 0;
/// `FROM_BOUNDINGSPHERE_3D`.
const FROM_BOUNDINGSPHERE_3D: i32 = 3;
/// `TOPPLE_OPTIONS_NO_BOUNCE` ("//No Bounce").
const TOPPLE_OPTIONS_NO_BOUNCE: u32 = 1;
/// The module's class name, which `getTFU` looks for (`findUpdateModule`).
const CLASS_NAME: &[u8] = b"TensileFormationUpdate";

/// `class TensileFormationUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct TensileFormationUpdateModuleData {
    /// The base (`UpdateModuleData`, which adds nothing to `ModuleData`).
    pub update: UpdateModuleData,
    /// `m_enabled`.
    pub enabled: bool,
    /// `m_crackSound`, by name.
    pub crack_sound: AudioEventName,
}

impl Default for TensileFormationUpdateModuleData {
    /// `TensileFormationUpdateModuleData()`.
    fn default() -> Self {
        TensileFormationUpdateModuleData { update: UpdateModuleData::new(), enabled: false, crack_sound: AudioEventName::default() }
    }
}

/// `TensileFormationUpdateModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<TensileFormationUpdateModuleData>; 3] = [
    FieldParse::new("Enabled", |i, d, _| { d.enabled = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("CrackSound", |i, d, _| d.crack_sound.parse(i), UserData::None),
    FieldParse::end(),
];

impl TensileFormationUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, TensileFormationUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<TensileFormationUpdateModuleData> {
        let mut data = TensileFormationUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, TensileFormationUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `struct TensileLink`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TensileLink {
    /// `id`.
    pub id: ObjectId,
    /// `tensor`: the other's position less this one's, when linked.
    pub tensor: Coord3D,
}

/// `class TensileFormationUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct TensileFormationUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getTensileFormationUpdateModuleData()`.
    data: Arc<TensileFormationUpdateModuleData>,
    /// `m_links[4]`: "standard textile algorithm".
    links: [TensileLink; 4],
    /// `m_inertia`.
    inertia: Coord3D,
    /// `m_enabled`.
    enabled: bool,
    /// `m_linksInited`.
    links_inited: bool,
    /// `m_motionlessCounter` (set to 0, never used).
    motionless_counter: u32,
    /// `m_life`.
    life: u32,
    /// `m_lowestSlideElevation`.
    lowest_slide_elevation: f32,
    /// `m_crackSound`'s playing handle (`setPlayingHandle`; its name and object are the data's and this object).
    crack_sound_handle: u32,
}

impl PartialEq for TensileFormationUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl TensileFormationUpdate {
    /// `getInterfaceMask()`: `UpdateModule`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the state.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<TensileFormationUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(TensileFormationUpdate {
            update,
            enabled: data.enabled,
            data,
            links: [TensileLink { id: INVALID_ID, tensor: Coord3D { x: 0.0, y: 0.0, z: 0.0 } }; 4],
            inertia: Coord3D { x: 0.0, y: 0.0, z: 0.0 },
            links_inited: false,
            motionless_counter: 0,
            life: 0,
            lowest_slide_elevation: 255.0,
            crack_sound_handle: 0,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getTensileFormationUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<TensileFormationUpdateModuleData> {
        &self.data
    }

    /// `setEnabled(enabled)`: "enable/disable formation".
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// The state the test compares, as words: enabled, linked, the motionless counter, the life, the lowest elevation, the inertia, the four
    /// links (id, tensor), the crack sound's handle.
    pub fn state_words(&self) -> [u32; 25] {
        let mut w = [0u32; 25];
        w[0] = u32::from(self.enabled);
        w[1] = u32::from(self.links_inited);
        w[2] = self.motionless_counter;
        w[3] = self.life;
        w[4] = self.lowest_slide_elevation.to_bits();
        w[5] = self.inertia.x.to_bits();
        w[6] = self.inertia.y.to_bits();
        w[7] = self.inertia.z.to_bits();
        for (k, l) in self.links.iter().enumerate() {
            w[8 + 4 * k] = l.id as u32;
            w[9 + 4 * k] = l.tensor.x.to_bits();
            w[10 + 4 * k] = l.tensor.y.to_bits();
            w[11 + 4 * k] = l.tensor.z.to_bits();
        }
        w[24] = self.crack_sound_handle;
        w
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    /// `crc` with the scheduling half the slot holds: the base's.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// The members of the formation `ThePartitionManager->iterateObjectsInRange(obj, range, FROM_BOUNDINGSPHERE_3D, { tfmFilter })` meets.
    fn formation_in_range(world: &mut World, id: ObjectId, range: f32) -> Vec<ObjectId> {
        let found = world.partition_iterate_objects_in_range_of(id, range, FROM_BOUNDINGSPHERE_3D);
        // `PartitionFilterTensileFormationMember::allow`: `getTFU(objOther) != NULL`
        found.into_iter().filter(|&o| world.find_module(o, CLASS_NAME).is_some()).collect()
    }

    /// `initLinks()`: the nearest members of the formation, then a random orientation.
    fn init_links(&mut self, world: &mut World, id: ObjectId) {
        self.links_inited = true;
        let Some(my_pos) = world.objects.find(id).map(|o| o.get_position()) else { return };
        let mut closest_distance = 99999.9f32;
        for other in Self::formation_in_range(world, id, 1000.0) {
            let Some(their_pos) = world.objects.find(other).map(|o| o.get_position()) else { continue };
            let delta = Coord3D { x: their_pos.x - my_pos.x, y: their_pos.y - my_pos.y, z: their_pos.z - my_pos.z };
            let this_distance = delta.length();
            if closest_distance > this_distance || (crate::logic::modules::plant(2043) && closest_distance == this_distance) {
                closest_distance = this_distance;
                // push a new link
                for t in (1..4).rev().filter(|_| !crate::logic::modules::plant(2042)) {
                    self.links[t] = self.links[t - 1];
                }
                self.links[0] = TensileLink { id: other, tensor: delta };
            }
        }
        let angle = world.random.game_logic_random_value_real(-PI, PI);
        world.set_orientation(id, angle);
    }

    /// The body's `setDamageState(state)` (an object without a body is skipped, as `propagateDislodgement` checks).
    fn set_body_damage_state(world: &mut World, id: ObjectId, state: BodyDamageType) {
        if let Some(body) = world.get_body_module(id) {
            world.module_call(id, body, ModuleCall::SetDamageState(state));
        }
    }

    /// `propagateDislodgement(enabled)`: the members within 100 and the links are damaged (so they enable themselves).
    fn propagate_dislodgement(&self, world: &mut World, id: ObjectId) {
        for other in Self::formation_in_range(world, id, 100.0) {
            Self::set_body_damage_state(world, other, BODY_DAMAGED);
        }
        for t in 0..4 {
            let other = self.links[t].id;
            if world.objects.find(other).is_some() {
                Self::set_body_damage_state(world, other, BODY_DAMAGED);
            }
        }
    }

    fn model(world: &mut World, id: ObjectId, set: bool, flag: ModelConditionFlagType) {
        let op = if set { ModelConditionOp::Set(vec![flag]) } else { ModelConditionOp::Clear(vec![flag]) };
        world.emit(ObjectClientEvent::DrawableModelCondition { object_id: id, op });
    }

    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> Result<UpdateSleepTime, Undefined> {
        let id = me.object;
        if !self.links_inited {
            self.init_links(world, id);
        }
        if !self.enabled {
            // We are all going to sit here idle and do not much of anything until one of us gets hurt
            let body = world.get_body_module(id).ok_or(Undefined::NullBodyModule)?;
            let bdt = world.module_call(id, body, ModuleCall::GetDamageState).as_int(0);
            if bdt >= BODY_DAMAGED {
                // Then the hurt one (maybe me) will tell every other member of the formation to enable, so we will start moving
                self.set_enabled(true);
                if !crate::logic::modules::plant(2047) {
                    world.pathfinder_remove_wall_from_footprint(id);
                }
                // Alert players of disaster happening
                if crate::logic::modules::plant(2048) || !world.audio_is_currently_playing(self.crack_sound_handle) {
                    self.crack_sound_handle = world.audio_add_event(self.data.crack_sound.event_name.clone(), id);
                }
            } else {
                return Ok(update_sleep(if crate::logic::modules::plant(2044) { 31 } else { 30 }));
            }
        }
        if !world.has_drawable(id) {
            return Ok(UPDATE_SLEEP_NONE);
        }
        self.life = self.life.wrapping_add(1);
        if self.life > 300 {
            Self::model(world, id, false, MODELCONDITION_MOVING);
            Self::model(world, id, false, MODELCONDITION_FREEFALL);
            Self::model(world, id, false, MODELCONDITION_POST_COLLAPSE);
            let body = world.get_body_module(id).ok_or(Undefined::NullBodyModule)?;
            world.module_call(id, body, ModuleCall::SetDamageState(if crate::logic::modules::plant(2045) { BODY_DAMAGED } else { BODY_RUBBLE }));
            world.pathfinder_create_wall_from_footprint(id);
            return Ok(UPDATE_SLEEP_FOREVER);
        }
        if self.life % 30 == if crate::logic::modules::plant(2046) { 28 } else { 29 } {
            self.propagate_dislodgement(world, id);
        }
        // APPLY PHYSICS
        let (pos, major_radius) = match world.objects.find(id) {
            Some(o) => (o.get_position(), o.geometry_info.get_major_radius()),
            None => return Ok(UPDATE_SLEEP_NONE),
        };
        // which way does the ground slope?
        let (_, normal) = world.terrain_get_ground_height_and_normal(pos.x, pos.y);
        let mut slope = Coord3D { x: normal.x, y: normal.y, z: 0.0 };
        let steepness = 1.0 - normal.z;
        slope.scale(0.3 + steepness);
        self.inertia.add(&slope);
        let friction = if crate::logic::modules::plant(2041) { 0.96 } else { 0.95 };
        self.inertia.scale(friction);
        let mut new_pos = Coord3D { x: pos.x + self.inertia.x, y: pos.y + self.inertia.y, z: 0.0 };
        // flow down the slope, and rest on the surface here
        new_pos.z = world.terrain_get_ground_height(new_pos.x, new_pos.y);
        let tree = world.partition_get_closest_object_at(new_pos, major_radius, FROM_CENTER_2D).ok_or(Undefined::NullObject)?;
        if world.objects.find(tree).is_some_and(|o| o.is_kind_of(KINDOF_SHRUBBERY)) {
            let speed = self.inertia.length();
            world.topple(tree, Some(self.inertia), speed, TOPPLE_OPTIONS_NO_BOUNCE);
        }
        // APPLY TENSORS
        for t in 0..4 {
            let Some(their_pos) = world.objects.find(self.links[t].id).map(|o| o.get_position()) else { continue };
            let mut desired_pos = their_pos;
            desired_pos.sub(&self.links[t].tensor);
            new_pos.x = if crate::logic::modules::plant(2049) { new_pos.x * 0.92 + desired_pos.x * 0.08 } else { new_pos.x * 0.93 + desired_pos.x * 0.07 };
            new_pos.y = new_pos.y * 0.93 + desired_pos.y * 0.07;
            // `MIN(m_lowestSlideElevation, TheTerrainLogic->getGroundHeight(..))` is the macro `a < b ? a : b`: the ground is read for the
            // comparison and again when it is the answer
            let ground = world.terrain_get_ground_height(new_pos.x, new_pos.y);
            new_pos.z = if self.lowest_slide_elevation < ground { self.lowest_slide_elevation } else { world.terrain_get_ground_height(new_pos.x, new_pos.y) };
        }
        Self::model(world, id, true, MODELCONDITION_POST_COLLAPSE);
        Self::model(world, id, self.life < 200, MODELCONDITION_MOVING);
        Self::model(world, id, (pos.z - new_pos.z).abs() > 0.2 && self.life < 100, MODELCONDITION_FREEFALL);
        self.lowest_slide_elevation = new_pos.z;
        world.set_position(id, &new_pos);
        Ok(UPDATE_SLEEP_NONE)
    }
}

impl UpdateBehavior for TensileFormationUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.update_inner(world, me) {
            Ok(s) => s,
            Err(u) => {
                world.cuts.module_errors.push((me, IniError::undefined(u)));
                UPDATE_SLEEP_NONE
            }
        }
    }
    /// The constructor's world half: "temporarily treat this object as an obstacle".
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        world.pathfinder_create_wall_from_footprint(me.object);
    }
    /// `xfer`'s fields after the base: `m_enabled`.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_bool(&mut self.enabled)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::TensileFormationUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::TensileFormationUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for TensileFormationUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then `m_enabled`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        self.update = base;
        r
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `TensileFormationUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::TensileFormationUpdate(Arc::new(TensileFormationUpdateModuleData::friend_new_module_data(ini)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = TensileFormationUpdateModuleData::default();
        assert!(!d.enabled);
        assert!(d.crack_sound.event_name.is_empty());
        let m = TensileFormationUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        let w = m.state_words();
        assert_eq!(w[4], 255.0f32.to_bits());
        assert_eq!(w[8], INVALID_ID as u32);
    }
}
