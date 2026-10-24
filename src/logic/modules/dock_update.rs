// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DockUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/DockUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DockUpdate`: the abstract base of the docks (repair pads, supply centres and warehouses, the railed transport's dock). It keeps the
//! queue of dockers waiting at its approach positions, lets one at a time in (the active docker), and gives the dockers the positions of its
//! dock path (the art's `DockStart`, `DockAction`, `DockEnd` and `DockWaitingNN` bones, read once from the drawable). A derived dock adds
//! `action()`: [`RepairDockUpdate`](crate::logic::modules::repair_dock_update) is the first.
//!
//! [`DockUpdate`] is the state and the methods; a derived module owns one and hands it the [`ModuleCall`]s of `DockUpdateInterface`
//! ([`DockUpdate::dock_call`]). The data is [`DockUpdateModuleData`], whose fields a derived data adds to its own table with
//! [`dock_update_field_parse!`](crate::dock_update_field_parse).
//!
//! # Upstream facts kept
//!
//! - `getDockPosition` and `getExitPosition` test whether the **enter** position is zero, not their own (a building whose art has a
//!   `DockStart` bone but no `DockAction` gives the bone's local origin pushed through the transform).
//! - The xfer saves the "reached" flags but **never loads them**: `Bool unpack = m_approachPositionReached[i]; xfer->xferBool(&unpack);`.
//!   After a load the flags are the module's own (resized, new ones false).
//! - The xfer does not save `m_numberApproachPositionBones`; a load keeps the live value.
//! - With no approach bones (`m_numberApproachPositionBones == 0`, a dynamic dock or an art without them) an approach position is biased
//!   half a major radius towards the docker (WWMath's `Vector3::Normalize`, which leaves a zero vector alone).
//! - `update` never sleeps; it lets the first docker who has reached its approach position in, unless the dock is crippled.
//!
//! # Where upstream is undefined
//!
//! Recorded in `world.cuts.module_errors`; the call returns:
//! - the bones read into an approach list longer than the art's `DockWaiting` bones (the rest of the local array is uninitialised:
//!   `UninitialisedLocal`), or longer than the local array of 10 (`ArrayIndexOutOfRange`);
//! - an index outside the approach list (`advanceApproachPosition`, `isClearToAdvance`: `ArrayIndexOutOfRange`);
//! - a docker that is no object (`NullObject`), and a negative approach count other than -1 (the constructor's `resize`:
//!   `ArrayIndexOutOfRange`, reported by [`DockUpdate::new`]).
//!
//! `p1w13-sonnet`, batch R43, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{IniResult, Undefined};
use crate::common::kind_of::{KINDOF_DOZER, KINDOF_HARVESTER, KINDOF_IGNORE_DOCKING_BONES, KINDOF_SUPPLY_SOURCE};
use crate::common::model_state::{
    MODELCONDITION_DOCKING, MODELCONDITION_DOCKING_ACTIVE, MODELCONDITION_DOCKING_BEGINNING, MODELCONDITION_DOCKING_ENDING, MODELCONDITION_MOVING,
};
use crate::common::module::ObjectId;
use crate::common::xfer::{Xfer, XferResult};
use crate::logic::module::update_module::{UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::ww::vector3::Vector3;

/// `DEFAULT_APPROACH_VECTOR_SIZE`.
pub const DEFAULT_APPROACH_VECTOR_SIZE: i32 = 10;
/// `DYNAMIC_APPROACH_VECTOR_FLAG`: "A positive number is an absolute, DYNAMIC_APPROACH_VECTOR_FLAG means dynamic vector".
pub const DYNAMIC_APPROACH_VECTOR_FLAG: i32 = -1;

/// `class DockUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct DockUpdateModuleData {
    /// The base.
    pub update: UpdateModuleData,
    /// `m_numberApproachPositionsData`.
    pub number_approach_positions_data: i32,
    /// `m_isAllowPassthrough`.
    pub is_allow_passthrough: bool,
}

impl Default for DockUpdateModuleData {
    /// `DockUpdateModuleData()`.
    fn default() -> Self {
        DockUpdateModuleData { update: UpdateModuleData::new(), number_approach_positions_data: 0, is_allow_passthrough: true }
    }
}

