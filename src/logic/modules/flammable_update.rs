// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/FlammableUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FlammableUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FlammableUpdate`: sets its object on fire when it has taken enough flame damage (`DAMAGE_FLAME` or `DAMAGE_PARTICLE_BEAM`), keeps it burning for a
//! while (taking damage at an interval, playing a burning sound, optionally spreading the fire), turns it "burned" on an independent timer, and puts
//! the fire out. It is an update module and a damage listener (`onDamage`), and a sleepy one: it sleeps for ever until the fire starts.
//!
//! The sound is [`World::audio_add_event`] / [`World::audio_remove_event`] (the handle is the audio manager's); `FireSpreadUpdate::startFireSpreading()`
//! is a [`ModuleCall::StartFireSpreading`] to the object's module of that class; `getBodyModule()->setAflame` is a [`ModuleCall::SetAflame`] (an
//! object with no body module is dereferenced: [`Undefined::NullBodyModule`]). `tryToIgnite()` and `wouldIgnite()` are the calls `FireSpreadUpdate`
//! makes ([`ModuleCall::TryToIgnite`], [`ModuleCall::WouldIgnite`]).
//!
//! # Upstream facts kept
//!
//! - `m_status` is saved as raw bytes (`xferUser`, the size of the enum).
//! - `~FlammableUpdate` (`stopBurningSound`) is not sent when the object goes (the model has no hook for a module's destruction): **open (deviation)**.
//! - A burned object that is set alight again never ignites (`FS_BURNED`); an object whose `AflameDuration` is 0 sleeps for ever the frame it ignites.
//!
//! `p1w12-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::{MODELCONDITION_AFLAME, MODELCONDITION_SMOLDERING};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DAMAGE};
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_AFLAME, OBJECT_STATUS_BURNED};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_FLAME, DAMAGE_PARTICLE_BEAM};
use crate::logic::weapon_fire::DEATH_BURNED;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `FS_NORMAL`.
pub const FS_NORMAL: u32 = 0;
/// `FS_AFLAME`.
pub const FS_AFLAME: u32 = 1;
/// `FS_BURNED`.
pub const FS_BURNED: u32 = 2;

/// `class FlammableUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct FlammableUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_burnedDelay`: how long before I am burned; 0 means never.
    pub burned_delay: u32,
    /// `m_aflameDuration`: how long I stay aflame, independent of burned.
    pub aflame_duration: u32,
    /// `m_aflameDamageDelay`: while aflame, I take damage this often; 0 means never.
    pub aflame_damage_delay: u32,
    /// `m_aflameDamageAmount`.
    pub aflame_damage_amount: i32,
    /// `m_burningSoundName`: the sound to loop-play while burning.
    pub burning_sound_name: Vec<u8>,
    /// `m_flameDamageLimitData`.
    pub flame_damage_limit_data: f32,
    /// `m_flameDamageExpirationDelay`.
    pub flame_damage_expiration_delay: u32,
}

impl Default for FlammableUpdateModuleData {
    /// `FlammableUpdateModuleData()`.
    fn default() -> Self {
        FlammableUpdateModuleData {
            update: UpdateModuleData::new(),
            burned_delay: 0,
            aflame_duration: 0,
            aflame_damage_delay: 0,
            aflame_damage_amount: 0,
            burning_sound_name: Vec::new(),
            flame_damage_limit_data: 20.0,
            flame_damage_expiration_delay: LOGICFRAMES_PER_SECOND as u32 * 2,
        }
    }
}

