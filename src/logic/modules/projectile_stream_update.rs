// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ProjectileStreamUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProjectileStreamUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ProjectileStreamUpdate`: the object that draws a stream of projectiles (a flamethrower's, a toxin sprayer's) as one line. The weapon
//! tells it each projectile it fires; it keeps their ids in a circular list of 20, broken by a hole when the target changes, drops the
//! dead ones from the front, and destroys itself once its list is empty and its owner is gone. `getAllPoints` gives the draw module the
//! projectiles' positions (a hole is the origin; over a vehicle owner the stream skims its roof).
//!
//! The calls are [`ModuleCall::ProjectileStreamAddProjectile`] and [`ModuleCall::ProjectileStreamSetPosition`];
//! [`ProjectileStreamUpdate::get_all_points`] is `getAllPoints`.
//!
//! # Upstream facts kept
//!
//! - The constructor declares a local `m_projectileIDs` that shadows the member and fills the local: **the member array is never
//!   initialised** upstream, and the xfer saves all 20 slots. This port starts them at `INVALID_ID` (the slots a running stream reads are
//!   always written first; only the save shows the difference).
//! - A shot at an object breaks the stream when the object changes and clears the target position; a shot at a position breaks it when the
//!   position changes (exact `Coord3D` equality) and clears the target object; a shot at neither only records the projectile.
//! - `update` never sleeps. The xfer is version 2: the version, the `UpdateModule` base, the 20 ids as raw bytes, the next free index,
//!   the first valid index, the owner, and (version 2) the target object and position. The CRC is the base's.
//! - `getAllPoints`: over a `VEHICLE` owner, a projectile within 1.5 major radii (2D) is raised to at least the owner's top + 0.5
//!   (MSVC's `MAX`).
//!
//! # Where upstream is undefined
//!
//! An index loaded out of 0-19 used to index the list (`ArrayIndexOutOfRange`). The case is recorded in `world.cuts.module_errors` and the
//! call returns.
//!
//! `p1w13-sonnet`, batch R42, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{IniError, IniErrorKind, IniResult, Undefined};
use crate::common::kind_of::KINDOF_VEHICLE;
use crate::common::module::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `MAX_PROJECTILE_STREAM`.
pub const MAX_PROJECTILE_STREAM: usize = 20;

/// `class ProjectileStreamUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct ProjectileStreamUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `m_projectileIDs` (see the module docs: never initialised upstream).
    projectile_ids: [ObjectId; MAX_PROJECTILE_STREAM],
    /// `m_nextFreeIndex`.
    next_free_index: i32,
    /// `m_firstValidIndex`.
    first_valid_index: i32,
    /// `m_owningObject`.
    owning_object: ObjectId,
    /// `m_targetObject`.
    target_object: ObjectId,
    /// `m_targetPosition`.
    target_position: Coord3D,
}

impl PartialEq for ProjectileStreamUpdate {
    /// The same object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && self.state_words() == other.state_words()
    }
}

/// The slot of an index, or `ArrayIndexOutOfRange`.
fn slot(i: i32) -> Result<usize, Undefined> {
    usize::try_from(i).ok().filter(|&i| i < MAX_PROJECTILE_STREAM).ok_or(Undefined::ArrayIndexOutOfRange)
}

/// `(i + 1) % MAX_PROJECTILE_STREAM` (C's `%`, which keeps the sign).
fn next(i: i32) -> i32 {
    i.wrapping_add(1) % MAX_PROJECTILE_STREAM as i32
}