/// `DockUpdateModuleData::buildFieldParse`'s two fields, as `FieldParse` entries of a derived data type whose `DockUpdateModuleData` is the
/// field `$dock` (a derived data adds them after `UpdateModuleData::buildFieldParse` and before its own).
#[macro_export]
macro_rules! dock_update_field_parse {
    ($ty:ty, $dock:ident) => {
        [
            $crate::common::ini::FieldParse::<$ty>::new(
                "NumberApproachPositions",
                |i, d, _| {
                    d.$dock.number_approach_positions_data = i.parse_int()?;
                    Ok(())
                },
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::<$ty>::new(
                "AllowsPassthrough",
                |i, d, _| {
                    d.$dock.is_allow_passthrough = i.parse_bool()?;
                    Ok(())
                },
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::<$ty>::end(),
        ]
    };
}

/// `class DockUpdate : public UpdateModule, public DockUpdateInterface`, without the `UpdateModule` base (the derived module's).
#[derive(Clone, Debug, PartialEq)]
pub struct DockUpdate {
    /// The data.
    data: Arc<DockUpdateModuleData>,
    /// `m_enterPosition`, `m_dockPosition`, `m_exitPosition`: the bones, in the object's space.
    enter_position: Coord3D,
    dock_position: Coord3D,
    exit_position: Coord3D,
    /// `m_numberApproachPositions`.
    number_approach_positions: i32,
    /// `m_numberApproachPositionBones`.
    number_approach_position_bones: i32,
    /// `m_positionsLoaded`.
    positions_loaded: bool,
    /// `m_approachPositions`, `m_approachPositionOwners`, `m_approachPositionReached`.
    approach_positions: Vec<Coord3D>,
    approach_position_owners: Vec<ObjectId>,
    approach_position_reached: Vec<bool>,
    /// `m_activeDocker`.
    active_docker: ObjectId,
    /// `m_dockerInside`.
    docker_inside: bool,
    /// `m_dockCrippled`.
    dock_crippled: bool,
    /// `m_dockOpen`.
    dock_open: bool,
}

const ZERO: Coord3D = Coord3D { x: 0.0, y: 0.0, z: 0.0 };

/// What `reserveApproachPosition` and `advanceApproachPosition` give back: the answer, and the position and index (the caller's, unchanged,
/// when not written).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ApproachAnswer {
    pub ok: bool,
    pub position: Option<Coord3D>,
    pub index: i32,
}

impl DockUpdate {
    /// The constructor's state; `ArrayIndexOutOfRange` for a negative approach count other than the dynamic flag (`resize` of a huge size).
    pub fn new(data: Arc<DockUpdateModuleData>) -> Result<DockUpdate, Undefined> {
        let number = data.number_approach_positions_data;
        let size = if number != DYNAMIC_APPROACH_VECTOR_FLAG {
            // "Not dynamic, so make this the size"
            usize::try_from(number).map_err(|_| Undefined::ArrayIndexOutOfRange)?
        } else {
            // "Otherwise, make a default size, and plan on growing it later"
            DEFAULT_APPROACH_VECTOR_SIZE as usize
        };
        Ok(DockUpdate {
            data,
            enter_position: ZERO,
            dock_position: ZERO,
            exit_position: ZERO,
            number_approach_positions: number,
            number_approach_position_bones: -1,
            positions_loaded: false,
            approach_positions: vec![ZERO; size],
            approach_position_owners: vec![INVALID_ID; size],
            approach_position_reached: vec![false; size],
            active_docker: INVALID_ID,
            docker_inside: false,
            dock_crippled: false,
            dock_open: true,
        })
    }

    /// The state the test compares.
    pub fn state_words(&self) -> Vec<u32> {
        let c = |p: &Coord3D| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
        let mut w = Vec::new();
        for p in [&self.enter_position, &self.dock_position, &self.exit_position] {
            w.extend(c(p));
        }
        w.extend([
            self.number_approach_positions as u32,
            self.number_approach_position_bones as u32,
            u32::from(self.positions_loaded),
            self.active_docker as u32,
            u32::from(self.docker_inside),
            u32::from(self.dock_crippled),
            u32::from(self.dock_open),
            self.approach_positions.len() as u32,
        ]);
        for k in 0..self.approach_positions.len() {
            w.extend(c(&self.approach_positions[k]));
            w.push(self.approach_position_owners.get(k).map_or(u32::MAX, |&o| o as u32));
            w.push(self.approach_position_reached.get(k).map_or(u32::MAX, |&r| u32::from(r)));
        }
        w
    }

    /// `m_activeDocker`.
    pub fn active_docker(&self) -> ObjectId {
        self.active_docker
    }

    /// `m_dockerInside`.
    pub fn docker_inside(&self) -> bool {
        self.docker_inside
    }

    /// `isDockOpen()`.
    pub fn is_dock_open(&self) -> bool {
        self.dock_open
    }

    fn docker_exists(world: &World, docker: ObjectId) -> Result<(), Undefined> {
        world.objects.find(docker).map(|_| ()).ok_or(Undefined::NullObject)
    }

    /// `isClearToApproach(docker)`.
    pub fn is_clear_to_approach(&self, world: &World, docker: ObjectId) -> Result<bool, Undefined> {
        // "If we allow infinite approaches, we don't even need to look up.  Just say yes."
        if self.number_approach_positions == DYNAMIC_APPROACH_VECTOR_FLAG {
            return Ok(true);
        }
        Self::docker_exists(world, docker)?;
        Ok(self.approach_position_owners.iter().any(|&o| o == INVALID_ID || o == docker))
    }

    /// `reserveApproachPosition(docker, position, index)`.
    pub fn reserve_approach_position(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        docker: ObjectId,
        position: Option<Coord3D>,
        index: i32,
    ) -> Result<ApproachAnswer, Undefined> {
        let no = ApproachAnswer { ok: false, position, index };
        // load dock positions if not loaded yet
        if !self.positions_loaded {
            self.load_dock_positions(world, me)?;
        }
        // sanity
        if position.is_none() {
            return Ok(no);
        }
        Self::docker_exists(world, docker)?;
        for k in 0..self.approach_position_owners.len() {
            if self.approach_position_owners[k] == docker {
                let p = self.compute_approach_position(world, me, k, docker)?;
                return Ok(ApproachAnswer { ok: true, position: Some(p), index: k as i32 });
            }
            if self.approach_position_owners[k] == INVALID_ID {
                self.approach_position_owners[k] = docker;
                let p = self.compute_approach_position(world, me, k, docker)?;
                return Ok(ApproachAnswer { ok: true, position: Some(p), index: k as i32 });
            }
        }
        // "If I make it out of the loop, I am full, so dynamic approach buildings should make a new entry instead of saying no"
        if self.number_approach_positions == DYNAMIC_APPROACH_VECTOR_FLAG && !crate::logic::modules::plant(2131) {
            self.approach_positions.push(ZERO);
            self.approach_position_owners.push(INVALID_ID);
            self.approach_position_reached.push(false);
            self.load_dock_positions(world, me)?; // "refresh this new one"
            let k = self.approach_position_owners.len() - 1; // "The new last spot"
            self.approach_position_owners[k] = docker;
            let p = self.compute_approach_position(world, me, k, docker)?;
            return Ok(ApproachAnswer { ok: true, position: Some(p), index: k as i32 });
        }
        Ok(no)
    }

    /// `advanceApproachPosition(docker, position, index)`.
    pub fn advance_approach_position(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        docker: ObjectId,
        position: Option<Coord3D>,
        index: i32,
    ) -> Result<ApproachAnswer, Undefined> {
        let no = ApproachAnswer { ok: false, position, index };
        if !self.positions_loaded {
            self.load_dock_positions(world, me)?;
        }
        if position.is_none() || index <= 0 {
            return Ok(no);
        }
        let his = index as usize;
        let previous = his - 1;
        if previous >= self.approach_position_owners.len() {
            return Err(Undefined::ArrayIndexOutOfRange);
        }
        if self.approach_position_owners[previous] != INVALID_ID {
            return Ok(no);
        }
        Self::docker_exists(world, docker)?;
        if his >= self.approach_position_owners.len() || his >= self.approach_position_reached.len() {
            return Err(Undefined::ArrayIndexOutOfRange);
        }
        self.approach_position_owners[previous] = docker;
        self.approach_position_reached[previous] = false;
        if !crate::logic::modules::plant(2132) {
            self.approach_position_owners[his] = INVALID_ID;
        }
        self.approach_position_reached[his] = false;
        let p = self.compute_approach_position(world, me, previous, docker)?;
        Ok(ApproachAnswer { ok: true, position: Some(p), index: previous as i32 })
    }

    /// `isClearToEnter(docker)`.
    pub fn is_clear_to_enter(&self, world: &World, docker: ObjectId) -> Result<bool, Undefined> {
        Self::docker_exists(world, docker)?;
        Ok(docker == self.active_docker)
    }

    /// `isClearToAdvance(docker, dockerIndex)`.
    pub fn is_clear_to_advance(&self, world: &World, docker: ObjectId, index: i32) -> Result<bool, Undefined> {
        if index < 0 {
            return Ok(false);
        }
        Self::docker_exists(world, docker)?;
        let i = index as usize;
        let (Some(&owner), Some(&reached)) = (self.approach_position_owners.get(i), self.approach_position_reached.get(i)) else {
            return Err(Undefined::ArrayIndexOutOfRange);
        };
        let correct_request = docker == owner;
        let next_spot_free = i > 0 && self.approach_position_owners.get(i - 1).ok_or(Undefined::ArrayIndexOutOfRange)? == &INVALID_ID;
        Ok(correct_request && reached && next_spot_free)
    }

    /// `getEnterPosition(docker, position)`, `getDockPosition` (`which` 1) and `getExitPosition` (2): `None` for a `NULL` position.
    pub fn get_path_position(&mut self, world: &mut World, me: ModuleHandle, docker: ObjectId, position: bool, which: u32) -> Result<Option<Coord3D>, Undefined> {
        if !self.positions_loaded {
            self.load_dock_positions(world, me)?;
        }
        // sanity
        if !position {
            return Ok(None);
        }
        // "If I don't have a bone, you are fine where you are" (all three test the enter position)
        if self.enter_position == ZERO {
            let docker_pos = world.objects.find(docker).ok_or(Undefined::NullObject)?.get_position();
            // "unless you fly, in which case I should recenter you" (the enter position only)
            if which == 0 && world.is_using_airborne_locomotor(docker) && !crate::logic::modules::plant(2133) {
                return Ok(world.objects.find(me.object).map(|o| o.get_position()));
            }
            return Ok(Some(docker_pos));
        }
        let bone = match which {
            0 => self.enter_position,
            1 => self.dock_position,
            _ => self.exit_position,
        };
        // take local space position and convert to world space
        Ok(Some(world.contain_convert_bone_pos_to_world_pos(me.object, &bone)))
    }

    /// `onApproachReached(docker)`.
    pub fn on_approach_reached(&mut self, world: &World, docker: ObjectId) -> Result<(), Undefined> {
        Self::docker_exists(world, docker)?;
        if let Some(k) = self.approach_position_owners.iter().position(|&o| o == docker) {
            self.approach_position_reached[k] = true;
        }
        Ok(())
    }

    /// `onEnterReached(docker)`.
    pub fn on_enter_reached(&mut self, world: &mut World, me: ModuleHandle, docker: ObjectId) -> Result<(), Undefined> {
        Self::docker_exists(world, docker)?;
        let (clear, set) = ([MODELCONDITION_DOCKING_ENDING], [MODELCONDITION_DOCKING_BEGINNING, MODELCONDITION_DOCKING]);
        world.clear_and_set_model_condition_flags(me.object, &clear, &set);
        world.clear_and_set_model_condition_flags(docker, &clear, &set);
        self.docker_inside = true;
        if let Some(k) = self.approach_position_owners.iter().position(|&o| o == docker) {
            self.approach_position_owners[k] = INVALID_ID;
            self.approach_position_reached[k] = false;
        }
        Ok(())
    }

    /// `onDockReached(docker)`.
    pub fn on_dock_reached(&mut self, world: &mut World, me: ModuleHandle, docker: ObjectId) -> Result<(), Undefined> {
        Self::docker_exists(world, docker)?;
        world.clear_and_set_model_condition_state(me.object, MODELCONDITION_DOCKING_BEGINNING, MODELCONDITION_DOCKING_ACTIVE);
        world.clear_and_set_model_condition_state(docker, MODELCONDITION_DOCKING_BEGINNING, MODELCONDITION_DOCKING_ACTIVE);
        Ok(())
    }

    /// `onExitReached(docker)`.
    pub fn on_exit_reached(&mut self, world: &mut World, me: ModuleHandle, docker: ObjectId) -> Result<(), Undefined> {
        Self::docker_exists(world, docker)?;
        let (clear, set) = ([MODELCONDITION_DOCKING_ACTIVE, MODELCONDITION_DOCKING], [MODELCONDITION_DOCKING_ENDING]);
        world.clear_and_set_model_condition_flags(me.object, &clear, &set);
        world.clear_and_set_model_condition_flags(docker, &clear, &set);
        if !crate::logic::modules::plant(2134) {
            self.docker_inside = false;
        }
        if docker == self.active_docker {
            self.active_docker = INVALID_ID;
        }
        Ok(())
    }

    /// `cancelDock(docker)`: "Clear me from any reserved points, and if I was the reason you were Busy, you aren't anymore."
    pub fn cancel_dock(&mut self, world: &mut World, me: ModuleHandle, docker: ObjectId) -> Result<(), Undefined> {
        Self::docker_exists(world, docker)?;
        for k in 0..self.approach_position_owners.len() {
            if self.approach_position_owners[k] == docker {
                self.approach_position_owners[k] = INVALID_ID;
                self.approach_position_reached[k] = false;
            }
        }
        if self.active_docker == docker {
            let docking_object = world.objects.find(self.active_docker).is_some();
            self.active_docker = INVALID_ID;
            self.docker_inside = false;
            // "clear any model conditions related to docking that may be set on us and them"
            let clear = [MODELCONDITION_DOCKING_ENDING, MODELCONDITION_DOCKING_BEGINNING, MODELCONDITION_DOCKING_ACTIVE, MODELCONDITION_DOCKING];
            let mut clear = clear.to_vec();
            clear.sort_unstable();
            world.contain_clear_model_condition_flags(me.object, &clear);
            if docking_object {
                world.contain_clear_model_condition_flags(docker, &clear);
            }
        }
        Ok(())
    }

    /// `update()`: "In charge of lifting dock restriction for one registered as Approached if all is ready".
    pub fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if self.active_docker == INVALID_ID && !self.dock_crippled {
            // "if setDockCrippled has been called, I will never give enterance permission."
            if let Some(k) = self.approach_position_reached.iter().position(|&r| r) {
                let k = if crate::logic::modules::plant(2135) { self.approach_position_reached.iter().rposition(|&r| r).unwrap_or(k) } else { k };
                self.active_docker = self.approach_position_owners.get(k).copied().unwrap_or(INVALID_ID);
                return UPDATE_SLEEP_NONE;
            }
        } else if world.objects.find(me.object).is_some_and(|o| o.is_kind_of(KINDOF_SUPPLY_SOURCE)) {
            let worker = world.objects.find(self.active_docker).is_some_and(|d| d.is_kind_of(KINDOF_DOZER) && d.is_kind_of(KINDOF_HARVESTER));
            if worker && world.has_drawable(self.active_docker) && world.reads.drawable_model_condition_flags(self.active_docker).test(MODELCONDITION_DOCKING_BEGINNING) {
                world.contain_clear_model_condition_flags(self.active_docker, &[MODELCONDITION_MOVING]);
            }
        }
        UPDATE_SLEEP_NONE
    }

    /// `computeApproachPosition(positionIndex, forWhom)`: "Do a smart lookup of this bone position".
    fn compute_approach_position(&mut self, world: &mut World, me: ModuleHandle, k: usize, for_whom: ObjectId) -> Result<Coord3D, Undefined> {
        if !self.positions_loaded {
            self.load_dock_positions(world, me)?;
        }
        let us = me.object;
        // "Start with the pristine bone, then convert it to the world, then find a clean spot around it."
        let mut working = world.contain_convert_bone_pos_to_world_pos(us, &self.approach_positions[k]);
        let our = world.objects.find(us).ok_or(Undefined::NullObject)?;
        let (our_position, major_radius) = (our.get_position(), our.geometry_info.get_major_radius());
        let their_position = world.objects.find(for_whom).ok_or(Undefined::NullObject)?.get_position();
        if self.number_approach_position_bones == 0 {
            // "A Boneless building wants to bias towards the caller for the arbitrary position"
            let mut offset = Vector3::new(their_position.x - our_position.x, their_position.y - our_position.y, their_position.z - our_position.z);
            offset.normalize();
            let half = if crate::logic::modules::plant(2136) { major_radius } else { major_radius / 2.0 };
            let offset = offset * half;
            working.x += offset.x;
            working.y += offset.y;
            working.z += offset.z;
        }
        // "Flyers can ignore us, so they can approach right over us if they want."
        let ignore = if world.is_using_airborne_locomotor(for_whom) { us } else { INVALID_ID };
        Ok(world.dock_partition_find_position_around(&working, 0.0, 100.0, for_whom, ignore).unwrap_or(working))
    }

    /// `loadDockPositions()`: the bones, once, from the drawable (nothing without one: the next call tries again).
    fn load_dock_positions(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        let obj = me.object;
        if !world.has_drawable(obj) {
            return Ok(());
        }
        // "Some of the GLASupplyStash assets still have docking bones in them" (Patch 1.03)
        if world.objects.find(obj).is_some_and(|o| o.is_kind_of(KINDOF_IGNORE_DOCKING_BONES)) {
            self.number_approach_position_bones = 0;
            self.positions_loaded = true;
            return Ok(());
        }
        for (name, slot) in [(&b"DockStart"[..], &mut self.enter_position), (b"DockAction", &mut self.dock_position), (b"DockEnd", &mut self.exit_position)] {
            if let Some(p) = world.drawable_pristine_bones(obj, name, 0, 1).first() {
                *slot = *p;
            }
        }
        if self.number_approach_positions != DYNAMIC_APPROACH_VECTOR_FLAG {
            // "Dynamic means no bones"
            let n = self.number_approach_positions;
            let bones = world.drawable_pristine_bones(obj, b"DockWaiting", 1, n);
            if bones.len() > DEFAULT_APPROACH_VECTOR_SIZE as usize {
                return Err(Undefined::ArrayIndexOutOfRange); // written past `approachBones[DEFAULT_APPROACH_VECTOR_SIZE]`
            }
            self.number_approach_position_bones = bones.len() as i32;
            if n as usize == self.approach_positions.len() {
                // "safeguard: will always be true"
                if n > DEFAULT_APPROACH_VECTOR_SIZE {
                    return Err(Undefined::ArrayIndexOutOfRange);
                }
                if (bones.len() as i32) < n {
                    return Err(Undefined::UninitialisedLocal);
                }
                self.approach_positions[..n as usize].copy_from_slice(&bones);
            }
        } else {
            self.number_approach_position_bones = 0;
        }
        self.positions_loaded = true;
        Ok(())
    }

    /// `xfer`'s fields after the `UpdateModule` base (version 1 for every dock).
    pub fn xfer_fields(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        xfer.xfer_coord3d(&mut self.enter_position)?;
        xfer.xfer_coord3d(&mut self.dock_position)?;
        xfer.xfer_coord3d(&mut self.exit_position)?;
        xfer.xfer_int(&mut self.number_approach_positions)?;
        xfer.xfer_bool(&mut self.positions_loaded)?;
        let mut size = self.approach_positions.len() as i32;
        xfer.xfer_int(&mut size)?;
        let size = usize::try_from(size).unwrap_or(0);
        self.approach_positions.resize(size, ZERO);
        for p in &mut self.approach_positions {
            xfer.xfer_coord3d(p)?;
        }
        let mut size = self.approach_position_owners.len() as i32;
        xfer.xfer_int(&mut size)?;
        let size = usize::try_from(size).unwrap_or(0);
        self.approach_position_owners.resize(size, INVALID_ID);
        for o in &mut self.approach_position_owners {
            xfer.xfer_object_id(o)?;
        }
        let mut size = self.approach_position_reached.len() as i32;
        xfer.xfer_int(&mut size)?;
        let size = usize::try_from(size).unwrap_or(0);
        self.approach_position_reached.resize(size, false);
        for r in &mut self.approach_position_reached {
            // "Vector of Bool gets packed as bitfield internally": the copy is xferred, and a load leaves the flag as it was
            let mut unpack = *r;
            xfer.xfer_bool(&mut unpack)?;
            if crate::logic::modules::plant(2137) {
                *r = unpack;
            }
        }
        xfer.xfer_object_id(&mut self.active_docker)?;
        xfer.xfer_bool(&mut self.docker_inside)?;
        xfer.xfer_bool(&mut self.dock_crippled)?;
        xfer.xfer_bool(&mut self.dock_open)
    }

    /// The calls of `DockUpdateInterface` (`None`: not one of them). `rally_point_after_dock` is the derived dock's
    /// `isRallyPointAfterDockType()`.
    pub fn dock_call(&mut self, world: &mut World, me: ModuleHandle, call: &ModuleCall, rally_point_after_dock: bool) -> Option<Result<ModuleReply, Undefined>> {
        let approach = |r: Result<ApproachAnswer, Undefined>| r.map(|a| ModuleReply::Approach { ok: a.ok, position: a.position, index: a.index });
        let done = |r: Result<(), Undefined>| r.map(|()| ModuleReply::Done);
        let path = |r: Result<Option<Coord3D>, Undefined>| r.map(|p| p.map_or(ModuleReply::Done, ModuleReply::Coord));
        Some(match *call {
            ModuleCall::HasDockUpdateInterface => Ok(ModuleReply::Bool(true)),
            ModuleCall::IsClearToApproach { docker } => self.is_clear_to_approach(world, docker).map(ModuleReply::Bool),
            ModuleCall::DockReserveApproachPosition { docker, position, index } => approach(self.reserve_approach_position(world, me, docker, position, index)),
            ModuleCall::DockAdvanceApproachPosition { docker, position, index } => approach(self.advance_approach_position(world, me, docker, position, index)),
            ModuleCall::DockIsClearToEnter { docker } => self.is_clear_to_enter(world, docker).map(ModuleReply::Bool),
            ModuleCall::DockIsClearToAdvance { docker, index } => self.is_clear_to_advance(world, docker, index).map(ModuleReply::Bool),
            ModuleCall::DockGetEnterPosition { docker, position } => path(self.get_path_position(world, me, docker, position, 0)),
            ModuleCall::DockGetDockPosition { docker, position } => path(self.get_path_position(world, me, docker, position, 1)),
            ModuleCall::DockGetExitPosition { docker, position } => path(self.get_path_position(world, me, docker, position, 2)),
            ModuleCall::DockOnApproachReached { docker } => done(self.on_approach_reached(world, docker)),
            ModuleCall::DockOnEnterReached { docker } => done(self.on_enter_reached(world, me, docker)),
            ModuleCall::DockOnDockReached { docker } => done(self.on_dock_reached(world, me, docker)),
            ModuleCall::DockOnExitReached { docker } => done(self.on_exit_reached(world, me, docker)),
            ModuleCall::DockCancelDock { docker } => done(self.cancel_dock(world, me, docker)),
            ModuleCall::DockIsOpen => Ok(ModuleReply::Bool(self.dock_open)),
            ModuleCall::DockSetOpen(open) => {
                self.dock_open = open;
                Ok(ModuleReply::Done)
            }
            ModuleCall::DockIsAllowPassthroughType => Ok(ModuleReply::Bool(self.data.is_allow_passthrough)),
            ModuleCall::DockIsRallyPointAfterDockType => Ok(ModuleReply::Bool(rally_point_after_dock)),
            ModuleCall::DockSetDockCrippled(setting) => {
                // "At this level, Crippling means I will accept Approach requests, but I will never grant Enter clearence."
                self.dock_crippled = setting;
                Ok(ModuleReply::Done)
            }
            _ => return None,
        })
    }
}

/// `DockUpdate::crc`, `xfer`'s version and `loadPostProcess` are the derived module's ([`UpdateModule`](crate::logic::module::update_module::UpdateModule)'s).
pub fn new_dock_update(data: Arc<DockUpdateModuleData>) -> IniResult<DockUpdate> {
    DockUpdate::new(data).map_err(crate::common::ini::IniError::undefined)
}
