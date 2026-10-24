// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ObjectDefectionHelper.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/ObjectDefectionHelper.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ObjectDefectionHelper`: times the period after a defection (`Object::defect`) in which the defector is "undetected" and invulnerable.
//! `startDefectionTimer(numFrames, withDefectorFX)` (`ModuleCall::StartDefectionTimer`) records the period and wakes the module; every
//! frame `update()` ends the period (`friend_setUndetectedDefector(FALSE)`) when it is over, when the object is dead or when it fires a weapon,
//! and while it lasts flashes the drawable at a rate that grows as the end comes (`m_defectionDetectionFlashPhase`, a real accumulated in single
//! precision) with a tick sound on each flash; at the end a white flash and the "ding" sound. Without `withDefectorFX` (a pilot made merely
//! invulnerable) there is no flash and no sound. It runs while the object is disabled (`DISABLEDMASK_ALL`).
//!
//! `p1w11-sonnet`, 2026-10-21.

use crate::common::disabled_types::disabledmask_all;
use crate::common::ini::IniResult;
use crate::common::module::ObjectId;
use crate::common::object_status_types::OBJECT_STATUS_IS_FIRING_WEAPON;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::update_module::{
    SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER,
    UPDATE_SLEEP_NONE,
};
use crate::logic::modules::object_helper::ObjectHelper;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::{ObjectClientEvent, ObjectSound};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use std::sync::Arc;

/// `DEFECTION_DETECTION_TIME_MAX`: `LOGICFRAMES_PER_SECOND * 10`.
pub const DEFECTION_DETECTION_TIME_MAX: i32 = 30 * 10;

/// `class ObjectDefectionHelper : public ObjectHelper`.
#[derive(Clone, Debug)]
pub struct ObjectDefectionHelper {
    helper: ObjectHelper,
    /// `m_defectionDetectionStart`: "absolute frame, NOT counter".
    defection_detection_start: u32,
    /// `m_defectionDetectionEnd`.
    defection_detection_end: u32,
    /// `m_defectionDetectionFlashPhase`: "keeps track of the flashing rate logarithmic curve".
    defection_detection_flash_phase: f32,
    /// `m_doDefectorFX`.
    do_defector_fx: bool,
}

impl PartialEq for ObjectDefectionHelper {
    /// The same object and the same state (reals by bits).
    fn eq(&self, other: &Self) -> bool {
        self.helper.update_module().behavior().object()
            == other.helper.update_module().behavior().object()
            && self.defection_detection_start == other.defection_detection_start
            && self.defection_detection_end == other.defection_detection_end
            && self.defection_detection_flash_phase.to_bits()
                == other.defection_detection_flash_phase.to_bits()
            && self.do_defector_fx == other.do_defector_fx
    }
}

impl ObjectDefectionHelper {
    /// The first half of the constructor, which touches no world: the `ObjectHelper`, then the four fields zero (`m_defectionDetectionFlashPhase =
    /// FALSE`).
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<UpdateModuleData>>,
    ) -> IniResult<Self> {
        let mut helper =
            ObjectHelper::unconstructed(object, module_data, SleepyUpdatePhase::Normal)?;
        // `getDisabledTypesToProcess()` is `DISABLEDMASK_ALL`; the scheduler reads the arena's base
        helper
            .update_module_mut()
            .set_disabled_types_to_process(disabledmask_all());
        Ok(ObjectDefectionHelper {
            helper,
            defection_detection_start: 0,
            defection_detection_end: 0,
            defection_detection_flash_phase: 0.0,
            do_defector_fx: false,
        })
    }

    /// The `ObjectHelper` base.
    pub fn helper(&self) -> &ObjectHelper {
        &self.helper
    }

    /// The module's own state as words, for the tests that compare it with upstream's.
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            self.defection_detection_start,
            self.defection_detection_end,
            self.defection_detection_flash_phase.to_bits(),
            u32::from(self.do_defector_fx),
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
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version, `ObjectHelper::xfer`, the fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        ObjectHelper::xfer_with(update, xfer)?;
        self.xfer_fields(xfer)
    }

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// The fields after the base.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.defection_detection_start)?;
        xfer.xfer_unsigned_int(&mut self.defection_detection_end)?;
        xfer.xfer_real(&mut self.defection_detection_flash_phase)?;
        xfer.xfer_bool(&mut self.do_defector_fx)
    }

    /// `startDefectionTimer(numFrames, withDefectorFX)`.
    pub fn start_defection_timer(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        num_frames: u32,
        with_defector_fx: bool,
    ) {
        let undetected = world
            .objects
            .find(me.object)
            .is_some_and(|o| o.is_undetected_defector());
        if !undetected {
            update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
            return;
        }
        let now = world.frame();
        self.defection_detection_start = now;
        self.defection_detection_end = if crate::logic::modules::plant(791) {
            now
        } else {
            now.wrapping_add(num_frames)
        };
        self.defection_detection_flash_phase = 0.0;
        self.do_defector_fx = with_defector_fx;
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
    }
}

