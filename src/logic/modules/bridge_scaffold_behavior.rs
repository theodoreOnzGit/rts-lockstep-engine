// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/BridgeScaffoldBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeScaffoldBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `BridgeScaffoldBehavior`: a piece of the scaffold a bridge is repaired with. The bridge (`BridgeBehavior`, untranslated) gives it three
//! positions and a motion; it rises out of the ground, moves across to its place on the bridge and stops, or, torn down, moves back and sinks
//! and is destroyed. Each frame it moves towards its target at a speed that falls off over the last quarter of the leg (to 8% of the top
//! speed, and never below 0.001).
//!
//! The interface the bridge uses (`BridgeScaffoldBehaviorInterface`) is the module calls [`ModuleCall::ScaffoldSetPositions`],
//! `ScaffoldSetMotion`, `ScaffoldGetCurrentMotion`, `ScaffoldReverseMotion`, `ScaffoldSetLateralSpeed` and `ScaffoldSetVerticalSpeed`;
//! `getBridgeScaffoldBehaviorInterfaceFromObject` is [`World::bridge_scaffold_interface`].
//!
//! # Upstream facts kept
//!
//! - The module data is the plain `ModuleData` (`MAKE_STANDARD_MODULE_MACRO`, no data class).
//! - `setMotion(STM_STILL)` keeps the target it had; the motion is saved as the four bytes of the enum (`xferUser`), and the speeds and the
//!   three positions and the target are saved, after the `UpdateModule` base.
//! - The overshoot test is a dot product `<= 0`: a position already on the target arrives at once.
//! - A leg of length zero divides by zero (an infinite or NaN speed, clamped by comparisons a NaN fails: the NaN moves the object).
//!
//! # Where upstream is undefined
//!
//! An update in a motion outside the enum (a load of other bytes): the leg's `start` and `end` are never set ([`Undefined::UninitialisedLocal`]).
//! The case is recorded in `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, batch R35, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::ini::{IniError, IniErrorKind, IniResult, Undefined};
use crate::common::module::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `enum ScaffoldTargetMotion`: `STM_STILL`.
pub const STM_STILL: i32 = 0;
/// `STM_RISE`.
pub const STM_RISE: i32 = 1;
/// `STM_BUILD_ACROSS`.
pub const STM_BUILD_ACROSS: i32 = 2;
/// `STM_TEAR_DOWN_ACROSS`.
pub const STM_TEAR_DOWN_ACROSS: i32 = 3;
/// `STM_SINK`.
pub const STM_SINK: i32 = 4;

const ZERO: Coord3D = Coord3D { x: 0.0, y: 0.0, z: 0.0 };

/// `class BridgeScaffoldBehavior : public UpdateModule, public BridgeScaffoldBehaviorInterface`.
#[derive(Clone, Debug)]
pub struct BridgeScaffoldBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `m_targetMotion`: "which way our motion should be going".
    target_motion: i32,
    /// `m_createPos`: "initial position of object creation (in ground)".
    create_pos: Coord3D,
    /// `m_riseToPos`: "position we 'rise to' out of the ground".
    rise_to_pos: Coord3D,
    /// `m_buildPos`: "position we move to and stop at on the bridge surface".
    build_pos: Coord3D,
    /// `m_lateralSpeed`.
    lateral_speed: f32,
    /// `m_verticalSpeed`.
    vertical_speed: f32,
    /// `m_targetPos`: "current target position for our motion type".
    target_pos: Coord3D,
}

impl PartialEq for BridgeScaffoldBehavior {
    /// The same object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && self.state_words() == other.state_words()
    }
}