/// `FlammableUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<FlammableUpdateModuleData>; 8] = [
    FieldParse::new("BurnedDelay", |ini, d, _| { d.burned_delay = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("AflameDuration", |ini, d, _| { d.aflame_duration = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("AflameDamageDelay", |ini, d, _| { d.aflame_damage_delay = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("AflameDamageAmount", |ini, d, _| { d.aflame_damage_amount = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("BurningSoundName", |ini, d, _| { d.burning_sound_name = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("FlameDamageLimit", |ini, d, _| { d.flame_damage_limit_data = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("FlameDamageExpiration", |ini, d, _| { d.flame_damage_expiration_delay = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl FlammableUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, FlammableUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<FlammableUpdateModuleData> {
        let mut data = FlammableUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, FlammableUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class FlammableUpdate : public UpdateModule, public DamageModuleInterface`.
#[derive(Clone, Debug)]
pub struct FlammableUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getFlammableUpdateModuleData()`.
    data: Arc<FlammableUpdateModuleData>,
    /// `m_status`: `FS_NORMAL`, `FS_AFLAME` or `FS_BURNED`.
    status: u32,
    /// `m_aflameEndFrame`.
    aflame_end_frame: u32,
    /// `m_burnedEndFrame`.
    burned_end_frame: u32,
    /// `m_damageEndFrame`.
    damage_end_frame: u32,
    /// `m_audioHandle`.
    audio_handle: u32,
    /// `m_flameDamageLimit`.
    flame_damage_limit: f32,
    /// `m_lastFlameDamageDealt`.
    last_flame_damage_dealt: u32,
}

impl PartialEq for FlammableUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.status == other.status
            && self.aflame_end_frame == other.aflame_end_frame
            && self.burned_end_frame == other.burned_end_frame
            && self.damage_end_frame == other.damage_end_frame
            && self.audio_handle == other.audio_handle
            && self.flame_damage_limit.to_bits() == other.flame_damage_limit.to_bits()
            && self.last_flame_damage_dealt == other.last_flame_damage_dealt
    }
}

impl FlammableUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DAMAGE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DAMAGE
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the state.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<FlammableUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        let flame_damage_limit = data.flame_damage_limit_data;
        Ok(FlammableUpdate {
            update,
            data,
            status: FS_NORMAL,
            aflame_end_frame: 0,
            burned_end_frame: 0,
            damage_end_frame: 0,
            audio_handle: 0,
            flame_damage_limit,
            last_flame_damage_dealt: 0,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getFlammableUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<FlammableUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's: the status, the three end frames, the audio handle, the flame damage
    /// limit (float bits) and the frame of the last flame damage.
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            self.status,
            self.aflame_end_frame,
            self.burned_end_frame,
            self.damage_end_frame,
            self.audio_handle,
            self.flame_damage_limit.to_bits(),
            self.last_flame_damage_dealt,
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

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `onDamage(damageInfo)`.
    pub fn on_damage(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) -> Result<(), IniError> {
        if info.input.damage_type == DAMAGE_FLAME || info.input.damage_type == DAMAGE_PARTICLE_BEAM {
            let now = world.frame();
            if now.wrapping_sub(self.data.flame_damage_expiration_delay) > self.last_flame_damage_dealt {
                // If it has been a long time since our last flame damage, reset the threshold
                if !crate::logic::modules::plant(984) {
                    self.flame_damage_limit = self.data.flame_damage_limit_data;
                }
            }
            self.last_flame_damage_dealt = if crate::logic::modules::plant(981) { self.last_flame_damage_dealt } else { now };

            let (aflame, burned) = world.objects.find(me.object).map_or((false, false), |o| (o.test_status(OBJECT_STATUS_AFLAME), o.test_status(OBJECT_STATUS_BURNED)));
            if !aflame && !burned {
                // If I'm not on fire, and I haven't burned up, see if I should try to catch fire.
                self.flame_damage_limit -= info.out.actual_damage_dealt;
                if self.flame_damage_limit <= 0.0 {
                    self.try_to_ignite(world, me)?;
                }
            }
        }
        Ok(())
    }

    /// `calcSleepTime()`.
    fn calc_sleep_time(&self, world: &World) -> UpdateSleepTime {
        let now = world.frame();
        if self.status == FS_AFLAME && self.aflame_end_frame != 0 && self.aflame_end_frame > now {
            let mut soonest = self.aflame_end_frame;
            if self.burned_end_frame != 0 && self.burned_end_frame < soonest && self.burned_end_frame > now {
                soonest = self.burned_end_frame;
            }
            if self.damage_end_frame != 0 && self.damage_end_frame < soonest && self.damage_end_frame > now && !crate::logic::modules::plant(985) {
                soonest = self.damage_end_frame;
            }
            // UPDATE_SLEEP requires a count-of-frames, not an absolute-frame, so subtract 'now'
            return update_sleep(soonest.wrapping_sub(now));
        }
        UPDATE_SLEEP_FOREVER
    }

    /// `tryToIgnite()`: "FlammableDamage uses this. It is up to me to decide if I am burnable".
    pub fn try_to_ignite(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        if self.status == FS_NORMAL || crate::logic::modules::plant(987) {
            let mut aflame = ObjectStatusMaskType::default();
            aflame.set(OBJECT_STATUS_AFLAME, 1);
            world.set_status(me.object, aflame, true);
            self.set_body_aflame(world, me.object, true)?;
            world.set_model_condition_state(me.object, MODELCONDITION_AFLAME);
            self.start_burning_sound(world, me.object);

            // bleah. this sucks. (srj)
            if let Some(fu) = world.find_module(me.object, b"FireSpreadUpdate").filter(|_| !crate::logic::modules::plant(986)) {
                world.module_call(me.object, fu, ModuleCall::StartFireSpreading);
            }

            self.status = FS_AFLAME;

            let d = self.data.clone();
            let now = world.frame();
            self.aflame_end_frame = now.wrapping_add(d.aflame_duration);
            self.burned_end_frame = if d.burned_delay != 0 || crate::logic::modules::plant(989) { now.wrapping_add(d.burned_delay) } else { 0 };
            self.damage_end_frame = if d.aflame_damage_delay != 0 { now.wrapping_add(d.aflame_damage_delay) } else { 0 };

            let t = self.calc_sleep_time(world);
            update_behavior::set_wake_frame(world, t);
        }
        Ok(())
    }

    /// `wouldIgnite()`.
    pub fn would_ignite(&self) -> bool {
        self.status == FS_NORMAL
    }

    /// `me->getBodyModule()->setAflame(setting)`; `NULL` body is dereferenced.
    fn set_body_aflame(&self, world: &mut World, me: ObjectId, setting: bool) -> Result<(), IniError> {
        let body = world.get_body_module(me).ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
        world.module_call(me, body, ModuleCall::SetAflame(setting));
        Ok(())
    }

    /// `doAflameDamage()`.
    fn do_aflame_damage(&mut self, world: &mut World, me: ObjectId) {
        let mut info = DamageInfo::default();
        info.input.amount = self.data.aflame_damage_amount as f32;
        info.input.source_id = me;
        info.input.damage_type = DAMAGE_FLAME;
        info.input.death_type = DEATH_BURNED;
        world.attempt_damage(me, &mut info);
    }

    /// `startBurningSound()`.
    fn start_burning_sound(&mut self, world: &mut World, me: ObjectId) {
        self.audio_handle = world.audio_add_event(self.data.burning_sound_name.clone(), me);
    }

    /// `stopBurningSound()`.
    fn stop_burning_sound(&mut self, world: &mut World) {
        if self.audio_handle != 0 {
            world.audio_remove_event(self.audio_handle);
            self.audio_handle = 0;
        }
    }

    /// The body of `update()` with the undefined behaviour as an error.
    fn update_checked(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        let now = world.frame();
        let data = self.data.clone();

        if self.damage_end_frame != 0 && now >= self.damage_end_frame {
            self.damage_end_frame = now.wrapping_add(data.aflame_damage_delay);
            self.do_aflame_damage(world, me.object);
        }

        if self.burned_end_frame != 0 && now >= self.burned_end_frame {
            // So this status is set, but I am still aflame on an independent timer.
            let mut burned = ObjectStatusMaskType::default();
            burned.set(OBJECT_STATUS_BURNED, 1);
            world.set_status(me.object, burned, true);
            world.set_model_condition_state(me.object, MODELCONDITION_SMOLDERING);
        }

        if self.aflame_end_frame != 0 && now >= self.aflame_end_frame {
            // This is the important one. I am no longer on fire.
            if world.objects.find(me.object).is_some_and(|o| o.test_status(OBJECT_STATUS_BURNED)) && !crate::logic::modules::plant(982) {
                // If I am burned, then I will never catch fire again.
                self.status = FS_BURNED;
            } else {
                // otherwise I am free to burn again
                self.status = FS_NORMAL;
            }
            if !crate::logic::modules::plant(988) {
                self.stop_burning_sound(world);
            }
            let mut aflame = ObjectStatusMaskType::default();
            aflame.set(OBJECT_STATUS_AFLAME, 1);
            world.clear_status(me.object, aflame);
            self.set_body_aflame(world, me.object, false)?;
            world.clear_model_condition_state(me.object, MODELCONDITION_AFLAME);
        }
        Ok(())
    }
}

impl UpdateBehavior for FlammableUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(e) = self.update_checked(world, me) {
            world.cuts.module_errors.push((me, e));
            return UPDATE_SLEEP_FOREVER;
        }
        self.calc_sleep_time(world)
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        // flammability status (`xferUser`: the raw bytes of the enum)
        let mut bytes = self.status.to_le_bytes();
        xfer.xfer_user(&mut bytes)?;
        self.status = u32::from_le_bytes(bytes);
        xfer.xfer_unsigned_int(&mut self.aflame_end_frame)?;
        xfer.xfer_unsigned_int(&mut self.burned_end_frame)?;
        xfer.xfer_unsigned_int(&mut self.damage_end_frame)?;
        xfer.xfer_real(&mut self.flame_damage_limit)?;
        if crate::logic::modules::plant(983) {
            return Ok(());
        }
        xfer.xfer_unsigned_int(&mut self.last_flame_damage_dealt)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::FlammableUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FlammableUpdate(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDamage(info) => {
                if let Err(e) = self.on_damage(world, me, &info) {
                    world.cuts.module_errors.push((me, e));
                }
                ModuleReply::Done
            }
            ModuleCall::TryToIgnite => {
                if let Err(e) = self.try_to_ignite(world, me) {
                    world.cuts.module_errors.push((me, e));
                }
                ModuleReply::Done
            }
            ModuleCall::WouldIgnite => ModuleReply::Bool(self.would_ignite()),
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for FlammableUpdate {
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

/// The factory's `NewModuleDataProc` for `FlammableUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::FlammableUpdate(Arc::new(FlammableUpdateModuleData::friend_new_module_data(ini)?)))
}
