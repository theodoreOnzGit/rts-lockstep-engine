// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/NeutronBlastBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/NeutonBlastBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `NeutronBlastBehavior`: when its object (a neutron shell) dies, everything alive in the blast radius loses its crew: infantry die, a
//! container's passengers die, a vehicle (not a drone) is left unmanned and neutral, or, if it is a cliff jumper (a combat bike), dies.
//! The update never runs (asleep from the constructor on).
//!
//! # Where the world comes in
//!
//! - The scan is [`World::partition_iterate_alive_same_map_in_range_at`] (`FROM_CENTER_2D`, `PartitionFilterAlive` and
//!   `PartitionFilterSameMapStatus`, which the partition manager applies: layer 12's).
//! - The kills are [`World::kill`]; the passengers are the contain module's ([`ModuleCall::KillAllContained`]); the vehicle's crew is
//!   [`World::body_set_disabled`], the AI's idle ([`ModuleCall::AiIdle`]), the deselection (`CutCall::GameLogicDeselectObject`), the terrain
//!   decal ([`World::drawable_set_terrain_decal`]) and [`World::body_set_team_to_neutral_default`], the calls `ActiveBody`'s sniping makes.
//!
//! # Upstream facts kept
//!
//! - An object is hit unless the blast ignores the air and the object is an aircraft or an airborne target.
//! - An infantry container is killed and then has its passengers killed; a vehicle that is also infantry is killed and then left unmanned.
//! - The relationship checked is the shell's to the victim (`getObject()->getRelationship(obj)`).
//! - `xfer` saves the version and the `UpdateModule` base; `crc` the base.
//!
//! `p1w13-sonnet`, batch R35, 2026-10-24.

use std::sync::Arc;

use crate::common::disabled_types::DISABLED_UNMANNED;
use crate::common::game_common::{Relationship, PLAYERMASK_ALL};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::kind_of::{KINDOF_AIRCRAFT, KINDOF_CLIFF_JUMPER, KINDOF_DRONE, KINDOF_INFANTRY, KINDOF_VEHICLE};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::object_status_types::OBJECT_STATUS_AIRBORNE_TARGET;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `FROM_CENTER_2D` (`DistanceCalculationType`).
const FROM_CENTER_2D: i32 = 0;
/// `TERRAIN_DECAL_NONE` (`TerrainDecalType`).
const TERRAIN_DECAL_NONE: i32 = 8;

/// `class NeutronBlastBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct NeutronBlastBehaviorModuleData {
    /// The base (`UpdateModuleData`, which adds nothing to `ModuleData`).
    pub update: UpdateModuleData,
    /// `m_blastRadius`.
    pub blast_radius: f32,
    /// `m_isAffectAirborne`.
    pub is_affect_airborne: bool,
    /// `m_affectAllies`.
    pub affect_allies: bool,
}

impl Default for NeutronBlastBehaviorModuleData {
    /// `NeutronBlastBehaviorModuleData()`.
    fn default() -> Self {
        NeutronBlastBehaviorModuleData { update: UpdateModuleData::new(), blast_radius: 10.0, is_affect_airborne: true, affect_allies: true }
    }
}

/// `NeutronBlastBehaviorModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<NeutronBlastBehaviorModuleData>; 4] = [
    FieldParse::new("BlastRadius", |i, d, _| { d.blast_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("AffectAirborne", |i, d, _| { d.is_affect_airborne = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("AffectAllies", |i, d, _| { d.affect_allies = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl NeutronBlastBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, NeutronBlastBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<NeutronBlastBehaviorModuleData> {
        let mut data = NeutronBlastBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, NeutronBlastBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class NeutronBlastBehavior : public UpdateModule, public DieModuleInterface`.
#[derive(Clone, Debug)]
pub struct NeutronBlastBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getNeutronBlastBehaviorModuleData()`.
    data: Arc<NeutronBlastBehaviorModuleData>,
}

impl PartialEq for NeutronBlastBehavior {
    /// The same object and the same data object (the module has no state of its own).
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl NeutronBlastBehavior {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<NeutronBlastBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(NeutronBlastBehavior { update, data })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getNeutronBlastBehaviorModuleData()`).
    pub fn module_data(&self) -> &Arc<NeutronBlastBehaviorModuleData> {
        &self.data
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

    /// `onDie(damageInfo)`: "on death, perform the Neutron Blast!!".
    fn on_die(&self, world: &mut World, me: ModuleHandle) {
        let us = me.object;
        let Some(pos) = world.objects.find(us).map(|o| o.get_position()) else { return };
        let d = self.data.clone();
        let hit_air = d.is_affect_airborne;
        for obj in world.partition_iterate_alive_same_map_in_range_at(us, pos, d.blast_radius, FROM_CENTER_2D) {
            let Some(o) = world.objects.find(obj) else { continue };
            if hit_air || (!o.is_kind_of(KINDOF_AIRCRAFT) && (!o.test_status(OBJECT_STATUS_AIRBORNE_TARGET) || crate::logic::modules::plant(2051))) {
                self.neutron_blast_to_object(world, us, obj);
            }
        }
    }

    /// `neutronBlastToObject(obj)`.
    fn neutron_blast_to_object(&self, world: &mut World, us: ObjectId, obj: ObjectId) {
        // early exit check
        if obj == us || world.objects.find(obj).is_none() {
            return;
        }
        // Check for allies and quick exit if we are not suppose to hurt our own.
        if !self.data.affect_allies && world.get_relationship(us, obj) == if crate::logic::modules::plant(2052) { Relationship::Enemies } else { Relationship::Allies } {
            return;
        }
        let is = |world: &World, k| world.objects.find(obj).is_some_and(|o| o.is_kind_of(k));
        // Kill if object is infantry
        if is(world, KINDOF_INFANTRY) {
            world.kill(obj, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
        }
        // Kill all contained if it is a container
        if let Some(contain) = world.get_contain(obj) {
            world.module_call(obj, contain, ModuleCall::KillAllContained);
        }
        // Kill pilots of vehicles
        if is(world, KINDOF_VEHICLE) && (!is(world, KINDOF_DRONE) || crate::logic::modules::plant(2053)) {
            if is(world, KINDOF_CLIFF_JUMPER) && !crate::logic::modules::plant(2054) {
                // If the vehicle is a combat bike, kill the whole thing
                world.kill(obj, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
            } else {
                // Just kill the pilot of the vehicle. Make it unmanned, so units can easily check the ability to "take control of it"
                world.body_set_disabled(obj, DISABLED_UNMANNED);
                if let Some(ai) = world.objects.find(obj).and_then(|o| o.ai) {
                    world.module_call(obj, ai, ModuleCall::AiIdle);
                }
                world.cut(CutCall::GameLogicDeselectObject { object: obj, player_mask: PLAYERMASK_ALL, affect_client: true });
                // Clear any terrain decals here
                if world.has_drawable(obj) && !crate::logic::modules::plant(2055) {
                    world.drawable_set_terrain_decal(obj, TERRAIN_DECAL_NONE);
                }
                // Convert it to the neutral team so it renders gray giving visual representation that it is unmanned.
                world.body_set_team_to_neutral_default(obj);
            }
        }
    }
}

impl UpdateBehavior for NeutronBlastBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    /// "The update callback": asleep.
    fn update(&mut self, _world: &mut World, _me: ModuleHandle) -> UpdateSleepTime {
        UPDATE_SLEEP_FOREVER
    }
    /// The constructor's world half: `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`.
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    /// The die interface: `onDie` (no die mux).
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDie(_) => {
                self.on_die(world, me);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::NeutronBlastBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::NeutronBlastBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for NeutronBlastBehavior {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version and the base.
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

/// The factory's `NewModuleDataProc` for `NeutronBlastBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::NeutronBlastBehavior(Arc::new(NeutronBlastBehaviorModuleData::friend_new_module_data(ini)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = NeutronBlastBehaviorModuleData::default();
        assert_eq!(d.blast_radius, 10.0);
        assert!(d.is_affect_airborne && d.affect_allies);
    }
}
