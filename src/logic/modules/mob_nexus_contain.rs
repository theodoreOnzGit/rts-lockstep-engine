// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/MobNexusContain.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/MobNexusContain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `MobNexusContain`: the container that holds an angry mob's members. It is `TransportContain` written again on `OpenContain` (slots, an exit bone, an
//! initial payload made when the object is, riders healed by `HealthRegen%PerSec`), without the transport's exit delay, aggression, weapon set or
//! capture rules.
//!
//! # Design
//!
//! As [`OpenContain`] for a class derived from it (see `heal_contain`). The code is the transport's (`transport_contain`, where the two classes say the
//! same thing) translated again from this file, because upstream wrote it again: the two differ in small ways that are kept.
//!
//! # Upstream facts kept
//!
//! - `isValidContainerFor` replaces a rider that is a special zero-slot container (a parachute) by the first object it holds, reading `*begin()` of its
//!   list unchecked: an empty parachute's list reads `end()` ([`Undefined::EndIteratorDereferenced`], recorded, the test's driver stops at `end()`).
//!   Only allied riders, as `TransportContain`: "it considers me an ally".
//! - `update` heals before it runs the base's update (the transport heals after), from the object's contain module's list (`getObject()->getContain()`,
//!   this module), and its `while( *it )` loop reads `end()` after the last rider ([`Undefined::EndIteratorDereferenced`]); a rider with no body is
//!   undefined ([`Undefined::NullBodyModule`]).
//! - The initial payload is made in `onObjectCreated`, not on the first update; a template that does not exist is `newObject(NULL)`, which throws
//!   `ERROR_BAD_ARG` out of `onObjectCreated` (recorded); an object with no controlling player is undefined ([`Undefined::NullControllingPlayer`]).
//! - `reserveDoorForExit` is the transport's `isSpecificRiderFreeToExit`, but a rider with no AI cannot exit (the transport also asks for a locomotor).
//! - `tryToEvacuate` exits every rider a door is reserved for, and marks stealth-garrison riders detected when asked to expose them.
//!
//! `p1w13-sonnet`, batch R9, 2026-10-22.

use std::sync::Arc;

use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::game_common::{Relationship, SECONDS_PER_LOGICFRAME_REAL};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KINDOF_INFANTRY, KINDOF_STEALTH_GARRISON};
use crate::common::model_state::MODELCONDITION_LOADED;
use crate::common::module::ModuleDataKind;
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{ExitDoorType, UpdateModule, UpdateSleepTime, DOOR_1, DOOR_NONE_AVAILABLE};
use crate::logic::modules::open_contain::{self, out, OpenContain, OpenContainModuleData, OpenContainVirtuals};
use crate::logic::modules::transport_contain::{InitialPayload, FREE_TO_EXIT};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class MobNexusContainModuleData : public OpenContainModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct MobNexusContainModuleData {
    /// The `OpenContainModuleData` base.
    pub open: OpenContainModuleData,
    /// `m_slotCapacity` (`Slots`): "max units that can be inside us".
    pub slot_capacity: i32,
    /// `m_exitPitchRate` (`ExitPitchRate`, an angular velocity).
    pub exit_pitch_rate: f32,
    /// `m_scatterNearbyOnExit` (`ScatterNearbyOnExit`).
    pub scatter_nearby_on_exit: bool,
    /// `m_orientLikeContainerOnExit` (`OrientLikeContainerOnExit`).
    pub orient_like_container_on_exit: bool,
    /// `m_keepContainerVelocityOnExit` (`KeepContainerVelocityOnExit`).
    pub keep_container_velocity_on_exit: bool,
    /// `m_exitBone` (`ExitBone`).
    pub exit_bone: Vec<u8>,
    /// `m_initialPayload` (`InitialPayload`).
    pub initial_payload: InitialPayload,
    /// `m_healthRegen` (`HealthRegen%PerSec`, read as a plain real).
    pub health_regen: f32,
}

