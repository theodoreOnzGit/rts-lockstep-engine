// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/MissileLauncherBuildingUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/MissileLauncherBuildingUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `MissileLauncherBuildingUpdate`: the door of a superweapon's silo (a special power update module). The door opens `DoorOpenTime` before
//! the special power is ready, is popped open if the power is ready first, waits `DoorWaitOpenTime` after the power fires
//! (`initiateIntentToDoSpecialPower`), then closes over `DoorCloseTime` (at most half the time to the next ready frame). Each state sets the
//! door's model condition, plays its FX at the building, and the open state loops the idle sound; the drawable's animation is timed to the
//! state's timeout.
//!
//! # Upstream facts kept
//!
//! - `m_specialPowerModule` is not set by the constructor: the memory pool's zero, `NULL`, so the first `update` (of a finished building) looks
//!   the module up; it is not saved, and a lookup that finds nothing is tried again on the next update. `initiateIntentToDoSpecialPower`
//!   before it is found dereferences `NULL`: `Undefined::NullSpecialPowerModule`.
//! - `xfer` extends `UpdateModule`'s, not `SpecialPowerUpdateModule`'s (no second version byte); the door state, the timeout state (as
//!   their raw words) and the timeout frame are saved; the idle sound's playing handle is not.
//! - The closing timeout's cap is `now + delta / 2` with `delta` the signed distance to the ready frame, added as unsigned.
//!
//! `p1w13-sonnet`, batch R53, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::{MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_DOOR_1_WAITING_OPEN, MODELCONDITION_DOOR_1_WAITING_TO_CLOSE};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::special_power_update_module::SpecialPowerUpdateModule;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object::ModuleIndex;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `DoorStateType`.
pub const DOOR_CLOSED: i32 = 0;
pub const DOOR_OPENING: i32 = 1;
pub const DOOR_OPEN: i32 = 2;
pub const DOOR_WAITING_TO_CLOSE: i32 = 3;
pub const DOOR_CLOSING: i32 = 4;

/// `class MissileLauncherBuildingUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default)]
pub struct MissileLauncherBuildingUpdateModuleData {
    pub update: UpdateModuleData,
    /// `m_specialPowerTemplate`, by name (found in the store when used; an unknown name, and `None`, is `NULL`).
    pub special_power_template: Option<Vec<u8>>,
    /// `m_doorOpenTime`, `m_doorWaitOpenTime`, `m_doorClosingTime` (frames).
    pub door_open_time: u32,
    pub door_wait_open_time: u32,
    pub door_closing_time: u32,
    /// `m_openingFX`, `m_openFX`, `m_waitingToCloseFX`, `m_closingFX`, `m_closedFX`, by name (`None`: no list).
    pub opening_fx: Option<Vec<u8>>,
    pub open_fx: Option<Vec<u8>>,
    pub waiting_to_close_fx: Option<Vec<u8>>,
    pub closing_fx: Option<Vec<u8>>,
    pub closed_fx: Option<Vec<u8>>,
    /// `m_openIdleAudio`'s event name.
    pub open_idle_audio: Vec<u8>,
}

/// `INI::parseFXList`: `None` is no list; any other name is looked up when used.
fn parse_fx(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let token = ini.get_next_token(None)?;
    Ok((!token.eq_ignore_ascii_case(b"None")).then_some(token))
}

