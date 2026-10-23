// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DynamicGeometryInfoUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DynamicGeometryInfoUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DynamicGeometryInfoUpdate`: grows or shrinks an object's geometry (a gas cloud, a firestorm) from an initial height and radii to final ones,
//! linearly over `TransitionTime` frames, after `InitialDelay` frames (at least one); with `ReverseAtTransitionTime` it then goes back once.
//!
//! The module data is `ModuleData`-derived upstream; the `UpdateModuleData` base here has no fields and parses none, so it is the same.
//!
//! # Upstream fact kept: `m_direction`
//!
//! The constructor never sets `m_direction`, and `xfer` saves it (`xferUser`, 4 bytes): upstream saves indeterminate bytes. The translation
//! holds 0 (and saves and loads it as upstream does); nothing reads it.
use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::common::geometry::GeometryInfo;

/// `class DynamicGeometryInfoUpdateModuleData : public ModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicGeometryInfoUpdateModuleData {
    /// The base (`UpdateModuleData`, which adds nothing to `ModuleData`).
    pub update: UpdateModuleData,
    /// `m_initialDelay`.
    pub initial_delay: u32,
    /// `m_initialHeight`.
    pub initial_height: f32,
    /// `m_initialMajorRadius`.
    pub initial_major_radius: f32,
    /// `m_initialMinorRadius`.
    pub initial_minor_radius: f32,
    /// `m_finalHeight`.
    pub final_height: f32,
    /// `m_finalMajorRadius`.
    pub final_major_radius: f32,
    /// `m_finalMinorRadius`.
    pub final_minor_radius: f32,
    /// `m_transitionTime`.
    pub transition_time: u32,
    /// `m_reverseAtTransitionTime`.
    pub reverse_at_transition_time: bool,
}

impl Default for DynamicGeometryInfoUpdateModuleData {
    /// `DynamicGeometryInfoUpdateModuleData()`: everything 0 but the transition time, 1.
    fn default() -> Self {
        DynamicGeometryInfoUpdateModuleData {
            update: UpdateModuleData::default(),
            initial_delay: 0,
            initial_height: 0.0,
            initial_major_radius: 0.0,
            initial_minor_radius: 0.0,
            final_height: 0.0,
            final_major_radius: 0.0,
            final_minor_radius: 0.0,
            transition_time: 1,
            reverse_at_transition_time: false,
        }
    }
}