impl Default for MobNexusContainModuleData {
    /// `MobNexusContainModuleData()`: "by default we say that MobNexae can have infantry inside them".
    fn default() -> Self {
        let mut open = OpenContainModuleData::default();
        let mut infantry = crate::common::kind_of::KindOfMaskType::new();
        infantry.set(KINDOF_INFANTRY, 1);
        open.allow_inside_kind_of = infantry;
        MobNexusContainModuleData {
            open,
            slot_capacity: 0,
            exit_pitch_rate: 0.0,
            scatter_nearby_on_exit: true,
            orient_like_container_on_exit: false,
            keep_container_velocity_on_exit: false,
            exit_bone: Vec::new(),
            initial_payload: InitialPayload { name: Vec::new(), count: 0 },
            health_regen: 0.0,
        }
    }
}

/// Lifts entry `k` of `OpenContainModuleData`'s table onto the `open` base.
fn lift_open(k: usize, i: &mut Ini<'_>, d: &mut MobNexusContainModuleData, u: &UserData) -> IniResult<()> {
    match open_contain::FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

/// Lifts entry `k` of the die mux table onto the `open` base.
fn lift_die(k: usize, i: &mut Ini<'_>, d: &mut MobNexusContainModuleData, u: &UserData) -> IniResult<()> {
    match open_contain::DIE_MUX_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

static LIFTED_OPEN: [FieldParse<MobNexusContainModuleData>; 16] = [
    FieldParse::new("ContainMax", |i, d, u| lift_open(0, i, d, u), UserData::None),
    FieldParse::new("EnterSound", |i, d, u| lift_open(1, i, d, u), UserData::None),
    FieldParse::new("ExitSound", |i, d, u| lift_open(2, i, d, u), UserData::None),
    FieldParse::new("DamagePercentToUnits", |i, d, u| lift_open(3, i, d, u), UserData::None),
    FieldParse::new("BurnedDeathToUnits", |i, d, u| lift_open(4, i, d, u), UserData::None),
    FieldParse::new("AllowInsideKindOf", |i, d, u| lift_open(5, i, d, u), UserData::None),
    FieldParse::new("ForbidInsideKindOf", |i, d, u| lift_open(6, i, d, u), UserData::None),
    FieldParse::new("PassengersAllowedToFire", |i, d, u| lift_open(7, i, d, u), UserData::None),
    FieldParse::new("PassengersInTurret", |i, d, u| lift_open(8, i, d, u), UserData::None),
    FieldParse::new("NumberOfExitPaths", |i, d, u| lift_open(9, i, d, u), UserData::None),
    FieldParse::new("DoorOpenTime", |i, d, u| lift_open(10, i, d, u), UserData::None),
    FieldParse::new("WeaponBonusPassedToPassengers", |i, d, u| lift_open(11, i, d, u), UserData::None),
    FieldParse::new("AllowAlliesInside", |i, d, u| lift_open(12, i, d, u), UserData::None),
    FieldParse::new("AllowEnemiesInside", |i, d, u| lift_open(13, i, d, u), UserData::None),
    FieldParse::new("AllowNeutralInside", |i, d, u| lift_open(14, i, d, u), UserData::None),
    FieldParse::end(),
];

static LIFTED_DIE: [FieldParse<MobNexusContainModuleData>; 5] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die(3, i, d, u), UserData::None),
    FieldParse::end(),
];

/// `MobNexusContainModuleData::parseInitialPayload`: a name, then an optional count (`INI::scanInt`; 1 when there is none).
fn parse_initial_payload(ini: &mut Ini<'_>, d: &mut MobNexusContainModuleData, _u: &UserData) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let count = match ini.get_next_token_or_null(None) {
        Some(c) => Ini::scan_int(&c)?,
        None => 1,
    };
    d.initial_payload.name = name;
    d.initial_payload.count = count;
    Ok(())
}