static FIELD_PARSE: [FieldParse<MissileLauncherBuildingUpdateModuleData>; 11] = [
    FieldParse::new(
        "SpecialPowerTemplate",
        |i, d, _| {
            d.special_power_template = Some(i.get_next_token(None)?);
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("DoorOpenTime", |i, d, _| { d.door_open_time = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("DoorWaitOpenTime", |i, d, _| { d.door_wait_open_time = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("DoorCloseTime", |i, d, _| { d.door_closing_time = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("DoorOpeningFX", |i, d, _| { d.opening_fx = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DoorOpenFX", |i, d, _| { d.open_fx = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DoorWaitingToCloseFX", |i, d, _| { d.waiting_to_close_fx = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DoorClosingFX", |i, d, _| { d.closing_fx = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DoorClosedFX", |i, d, _| { d.closed_fx = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DoorOpenIdleAudio", |i, d, _| { d.open_idle_audio = i.get_next_token(None)?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl MissileLauncherBuildingUpdateModuleData {
    /// `buildFieldParse(p)`: the update base's, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, MissileLauncherBuildingUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<MissileLauncherBuildingUpdateModuleData> {
        let mut data = MissileLauncherBuildingUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, MissileLauncherBuildingUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::MissileLauncherBuildingUpdate(Arc::new(MissileLauncherBuildingUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class MissileLauncherBuildingUpdate : public SpecialPowerUpdateModule`.
#[derive(Clone, Debug)]
pub struct MissileLauncherBuildingUpdate {
    base: SpecialPowerUpdateModule,
    data: Arc<MissileLauncherBuildingUpdateModuleData>,
    /// `m_specialPowerModule` (the pool's zero, `NULL`, until the first update finds it).
    special_power_module: Option<ModuleIndex>,
    /// `m_doorState`, `m_timeoutState`, `m_timeoutFrame`.
    door_state: i32,
    timeout_state: i32,
    timeout_frame: u32,
    /// `m_openIdleAudio`'s playing handle.
    open_idle_handle: u32,
}

impl PartialEq for MissileLauncherBuildingUpdate {
    fn eq(&self, other: &Self) -> bool {
        self.base.update().behavior().object() == other.base.update().behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.special_power_module == other.special_power_module
            && self.state_words() == other.state_words()
    }
}

impl MissileLauncherBuildingUpdate {
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerUpdateModule::get_interface_mask()
    }

    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<MissileLauncherBuildingUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = SpecialPowerUpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(MissileLauncherBuildingUpdate {
            base,
            data,
            special_power_module: None,
            door_state: DOOR_CLOSED,
            timeout_state: DOOR_CLOSED,
            timeout_frame: 0,
            open_idle_handle: 0,
        })
    }

    pub fn update_module(&self) -> &UpdateModule {
        self.base.update()
    }

    pub fn module_data(&self) -> &Arc<MissileLauncherBuildingUpdateModuleData> {
        &self.data
    }

    /// The door state, the timeout state and frame, the idle sound's handle, `isActive`.
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            self.door_state as u32,
            self.timeout_state as u32,
            self.timeout_frame,
            self.open_idle_handle,
            u32::from(self.door_state != self.timeout_state),
        ]
    }

    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    fn template_id(world: &World, name: &Option<Vec<u8>>) -> Option<u32> {
        name.as_deref().and_then(|n| world.statics.special_power_store.find_special_power_template(n)).map(|t| t.id())
    }

    fn ready_frame(world: &mut World, us: ObjectId, sp: ModuleIndex) -> u32 {
        world.module_call(us, sp, ModuleCall::GetReadyFrame).as_int(0) as u32
    }

    /// `m_openIdleAudio.isCurrentlyPlaying()` then `TheAudio->removeAudioEvent(...)`.
    fn stop_idle_audio(&mut self, world: &mut World) {
        if world.audio_is_currently_playing(self.open_idle_handle) && !crate::logic::modules::plant(2231) {
            world.audio_remove_event(self.open_idle_handle);
        }
    }

    fn do_fx(world: &mut World, us: ObjectId, fx: &Option<Vec<u8>>) {
        if let Some(fx) = fx.clone() {
            let position = world.objects.find(us).map_or_else(Coord3D::default, |o| o.get_position());
            world.emit(ObjectClientEvent::FxListDoFxPos { fx, position });
        }
    }

    /// `switchToState(dst)`.
    fn switch_to_state(&mut self, world: &mut World, me: ModuleHandle, dst: i32) -> Result<(), IniError> {
        if self.door_state == dst {
            return Ok(());
        }
        let d = self.data.clone();
        let us = me.object;
        let now = world.frame();
        let (clr, set): (Vec<i32>, Vec<i32>);
        match dst {
            DOOR_CLOSED => {
                clr = vec![MODELCONDITION_DOOR_1_WAITING_TO_CLOSE, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_DOOR_1_WAITING_OPEN];
                set = vec![];
                self.timeout_frame = 0;
                self.timeout_state = DOOR_CLOSED;
                Self::do_fx(world, us, &d.closed_fx);
                self.stop_idle_audio(world);
            }
            DOOR_OPENING => {
                clr = vec![MODELCONDITION_DOOR_1_WAITING_TO_CLOSE, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_WAITING_OPEN];
                set = vec![MODELCONDITION_DOOR_1_OPENING];
                let sp = self.special_power_module.ok_or_else(|| IniError::undefined(Undefined::NullSpecialPowerModule))?;
                self.timeout_frame = Self::ready_frame(world, us, sp).wrapping_sub(if crate::logic::modules::plant(2232) { 0 } else { 1 });
                self.timeout_state = DOOR_OPEN;
                Self::do_fx(world, us, &d.opening_fx);
                self.stop_idle_audio(world);
            }
            DOOR_OPEN => {
                clr = vec![MODELCONDITION_DOOR_1_WAITING_TO_CLOSE, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING];
                set = vec![MODELCONDITION_DOOR_1_WAITING_OPEN];
                self.timeout_frame = 0;
                self.timeout_state = DOOR_OPEN;
                Self::do_fx(world, us, &d.open_fx);
                if !world.audio_is_currently_playing(self.open_idle_handle) {
                    self.open_idle_handle = world.audio_add_event(d.open_idle_audio.clone(), us);
                }
            }
            DOOR_WAITING_TO_CLOSE => {
                clr = vec![MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_DOOR_1_WAITING_OPEN];
                set = vec![MODELCONDITION_DOOR_1_WAITING_TO_CLOSE];
                self.timeout_frame = now.wrapping_add(d.door_wait_open_time);
                self.timeout_state = if crate::logic::modules::plant(2233) { DOOR_CLOSED } else { DOOR_CLOSING };
                Self::do_fx(world, us, &d.waiting_to_close_fx);
                self.stop_idle_audio(world);
            }
            _ => {
                // DOOR_CLOSING
                clr = vec![MODELCONDITION_DOOR_1_WAITING_TO_CLOSE, MODELCONDITION_DOOR_1_WAITING_OPEN, MODELCONDITION_DOOR_1_OPENING];
                set = vec![MODELCONDITION_DOOR_1_CLOSING];
                self.timeout_frame = now.wrapping_add(d.door_closing_time);
                let sp = self.special_power_module.ok_or_else(|| IniError::undefined(Undefined::NullSpecialPowerModule))?;
                let delta = Self::ready_frame(world, us, sp).wrapping_sub(now) as i32;
                let cap = now.wrapping_add((delta / 2) as u32);
                if self.timeout_frame > cap && !crate::logic::modules::plant(2234) {
                    self.timeout_frame = cap;
                }
                self.timeout_state = DOOR_CLOSED;
                Self::do_fx(world, us, &d.closing_fx);
                self.stop_idle_audio(world);
            }
        }
        self.door_state = dst;
        world.clear_and_set_model_condition_flags(us, &clr, &set);
        if self.timeout_frame > 0 && world.has_drawable(us) && !crate::logic::modules::plant(2235) {
            world.emit(ObjectClientEvent::DrawableSetAnimationLoopDuration { object_id: us, frames: self.timeout_frame.wrapping_sub(now) });
        }
        Ok(())
    }

    fn run_update(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        let d = self.data.clone();
        let us = me.object;
        let now = world.frame();
        if world.objects.find(us).is_some_and(|o| o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION)) {
            return Ok(());
        }
        if self.special_power_module.is_none() {
            let template = Self::template_id(world, &d.special_power_template);
            self.special_power_module = world.get_special_power_module(us, template);
        }
        if let Some(sp) = self.special_power_module {
            let ready_frame = Self::ready_frame(world, us, sp);
            let open_time = if crate::logic::modules::plant(2239) { 0 } else { d.door_open_time };
            let when_to_start_opening = ready_frame.saturating_sub(open_time);
            if self.timeout_frame != 0 && now > self.timeout_frame {
                let to = self.timeout_state;
                self.switch_to_state(world, me, to)?;
            }
            let ready = world.module_call(us, sp, ModuleCall::IsReady).as_bool();
            if self.door_state != DOOR_OPEN && ready && !crate::logic::modules::plant(2236) {
                // "had to POP the door open!"
                self.switch_to_state(world, me, DOOR_OPEN)?;
            } else if self.door_state == DOOR_CLOSED && now >= when_to_start_opening {
                self.switch_to_state(world, me, DOOR_OPENING)?;
            }
        }
        Ok(())
    }

    /// `initiateIntentToDoSpecialPower(template, ...)`.
    fn initiate_intent(&mut self, world: &mut World, me: ModuleHandle, template: u32) -> Result<bool, IniError> {
        let sp = self.special_power_module.ok_or_else(|| IniError::undefined(Undefined::NullSpecialPowerModule))?;
        let ours = match world.module_call(me.object, sp, ModuleCall::GetSpecialPowerTemplateId) {
            ModuleReply::Int(id) => id as u32,
            _ => 0,
        };
        if ours != template && !crate::logic::modules::plant(2238) {
            return Ok(false);
        }
        self.switch_to_state(world, me, DOOR_WAITING_TO_CLOSE)?;
        Ok(true)
    }
}

impl UpdateBehavior for MissileLauncherBuildingUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.base.update()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(e) = self.run_update(world, me) {
            world.cuts.module_errors.push((me, e));
        }
        UPDATE_SLEEP_NONE
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        // `xferUser(&m_doorState, sizeof(DoorStateType))`, the timeout state, the frame
        let mut b = self.door_state.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        self.door_state = i32::from_le_bytes(b);
        let mut b = self.timeout_state.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        self.timeout_state = i32::from_le_bytes(b);
        if crate::logic::modules::plant(2237) {
            return Ok(());
        }
        xfer.xfer_unsigned_int(&mut self.timeout_frame)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::MissileLauncherBuildingUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::MissileLauncherBuildingUpdate(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::InitiateIntentToDoSpecialPower { template, .. } => match self.initiate_intent(world, me, template) {
                Ok(b) => ModuleReply::Bool(b),
                Err(e) => {
                    world.cuts.module_errors.push((me, e));
                    ModuleReply::Bool(false)
                }
            },
            ModuleCall::DoesSpecialPowerUpdatePassScienceTest => {
                ModuleReply::Bool(self.base.does_special_power_update_pass_science_test(self.base.get_extra_required_science(), |_| false))
            }
            ModuleCall::DoesSpecialPowerHaveOverridableDestinationActive | ModuleCall::DoesSpecialPowerHaveOverridableDestination => ModuleReply::Bool(false),
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for MissileLauncherBuildingUpdate {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.update_mut().crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.base.update().clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        *self.base.update_mut() = base;
        r
    }
    fn load_post_process(&mut self) {
        self.base.update_mut().load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_door_starts_closed_with_no_power_module() {
        let m = MissileLauncherBuildingUpdate::unconstructed(1, Some(Arc::new(MissileLauncherBuildingUpdateModuleData::default()))).unwrap();
        assert_eq!(m.state_words(), vec![0, 0, 0, 0, 0]);
        assert!(m.special_power_module.is_none());
    }
}