/// `DynamicGeometryInfoUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<DynamicGeometryInfoUpdateModuleData>; 10] = [
    FieldParse::new("InitialDelay", |ini, d, _| { d.initial_delay = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("InitialHeight", |ini, d, _| { d.initial_height = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("InitialMajorRadius", |ini, d, _| { d.initial_major_radius = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("InitialMinorRadius", |ini, d, _| { d.initial_minor_radius = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("FinalHeight", |ini, d, _| { d.final_height = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("FinalMajorRadius", |ini, d, _| { d.final_major_radius = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("FinalMinorRadius", |ini, d, _| { d.final_minor_radius = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("TransitionTime", |ini, d, _| { d.transition_time = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ReverseAtTransitionTime", |ini, d, _| { d.reverse_at_transition_time = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl DynamicGeometryInfoUpdateModuleData {
    /// `buildFieldParse(p)`: the base's (no fields), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, DynamicGeometryInfoUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<DynamicGeometryInfoUpdateModuleData> {
        let mut data = DynamicGeometryInfoUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, DynamicGeometryInfoUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class DynamicGeometryInfoUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct DynamicGeometryInfoUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getDynamicGeometryInfoUpdateModuleData()`.
    data: Arc<DynamicGeometryInfoUpdateModuleData>,
    /// `m_startingDelayCountdown`.
    starting_delay_countdown: u32,
    /// `m_timeActive`.
    time_active: u32,
    /// `m_started`.
    started: bool,
    /// `m_finished`.
    finished: bool,
    /// `m_reverseAtTransitionTime`.
    reverse_at_transition_time: bool,
    /// `m_direction` (indeterminate upstream; see the module docs).
    direction: i32,
    /// `m_switchedDirections`.
    switched_directions: bool,
    /// `m_initialHeight`, `m_initialMajorRadius`, `m_initialMinorRadius`.
    initial: [f32; 3],
    /// `m_finalHeight`, `m_finalMajorRadius`, `m_finalMinorRadius`.
    final_: [f32; 3],
}

impl PartialEq for DynamicGeometryInfoUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl DynamicGeometryInfoUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and
    /// the countdown `max(InitialDelay, 1)`, the data's reverse flag and sizes.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<DynamicGeometryInfoUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(DynamicGeometryInfoUpdate {
            update,
            starting_delay_countdown: data.initial_delay.max(if crate::logic::modules::plant(1921) { 0 } else { 1 }),
            time_active: 0,
            started: false,
            finished: false,
            reverse_at_transition_time: data.reverse_at_transition_time,
            direction: 0,
            switched_directions: false,
            initial: [data.initial_height, data.initial_major_radius, data.initial_minor_radius],
            final_: [data.final_height, data.final_major_radius, data.final_minor_radius],
            data,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getDynamicGeometryInfoUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<DynamicGeometryInfoUpdateModuleData> {
        &self.data
    }


    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object: awake now.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version, that half, the field.
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


    /// The state the test compares: the countdown, the time active, the four flags, the direction and the six sizes (bits).
    pub fn state_words(&self) -> [u32; 13] {
        let mut w = [0u32; 13];
        w[0] = self.starting_delay_countdown;
        w[1] = self.time_active;
        w[2] = u32::from(self.started);
        w[3] = u32::from(self.finished);
        w[4] = u32::from(self.reverse_at_transition_time);
        w[5] = self.direction as u32;
        w[6] = u32::from(self.switched_directions);
        for k in 0..3 {
            w[7 + k] = self.initial[k].to_bits();
            w[10 + k] = self.final_[k].to_bits();
        }
        w
    }
}

impl UpdateBehavior for DynamicGeometryInfoUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if self.finished {
            return UPDATE_SLEEP_NONE;
        }
        if !self.started {
            self.starting_delay_countdown = self.starting_delay_countdown.wrapping_sub(1);
            if self.starting_delay_countdown > 0 {
                return UPDATE_SLEEP_NONE;
            }
            self.started = true;
        }
        let d = self.data.clone();
        let t = if crate::logic::modules::plant(1922) { self.time_active + 1 } else { self.time_active };
        let ratio = t as f32 / d.transition_time as f32;
        let new_height = self.initial[0] + (ratio * (self.final_[0] - self.initial[0]));
        let new_major = self.initial[1] + (ratio * (self.final_[1] - self.initial[1]));
        let new_minor = self.initial[2] + (ratio * (self.final_[2] - self.initial[2]));
        if let Some(old) = world.objects.find(me.object).map(|o| o.geometry_info.clone()) {
            let new_geom = GeometryInfo::new(old.get_geom_type(), old.get_is_small(), new_height, new_major, new_minor);
            world.set_geometry_info(me.object, &new_geom);
        }
        self.time_active = self.time_active.wrapping_add(1);
        if self.time_active > d.transition_time {
            if self.reverse_at_transition_time && !crate::logic::modules::plant(1923) {
                self.switched_directions = true;
                self.time_active = 0;
                self.reverse_at_transition_time = false;
                self.initial = [d.final_height, d.final_major_radius, d.final_minor_radius];
                self.final_ = [d.initial_height, d.initial_major_radius, d.initial_minor_radius];
            } else {
                self.finished = true;
            }
        }
        UPDATE_SLEEP_NONE
    }
    /// The constructor sets no wake frame.
    fn construct(&mut self, _world: &mut World, _me: ModuleHandle) {}
    /// The fields after the base, in upstream's order (`m_direction` as 4 bytes of user data).
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.starting_delay_countdown)?;
        xfer.xfer_unsigned_int(&mut self.time_active)?;
        xfer.xfer_bool(&mut self.started)?;
        if !crate::logic::modules::plant(1924) {
            xfer.xfer_bool(&mut self.finished)?;
        }
        xfer.xfer_bool(&mut self.reverse_at_transition_time)?;
        let mut bytes = self.direction.to_le_bytes();
        xfer.xfer_user(&mut bytes)?;
        self.direction = i32::from_le_bytes(bytes);
        xfer.xfer_bool(&mut self.switched_directions)?;
        for k in 0..3 {
            xfer.xfer_real(&mut self.initial[k])?;
        }
        for k in 0..3 {
            xfer.xfer_real(&mut self.final_[k])?;
        }
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::DynamicGeometryInfoUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::DynamicGeometryInfoUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for DynamicGeometryInfoUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the field.
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

/// The factory's `NewModuleDataProc` for `DynamicGeometryInfoUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::DynamicGeometryInfoUpdate(Arc::new(DynamicGeometryInfoUpdateModuleData::friend_new_module_data(ini)?)))
}
