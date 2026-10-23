// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/AnimationSteeringUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AnimationSteeringUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `AnimationSteeringUpdate`: the turn animations of a drawable that steers (a motorcycle leaning into a turn). Every frame, once the last
//! transition has had `MinTransitionTime`, an object with a physics behaviour and a drawable goes from straight to "centre to left/right" when
//! its physics turns, from there to "left/right to centre" when it stops turning that way, and back to straight (no turn condition) when it
//! goes straight.
//!
//! The turn is [`ModuleCall::PhysicsGetTurning`]; the conditions are the drawable's (`ObjectClientEvent::DrawableModelCondition`). Upstream
//! saves neither field, so a load keeps the animation state as it was.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{UpdateModule, UpdateModuleData, SleepyUpdatePhase, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::common::model_state::{
    ModelConditionFlagType, MODELCONDITION_CENTER_TO_LEFT, MODELCONDITION_CENTER_TO_RIGHT, MODELCONDITION_INVALID, MODELCONDITION_LEFT_TO_CENTER,
    MODELCONDITION_RIGHT_TO_CENTER,
};
use crate::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};

/// `TURN_NEGATIVE` (`PhysicsUpdate.h`).
const TURN_NEGATIVE: i32 = -1;
/// `TURN_NONE`.
const TURN_NONE: i32 = 0;
/// `TURN_POSITIVE`.
const TURN_POSITIVE: i32 = 1;

/// `class AnimationSteeringUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimationSteeringUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_transitionFrames` (`MinTransitionTime`).
    pub transition_frames: u32,
}

/// `AnimationSteeringUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<AnimationSteeringUpdateModuleData>; 2] = [
    FieldParse::new("MinTransitionTime", |ini, d, _| { d.transition_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl AnimationSteeringUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, AnimationSteeringUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<AnimationSteeringUpdateModuleData> {
        let mut data = AnimationSteeringUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, AnimationSteeringUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class AnimationSteeringUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct AnimationSteeringUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getAnimationSteeringUpdateModuleData()`.
    data: Arc<AnimationSteeringUpdateModuleData>,
    /// `m_currentTurnAnim` (`MODELCONDITION_INVALID`: straight).
    current_turn_anim: ModelConditionFlagType,
    /// `m_nextTransitionFrame`.
    next_transition_frame: u32,
}

impl PartialEq for AnimationSteeringUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same animation state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.current_turn_anim == other.current_turn_anim
            && self.next_transition_frame == other.next_transition_frame
    }
}

impl AnimationSteeringUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and straight with no transition pending.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<AnimationSteeringUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(AnimationSteeringUpdate { update, data, current_turn_anim: MODELCONDITION_INVALID, next_transition_frame: 0 })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getAnimationSteeringUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<AnimationSteeringUpdateModuleData> {
        &self.data
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

    /// `xfer` with the scheduling half the slot holds: the version, that half, the fields.
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

    /// The state the test compares: the turn animation, the next transition frame.
    pub fn state_words(&self) -> [u32; 2] {
        [self.current_turn_anim as u32, self.next_transition_frame]
    }

    /// `now + transitionFrames`, the animation `anim`.
    fn transition(&mut self, now: u32, anim: ModelConditionFlagType) {
        self.next_transition_frame = now.wrapping_add(self.data.transition_frames);
        self.current_turn_anim = anim;
    }
}

impl UpdateBehavior for AnimationSteeringUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let id = me.object;
        let physics = world.objects.find(id).and_then(|o| o.physics);
        let now = world.frame();
        if let Some(physics) = physics.filter(|_| world.has_drawable(id) && (now >= self.next_transition_frame || crate::logic::modules::plant(1881))) {
            let current_turn = world.module_call(id, physics, ModuleCall::PhysicsGetTurning).as_int(TURN_NONE);
            let model = |world: &mut World, op: ModelConditionOp| world.emit(ObjectClientEvent::DrawableModelCondition { object_id: id, op });
            match self.current_turn_anim {
                MODELCONDITION_INVALID => {
                    if current_turn == TURN_NEGATIVE {
                        model(world, ModelConditionOp::Set(vec![MODELCONDITION_CENTER_TO_RIGHT]));
                        self.transition(now, MODELCONDITION_CENTER_TO_RIGHT);
                    } else if current_turn == TURN_POSITIVE {
                        model(world, ModelConditionOp::Set(vec![MODELCONDITION_CENTER_TO_LEFT]));
                        self.transition(now, MODELCONDITION_CENTER_TO_LEFT);
                    }
                }
                MODELCONDITION_CENTER_TO_RIGHT => {
                    if current_turn != TURN_NEGATIVE {
                        model(world, ModelConditionOp::ClearAndSet { clear: vec![MODELCONDITION_CENTER_TO_RIGHT], set: vec![MODELCONDITION_RIGHT_TO_CENTER] });
                        self.transition(now, MODELCONDITION_RIGHT_TO_CENTER);
                    }
                }
                MODELCONDITION_CENTER_TO_LEFT => {
                    if current_turn != TURN_POSITIVE || crate::logic::modules::plant(1882) {
                        model(world, ModelConditionOp::ClearAndSet { clear: vec![MODELCONDITION_CENTER_TO_LEFT], set: vec![MODELCONDITION_LEFT_TO_CENTER] });
                        self.transition(now, MODELCONDITION_LEFT_TO_CENTER);
                    }
                }
                MODELCONDITION_LEFT_TO_CENTER | MODELCONDITION_RIGHT_TO_CENTER if current_turn == TURN_NONE => {
                    model(world, ModelConditionOp::Clear(vec![MODELCONDITION_LEFT_TO_CENTER, MODELCONDITION_RIGHT_TO_CENTER]));
                    self.next_transition_frame = if crate::logic::modules::plant(1883) { now.wrapping_add(self.data.transition_frames) } else { now };
                    self.current_turn_anim = MODELCONDITION_INVALID;
                }
                _ => {}
            }
        }
        UPDATE_SLEEP_NONE
    }
    /// The constructor sets no wake frame.
    fn construct(&mut self, _world: &mut World, _me: ModuleHandle) {}
    /// No fields after the base (upstream saves neither).
    fn xfer_fields(&mut self, _xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::AnimationSteeringUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::AnimationSteeringUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for AnimationSteeringUpdate {
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

/// The factory's `NewModuleDataProc` for `AnimationSteeringUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::AnimationSteeringUpdate(Arc::new(AnimationSteeringUpdateModuleData::friend_new_module_data(ini)?)))
}