impl UpdateBehavior for ObjectDefectionHelper {
    fn update_base(&self) -> &UpdateModule {
        self.helper.update_module()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let id = me.object;
        let draw = world.has_drawable(id);
        if !world
            .objects
            .find(id)
            .is_some_and(|o| o.is_undetected_defector())
        {
            return UPDATE_SLEEP_FOREVER;
        }
        let now = world.frame();
        if now >= self.defection_detection_end + u32::from(crate::logic::modules::plant(792)) {
            world.friend_set_undetected_defector(id, false);
            if draw && self.do_defector_fx {
                world.emit(ObjectClientEvent::DrawableFlashAsSelectedColor {
                    object_id: id,
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                });
                if !crate::logic::modules::plant(794) {
                    world.emit(ObjectClientEvent::Sound {
                        sound: ObjectSound::DefectorTimerDing,
                        object_id: id,
                        position: None,
                    });
                }
            }
            return UPDATE_SLEEP_FOREVER; // hey, we're done.
        }
        let (dead, firing) = world.objects.find(id).map_or((false, false), |o| {
            (
                o.is_effectively_dead(),
                o.test_status(OBJECT_STATUS_IS_FIRING_WEAPON),
            )
        });
        if dead || (firing && !crate::logic::modules::plant(795)) {
            world.friend_set_undetected_defector(id, false);
            return UPDATE_SLEEP_FOREVER; // hey, we're done.
        }
        if draw && self.do_defector_fx {
            // skip fx if merely 'invulnerable'
            let last_phase = (self.defection_detection_flash_phase as i32) & 1 != 0;
            let time_left = self.defection_detection_end.wrapping_sub(now);
            self.defection_detection_flash_phase += (if crate::logic::modules::plant(793) {
                0.25f32
            } else {
                0.5f32
            }) * (1.0f32
                - (time_left as f32 / DEFECTION_DETECTION_TIME_MAX as f32));
            let this_phase = (self.defection_detection_flash_phase as i32) & 1 != 0;
            if last_phase && !this_phase {
                world.emit(ObjectClientEvent::DrawableFlashAsSelected { object_id: id });
                world.emit(ObjectClientEvent::Sound {
                    sound: ObjectSound::DefectorTimerTick,
                    object_id: id,
                    position: None,
                });
            }
        }
        UPDATE_SLEEP_NONE
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        self.helper.construct(world);
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::ObjectDefectionHelper(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ObjectDefectionHelper(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::StartDefectionTimer {
                time,
                with_defector_fx,
            } => {
                self.start_defection_timer(world, me, time, with_defector_fx);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for ObjectDefectionHelper {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.helper.crc(xfer)
    }
    /// Version 1: the base, the start, the end, the flash phase, `m_doDefectorFX`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.helper.xfer(xfer)?;
        self.xfer_fields(xfer)
    }
    fn load_post_process(&mut self) {
        self.helper.load_post_process();
    }
}
