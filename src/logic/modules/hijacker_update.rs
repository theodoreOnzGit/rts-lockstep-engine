// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/HijackerUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HijackerUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `HijackerUpdate`: the hijacker riding in the vehicle he took (`ConvertToHijackedVehicleCrateCollide` sets it going). While flagged for update and
//! in the vehicle, he follows it ("Continually reset position of hijacker to match the position of the target"), remembers whether it is airborne and
//! where it is, and the two share the higher veterancy. When the vehicle is gone ("has safely ejected us") he goes back into the partition manager,
//! shows his drawable again, loses the no-collision, masked and unselectable bits, idles his AI and, if the vehicle was airborne, is put into a new
//! `ParachuteName` object at the eject position; then the module stops. It never sleeps.
//!
//! The calls of the crate are [`ModuleCall::HijackerSetTargetObject`], [`ModuleCall::HijackerSetIsInVehicle`] and [`ModuleCall::HijackerSetUpdate`];
//! `loadPostProcess`'s look-up of the target is [`ModuleCall::LoadPostProcess`].
//!
//! # Upstream facts kept
//!
//! - `AttachToTargetBone` is parsed and never read.
//! - The parachute is made whatever its template, and its contain module is read unchecked ([`Undefined::NullContainModule`]).
//! - The veterancy is shared with feedback here (the crate shares it without).

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_MASKED, OBJECT_STATUS_NO_COLLISIONS, OBJECT_STATUS_UNSELECTABLE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class HijackerUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HijackerUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_attachToBone` (never read).
    pub attach_to_bone: Vec<u8>,
    /// `m_parachuteName`.
    pub parachute_name: Vec<u8>,
}

/// `HijackerUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<HijackerUpdateModuleData>; 3] = [
    FieldParse::new("AttachToTargetBone", |ini, d, _| { d.attach_to_bone = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("ParachuteName", |ini, d, _| { d.parachute_name = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl HijackerUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, HijackerUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<HijackerUpdateModuleData> {
        let mut data = HijackerUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, HijackerUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class HijackerUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct HijackerUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getHijackerUpdateModuleData()`.
    data: Arc<HijackerUpdateModuleData>,
    /// `m_targetID`.
    target_id: ObjectId,
    /// `m_ejectPos`.
    eject_pos: Coord3D,
    /// `m_update`.
    update_flag: bool,
    /// `m_isInVehicle`.
    is_in_vehicle: bool,
    /// `m_wasTargetAirborne`.
    was_target_airborne: bool,
}

impl PartialEq for HijackerUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl HijackerUpdate {
    /// `HijackerUpdate(thing, moduleData)`: no target, not updating, not in a vehicle, not airborne, the eject position zero.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<HijackerUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(HijackerUpdate { update, data, target_id: INVALID_ID, eject_pos: Coord3D::default(), update_flag: false, is_in_vehicle: false, was_target_airborne: false })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getHijackerUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<HijackerUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's: the target, the eject position, the three flags.
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            self.target_id as u32,
            self.eject_pos.x.to_bits(),
            self.eject_pos.y.to_bits(),
            self.eject_pos.z.to_bits(),
            u32::from(self.update_flag),
            u32::from(self.is_in_vehicle),
            u32::from(self.was_target_airborne),
        ]
    }

    /// `setTargetObject(object)`: its id, or `INVALID_ID` for none.
    pub fn set_target_object(&mut self, object: Option<ObjectId>) {
        self.target_id = object.unwrap_or(INVALID_ID);
    }

    /// `getTargetObject()`: the target, if it is still an object.
    pub fn get_target_object(&self, world: &World) -> Option<ObjectId> {
        if self.target_id == INVALID_ID {
            return None;
        }
        world.objects.find(self.target_id).map(|_| self.target_id)
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

    /// `xfer` with the scheduling half the slot holds: the version, that half, then the fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds (the target's look-up needs the world: [`ModuleCall::LoadPostProcess`]).
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// The body of `update()`. An error is upstream's undefined behaviour.
    fn run(&mut self, world: &mut World, obj: ObjectId) -> Result<(), IniError> {
        // "have not flagged for updating"
        if !self.update_flag {
            return Ok(());
        }
        if !self.is_in_vehicle {
            self.was_target_airborne = false;
            return Ok(());
        }
        if let Some(target) = self.get_target_object(world).filter(|_| !crate::logic::modules::plant(1721)) {
            // "If hijacker has hijacked a vehicle, he needs to move along with it..."
            let pos = world.objects.find(target).map_or_else(Coord3D::default, |o| o.get_position());
            world.set_position(obj, &pos);
            self.was_target_airborne = world.is_significantly_above_terrain(target);
            self.eject_pos = pos;
            // "if while I am driving this American war vehicle, I gain skill points, I get to keep them when I wreck the vehicle"
            let level = |w: &World, id| w.objects.find(id).map_or(0, |o| o.experience.level as i32);
            let highest = level(world, target).max(if crate::logic::modules::plant(1722) { 0 } else { level(world, obj) });
            world.crate_experience_set_veterancy_level(obj, highest, true);
            world.crate_experience_set_veterancy_level(target, highest, true);
            return Ok(());
        }
        // "the car we have been "driving" is dead now, and has safely ejected us"
        world.contain_partition_register_object(obj);
        if world.has_drawable(obj) && !crate::logic::modules::plant(1723) {
            // "so it is time to unhide ourselves and be a pedestrian hijacker again"
            world.contain_drawable_set_hidden(obj, false);
        }
        let mut bits = ObjectStatusMaskType::new();
        for b in [OBJECT_STATUS_NO_COLLISIONS, OBJECT_STATUS_MASKED, OBJECT_STATUS_UNSELECTABLE] {
            bits.set(b, 1);
        }
        world.set_status(obj, bits, false);
        if let Some(ai) = world.objects.find(obj).and_then(|o| o.ai) {
            world.module_call(obj, ai, ModuleCall::AiIdle);
        }
        if self.was_target_airborne {
            if let Some(tmpl) = world.thing_factory_find_template(&self.data.parachute_name) {
                let team = world.get_team(obj);
                let container = world.crate_new_object(tmpl, team);
                world.set_position(container, &self.eject_pos);
                let contain = world.objects.find(container).and_then(|o| o.contain).ok_or_else(|| IniError::undefined(Undefined::NullContainModule))?;
                if world.module_call(container, contain, ModuleCall::IsValidContainerFor { obj, check_capacity: true }).as_bool() {
                    world.module_call(container, contain, ModuleCall::AddToContain(obj));
                }
            }
        }
        self.set_target_object(None);
        self.is_in_vehicle = false;
        self.update_flag = crate::logic::modules::plant(1724);
        self.was_target_airborne = false;
        Ok(())
    }
}

impl UpdateBehavior for HijackerUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(e) = self.run(world, me.object) {
            world.cuts.module_errors.push((me, e));
        }
        UPDATE_SLEEP_NONE
    }
    /// The fields after the base: the target, the eject position, the three flags.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_object_id(&mut self.target_id)?;
        xfer.xfer_coord3d(&mut self.eject_pos)?;
        xfer.xfer_bool(&mut self.update_flag)?;
        if crate::logic::modules::plant(1725) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.is_in_vehicle)?;
        xfer.xfer_bool(&mut self.was_target_airborne)
    }
    fn other_call(&mut self, world: &mut World, _me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::HijackerSetTargetObject(id) => {
                self.set_target_object((id != INVALID_ID).then_some(id));
                ModuleReply::Done
            }
            ModuleCall::HijackerSetIsInVehicle(i) => {
                self.is_in_vehicle = i;
                ModuleReply::Done
            }
            ModuleCall::HijackerSetUpdate(u) => {
                self.update_flag = u;
                ModuleReply::Done
            }
            // "set the target object": the object of the loaded id, if there is one
            ModuleCall::LoadPostProcess => {
                let found = world.objects.find(self.target_id).map(|_| self.target_id).filter(|_| !crate::logic::modules::plant(1726));
                self.set_target_object(found);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::HijackerUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::HijackerUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for HijackerUpdate {
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

/// The factory's `NewModuleDataProc` for `HijackerUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::HijackerUpdate(Arc::new(HijackerUpdateModuleData::friend_new_module_data(ini)?)))
}
