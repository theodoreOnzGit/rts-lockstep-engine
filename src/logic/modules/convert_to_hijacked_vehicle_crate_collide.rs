// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ConvertToHijackedVehicleCrateCollide.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/ConvertToHijackedVehicleCrateCollide.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ConvertToHijackedVehicleCrateCollide`: the hijacker who takes an enemy vehicle. `isValidToExecute` adds to the base: a living `other` that is not
//! immune to capture, an aircraft, a boat or a drone, not hijacked already, an enemy of the hijacker, and not a transport with passengers.
//! `executeCrateBehavior` acts only on its AI's goal (any `other` without an AI); tells the radar, plays Eva's `VehicleStolen` for the local player's
//! vehicle, moves it to the hijacker's player's default team, marks it `HIJACKED`, stops its AI where it stands (and every dozer task), plays
//! `HijackDriver`, hands it the hijacker's name and both the higher veterancy (without feedback). A vehicle that cannot eject its pilot takes the
//! hijacker with it (he is destroyed and the crate "acted"); otherwise the hijacker's `HijackerUpdate` rides along, and he leaves his group, idles,
//! gives the vehicle his vision and shroud ranges, leaves the partition manager and hides: the crate did not act, so he is not removed.
//!
//! Upstream facts kept: the vehicle's AI is called unchecked ([`Undefined::NullAi`]); the hijacker's controlling player is read unchecked for the
//! team ([`Undefined::NullControllingPlayer`]); the aircraft test is made twice; `EffectRange` is declared and never parsed.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined};
use crate::common::kind_of::{KINDOF_AIRCRAFT, KINDOF_BOAT, KINDOF_DRONE, KINDOF_IMMUNE_TO_CAPTURE, KINDOF_TRANSPORT};
use crate::common::game_common::Relationship;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_HIJACKED, OBJECT_STATUS_MASKED, OBJECT_STATUS_NO_COLLISIONS, OBJECT_STATUS_UNSELECTABLE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::crate_collide_data_field_parse;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::collide_module::CollideModule;
use crate::logic::modules::crate_collide::{crate_destroy_object, crate_is_valid_to_execute, crate_module_call, CrateBehavior, CrateCollideModuleData};
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::{EvaMessage, ObjectClientEvent, RadarCall};
use crate::logic::world::update_body::UpdateBody;

/// `DOZER_TASK_FIRST` and `DOZER_NUM_TASKS` (`DozerAIUpdate.h`: build, repair, fortify).
const DOZER_TASK_FIRST: i32 = 0;
const DOZER_NUM_TASKS: i32 = 3;

/// `class ConvertToHijackedVehicleCrateCollideModuleData : public CrateCollideModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConvertToHijackedVehicleCrateCollideModuleData {
    /// The `CrateCollideModuleData` base.
    pub crate_data: CrateCollideModuleData,
    /// `m_rangeOfEffect` (no field parses it, nothing reads it).
    pub range_of_effect: u32,
}

impl ConvertToHijackedVehicleCrateCollideModuleData {
    /// `ConvertToHijackedVehicleCrateCollideModuleData()`.
    pub fn new() -> Self {
        ConvertToHijackedVehicleCrateCollideModuleData { crate_data: CrateCollideModuleData::new(), range_of_effect: 0 }
    }

    /// `buildFieldParse(p)`: the module data base and the crate fields (the class adds none).
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, ConvertToHijackedVehicleCrateCollideModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ConvertToHijackedVehicleCrateCollideModuleData> {
        let mut data = ConvertToHijackedVehicleCrateCollideModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ConvertToHijackedVehicleCrateCollideModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `CrateCollideModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<ConvertToHijackedVehicleCrateCollideModuleData>; 12] =
    crate_collide_data_field_parse!(ConvertToHijackedVehicleCrateCollideModuleData, crate_data);

/// `class ConvertToHijackedVehicleCrateCollide : public CrateCollide`.
#[derive(Clone, Debug)]
pub struct ConvertToHijackedVehicleCrateCollide {
    /// The `CollideModule` base.
    collide: CollideModule,
    /// `getConvertToHijackedVehicleCrateCollideModuleData()`.
    data: Arc<ConvertToHijackedVehicleCrateCollideModuleData>,
}

impl PartialEq for ConvertToHijackedVehicleCrateCollide {
    /// The same object and the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.collide.behavior().object() == other.collide.behavior().object() && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl ConvertToHijackedVehicleCrateCollide {
    /// `ConvertToHijackedVehicleCrateCollide(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<ConvertToHijackedVehicleCrateCollideModuleData>>) -> IniResult<ConvertToHijackedVehicleCrateCollide> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let collide = CollideModule::new(object, Some(Arc::new(data.crate_data.collide.clone())))?;
        Ok(ConvertToHijackedVehicleCrateCollide { collide, data })
    }

