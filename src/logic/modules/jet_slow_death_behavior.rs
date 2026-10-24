// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/JetSlowDeathBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/JetSlowDeathBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `JetSlowDeathBehavior`: a [`SlowDeathBehavior`] for aircraft. A jet that dies on (or near) the ground, or on a carrier's deck, blows up
//! there (its ground-death FX and OCL) and is destroyed. One that dies in the air begins the ordinary slow death and then falls: its FX and
//! OCL of the initial death, a looping death sound, its locomotor's lift cut (by `FallHowFast`) and turning stopped, and every frame a roll
//! (`RollRate`, multiplied by `RollRateDelta` each frame). After `DelaySecondaryFromInitialDeath` it plays its secondary FX and OCL; when it
//! reaches the ground of the layer under it (or hits a tree) the sound stops, its hit-ground FX and OCL play and it pitches; after
//! `DelayFinalBlowUpFromHitGround` it blows up for the last time and is destroyed.
//!
//! The module is a [`SlowDeathBehavior`] (the base's state and methods, [`slow_death_field_parse!`](crate::slow_death_field_parse) for its data)
//! plus this class's: `OnDie` and `BeginSlowDeath` are this class's, the other calls the base's.
//!
//! # Upstream facts kept
//!
//! - The ground death does not ask the die mux (`isDieApplicable`): any death near the ground blows the jet up.
//! - `OBJECT_STATUS_DECK_HEIGHT_OFFSET` is cleared after either death.
//! - The secondary blast's timer is only looked at before the jet reaches the ground; a jet that reaches the ground at frame 0 never counts as
//!   on the ground (`m_timerOnGroundFrame == 0` is the test).
//! - The sound is not saved (nor its handle): after a load, reaching the ground removes handle 0.
//! - The xfer is version 1: the version, `SlowDeathBehavior`'s (its own version 1, the `UpdateModule` base, its fields), then the death
//!   frame, the on-ground frame and the roll rate.
//!
//! # Where upstream is undefined
//!
//! A jet that begins its slow death with no AI (`NullAi`) or no current locomotor (`getCurLocomotor()->` through `NULL`: `NullObject`),
//! recorded in `world.cuts.module_errors`; and the base's.
//!
//! `p1w13-sonnet`, batch R46, 2026-10-24.

use std::sync::Arc;

use crate::common::game_type::{INVALID_ID, LAYER_GROUND};
use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KINDOF_SHRUBBERY;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::object_status_types::OBJECT_STATUS_DECK_HEIGHT_OFFSET;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{UpdateModule, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::slow_death_behavior::{SlowDeathBehavior, SlowDeathBehaviorModuleData};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::object_state::mask_of;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// An FX list by name (`None` for `NULL`).
type FxName = Option<Vec<u8>>;

/// `class JetSlowDeathBehaviorModuleData : public SlowDeathBehaviorModuleData`.
#[derive(Clone, Debug)]
pub struct JetSlowDeathBehaviorModuleData {
    /// The base.
    pub slow: SlowDeathBehaviorModuleData,
    /// `m_fxOnGroundDeath`, `m_oclOnGroundDeath`: "for deaths on the ground".
    pub fx_on_ground_death: FxName,
    pub ocl_on_ground_death: Option<Vec<u8>>,
    /// `m_fxInitialDeath`, `m_oclInitialDeath`: "for the initial death in the air".
    pub fx_initial_death: FxName,
    pub ocl_initial_death: Option<Vec<u8>>,
    /// `m_delaySecondaryFromInitialDeath`, `m_fxSecondary`, `m_oclSecondary`.
    pub delay_secondary_from_initial_death: u32,
    pub fx_secondary: FxName,
    pub ocl_secondary: Option<Vec<u8>>,
    /// `m_fxHitGround`, `m_oclHitGround`.
    pub fx_hit_ground: FxName,
    pub ocl_hit_ground: Option<Vec<u8>>,
    /// `m_delayFinalBlowUpFromHitGround`, `m_fxFinalBlowUp`, `m_oclFinalBlowUp`.
    pub delay_final_blow_up_from_hit_ground: u32,
    pub fx_final_blow_up: FxName,
    pub ocl_final_blow_up: Option<Vec<u8>>,
    /// `m_deathLoopSound`'s event name (empty: none).
    pub death_loop_sound: Vec<u8>,
    /// `m_rollRate`, `m_rollRateDelta`, `m_pitchRate`, `m_fallHowFast`.
    pub roll_rate: f32,
    pub roll_rate_delta: f32,
    pub pitch_rate: f32,
    pub fall_how_fast: f32,
}

impl Default for JetSlowDeathBehaviorModuleData {
    /// `JetSlowDeathBehaviorModuleData()`.
    fn default() -> Self {
        JetSlowDeathBehaviorModuleData {
            slow: SlowDeathBehaviorModuleData::default(),
            fx_on_ground_death: None,
            ocl_on_ground_death: None,
            fx_initial_death: None,
            ocl_initial_death: None,
            delay_secondary_from_initial_death: 0,
            fx_secondary: None,
            ocl_secondary: None,
            fx_hit_ground: None,
            ocl_hit_ground: None,
            delay_final_blow_up_from_hit_ground: 0,
            fx_final_blow_up: None,
            ocl_final_blow_up: None,
            death_loop_sound: Vec::new(),
            roll_rate: 0.0,
            roll_rate_delta: 1.0,
            pitch_rate: 0.0,
            fall_how_fast: 0.0,
        }
    }
}

/// `INI::parseFXList`: the next token, `None` for `NULL`.
fn parse_fx(ini: &mut Ini<'_>) -> IniResult<FxName> {
    let name = ini.get_next_token(None)?;
    Ok(if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) })
}

