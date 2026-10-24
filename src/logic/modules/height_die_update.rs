// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/HeightDieUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HeightDieUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `HeightDieUpdate`: kills its object when it falls below a height above the terrain (or above the tallest structure under it), and destroys the
//! particle systems attached to it below another height. Never sleeps.
//!
//! Terrain and partition queries are layer 12's: [`World::terrain_get_ground_height`], [`World::terrain_get_highest_layer_for_destination`],
//! [`World::terrain_get_layer_height`] and [`World::partition_iterate_structures_in_range`] record a [`CutCall`] and answer from the tables
//! `cuts.terrain_*` and `cuts.range_scan`. `getObject()->kill()` is [`World::kill`] (`DAMAGE_UNRESISTABLE`, `DEATH_NORMAL`);
//! `TheParticleSystemManager->destroyAttachedSystems` is [`World::particle_destroy_attached_systems`].
//!
//! # Upstream facts kept
//!
//! - `pos` is a pointer to the object's own position: after `setPosition(&ground)` it reads the snapped position, so the particle test and
//!   `m_lastPosition` see the position after the snap, not the one the frame began with.
//! - An object that is killed has `m_hasDied` set and the die test is not made again, but the particle test still is, once.
//! - `kill()` of an object with no body module dereferences `NULL` ([`Undefined::NullBodyModule`], recorded in `cuts.module_errors`).
//! - Version 1 saves have no `m_earliestDeathFrame`: it loads as 0 (the check `== UINT_MAX` then never sets it).
//!
//! `p1w13-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_type::{INVALID_ID, LAYER_GROUND};
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData,
};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class HeightDieUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct HeightDieUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_targetHeightAboveTerrain`: die at this height above terrain.
    pub target_height_above_terrain: f32,
    /// `m_targetHeightIncludesStructures`: target height considers terrain AND structure height underneath us.
    pub target_height_includes_structures: bool,
    /// `m_onlyWhenMovingDown`: don't detonate unless moving in downward z dir.
    pub only_when_moving_down: bool,
    /// `m_destroyAttachedParticlesAtHeight`: HACK, destroy any attached particle system of object when below this height.
    pub destroy_attached_particles_at_height: f32,
    /// `m_snapToGroundOnDeath`: snap to the ground when killed.
    pub snap_to_ground_on_death: bool,
    /// `m_initialDelay`: don't explode before this time.
    pub initial_delay: u32,
}

impl Default for HeightDieUpdateModuleData {
    /// `HeightDieUpdateModuleData()`.
    fn default() -> Self {
        HeightDieUpdateModuleData {
            update: UpdateModuleData::new(),
            target_height_above_terrain: 0.0,
            target_height_includes_structures: false,
            only_when_moving_down: false,
            destroy_attached_particles_at_height: -1.0,
            snap_to_ground_on_death: false,
            initial_delay: 0,
        }
    }
}

/// `HeightDieUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<HeightDieUpdateModuleData>; 7] = [
    FieldParse::new(
        "TargetHeight",
        |ini, d, _| {
            d.target_height_above_terrain = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "TargetHeightIncludesStructures",
        |ini, d, _| {
            d.target_height_includes_structures = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "OnlyWhenMovingDown",
        |ini, d, _| {
            d.only_when_moving_down = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "DestroyAttachedParticlesAtHeight",
        |ini, d, _| {
            d.destroy_attached_particles_at_height = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "SnapToGroundOnDeath",
        |ini, d, _| {
            d.snap_to_ground_on_death = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "InitialDelay",
        |ini, d, _| {
            d.initial_delay = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl HeightDieUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, HeightDieUpdateModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<HeightDieUpdateModuleData> {
        let mut data = HeightDieUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, HeightDieUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class HeightDieUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct HeightDieUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getHeightDieUpdateModuleData()`.
    data: Arc<HeightDieUpdateModuleData>,
    /// `m_hasDied`.
    has_died: bool,
    /// `m_particlesDestroyed`.
    particles_destroyed: bool,
    /// `m_lastPosition`.
    last_position: Coord3D,
    /// `m_earliestDeathFrame` (`UINT_MAX` until the first update).
    earliest_death_frame: u32,
}

impl PartialEq for HeightDieUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.has_died == other.has_died
            && self.particles_destroyed == other.particles_destroyed
            && self.last_position.x.to_bits() == other.last_position.x.to_bits()
            && self.last_position.y.to_bits() == other.last_position.y.to_bits()
            && self.last_position.z.to_bits() == other.last_position.z.to_bits()
            && self.earliest_death_frame == other.earliest_death_frame
    }
}

impl HeightDieUpdate {
    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the initial state.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<HeightDieUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(HeightDieUpdate {
            update,
            data,
            has_died: false,
            particles_destroyed: false,
            last_position: Coord3D {
                x: -1.0,
                y: -1.0,
                z: -1.0,
            },
            earliest_death_frame: u32::MAX,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getHeightDieUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<HeightDieUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's: the two flags, the last position (float bits) and the earliest
    /// death frame.
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            u32::from(self.has_died),
            u32::from(self.particles_destroyed),
            self.last_position.x.to_bits(),
            self.last_position.y.to_bits(),
            self.last_position.z.to_bits(),
            self.earliest_death_frame,
        ]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls (none).
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version, that half, then the fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// The body of `update()`. An error is upstream's undefined behaviour.
    fn run(&mut self, world: &mut World, me: ObjectId) -> Result<UpdateSleepTime, IniError> {
        let now = world.frame();
        let md = self.data.clone();
        if self.earliest_death_frame == u32::MAX {
            self.earliest_death_frame = if crate::logic::modules::plant(1101) {
                now
            } else {
                now.wrapping_add(md.initial_delay)
            };
        }

        // If at least a one frame delay has been set, then stop for a while
        if self.earliest_death_frame > now
            || (crate::logic::modules::plant(1102) && self.earliest_death_frame == now)
        {
            return Ok(UPDATE_SLEEP_NONE);
        }

        // do nothing if we're contained within other objects ... like a transport
        let Some(o) = world.objects.find(me) else {
            return Ok(UPDATE_SLEEP_NONE);
        };
        if o.contained_by != INVALID_ID && !crate::logic::modules::plant(1103) {
            // keep track of our last position even though we're not doing anything yet
            self.last_position = o.get_position();
            // get outta here
            return Ok(UPDATE_SLEEP_NONE);
        }

        // get our current position
        let pos = o.get_position();

        let mut direction_ok = true;
        if !self.has_died {
            if md.only_when_moving_down
                && (pos.z >= self.last_position.z
                    && !(crate::logic::modules::plant(1104) && pos.z == self.last_position.z))
            {
                direction_ok = false;
            }

            // get the terrain height
            let mut terrain_height_at_pos = world.terrain_get_ground_height(pos.x, pos.y);

            // if including structures, check for bridges
            if md.target_height_includes_structures && !crate::logic::modules::plant(1105) {
                let layer = world.terrain_get_highest_layer_for_destination(&pos);
                if layer != LAYER_GROUND {
                    let layer_height = world.terrain_get_layer_height(pos.x, pos.y, layer);
                    if layer_height > terrain_height_at_pos || crate::logic::modules::plant(1106) {
                        terrain_height_at_pos = layer_height;
                    }
                }
            }

            // our target height to die at is by default the height specified in the INI entry above the terrain ... we may change our target height if
            // we care about dying above objects under us (see below)
            let mut target_height = terrain_height_at_pos + md.target_height_above_terrain;

            // if we consider objects under us ... we will die when we are the specified distance above those objects
            if md.target_height_includes_structures {
                // scan all objects in the radius of our extent and find the tallest height among them
                let range = world
                    .objects
                    .find(me)
                    .map_or(0.0, |o| o.geometry_info.get_bounding_circle_radius());
                let found = world.partition_iterate_structures_in_range(me, range);

                let mut tallest_height = 0.0f32;
                for obj in found {
                    // ignore ourselves
                    if obj == me && !crate::logic::modules::plant(1107) {
                        continue;
                    }

                    // store the height of the tallest object under us
                    let this_height = world
                        .objects
                        .find(obj)
                        .map_or(0.0, |o| o.geometry_info.get_max_height_above_position());
                    if this_height > tallest_height {
                        tallest_height = this_height;
                    }
                }

                // our target height is either the height above the terrain as specified by the INI entry for the object that has this update ... or it is
                // the building height of the tallest thing under us
                if tallest_height > md.target_height_above_terrain {
                    target_height = if crate::logic::modules::plant(1108) {
                        tallest_height
                    } else {
                        tallest_height + terrain_height_at_pos
                    };
                }
            }

            // if we are below the target height ... DIE!
            if pos.z < target_height && direction_ok {
                // if we're supposed to snap us to the ground on death do so AND: even if we're not snapping to ground, be sure we don't go BELOW ground
                if md.snap_to_ground_on_death
                    || (pos.z < terrain_height_at_pos && !crate::logic::modules::plant(1109))
                {
                    let ground = Coord3D {
                        x: pos.x,
                        y: pos.y,
                        z: terrain_height_at_pos,
                    };
                    world.set_position(me, &ground);
                }

                // kill the object (`getBodyModule()->getMaxHealth()` of a `NULL` body)
                if world.get_body_module(me).is_none() {
                    return Err(IniError::undefined(Undefined::NullBodyModule));
                }
                world.kill(me, DAMAGE_UNRESISTABLE, DEATH_NORMAL);

                // we have died ... don't do this again
                self.has_died = !crate::logic::modules::plant(1110);
            }
        }

        // `pos` points into the object: after a snap it is the snapped position
        let Some(pos_now) = world.objects.find(me).map(|o| o.get_position()) else {
            return Ok(UPDATE_SLEEP_NONE);
        };
        let pos = if crate::logic::modules::plant(1112) {
            pos
        } else {
            pos_now
        };

        // if our height is below the destroy attached particles height above the terrain, clean them up from the particle system
        let below = if crate::logic::modules::plant(1114) {
            pos.z <= md.destroy_attached_particles_at_height
        } else {
            pos.z < md.destroy_attached_particles_at_height
        };
        if !self.particles_destroyed
            && below
            && (self.has_died || direction_ok || crate::logic::modules::plant(1111))
        {
            // destroy them
            world.particle_destroy_attached_systems(me);

            // don't do this again
            self.particles_destroyed = true;
        }

        // save our current position as the last position we monitored
        self.last_position = pos;

        Ok(UPDATE_SLEEP_NONE)
    }
}

impl UpdateBehavior for HeightDieUpdate {
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.run(world, me.object) {
            Ok(t) => t,
            Err(e) => {
                world.cuts.module_errors.push((me, e));
                UPDATE_SLEEP_NONE
            }
        }
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        // has died
        xfer.xfer_bool(&mut self.has_died)?;
        // particles destroyed
        xfer.xfer_bool(&mut self.particles_destroyed)?;
        // last position
        xfer.xfer_coord3d(&mut self.last_position)?;
        if version >= 2 {
            if !crate::logic::modules::plant(1113) {
                xfer.xfer_unsigned_int(&mut self.earliest_death_frame)?;
            }
        } else {
            self.earliest_death_frame = 0;
        }
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::HeightDieUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::HeightDieUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for HeightDieUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the fields.
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

/// The factory's `NewModuleDataProc` for `HeightDieUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::HeightDieUpdate(Arc::new(
        HeightDieUpdateModuleData::friend_new_module_data(ini)?,
    )))
}