impl BridgeScaffoldBehavior {
    /// `getInterfaceMask()`: `UpdateModule`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The constructor, which touches no world: the base (`INI_INVALID_DATA` for null data) and the state.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<UpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(data), SleepyUpdatePhase::Normal)?;
        Ok(BridgeScaffoldBehavior {
            update,
            target_motion: STM_STILL,
            create_pos: ZERO,
            rise_to_pos: ZERO,
            build_pos: ZERO,
            lateral_speed: 1.0,
            vertical_speed: 1.0,
            target_pos: ZERO,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The state the test compares, as words: the motion, the three positions, the two speeds, the target.
    pub fn state_words(&self) -> [u32; 15] {
        let c = |p: &Coord3D| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
        let mut w = [0u32; 15];
        w[0] = self.target_motion as u32;
        w[1..4].copy_from_slice(&c(&self.create_pos));
        w[4..7].copy_from_slice(&c(&self.rise_to_pos));
        w[7..10].copy_from_slice(&c(&self.build_pos));
        w[10] = self.lateral_speed.to_bits();
        w[11] = self.vertical_speed.to_bits();
        w[12..15].copy_from_slice(&c(&self.target_pos));
        w
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half: nothing.
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

    /// `setPositions(createPos, riseToPos, buildPos)`.
    pub fn set_positions(&mut self, create_pos: Coord3D, rise_to_pos: Coord3D, build_pos: Coord3D) {
        self.create_pos = create_pos;
        self.rise_to_pos = rise_to_pos;
        self.build_pos = build_pos;
    }

    /// `setMotion(targetMotion)`: "set us moving to the right target position for the requested motion type".
    pub fn set_motion(&mut self, target_motion: i32) {
        self.target_motion = target_motion;
        match target_motion {
            STM_RISE | STM_TEAR_DOWN_ACROSS => self.target_pos = self.rise_to_pos,
            STM_BUILD_ACROSS => self.target_pos = self.build_pos,
            STM_SINK => self.target_pos = self.create_pos,
            _ => {}
        }
    }

    /// `getCurrentMotion()`.
    pub fn get_current_motion(&self) -> i32 {
        self.target_motion
    }

    /// `reverseMotion()`: "whatever our current state of motion is, reverse it".
    pub fn reverse_motion(&mut self) {
        match self.target_motion {
            STM_STILL if crate::logic::modules::plant(2059) => self.set_motion(STM_BUILD_ACROSS),
            STM_STILL | STM_BUILD_ACROSS => self.set_motion(STM_TEAR_DOWN_ACROSS),
            STM_RISE => self.set_motion(STM_SINK),
            STM_TEAR_DOWN_ACROSS => self.set_motion(STM_BUILD_ACROSS),
            STM_SINK => self.set_motion(STM_RISE),
            _ => {}
        }
    }

    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        // do nothing if we're not in motion
        if self.target_motion == STM_STILL {
            return Ok(());
        }
        let us = me.object;
        let Some(our_pos) = world.objects.find(us).map(|o| o.get_position()) else { return Ok(()) };
        // compute direction vector from our position to the target position
        let dir_v = Coord3D { x: self.target_pos.x - our_pos.x, y: self.target_pos.y - our_pos.y, z: self.target_pos.z - our_pos.z };
        // use normalized direction vector "v" to do the pulling movement
        let mut v = dir_v;
        v.normalize();
        // depending on our motion type, we move at different speeds
        let (top_speed, start, end) = match self.target_motion {
            STM_RISE => (self.vertical_speed, self.create_pos, self.rise_to_pos),
            STM_SINK => (self.vertical_speed, self.rise_to_pos, self.create_pos),
            STM_BUILD_ACROSS => (self.lateral_speed, self.rise_to_pos, self.build_pos),
            STM_TEAR_DOWN_ACROSS => (self.lateral_speed, self.build_pos, self.rise_to_pos),
            _ => return Err(Undefined::UninitialisedLocal),
        };
        // adjust speed so it's slower at the end of motion
        let total_distance = Coord3D { x: end.x - start.x, y: end.y - start.y, z: end.z - start.z }.length() * if crate::logic::modules::plant(2057) { 0.5 } else { 0.25 };
        let our_distance = Coord3D { x: end.x - our_pos.x, y: end.y - our_pos.y, z: end.z - our_pos.z }.length();
        let mut speed = (our_distance / total_distance) * top_speed;
        let min_speed = top_speed * if crate::logic::modules::plant(2056) { 0.1 } else { 0.08 };
        if speed < min_speed {
            speed = min_speed;
        }
        if speed > top_speed {
            speed = top_speed;
        }
        // make sure that speed can't get so incredibly small that we never finish our movement
        if speed < 0.001 {
            speed = 0.001;
        }
        // compute the new position given the speed
        let mut new_pos = Coord3D { x: v.x * speed + our_pos.x, y: v.y * speed + our_pos.y, z: v.z * speed + our_pos.z };
        // will this new position push us beyond our target destination?
        let too_far = Coord3D { x: self.target_pos.x - new_pos.x, y: self.target_pos.y - new_pos.y, z: self.target_pos.z - new_pos.z };
        let dot = too_far.x * dir_v.x + too_far.y * dir_v.y + too_far.z * dir_v.z;
        if dot < 0.0 || (dot == 0.0 && !crate::logic::modules::plant(2058)) {
            // use the destination position
            new_pos = self.target_pos;
            // we have reached our target position, switch motion to the next position in the chain
            match self.target_motion {
                STM_RISE => self.set_motion(STM_BUILD_ACROSS),
                STM_BUILD_ACROSS => self.set_motion(STM_STILL),
                STM_TEAR_DOWN_ACROSS => self.set_motion(STM_SINK),
                // we are done with a sinking motion, destroy the scaffold object as our job is done
                _ => world.body_destroy_object(us),
            }
        }
        // set the new position
        world.set_position(us, &new_pos);
        Ok(())
    }
}

impl World {
    /// `BridgeScaffoldBehavior::getBridgeScaffoldBehaviorInterfaceFromObject(obj)`: the first behaviour module with the interface, which only
    /// this module has.
    pub fn bridge_scaffold_interface(&self, id: ObjectId) -> Option<crate::logic::world::object::ModuleIndex> {
        self.find_module(id, b"BridgeScaffoldBehavior")
    }
}

impl UpdateBehavior for BridgeScaffoldBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(u) = self.update_inner(world, me) {
            world.cuts.module_errors.push((me, IniError::undefined(u)));
        }
        // do not sleep
        UPDATE_SLEEP_NONE
    }
    /// `xfer`'s fields after the base: the motion (four bytes), the three positions, the speeds, the target.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        let mut motion = self.target_motion.to_le_bytes();
        xfer.xfer_user(&mut motion)?;
        self.target_motion = i32::from_le_bytes(motion);
        xfer.xfer_coord3d(&mut self.create_pos)?;
        xfer.xfer_coord3d(&mut self.rise_to_pos)?;
        xfer.xfer_coord3d(&mut self.build_pos)?;
        xfer.xfer_real(&mut self.lateral_speed)?;
        xfer.xfer_real(&mut self.vertical_speed)?;
        xfer.xfer_coord3d(&mut self.target_pos)
    }
    /// `BridgeScaffoldBehaviorInterface`.
    fn other_call(&mut self, _world: &mut World, _me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::ScaffoldSetPositions { create_pos, rise_to_pos, build_pos } => {
                self.set_positions(create_pos, rise_to_pos, build_pos);
                ModuleReply::Done
            }
            ModuleCall::ScaffoldSetMotion(m) => {
                self.set_motion(m);
                ModuleReply::Done
            }
            ModuleCall::ScaffoldGetCurrentMotion => ModuleReply::Int(self.get_current_motion()),
            ModuleCall::ScaffoldReverseMotion => {
                self.reverse_motion();
                ModuleReply::Done
            }
            ModuleCall::ScaffoldSetLateralSpeed(s) => {
                self.lateral_speed = s;
                ModuleReply::Done
            }
            ModuleCall::ScaffoldSetVerticalSpeed(s) => {
                self.vertical_speed = s;
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::BridgeScaffoldBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::BridgeScaffoldBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for BridgeScaffoldBehavior {
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
    fn reverse_motion_follows_the_chain() {
        let mut m = BridgeScaffoldBehavior::unconstructed(1, Some(Arc::new(UpdateModuleData::new()))).unwrap();
        m.set_positions(Coord3D { x: 1.0, y: 0.0, z: -5.0 }, Coord3D { x: 1.0, y: 0.0, z: 0.0 }, Coord3D { x: 9.0, y: 0.0, z: 0.0 });
        m.reverse_motion();
        assert_eq!(m.get_current_motion(), STM_TEAR_DOWN_ACROSS);
        assert_eq!(m.state_words()[12], 1.0f32.to_bits());
        m.reverse_motion();
        assert_eq!(m.get_current_motion(), STM_BUILD_ACROSS);
        m.set_motion(STM_STILL);
        assert_eq!(m.state_words()[12], 9.0f32.to_bits(), "STM_STILL keeps the target");
    }
}