    /// `CollideModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        CollideModule::get_interface_mask()
    }

    /// The data, as `getConvertToHijackedVehicleCrateCollideModuleData()`.
    pub fn module_data(&self) -> &Arc<ConvertToHijackedVehicleCrateCollideModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        crate_module_call(self, world, me, call)
    }
}

impl CrateBehavior for ConvertToHijackedVehicleCrateCollide {
    fn collide(&self) -> &CollideModule {
        &self.collide
    }
    fn crate_data(&self) -> &CrateCollideModuleData {
        &self.data.crate_data
    }

    /// `isValidToExecute(other)`.
    fn is_valid_to_execute(&self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<bool, IniError> {
        if !crate_is_valid_to_execute(&self.data.crate_data, world, me, other)? {
            return Ok(false);
        }
        let Some(o) = world.objects.find(other) else { return Ok(false) };
        if o.is_effectively_dead() {
            return Ok(false); // can't hijack a dead vehicle
        }
        // "Prevent hijackers from being able to hijack battle buses."
        if o.is_kind_of(KINDOF_IMMUNE_TO_CAPTURE) && !crate::logic::modules::plant(1731) {
            return Ok(false);
        }
        // "Can't hijack planes and boats!"
        if o.is_kind_of(KINDOF_AIRCRAFT) || o.is_kind_of(KINDOF_BOAT) {
            return Ok(false);
        }
        // "Can't hijack drones!"
        if o.is_kind_of(KINDOF_DRONE) && !crate::logic::modules::plant(1732) {
            return Ok(false);
        }
        if o.test_status(OBJECT_STATUS_HIJACKED) && !crate::logic::modules::plant(1733) {
            return Ok(false); // oops, sorry, I'll jack the next one.
        }
        let is_transport = o.is_kind_of(KINDOF_TRANSPORT);
        let contain = o.contain;
        // "Only hijack enemy objects"
        if (world.get_relationship(me.object, other) != Relationship::Enemies) != crate::logic::modules::plant(1734) {
            return Ok(false);
        }
        // "Allow empty transports to be hijacked." / "do not jack vehicles that may carry hostile passengers"
        if is_transport && !crate::logic::modules::plant(1735) {
            if let Some(c) = contain {
                if world.module_call(other, c, ModuleCall::GetContainCount).as_int(0) > 0 {
                    return Ok(false);
                }
            }
        }
        // "Make sure you can't hijack any aircraft (or hijack-enter)."
        if world.objects.find(other).is_some_and(|o| o.is_kind_of(KINDOF_AIRCRAFT)) {
            return Ok(false);
        }
        Ok(true)
    }

    /// `executeCrateBehavior(other)`.
    fn execute_crate_behavior(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<bool, IniError> {
        let obj = me.object;
        let ai = world.objects.find(obj).and_then(|o| o.ai);
        // "in order to prevent an unintentional conversion simply by having the terrorist walk too close to it"
        if let Some(ai) = ai {
            if world.module_call(obj, ai, ModuleCall::AiGetGoalObject).as_id() != other && !crate::logic::modules::plant(1736) {
                return Ok(false);
            }
        }
        world.emit(ObjectClientEvent::Radar { call: RadarCall::TryInfiltrationEvent, object_id: other, position: None });
        // "play the "vehicle stolen" EVA event if the local player is the victim!"
        if world.is_locally_controlled(other) && !crate::logic::modules::plant(1737) {
            world.emit(ObjectClientEvent::Eva { message: EvaMessage::VehicleStolen });
        }
        let player = world.get_controlling_player(obj).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        let team = world.players.get_nth(player).and_then(|p| p.default_team);
        world.crate_set_team(other, team);
        let mut hijacked = ObjectStatusMaskType::new();
        hijacked.set(OBJECT_STATUS_HIJACKED, 1);
        world.set_status(other, hijacked, true); // I claim this car in the name of the GLA
        let target_ai = world.objects.find(other).and_then(|o| o.ai).ok_or_else(|| IniError::undefined(Undefined::NullAi))?;
        let pos = world.objects.find(other).map(|o| o.get_position()).unwrap_or_default();
        if !crate::logic::modules::plant(1738) {
            world.module_call(other, target_ai, ModuleCall::AiMoveToPosition { pos, command_source: CMD_FROM_AI });
        }
        world.module_call(other, target_ai, ModuleCall::AiIdle);
        // "Just in case this target is a dozer, lets make him stop al his dozer tasks"
        if world.module_call(other, target_ai, ModuleCall::AiGetDozerAIInterface).as_bool() {
            for task in DOZER_TASK_FIRST..DOZER_NUM_TASKS {
                world.module_call(other, target_ai, ModuleCall::DozerCancelTask(task));
            }
        }
        world.emit(ObjectClientEvent::AudioAddEvent { name: b"HijackDriver".to_vec(), object_id: obj, position: None, player_index: None });
        // "transfer the hijacker's name to the car"
        let name = world.objects.find(obj).map(|o| o.name.clone()).unwrap_or_default();
        world.cut(CutCall::ScriptEngineTransferObjectName { name, object: other });
        let level = |w: &World, id| w.objects.find(id).map_or(0, |o| o.experience.level as i32);
        let highest = level(world, other).max(level(world, obj));
        world.crate_experience_set_veterancy_level(obj, highest, false);
        if !crate::logic::modules::plant(1739) {
            world.crate_experience_set_veterancy_level(other, highest, false);
        }
        let target_can_eject = world.objects.find(other).is_some_and(|o| o.modules.iter().any(|m| m.name == b"EjectPilotDie"));
        if !target_can_eject && !crate::logic::modules::plant(1740) {
            crate_destroy_object(world, obj);
            return Ok(true);
        }
        // "I we have made it this far, we are going to ride in this vehicle for a while"
        if let Some(hijacker) = world.find_module(obj, b"HijackerUpdate") {
            world.module_call(obj, hijacker, ModuleCall::HijackerSetTargetObject(other));
            world.module_call(obj, hijacker, ModuleCall::HijackerSetIsInVehicle(true));
            world.module_call(obj, hijacker, ModuleCall::HijackerSetUpdate(!crate::logic::modules::plant(1741)));
            // "flag bits so hijacker won't be selectible or collideable while within the vehicle"
            let mut bits = ObjectStatusMaskType::new();
            for b in [OBJECT_STATUS_NO_COLLISIONS, OBJECT_STATUS_MASKED, OBJECT_STATUS_UNSELECTABLE] {
                bits.set(b, 1);
            }
            world.set_status(obj, bits, true);
        }
        // THIS BLOCK HIDES THE HIJACKER AND REMOVES HIM FROM PARTITION MANAGER
        world.contain_leave_group(obj);
        if let Some(ai) = ai {
            // "By setting him to idle, we will prevent him from entering the target after this gets called."
            world.module_call(obj, ai, ModuleCall::AiIdle);
        }
        let (vision, shroud) = (world.get_vision_range(obj), world.get_shroud_clearing_range(obj));
        world.set_vision_range(other, vision);
        world.set_shroud_clearing_range(other, shroud);
        if !crate::logic::modules::plant(1742) {
            world.contain_partition_unregister_object(obj);
        }
        if world.has_drawable(obj) {
            world.contain_drawable_set_hidden(obj, true);
        }
        // "By returning FALSE, we will not remove the object (Hijacker)"
        Ok(false)
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::ConvertToHijackedVehicleCrateCollide(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ConvertToHijackedVehicleCrateCollide(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for ConvertToHijackedVehicleCrateCollide {
    /// `crc`: `CrateCollide::crc`, which is `CollideModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.collide.crc(xfer)
    }
    /// `xfer` (version 1, "Initial version"): the version, `CrateCollide::xfer` (its own version, then `CollideModule::xfer`).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let mut crate_version = current;
        xfer.xfer_version(&mut crate_version, current)?;
        self.collide.xfer(xfer)
    }
    /// `loadPostProcess`: `CollideModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.collide.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `ConvertToHijackedVehicleCrateCollide`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ConvertToHijackedVehicleCrateCollide(Arc::new(ConvertToHijackedVehicleCrateCollideModuleData::friend_new_module_data(ini)?)))
}