/// `MobNexusContainModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<MobNexusContainModuleData>; 9] = [
    FieldParse::new("Slots", |i, d, _| { d.slot_capacity = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ScatterNearbyOnExit", |i, d, _| { d.scatter_nearby_on_exit = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("OrientLikeContainerOnExit", |i, d, _| { d.orient_like_container_on_exit = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("KeepContainerVelocityOnExit", |i, d, _| { d.keep_container_velocity_on_exit = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("ExitBone", |i, d, _| { d.exit_bone = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("ExitPitchRate", |i, d, _| { d.exit_pitch_rate = i.parse_angular_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("InitialPayload", parse_initial_payload, UserData::None),
    FieldParse::new("HealthRegen%PerSec", |i, d, _| { d.health_regen = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl MobNexusContainModuleData {
    /// `MobNexusContainModuleData::buildFieldParse(p)`: `OpenContainModuleData::buildFieldParse(p)`, then this class's table.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, MobNexusContainModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&LIFTED_OPEN)?;
        p.add(&LIFTED_DIE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<MobNexusContainModuleData> {
        let mut data = MobNexusContainModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, MobNexusContainModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `MobNexusContain`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::MobNexusContain(Arc::new(MobNexusContainModuleData::friend_new_module_data(ini)?)))
}

/// `class MobNexusContain : public OpenContain, public TransportPassengerInterface`.
#[derive(Clone, Debug, PartialEq)]
pub struct MobNexusContain {
    oc: OpenContain,
    /// `getMobNexusContainModuleData()`.
    data: Arc<MobNexusContainModuleData>,
    /// `m_extraSlotsInUse`: the slots the riders take beyond one each.
    extra_slots_in_use: i32,
}

/// Records that upstream is undefined here.
fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
    world.cuts.module_errors.push((me, IniError::undefined(u)));
}

impl MobNexusContain {
    /// `getInterfaceMask()`: the base's.
    pub const fn get_interface_mask() -> i32 {
        OpenContain::get_interface_mask()
    }

    /// The constructor's first half: the base's (on a copy of its part of the data); no slots in use.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<MobNexusContainModuleData>>) -> IniResult<MobNexusContain> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let oc = OpenContain::unconstructed(object, Some(Arc::new(data.open.clone())))?;
        Ok(MobNexusContain { oc, data, extra_slots_in_use: 0 })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.oc.update_module()
    }

    /// The module data.
    pub fn module_data(&self) -> &Arc<MobNexusContainModuleData> {
        &self.data
    }

    /// The state, as words: the base's, then the extra slots.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = self.oc.state_words();
        v.push(self.extra_slots_in_use as u32);
        v
    }

    /// The constructor's second half: the base's.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.oc.world_construct(world, me);
    }

    /// `update()` as the scheduler runs it.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        open_contain::contain_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version (1), the base's, then the extra slots.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.oc.xfer_with(update, xfer)?;
        if crate::logic::modules::plant(1541) {
            return Ok(());
        }
        xfer.xfer_int(&mut self.extra_slots_in_use)
    }

    /// `crc` with the scheduling half: "extend base class".
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.oc.crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half: "extend base class".
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        self.oc.load_post_process_with(update);
    }

    /// `tryToEvacuate(exposeStealthedUnits)`: every rider a door is reserved for goes out (stealth-garrison riders detected when asked); whether anyone
    /// went. The list is the module's own, the iterator advanced before the exit.
    pub fn try_to_evacuate(&mut self, world: &mut World, me: ModuleHandle, expose_stealthed_units: bool) -> bool {
        let mut exited_anyone = false;
        for obj in self.oc.contain_list().to_vec() {
            let exit_door = self.reserve_door_for_exit(world, me, obj);
            if exit_door != DOOR_NONE_AVAILABLE {
                self.exit_object_via_door(world, me, obj, exit_door);
                exited_anyone = !crate::logic::modules::plant(1542);
                let garrison = world.objects.find(obj).is_some_and(|o| o.is_kind_of(KINDOF_STEALTH_GARRISON));
                if garrison && expose_stealthed_units {
                    if let Some(stealth) = world.objects.find(obj).and_then(|o| o.stealth) {
                        out(self, world, me, |w| w.module_call(obj, stealth, ModuleCall::StealthMarkAsDetected));
                    }
                }
            }
        }
        exited_anyone
    }
}