impl ProjectileStreamUpdate {
    /// `getInterfaceMask()`: `UpdateModule`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The constructor, which touches no world: the base (`INI_INVALID_DATA` for null data) and the state.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<UpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(data), SleepyUpdatePhase::Normal)?;
        Ok(ProjectileStreamUpdate {
            update,
            projectile_ids: [INVALID_ID; MAX_PROJECTILE_STREAM],
            next_free_index: 0,
            first_valid_index: 0,
            owning_object: INVALID_ID,
            target_object: INVALID_ID,
            target_position: Coord3D::default(),
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The state the test compares: the 20 ids, the two indices, the owner, the target and its position.
    pub fn state_words(&self) -> Vec<u32> {
        let mut w: Vec<u32> = self.projectile_ids.iter().map(|&id| id as u32).collect();
        w.extend([
            self.next_free_index as u32,
            self.first_valid_index as u32,
            self.owning_object as u32,
            self.target_object as u32,
            self.target_position.x.to_bits(),
            self.target_position.y.to_bits(),
            self.target_position.z.to_bits(),
        ]);
        w
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object (nothing).
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

    /// `addProjectile(sourceID, newID, victimID, victimPos)`.
    fn add_projectile(&mut self, source: ObjectId, new_id: ObjectId, victim: ObjectId, victim_pos: Option<Coord3D>) -> Result<(), Undefined> {
        if self.owning_object == INVALID_ID {
            self.owning_object = source;
        }
        // "Keep track of target, and insert a hole if the target has changed"
        if victim != INVALID_ID {
            // Object shot
            if victim != self.target_object {
                // "Changed targets, insert a hole to break the stream"
                if !crate::logic::modules::plant(2121) {
                    self.projectile_ids[slot(self.next_free_index)?] = INVALID_ID;
                    self.next_free_index = next(self.next_free_index);
                }
                self.target_object = victim;
            }
            // "Clear position so we know we are an object shot"
            self.target_position = Coord3D::default();
        } else if let Some(pos) = victim_pos {
            if self.target_position != pos {
                // "New position, so insert hole"
                self.projectile_ids[slot(self.next_free_index)?] = INVALID_ID;
                self.next_free_index = next(self.next_free_index);
                if !crate::logic::modules::plant(2128) {
                    self.target_position = pos;
                }
            }
            // "Clear object so we know we are a position shot"
            if !crate::logic::modules::plant(2122) {
                self.target_object = INVALID_ID;
            }
        }
        // "Keep track of the id in a circular array"
        self.projectile_ids[slot(self.next_free_index)?] = new_id;
        self.next_free_index = next(self.next_free_index);
        Ok(())
    }

    /// `cullFrontOfList()`: "chew off the front if they are gone. Don't chew on the middle, as bad ones there are just a break in the chain".
    fn cull_front_of_list(&mut self, world: &World) -> Result<(), Undefined> {
        while self.first_valid_index != self.next_free_index && world.objects.find(self.projectile_ids[slot(self.first_valid_index)?]).is_none() {
            self.first_valid_index = next(self.first_valid_index);
            if crate::logic::modules::plant(2126) {
                break;
            }
        }
        Ok(())
    }

    /// `considerDying()`: "if I have no projectiles to watch, and my master is dead, then yes, I want to die".
    fn consider_dying(&self, world: &World) -> bool {
        let empty = self.first_valid_index == self.next_free_index || crate::logic::modules::plant(2123);
        empty && (self.owning_object != INVALID_ID || crate::logic::modules::plant(2129)) && world.objects.find(self.owning_object).is_none()
    }

    /// `update()`.
    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        self.cull_front_of_list(world)?;
        if self.consider_dying(world) {
            world.body_destroy_object(me.object);
        }
        Ok(())
    }

    /// `getAllPoints(points, count)`: the positions from the first valid to the next free slot (a hole, or a projectile gone, is the
    /// origin); over a vehicle owner, a projectile near it skims its roof.
    pub fn get_all_points(&self, world: &World) -> Result<Vec<Coord3D>, Undefined> {
        let mut points = Vec::new();
        let mut index = self.first_valid_index;
        let owner = world.objects.find(self.owning_object);
        while index != self.next_free_index {
            let mut point = Coord3D::default();
            if let Some(p) = world.objects.find(self.projectile_ids[slot(index)?]) {
                point = p.get_position();
                if let Some(obj) = owner.filter(|o| o.is_kind_of(KINDOF_VEHICLE)) {
                    // "this makes the stream skim along my roof, if I have a roof"
                    let pos = obj.get_position();
                    let my_top = obj.geometry_info.get_max_height_above_position() + pos.z + 0.5;
                    let delta = Coord3D { x: pos.x - point.x, y: pos.y - point.y, z: 0.0 };
                    let reach = if crate::logic::modules::plant(2124) { 1.0 } else { 1.5 };
                    if delta.length() <= obj.geometry_info.get_major_radius() * reach {
                        // MSVC's MAX(a, b) is `((a) > (b) ? (a) : (b))`
                        point.z = if point.z > my_top && !crate::logic::modules::plant(2127) { point.z } else { my_top };
                    }
                }
            }
            points.push(point);
            index = next(index);
        }
        Ok(points)
    }

    fn undefined(world: &mut World, me: ModuleHandle, r: Result<(), Undefined>) {
        if let Err(u) = r {
            world.cuts.module_errors.push((me, IniError::undefined(u)));
        }
    }
}

impl UpdateBehavior for ProjectileStreamUpdate {
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let r = self.update_inner(world, me);
        Self::undefined(world, me, r);
        UPDATE_SLEEP_NONE
    }
    /// `xfer`'s fields after the base.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        let mut raw = [0u8; 4 * MAX_PROJECTILE_STREAM];
        for (k, id) in self.projectile_ids.iter().enumerate() {
            raw[4 * k..4 * k + 4].copy_from_slice(&(*id as u32).to_le_bytes());
        }
        xfer.xfer_user(&mut raw)?;
        for (k, id) in self.projectile_ids.iter_mut().enumerate() {
            *id = u32::from_le_bytes(raw[4 * k..4 * k + 4].try_into().expect("four bytes")) as ObjectId;
        }
        xfer.xfer_int(&mut self.next_free_index)?;
        xfer.xfer_int(&mut self.first_valid_index)?;
        xfer.xfer_object_id(&mut self.owning_object)?;
        if version >= 2 && !crate::logic::modules::plant(2125) {
            xfer.xfer_object_id(&mut self.target_object)?;
            xfer.xfer_coord3d(&mut self.target_position)?;
        }
        Ok(())
    }
    /// The calls of the weapon that fires the stream.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::ProjectileStreamAddProjectile { source, new_id, victim, victim_pos } => {
                let r = self.add_projectile(source, new_id, victim, victim_pos);
                Self::undefined(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::ProjectileStreamSetPosition(pos) => {
                // "I need to exist at the place I want to draw since only (near) on screen Drawables get updated"
                world.set_position(me.object, &pos);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::ProjectileStreamUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ProjectileStreamUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for ProjectileStreamUpdate {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_target_breaks_the_stream() {
        let mut m = ProjectileStreamUpdate::unconstructed(1, Some(Arc::new(UpdateModuleData::new()))).unwrap();
        m.add_projectile(2, 10, 5, None).unwrap();
        m.add_projectile(2, 11, 5, None).unwrap();
        let w = m.state_words();
        assert_eq!((&w[..3], w[20], w[23]), (&[INVALID_ID as u32, 10, 11][..], 3, 5), "one hole, then both shots");
        let p = Coord3D { x: 1.0, y: 2.0, z: 3.0 };
        m.add_projectile(2, 12, INVALID_ID, Some(p)).unwrap();
        m.add_projectile(2, 13, INVALID_ID, Some(p)).unwrap();
        let w = m.state_words();
        assert_eq!((w[3], w[4], w[5], w[20], w[22], w[23]), (INVALID_ID as u32, 12, 13, 6, 2, INVALID_ID as u32), "a position shot clears the target object");
    }
}
