// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DynamicShroudClearingRangeUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DynamicShroudClearingRangeUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DynamicShroudClearingRangeUpdate`: the spy satellite's shroud clearing, which grows from nothing to the object's native clearing range, holds it, and
//! shrinks to a final vision, on a countdown, with a ring of thirty pseudo-wireframe grid decals that sweep outward while it grows. Never sleeps until it
//! is done, then does so for good.
//!
//! The decals are the client's (`RadiusDecal`, drawn by the projected shadow manager): each call on one is an [`ObjectClientEvent::GridDecal`].
//! `getShroudClearingRange()` / `setShroudClearingRange()` are [`World::get_shroud_clearing_range`] / [`World::set_shroud_clearing_range`].
//!
//! # Upstream facts kept
//!
//! - `m_state` is not saved: after a load it is whatever the object was made with ("not started").
//! - The deadlines are `UnsignedInt`s computed from an `Int` countdown (`m_stateCountDown - delay`), so a delay longer than the total wraps, and every
//!   comparison with the `Int` countdown is made unsigned.
//! - The constructor returns before clearing the decals when the object has no controlling player; `update()` makes them anyway, with a `NULL` owner.
//! - `animateGridDecals` truncates `(Int)pos.x` before `% 23`; an `Int` outside range, or an overflow of `(m_totalFrames - m_stateCountDown) * 2`, is
//!   undefined ([`Undefined::FloatToIntOutOfRange`], [`Undefined::SignedIntOverflow`]).
//! - `m_doSpySatFX` and `m_visionChangePerInterval` are never used.
//! - The `max(1.0f, ..)` of the growing and shrinking divisors is dead code: the state is entered only when the time is at least 1 (checked by a plant that
//!   removes it, which no case catches).
//!
//! `p1w13-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::base_type::{Coord3D, PI};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::trig;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::radius_decal::RadiusDecalTemplate;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::{GridDecalOp, ObjectClientEvent};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `GRID_FX_DECAL_COUNT`.
pub const GRID_FX_DECAL_COUNT: u32 = 30;

/// `enum DSCRU_STATE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DscruState {
    /// `DSCRU_NOT_STARTED_YET`.
    NotStartedYet,
    /// `DSCRU_GROWING`.
    Growing,
    /// `DSCRU_SUSTAINING`.
    Sustaining,
    /// `DSCRU_SHRINKING`.
    Shrinking,
    /// `DSCRU_DONE_FOREVER`.
    DoneForever,
    /// `DSCRU_SLEEPING`.
    Sleeping,
}

impl DscruState {
    /// The enumerator's value.
    pub fn value(self) -> u32 {
        self as u32
    }
}

/// `class DynamicShroudClearingRangeUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicShroudClearingRangeUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_shrinkDelay`: wait until.
    pub shrink_delay: u32,
    /// `m_shrinkTime`: then shrink this fast.
    pub shrink_time: u32,
    /// `m_growDelay`: wait until.
    pub grow_delay: u32,
    /// `m_growTime`: then grow this fast.
    pub grow_time: u32,
    /// `m_finalVision`: then change to this.
    pub final_vision: f32,
    /// `m_changeInterval`: and update my Object every this long.
    pub change_interval: u32,
    /// `m_growInterval`: update every this long while growing.
    pub grow_interval: u32,
    /// `m_doSpySatFX` (never read).
    pub do_spy_sat_fx: bool,
    /// `m_gridDecalTemplate`: for the pseudo-wireframe decal effect.
    pub grid_decal_template: RadiusDecalTemplate,
}

impl Default for DynamicShroudClearingRangeUpdateModuleData {
    /// `DynamicShroudClearingRangeUpdateModuleData()`.
    fn default() -> Self {
        DynamicShroudClearingRangeUpdateModuleData {
            update: UpdateModuleData::new(),
            shrink_delay: 0,
            shrink_time: 0,
            grow_delay: 0,
            grow_time: 0,
            final_vision: 0.0,
            change_interval: 0,
            grow_interval: 0,
            do_spy_sat_fx: false,
            grid_decal_template: RadiusDecalTemplate::default(),
        }
    }
}

/// `DynamicShroudClearingRangeUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<DynamicShroudClearingRangeUpdateModuleData>; 9] = [
    FieldParse::new("ChangeInterval", |ini, d, _| { d.change_interval = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("GrowInterval", |ini, d, _| { d.grow_interval = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ShrinkDelay", |ini, d, _| { d.shrink_delay = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ShrinkTime", |ini, d, _| { d.shrink_time = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("GrowDelay", |ini, d, _| { d.grow_delay = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("GrowTime", |ini, d, _| { d.grow_time = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("FinalVision", |ini, d, _| { d.final_vision = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("GridDecalTemplate", |ini, d, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut d.grid_decal_template), UserData::None),
    FieldParse::end(),
];

impl DynamicShroudClearingRangeUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, DynamicShroudClearingRangeUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<DynamicShroudClearingRangeUpdateModuleData> {
        let mut data = DynamicShroudClearingRangeUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, DynamicShroudClearingRangeUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class DynamicShroudClearingRangeUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct DynamicShroudClearingRangeUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getDynamicShroudClearingRangeUpdateModuleData()`.
    data: Arc<DynamicShroudClearingRangeUpdateModuleData>,
    /// `m_state` (not saved).
    state: DscruState,
    /// `m_stateCountDown`.
    state_count_down: i32,
    /// `m_totalFrames`.
    total_frames: i32,
    /// `m_growStartDeadline`.
    grow_start_deadline: u32,
    /// `m_sustainDeadline`.
    sustain_deadline: u32,
    /// `m_shrinkStartDeadline`.
    shrink_start_deadline: u32,
    /// `m_doneForeverFrame`: just in case interval and state timing goes awry; this supersedes and makes us quit.
    done_forever_frame: u32,
    /// `m_changeIntervalCountdown`: how long till I change my vision range again.
    change_interval_countdown: u32,
    /// `m_decalsCreated`.
    decals_created: bool,
    /// `m_visionChangePerInterval` (never changed).
    vision_change_per_interval: f32,
    /// `m_nativeClearingRange`: what is my object's native vision range?
    native_clearing_range: f32,
    /// `m_currentClearingRange`.
    current_clearing_range: f32,
}

impl PartialEq for DynamicShroudClearingRangeUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.state == other.state
            && self.state_count_down == other.state_count_down
            && self.total_frames == other.total_frames
            && self.grow_start_deadline == other.grow_start_deadline
            && self.sustain_deadline == other.sustain_deadline
            && self.shrink_start_deadline == other.shrink_start_deadline
            && self.done_forever_frame == other.done_forever_frame
            && self.change_interval_countdown == other.change_interval_countdown
            && self.decals_created == other.decals_created
            && self.vision_change_per_interval.to_bits() == other.vision_change_per_interval.to_bits()
            && self.native_clearing_range.to_bits() == other.native_clearing_range.to_bits()
            && self.current_clearing_range.to_bits() == other.current_clearing_range.to_bits()
    }
}

/// `(Int)x` for a float: undefined outside the range of `Int`.
fn float_to_int(x: f32) -> Result<i32, IniError> {
    if (-2_147_483_648.0..2_147_483_648.0).contains(&x) {
        Ok(x as i32)
    } else {
        Err(IniError::undefined(Undefined::FloatToIntOutOfRange))
    }
}

impl DynamicShroudClearingRangeUpdate {
    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data); the state the
    /// constructor computes from the data is [`UpdateBehavior::construct`]'s.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<DynamicShroudClearingRangeUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(DynamicShroudClearingRangeUpdate {
            update,
            data,
            state: DscruState::NotStartedYet,
            state_count_down: 0,
            total_frames: 0,
            grow_start_deadline: 0,
            sustain_deadline: 0,
            shrink_start_deadline: 0,
            done_forever_frame: 0,
            change_interval_countdown: 0,
            decals_created: false,
            vision_change_per_interval: 0.0,
            native_clearing_range: 200.0,
            current_clearing_range: 0.0,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getDynamicShroudClearingRangeUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<DynamicShroudClearingRangeUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's: the state, the countdown, the total, the three deadlines, the
    /// done-forever frame, the interval countdown, the decals flag, and the three ranges (float bits).
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            self.state.value(),
            self.state_count_down as u32,
            self.total_frames as u32,
            self.grow_start_deadline,
            self.sustain_deadline,
            self.shrink_start_deadline,
            self.done_forever_frame,
            self.change_interval_countdown,
            u32::from(self.decals_created),
            self.vision_change_per_interval.to_bits(),
            self.native_clearing_range.to_bits(),
            self.current_clearing_range.to_bits(),
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

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `m_gridDecal[index].<op>`.
    fn decal(world: &mut World, me: ObjectId, index: u32, op: GridDecalOp) {
        world.emit(ObjectClientEvent::GridDecal { object_id: me, index, op });
    }

    /// `createGridDecals(tmpl, radius, pos)`.
    fn create_grid_decals(&self, world: &mut World, me: ObjectId, tmpl: &RadiusDecalTemplate, radius: f32, pos: Coord3D) {
        for d in 0..GRID_FX_DECAL_COUNT {
            Self::decal(world, me, d, GridDecalOp::Clear);
            let player = world.get_controlling_player(me);
            Self::decal(world, me, d, GridDecalOp::Create { template: tmpl.clone(), radius, position: pos, player });
            Self::decal(world, me, d, GridDecalOp::SetPosition(pos));
        }
    }

    /// `killGridDecals()`.
    fn kill_grid_decals(world: &mut World, me: ObjectId) {
        for d in 0..GRID_FX_DECAL_COUNT {
            Self::decal(world, me, d, GridDecalOp::Clear);
        }
    }

    /// `animateGridDecals()`. An error is upstream's undefined behaviour.
    fn animate_grid_decals(&self, world: &mut World, me: ObjectId) -> Result<(), IniError> {
        let ctr = world.objects.find(me).map_or_else(Coord3D::default, |o| o.get_position());
        let mut pos = Coord3D { x: 0.0, y: 0.0, z: 0.0 };

        // `(m_totalFrames - m_stateCountDown) * 2` is an `Int` expression
        let frames = self.total_frames.checked_sub(self.state_count_down).and_then(|f| f.checked_mul(2)).ok_or_else(|| IniError::undefined(Undefined::SignedIntOverflow))?;
        let radius = self.current_clearing_range + frames as f32;
        let mut angle = 0.0f32;
        let angle_inc = (PI * 2.0f32) / GRID_FX_DECAL_COUNT as f32;
        let opacity = if crate::logic::modules::plant(1126) {
            self.current_clearing_range / self.native_clearing_range
        } else {
            1.0f32 - (self.current_clearing_range / self.native_clearing_range)
        };

        for d in 0..GRID_FX_DECAL_COUNT {
            pos.x = ctr.x + (trig::sin(angle) * radius);
            pos.y = ctr.y + (trig::cos(angle) * radius);

            let modulus = if crate::logic::modules::plant(1127) { 22 } else { 23 };
            pos.x -= (float_to_int(pos.x)? % modulus) as f32;
            pos.y -= (float_to_int(pos.y)? % modulus) as f32;

            Self::decal(world, me, d, GridDecalOp::SetPosition(pos));
            Self::decal(world, me, d, GridDecalOp::SetOpacity(opacity));
            angle += angle_inc;
        }
        Ok(())
    }

    /// The body of `update()`. An error is upstream's undefined behaviour.
    fn run(&mut self, world: &mut World, me: ObjectId) -> Result<UpdateSleepTime, IniError> {
        if self.state == DscruState::Sleeping {
            return Ok(UPDATE_SLEEP_NONE);
        }
        // Housekeeping-----------------------------------------
        let md = self.data.clone();
        let current_frame = world.frame();

        if !self.decals_created {
            let pos = world.objects.find(me).map_or_else(Coord3D::default, |o| o.get_position());
            self.create_grid_decals(world, me, &md.grid_decal_template, if crate::logic::modules::plant(1125) { 99.0 } else { 100.0 }, pos);
            self.decals_created = true;
        }
        // -----------------------------------------------------

        // `m_stateCountDown` (an `Int`) is compared with the `UnsignedInt` deadlines as an unsigned value
        let count = self.state_count_down as u32;
        let past_done = if crate::logic::modules::plant(1135) { current_frame >= self.done_forever_frame } else { current_frame > self.done_forever_frame };
        if self.state_count_down <= 0 || past_done {
            self.state = DscruState::DoneForever;
        } else if count <= self.shrink_start_deadline {
            self.state = DscruState::Shrinking;
        } else if count <= self.sustain_deadline {
            self.state = DscruState::Sustaining;
        } else if count <= self.grow_start_deadline {
            self.state = DscruState::Growing;
        }

        match self.state {
            DscruState::NotStartedYet => {
                self.animate_grid_decals(world, me)?;
            }
            DscruState::Growing => {
                self.animate_grid_decals(world, me)?;

                let grow_time = md.grow_time as f32;
                let divisor = if crate::logic::modules::plant(1128) { grow_time + 1.0 } else if 1.0f32 > grow_time { 1.0f32 } else { grow_time };
                self.current_clearing_range += self.native_clearing_range / divisor;
                if self.current_clearing_range >= self.native_clearing_range {
                    self.state = DscruState::Sustaining;
                }
            }
            DscruState::Sustaining => {
                self.current_clearing_range = self.native_clearing_range;
                if !crate::logic::modules::plant(1136) {
                    Self::kill_grid_decals(world, me);
                }
            }
            DscruState::Shrinking => {
                let shrink_time = md.shrink_time as f32;
                let divisor = if 1.0f32 > shrink_time { 1.0f32 } else { shrink_time };
                self.current_clearing_range -= (if crate::logic::modules::plant(1129) { self.native_clearing_range } else { self.native_clearing_range - md.final_vision }) / divisor;
            }
            DscruState::DoneForever => {
                Self::kill_grid_decals(world, me);
                self.current_clearing_range = if crate::logic::modules::plant(1133) { self.native_clearing_range } else { md.final_vision };
            }
            DscruState::Sleeping => {}
        }

        if self.state_count_down > 0 {
            // it is important that this gets called every frame without sleeping because it handles animation and may need to respond to changing
            // vision range from scripts & stuff
            self.state_count_down -= 1;
        }

        if self.change_interval_countdown > 0 {
            self.change_interval_countdown -= 1;
        } else {
            // reset per change timer
            self.change_interval_countdown = if self.state == DscruState::Growing && !crate::logic::modules::plant(1131) { md.grow_interval } else { md.change_interval };
            world.set_shroud_clearing_range(me, self.current_clearing_range);
            if self.state == DscruState::DoneForever && !crate::logic::modules::plant(1130) {
                // We are done forever, and have done the final update, so sleep.
                self.state = DscruState::Sleeping;
            }
        }

        Ok(UPDATE_SLEEP_NONE)
    }
}

impl UpdateBehavior for DynamicShroudClearingRangeUpdate {
    const XFER_VERSION: XferVersion = 1;
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
    /// The rest of the constructor: the countdown and the deadlines from the data (`UnsignedInt` arithmetic on an `Int`), and, for an object with a
    /// controlling player, the native clearing range and thirty cleared decals.
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        let md = self.data.clone();
        self.change_interval_countdown = 0;
        self.vision_change_per_interval = 0.0;

        self.state_count_down = md.shrink_delay.wrapping_add(md.shrink_time) as i32; // total time
        self.total_frames = if crate::logic::modules::plant(1121) { self.state_count_down } else { std::cmp::max(1, self.state_count_down) };
        self.shrink_start_deadline = (self.state_count_down as u32).wrapping_sub(md.shrink_delay);
        self.grow_start_deadline = (self.state_count_down as u32).wrapping_sub(if crate::logic::modules::plant(1122) { md.shrink_delay } else { md.grow_delay });
        self.sustain_deadline = if crate::logic::modules::plant(1123) {
            self.grow_start_deadline.wrapping_add(md.grow_time)
        } else {
            self.grow_start_deadline.wrapping_sub(md.grow_time)
        };

        // a failsafe to force a shutdown on schedule
        self.done_forever_frame = if crate::logic::modules::plant(1124) { world.frame() } else { world.frame().wrapping_add(self.state_count_down as u32) };

        self.decals_created = false;
        self.native_clearing_range = 200.0; // a sensible default
        self.current_clearing_range = 0.0;

        self.state = DscruState::NotStartedYet;

        if world.objects.find(me.object).is_none() {
            return;
        }
        if world.get_controlling_player(me.object).is_none() {
            return;
        }

        if !crate::logic::modules::plant(1134) {
            self.native_clearing_range = world.get_shroud_clearing_range(me.object); // capture this before we start monkeying with it
        }

        Self::kill_grid_decals(world, me.object);
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        // shrink start countdown
        xfer.xfer_int(&mut self.state_count_down)?;
        xfer.xfer_int(&mut self.total_frames)?;
        xfer.xfer_unsigned_int(&mut self.grow_start_deadline)?;
        xfer.xfer_unsigned_int(&mut self.sustain_deadline)?;
        xfer.xfer_unsigned_int(&mut self.shrink_start_deadline)?;
        xfer.xfer_unsigned_int(&mut self.done_forever_frame)?;
        if !crate::logic::modules::plant(1132) {
            xfer.xfer_unsigned_int(&mut self.change_interval_countdown)?;
        }
        // vision change per interval
        xfer.xfer_bool(&mut self.decals_created)?;
        xfer.xfer_real(&mut self.vision_change_per_interval)?;
        xfer.xfer_real(&mut self.native_clearing_range)?;
        xfer.xfer_real(&mut self.current_clearing_range)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::DynamicShroudClearingRangeUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::DynamicShroudClearingRangeUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for DynamicShroudClearingRangeUpdate {
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

/// The factory's `NewModuleDataProc` for `DynamicShroudClearingRangeUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::DynamicShroudClearingRangeUpdate(Arc::new(DynamicShroudClearingRangeUpdateModuleData::friend_new_module_data(ini)?)))
}