impl OpenContainVirtuals for MobNexusContain {
    fn oc(&self) -> &OpenContain {
        &self.oc
    }
    fn oc_mut(&mut self) -> &mut OpenContain {
        &mut self.oc
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::MobNexusContain(self)
    }
    fn from_body(b: UpdateBody) -> Option<Self> {
        match b {
            UpdateBody::MobNexusContain(m) => Some(m),
            _ => None,
        }
    }

    /// `MobNexusContain::getContainMax()`: the slots.
    fn get_contain_max(&self, _world: &World) -> i32 {
        self.data.slot_capacity
    }

    /// `getExtraSlotsInUse()`.
    fn get_extra_slots_in_use(&mut self) -> i32 {
        self.extra_slots_in_use
    }

    /// `MobNexusContain::isValidContainerFor(rider, checkCapacity)`: a parachute is judged by what it holds; then the base's, an ally, slots.
    fn is_valid_container_for(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId, check_capacity: bool) -> bool {
        // sanity
        if rider == INVALID_ID || world.objects.find(rider).is_none() {
            return false;
        }
        let mut rider = rider;
        // "Check if we are a fake container, and if so, get an object inside it to see what kind this object *is*."
        // (the calls lend this module back to its slot: the rider may be the container itself)
        if let Some(c) = world.get_contain(rider) {
            if out(self, world, me, |w| w.module_call(rider, c, ModuleCall::IsSpecialZeroSlotContainer)).as_bool() {
                // "Report the first thing inside it!" (`*begin()`, unchecked)
                let items = out(self, world, me, |w| open_contain::view_contained_items_list(w, rider));
                match items.unwrap_or_default().first() {
                    Some(&first) if !crate::logic::modules::plant(1543) => rider = first,
                    Some(_) => {}
                    None => undefined(world, me, Undefined::EndIteratorDereferenced),
                }
            }
        }

        // extend functionality
        if !open_contain::base_is_valid_container_for(self, world, me, rider, check_capacity) {
            return false;
        }

        // "only allied objects can be MobNexused." "order matters: we want to know if IT considers ME to be an ally"
        if world.get_relationship(rider, me.object) != Relationship::Allies && !crate::logic::modules::plant(1544) {
            return false;
        }

        let slots = out(self, world, me, |w| w.get_transport_slot_count(rider));
        // "if 0, this object isn't MobNexusable."
        if slots == 0 {
            return false;
        }

        if check_capacity {
            let count = self.get_contain_count(world) as i32;
            if crate::logic::modules::plant(1545) {
                return count + slots <= self.get_contain_max(world);
            }
            // `Int + UnsignedInt + Int <= Int` (`getContainCount()` is unsigned): C++ compares unsigned, so a maximum of -1 takes anything
            (self.extra_slots_in_use as u32).wrapping_add(count as u32).wrapping_add(slots as u32) <= self.get_contain_max(world) as u32
        } else {
            true
        }
    }