/// `INI::parseObjectCreationList`: the next token, `None` for `NULL` (found in the store when it is used).
fn parse_ocl(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let name = ini.get_next_token(None)?;
    Ok(if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) })
}

/// `INI::parseAudioEventRTS`: the next token, the event's name unless it is `NoSound`.
fn parse_sound(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let token = ini.get_next_token(None)?;
    Ok((!token.eq_ignore_ascii_case(b"NoSound")).then_some(token))
}

fn slow_of(d: &mut JetSlowDeathBehaviorModuleData) -> &mut SlowDeathBehaviorModuleData {
    &mut d.slow
}

/// `SlowDeathBehaviorModuleData::buildFieldParse`'s table.
static SLOW_FIELD_PARSE: [FieldParse<JetSlowDeathBehaviorModuleData>; 20] = crate::slow_death_field_parse!(JetSlowDeathBehaviorModuleData, slow_of);

/// `JetSlowDeathBehaviorModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<JetSlowDeathBehaviorModuleData>; 18] = [
    FieldParse::new("FXOnGroundDeath", |i, d, _| { d.fx_on_ground_death = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("OCLOnGroundDeath", |i, d, _| { d.ocl_on_ground_death = parse_ocl(i)?; Ok(()) }, UserData::None),
    FieldParse::new("FXInitialDeath", |i, d, _| { d.fx_initial_death = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("OCLInitialDeath", |i, d, _| { d.ocl_initial_death = parse_ocl(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DelaySecondaryFromInitialDeath", |i, d, _| { d.delay_secondary_from_initial_death = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("FXSecondary", |i, d, _| { d.fx_secondary = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("OCLSecondary", |i, d, _| { d.ocl_secondary = parse_ocl(i)?; Ok(()) }, UserData::None),
    FieldParse::new("FXHitGround", |i, d, _| { d.fx_hit_ground = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("OCLHitGround", |i, d, _| { d.ocl_hit_ground = parse_ocl(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DelayFinalBlowUpFromHitGround", |i, d, _| { d.delay_final_blow_up_from_hit_ground = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("FXFinalBlowUp", |i, d, _| { d.fx_final_blow_up = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("OCLFinalBlowUp", |i, d, _| { d.ocl_final_blow_up = parse_ocl(i)?; Ok(()) }, UserData::None),
    FieldParse::new(
        "DeathLoopSound",
        |i, d, _| {
            if let Some(name) = parse_sound(i)? {
                d.death_loop_sound = name;
            }
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("RollRate", |i, d, _| { d.roll_rate = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("RollRateDelta", |i, d, _| { d.roll_rate_delta = i.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("PitchRate", |i, d, _| { d.pitch_rate = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("FallHowFast", |i, d, _| { d.fall_how_fast = i.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl JetSlowDeathBehaviorModuleData {
    /// `buildFieldParse(p)`: `SlowDeathBehaviorModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, JetSlowDeathBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&SLOW_FIELD_PARSE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<JetSlowDeathBehaviorModuleData> {
        let mut data = JetSlowDeathBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, JetSlowDeathBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::JetSlowDeathBehavior(Arc::new(JetSlowDeathBehaviorModuleData::friend_new_module_data(ini)?)))
}

/// `class JetSlowDeathBehavior : public SlowDeathBehavior`.
#[derive(Clone, Debug)]
pub struct JetSlowDeathBehavior {
    /// The base.
    base: SlowDeathBehavior,
    /// The data (this class's part; the base holds its own).
    data: Arc<JetSlowDeathBehaviorModuleData>,
    /// `m_timerDeathFrame`: "fx timer at death".
    timer_death_frame: u32,
    /// `m_timerOnGroundFrame`: "fx timer from hitting the ground".
    timer_on_ground_frame: u32,
    /// `m_rollRate`: "our roll rate".
    roll_rate: f32,
    /// `m_deathLoopSound`'s playing handle (not saved).
    death_loop_handle: u32,
}

impl PartialEq for JetSlowDeathBehavior {
    /// The same base, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words() && self.death_loop_handle == other.death_loop_handle
    }
}

impl JetSlowDeathBehavior {
    /// `getInterfaceMask()`: `SlowDeathBehavior`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The constructor's first half: the base's (`INI_INVALID_DATA` for null data or a probability modifier under 1), then this class's.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<JetSlowDeathBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = SlowDeathBehavior::unconstructed(object, Some(Arc::new(data.slow.clone())))?;
        Ok(JetSlowDeathBehavior { base, data, timer_death_frame: 0, timer_on_ground_frame: 0, roll_rate: 0.0, death_loop_handle: 0 })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.base.update_module()
    }

    /// The state the test compares: the base's, then the two frames and the roll rate.
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = self.base.state_words();
        w.extend([self.timer_death_frame, self.timer_on_ground_frame, self.roll_rate.to_bits()]);
        w
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half: the base's lists, then `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.base.find_effects(world);
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: this class's version, `SlowDeathBehavior::xfer`, then this class's fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
        update_behavior::xfer_with(&mut self.base, update, xfer)?;
        xfer.xfer_unsigned_int(&mut self.timer_death_frame)?;
        xfer.xfer_unsigned_int(&mut self.timer_on_ground_frame)?;
        if !crate::logic::modules::plant(2161) {
            xfer.xfer_real(&mut self.roll_rate)?;
        }
        Ok(())
    }

    /// `crc` with the scheduling half the slot holds: the base's.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `FXList::doFXObj(fx, us)` and `ObjectCreationList::create(ocl, us, NULL)` (an OCL the store does not have is `NULL`: nothing).
    fn fx_and_ocl(world: &mut World, us: ObjectId, fx: &FxName, ocl: &Option<Vec<u8>>) {
        if let Some(fx) = fx {
            world.emit(ObjectClientEvent::FxListDoFxObj { fx: fx.clone(), object_id: us });
        }
        if let Some(ocl) = ocl {
            if world.statics.clone().object_creation_list_store.find_object_creation_list(&mut world.names, ocl).is_some() {
                world.cut(CutCall::OclCreate { object: us, ocl: ocl.clone() });
            }
        }
    }

    /// `onDie(damageInfo)`.
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &crate::logic::damage::DamageInfo) {
        let us = me.object;
        let on_deck = world.objects.find(us).is_some_and(|o| o.test_status(OBJECT_STATUS_DECK_HEIGHT_OFFSET));
        if !world.is_significantly_above_terrain(us) || on_deck {
            // "we're on the ground, we blow up right away"
            let d = self.data.clone();
            Self::fx_and_ocl(world, us, &d.fx_on_ground_death, &d.ocl_on_ground_death);
            world.body_destroy_object(us);
        } else {
            // "we're in the air, do the slow death"
            self.base.on_die(world, me, info);
        }
        if !crate::logic::modules::plant(2162) {
            world.clear_status(us, mask_of(OBJECT_STATUS_DECK_HEIGHT_OFFSET));
        }
    }

    /// `beginSlowDeath(damageInfo)`.
    fn begin_slow_death(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        self.base.begin_slow_death(world, me);
        let us = me.object;
        let d = self.data.clone();
        // "record the frame we died on"
        self.timer_death_frame = world.frame();
        Self::fx_and_ocl(world, us, &d.fx_initial_death, &d.ocl_initial_death);
        // "start the death loop sound"
        if !d.death_loop_sound.is_empty() {
            self.death_loop_handle = world.audio_add_event(d.death_loop_sound.clone(), us);
        }
        self.roll_rate = d.roll_rate;
        // "nuke the locomotor's lift so we fall" and "no turning, we're dead"
        let ai = world.objects.find(us).and_then(|o| o.ai).ok_or(Undefined::NullAi)?;
        if !world.module_call(us, ai, ModuleCall::AiHasCurLocomotor).as_bool() {
            return Err(Undefined::NullObject);
        }
        let fall = if crate::logic::modules::plant(2163) { d.fall_how_fast } else { 1.0 - d.fall_how_fast };
        world.module_call(us, ai, ModuleCall::AiLocomotorSetMaxLift(-world.cuts.globals.gravity * fall));
        world.module_call(us, ai, ModuleCall::AiLocomotorSetMaxTurnRate(0.0));
        Ok(())
    }

    /// The part of `update()` after the base's.
    fn update_falling(&mut self, world: &mut World, me: ModuleHandle) {
        if !self.base.is_slow_death_activated() {
            return;
        }
        let us = me.object;
        let d = self.data.clone();
        let physics = world.objects.find(us).and_then(|o| o.physics);
        // "roll us around in the air"
        if let Some(p) = physics {
            world.module_call(us, p, ModuleCall::PhysicsSetRollRate(self.roll_rate));
        }
        // "adjust the roll rate over time"
        self.roll_rate *= if crate::logic::modules::plant(2164) { 1.0 } else { d.roll_rate_delta };
        let now = world.frame();
        if self.timer_on_ground_frame == 0 {
            // "we are still in the air" -- "Here we want to make sure we die if we collide with a tree on the way down"
            let pos = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
            let layer = world.contain_terrain_get_layer_for_destination(&pos);
            world.contain_set_layer(us, layer);
            let height = if layer == LAYER_GROUND {
                world.get_height_above_terrain(us)
            } else {
                let layer_height = world.terrain_get_layer_height(pos.x, pos.y, layer);
                let h = pos.z - layer_height;
                if (0.0..=1.0).contains(&h) && !crate::logic::modules::plant(2165) {
                    0.0
                } else {
                    h
                }
            };
            let mut hit_a_tree = false;
            if let Some(p) = physics {
                let tree = world.module_call(us, p, ModuleCall::PhysicsGetLastCollidee).as_id();
                if tree != INVALID_ID && world.objects.find(tree).is_some_and(|t| t.is_kind_of(KINDOF_SHRUBBERY)) {
                    hit_a_tree = true;
                }
            }
            if height <= 0.0 || hit_a_tree {
                // "we've hit the ground"
                if !crate::logic::modules::plant(2167) {
                    world.audio_remove_event(self.death_loop_handle);
                }
                Self::fx_and_ocl(world, us, &d.fx_hit_ground, &d.ocl_hit_ground);
                self.timer_on_ground_frame = now;
                // "start a pitch"
                if let Some(p) = physics.filter(|_| !crate::logic::modules::plant(2168)) {
                    world.module_call(us, p, ModuleCall::PhysicsSetPitchRate(d.pitch_rate));
                }
            }
            // "timer for secondary death"
            if self.timer_death_frame != 0 && now.wrapping_sub(self.timer_death_frame) >= d.delay_secondary_from_initial_death {
                Self::fx_and_ocl(world, us, &d.fx_secondary, &d.ocl_secondary);
                if !crate::logic::modules::plant(2169) {
                    self.timer_death_frame = 0;
                }
            }
        } else if now.wrapping_sub(self.timer_on_ground_frame) >= d.delay_final_blow_up_from_hit_ground && !crate::logic::modules::plant(2166) {
            // "we are on the ground, pay attention to the final explosion timers"
            Self::fx_and_ocl(world, us, &d.fx_final_blow_up, &d.ocl_final_blow_up);
            world.body_destroy_object(us);
        }
    }
}

impl UpdateBehavior for JetSlowDeathBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.base.update_base()
    }
    /// `update()`: the base's, then the fall (never sleeps).
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        UpdateBehavior::update(&mut self.base, world, me);
        self.update_falling(world, me);
        UPDATE_SLEEP_NONE
    }
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.base.construct(world, me);
    }
    /// `OnDie` and `BeginSlowDeath` are this class's; the slow death interface's other calls are the base's.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDie(info) => {
                crate::logic::modules::lend_clone(world, me, self, UpdateBody::JetSlowDeathBehavior, Self::from_body, |w, m| m.on_die(w, me, &info));
                ModuleReply::Done
            }
            ModuleCall::BeginSlowDeath(_) => {
                let r = crate::logic::modules::lend_clone(world, me, self, UpdateBody::JetSlowDeathBehavior, Self::from_body, |w, m| m.begin_slow_death(w, me));
                if let Err(u) = r {
                    world.cuts.module_errors.push((me, IniError::undefined(u)));
                }
                ModuleReply::Done
            }
            _ => self.base.other_call(world, me, call),
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::JetSlowDeathBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::JetSlowDeathBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for JetSlowDeathBehavior {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.crc(xfer)
    }
    /// `xfer`: see [`JetSlowDeathBehavior::xfer_with`].
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut update = self.base.update_module().clone();
        let r = self.xfer_with(&mut update, xfer);
        *self.base.update_module_mut() = update;
        r
    }
    fn load_post_process(&mut self) {
        self.base.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = JetSlowDeathBehaviorModuleData::default();
        assert_eq!((d.roll_rate_delta, d.fall_how_fast, d.slow.probability_modifier), (1.0, 0.0, 10));
        let m = JetSlowDeathBehavior::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words()[5..], [0, 0, 0]);
    }
}