    /// `MobNexusContain::onContaining(rider, wasSelected)`: held, its slots counted, and LOADED on the first.
    fn on_containing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId, was_selected: bool) {
        open_contain::base_on_containing(self, world, me, rider, was_selected);

        // "objects inside a MobNexus are held"
        out(self, world, me, |w| w.contain_set_disabled(rider, DISABLED_HELD));

        let slots = out(self, world, me, |w| w.get_transport_slot_count(rider));
        if !crate::logic::modules::plant(1546) {
            self.extra_slots_in_use += slots - 1;
        }

        // "when we go from holding nothing to holding something we have a model condition to visually show the change"
        if self.get_contain_count(world) == 1 {
            out(self, world, me, |w| w.contain_drawable_model_condition(me.object, MODELCONDITION_LOADED, true));
        }
    }

    /// `MobNexusContain::onRemoving(rider)`: let go at the exit bone, turned and pushed as the data says, scattered, its slots freed.
    fn on_removing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        open_contain::base_on_removing(self, world, me, rider);
        let us = me.object;

        // "object is no longer held inside a MobNexus"
        out(self, world, me, |w| w.contain_clear_disabled(rider, DISABLED_HELD));

        let d = self.data.clone();
        if !d.exit_bone.is_empty() {
            if let Some(world_pos) = out(self, world, me, |w| w.contain_exit_bone_world_position(us, &d.exit_bone)) {
                out(self, world, me, |w| w.contain_set_position(rider, &world_pos));
            }
        }

        if d.orient_like_container_on_exit != crate::logic::modules::plant(1547) {
            let angle = world.objects.find(us).map_or(0.0, |o| o.thing.orientation());
            out(self, world, me, |w| w.contain_set_orientation(rider, angle));
        }

        if d.keep_container_velocity_on_exit {
            let parent = world.objects.find(us).and_then(|o| o.physics);
            let child = world.objects.find(rider).and_then(|o| o.physics);
            if let (Some(parent), Some(child)) = (parent, child) {
                let mut starting_force = out(self, world, me, |w| w.module_call(us, parent, ModuleCall::PhysicsGetVelocity)).as_coord();
                let mass = out(self, world, me, |w| w.module_call(rider, child, ModuleCall::PhysicsGetMass)).as_real(0.0);
                starting_force.x *= mass;
                starting_force.y *= mass;
                if !crate::logic::modules::plant(1548) {
                    starting_force.z *= mass;
                }
                out(self, world, me, |w| w.module_call(rider, child, ModuleCall::PhysicsApplyMotiveForce(starting_force)));

                let com = out(self, world, me, |w| w.module_call(rider, child, ModuleCall::PhysicsGetCenterOfMassOffset)).as_real(0.0);
                let pitch_rate = com * d.exit_pitch_rate;
                out(self, world, me, |w| w.module_call(rider, child, ModuleCall::PhysicsSetPitchRate(pitch_rate)));
            }
        }

        if d.scatter_nearby_on_exit && !crate::logic::modules::plant(1549) {
            open_contain::scatter_to_nearby_position(self, world, me, rider);
        }

        let slots = out(self, world, me, |w| w.get_transport_slot_count(rider));
        self.extra_slots_in_use -= slots - 1;

        // "when we are empty again, clear the model condition for loaded"
        if self.get_contain_count(world) == 0 && !crate::logic::modules::plant(1550) {
            out(self, world, me, |w| w.contain_drawable_model_condition(us, MODELCONDITION_LOADED, false));
        }

        if out(self, world, me, |w| w.is_above_terrain(us)) {
            // "temporarily mark the guy as being allowed to fall (overriding his locomotor's stick-to-ground attribute)"
            if let Some(physics) = world.objects.find(rider).and_then(|o| o.physics) {
                out(self, world, me, |w| w.module_call(rider, physics, ModuleCall::PhysicsSetAllowToFall(true)));
            }
        }
    }

    /// `MobNexusContain::onObjectCreated()`: the initial payload, through the object's contain module (this one).
    fn on_object_created(&mut self, world: &mut World, me: ModuleHandle) {
        let d = self.data.clone();
        let count = d.initial_payload.count;
        let payload_template = world.thing_factory_find_template(&d.initial_payload.name);
        let us = me.object;
        for _ in 0..count {
            // "We are creating a MobNexus that comes with a initial payload, so add it now!"
            let Some(player) = world.get_controlling_player(us) else {
                undefined(world, me, Undefined::NullControllingPlayer);
                return;
            };
            let team = world.cuts.player_default_team(player);
            let Some(template) = payload_template else {
                // `newObject(NULL)` throws ERROR_BAD_ARG out of `onObjectCreated`
                world.cuts.module_errors.push((me, IniError::new(IniErrorKind::BadArg)));
                return;
            };
            let payload = out(self, world, me, |w| w.contain_new_payload_object(template, team, ObjectStatusMaskType::new()));
            if self.is_valid_container_for(world, me, payload, true) && !crate::logic::modules::plant(1551) {
                self.add_to_contain(world, me, payload);
            }
            // else DEBUG_CRASH: "PutInContainer is full, or not valid for the payload" (nothing in a release build)
        }
    }

    /// `MobNexusContain::update()`: the riders healed, then the base's ("extend").
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let d = self.data.clone();
        if d.health_regen != 0.0 && !crate::logic::modules::plant(1552) {
            // "This MobNexus has a health regeneration value, so go through and heal all inside." (`getObject()->getContain()`: this module)
            let us = me.object;
            let items = self.get_contained_items_list(world);
            for object in items {
                // "Determine if we need healing or not."
                let Some((health, max)) = out(self, world, me, |w| w.body_health(object)) else {
                    undefined(world, me, Undefined::NullBodyModule);
                    return open_contain::base_update(self, world, me);
                };
                if health < max || crate::logic::modules::plant(1553) {
                    // "Calculate the health to be regenerated on each unit."
                    let regen = max * d.health_regen / 100.0 * SECONDS_PER_LOGICFRAME_REAL;
                    out(self, world, me, |w| w.attempt_healing(object, regen, us));
                }
            }
            // upstream: `while( *it )` reads `end()` after the last rider
            undefined(world, me, Undefined::EndIteratorDereferenced);
        }
        // extend
        open_contain::base_update(self, world, me)
    }

    /// `MobNexusContain::reserveDoorForExit(objType, specificObject)`: "an override, not an extend".
    fn reserve_door_for_exit(&mut self, world: &mut World, me: ModuleHandle, specific: ObjectId) -> ExitDoorType {
        if specific == INVALID_ID {
            return DOOR_1; // "I can, in general, exit people."
        }
        let us = me.object;

        // "this is present solely for some MobNexuss to override, so that they can land before allowing people to exit..."
        if let Some(ai) = world.objects.find(us).and_then(|o| o.ai) {
            let free = out(self, world, me, |w| w.module_call(us, ai, ModuleCall::AiGetFreeToExit { exiter: us })).as_int(FREE_TO_EXIT);
            if (free != FREE_TO_EXIT) != crate::logic::modules::plant(1554) {
                return DOOR_NONE_AVAILABLE;
            }
        }

        // "I can always kick people out if I am in the air, I know what I'm doing"
        if world.is_using_airborne_locomotor(us) {
            return DOOR_1;
        }

        if world.objects.find(specific).and_then(|o| o.ai).is_none() {
            return DOOR_NONE_AVAILABLE;
        }

        // "He can't get to this spot naturally, so I can't force him there. (amphib MobNexus)" (his locomotor may be NULL: the pathfinder is asked anyway)
        let my_position = world.objects.find(us).map(|o| o.position).unwrap_or_default();
        if !out(self, world, me, |w| w.contain_valid_movement_terrain(us, specific, &my_position)) && !crate::logic::modules::plant(1555) {
            return DOOR_NONE_AVAILABLE;
        }
        DOOR_1
    }

    /// `tryToEvacuate(exposeStealthedUnits)` (the test's op 62).
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::TryToEvacuate { expose_stealthed_units } => ModuleReply::Bool(self.try_to_evacuate(world, me, expose_stealthed_units)),
            _ => ModuleReply::Unhandled,
        }
    }
}

impl UpdateBehavior for MobNexusContain {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.oc.update_module()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        OpenContainVirtuals::update(self, world, me)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::MobNexusContain(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::MobNexusContain(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, _world: &mut World, _me: ModuleHandle, _call: ModuleCall) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

impl Snapshot for MobNexusContain {
    /// `crc`: "extend base class".
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        Snapshot::crc(&mut self.oc, xfer)
    }

    /// `xfer` (version 1): the version, `OpenContain::xfer`, then the extra slots.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        Snapshot::xfer(&mut self.oc, xfer)?;
        if crate::logic::modules::plant(1541) {
            return Ok(());
        }
        xfer.xfer_int(&mut self.extra_slots_in_use)
    }

    /// `loadPostProcess`: "extend base class".
    fn load_post_process(&mut self) {
        Snapshot::load_post_process(&mut self.oc);
    }
}
