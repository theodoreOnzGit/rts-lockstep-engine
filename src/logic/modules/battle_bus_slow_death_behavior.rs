// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/BattleBusSlowDeathBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BattleBusSlowDeathBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `BattleBusSlowDeathBehavior`: a [`SlowDeathBehavior`] with a first, false death. A slow death that does not come from this module's own
//! `onDie` (another die module chose it) is the "undeath": the bus plays its start FX and OCL, idles its AI, stops and is thrown into the air
//! (`ThrowForce`, a random rotation), and its passengers take `PercentDamageToPassengers`. When it is back on the ground (not before
//! `GROUND_CHECK_DELAY` frames) it plays its hit-ground FX and OCL, takes `MODELCONDITION_SECOND_LIFE`, idles and stops again and is held
//! (`DISABLED_HELD`). From then on, with an `EmptyHulkDestructionDelay`, a bus left empty that long is killed (`DAMAGE_PENALTY`,
//! `DEATH_EXTRA_4`). Its own `onDie` is the real death: the base's slow death, run as the base runs it.
//!
//! The module is a [`SlowDeathBehavior`] (the base's state and methods, [`slow_death_field_parse!`](crate::slow_death_field_parse) for its data)
//! plus this class's: `OnDie` and `BeginSlowDeath` are this class's, the other calls the base's.
//!
//! # Upstream facts kept
//!
//! - `onDie` marks the death real *before* the base's `onDie`, which may choose this module and call its `beginSlowDeath`: that call then
//!   runs the base's slow death. The flags are set before the module is lent to its slot, so the nested call sees them.
//! - The undeath does not ask the die mux, nor `isSlowDeathActivated`: a bus already in its undeath that is chosen again is thrown again.
//! - The ground check is `groundCheckFrame < now` and `!isAboveTerrain()` (the height above the terrain not positive).
//! - With no `EmptyHulkDestructionDelay` the landing sleeps forever and the empty-hulk watch never starts; a bus with no contain module sleeps
//!   forever whenever the watch is reached.
//! - The empty-hulk timer is `now + delay`; a bus refilled before then clears it; the kill is `now > timer` (strictly).
//! - The xfer is version 1: the version, `SlowDeathBehavior`'s, then the two flags (bytes) and the two frames.
//!
//! # Where upstream is undefined
//!
//! The empty-hulk kill of a bus with no body module (`kill` reads `getBodyModule()->getMaxHealth()` unchecked: `NullBodyModule`), recorded in
//! `world.cuts.module_errors`; and the base's (`SlowDeathBehavior`).
//!
//! `p1w13-sonnet`, batch R47, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::DISABLED_HELD;
use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::MODELCONDITION_SECOND_LIFE;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DAMAGE_PENALTY, DEATH_EXTRA_4};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{update_sleep, UpdateModule, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::slow_death_behavior::{SlowDeathBehavior, SlowDeathBehaviorModuleData};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// "Check for colliding with the ground only after this long, to prevent hitting on the way up".
const GROUND_CHECK_DELAY: u32 = 10;
/// "Check for empty hulk every this often instead of every frame".
const EMPTY_HULK_CHECK_DELAY: u32 = 15;

/// An FX list or an object creation list by name (`None` for `NULL`).
type Named = Option<Vec<u8>>;

/// `class BattleBusSlowDeathBehaviorModuleData : public SlowDeathBehaviorModuleData`.
#[derive(Clone, Debug)]
pub struct BattleBusSlowDeathBehaviorModuleData {
    /// The base.
    pub slow: SlowDeathBehaviorModuleData,
    /// `m_fxStartUndeath`, `m_oclStartUndeath`: "executed on start of throw".
    pub fx_start_undeath: Named,
    pub ocl_start_undeath: Named,
    /// `m_fxHitGround`, `m_oclHitGround`: "for when we land".
    pub fx_hit_ground: Named,
    pub ocl_hit_ground: Named,
    /// `m_throwForce`: "how hard we are thrown in to the air".
    pub throw_force: f32,
    /// `m_percentDamageToPassengers`: "at the moment we throw up, how hard the people inside are hit".
    pub percent_damage_to_passengers: f32,
    /// `m_emptyHulkDestructionDelay`: "if non-zero, time empty before we kill ourselves".
    pub empty_hulk_destruction_delay: u32,
}

impl Default for BattleBusSlowDeathBehaviorModuleData {
    /// `BattleBusSlowDeathBehaviorModuleData()`.
    fn default() -> Self {
        BattleBusSlowDeathBehaviorModuleData {
            slow: SlowDeathBehaviorModuleData::default(),
            fx_start_undeath: None,
            ocl_start_undeath: None,
            fx_hit_ground: None,
            ocl_hit_ground: None,
            throw_force: 1.0,
            percent_damage_to_passengers: 0.0,
            empty_hulk_destruction_delay: 0,
        }
    }
}

/// `INI::parseFXList` and `INI::parseObjectCreationList`: the next token, `None` for `NULL` (an OCL is found in the store when it is used).
fn parse_named(ini: &mut Ini<'_>) -> IniResult<Named> {
    let name = ini.get_next_token(None)?;
    Ok(if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) })
}

fn slow_of(d: &mut BattleBusSlowDeathBehaviorModuleData) -> &mut SlowDeathBehaviorModuleData {
    &mut d.slow
}

/// `SlowDeathBehaviorModuleData::buildFieldParse`'s table.
static SLOW_FIELD_PARSE: [FieldParse<BattleBusSlowDeathBehaviorModuleData>; 20] = crate::slow_death_field_parse!(BattleBusSlowDeathBehaviorModuleData, slow_of);

/// `BattleBusSlowDeathBehaviorModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<BattleBusSlowDeathBehaviorModuleData>; 8] = [
    FieldParse::new("FXStartUndeath", |i, d, _| { d.fx_start_undeath = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("OCLStartUndeath", |i, d, _| { d.ocl_start_undeath = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("FXHitGround", |i, d, _| { d.fx_hit_ground = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("OCLHitGround", |i, d, _| { d.ocl_hit_ground = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("ThrowForce", |i, d, _| { d.throw_force = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("PercentDamageToPassengers", |i, d, _| { d.percent_damage_to_passengers = i.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("EmptyHulkDestructionDelay", |i, d, _| { d.empty_hulk_destruction_delay = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl BattleBusSlowDeathBehaviorModuleData {
    /// `buildFieldParse(p)`: `SlowDeathBehaviorModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, BattleBusSlowDeathBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&SLOW_FIELD_PARSE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<BattleBusSlowDeathBehaviorModuleData> {
        let mut data = BattleBusSlowDeathBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, BattleBusSlowDeathBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::BattleBusSlowDeathBehavior(Arc::new(BattleBusSlowDeathBehaviorModuleData::friend_new_module_data(ini)?)))
}

/// `class BattleBusSlowDeathBehavior : public SlowDeathBehavior`.
#[derive(Clone, Debug)]
pub struct BattleBusSlowDeathBehavior {
    /// The base.
    base: SlowDeathBehavior,
    /// The data (this class's part; the base holds its own).
    data: Arc<BattleBusSlowDeathBehaviorModuleData>,
    /// `m_isRealDeath`: "set by onDie, prevents the alternate processing caused by beginSlowDeath".
    is_real_death: bool,
    /// `m_isInFirstDeath`: "controls lifetime of alternate processing".
    is_in_first_death: bool,
    /// `m_groundCheckFrame`: "don't check for bouncing until at least here".
    ground_check_frame: u32,
    /// `m_penaltyDeathFrame`: "if non zero, kill us with Penalty damage at this frame".
    penalty_death_frame: u32,
}

impl PartialEq for BattleBusSlowDeathBehavior {
    /// The same base, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl BattleBusSlowDeathBehavior {
    /// `getInterfaceMask()`: `SlowDeathBehavior`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The constructor's first half: the base's (`INI_INVALID_DATA` for null data or a probability modifier under 1), then this class's.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<BattleBusSlowDeathBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = SlowDeathBehavior::unconstructed(object, Some(Arc::new(data.slow.clone())))?;
        Ok(BattleBusSlowDeathBehavior { base, data, is_real_death: false, is_in_first_death: false, ground_check_frame: 0, penalty_death_frame: 0 })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.base.update_module()
    }

    /// The state the test compares: the base's, then the two flags and the two frames.
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = self.base.state_words();
        w.extend([u32::from(self.is_real_death), u32::from(self.is_in_first_death), self.ground_check_frame, self.penalty_death_frame]);
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
        xfer.xfer_bool(&mut self.is_real_death)?;
        if !crate::logic::modules::plant(2171) {
            xfer.xfer_bool(&mut self.is_in_first_death)?;
        }
        xfer.xfer_unsigned_int(&mut self.ground_check_frame)?;
        xfer.xfer_unsigned_int(&mut self.penalty_death_frame)?;
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

    /// `FXList::doFXObj(fx, me)` and `ObjectCreationList::create(ocl, me, NULL)` (an OCL the store does not have is `NULL`: nothing).
    fn fx_and_ocl(world: &mut World, us: ObjectId, fx: &Named, ocl: &Named) {
        if let Some(fx) = fx {
            world.emit(ObjectClientEvent::FxListDoFxObj { fx: fx.clone(), object_id: us });
        }
        if let Some(ocl) = ocl {
            if world.statics.clone().object_creation_list_store.find_object_creation_list(&mut world.names, ocl).is_some() {
                world.cut(CutCall::OclCreate { object: us, ocl: ocl.clone() });
            }
        }
    }

    /// `me->getAI()->aiIdle(CMD_FROM_AI)` and `me->getPhysics()->clearAcceleration(); scrubVelocity2D(0)`, each if there is one.
    fn idle_and_stop(world: &mut World, us: ObjectId) -> Option<crate::logic::world::object::ModuleIndex> {
        if let Some(ai) = world.objects.find(us).and_then(|o| o.ai) {
            world.module_call(us, ai, ModuleCall::AiIdle);
        }
        let physics = world.objects.find(us).and_then(|o| o.physics);
        if let Some(p) = physics {
            world.module_call(us, p, ModuleCall::PhysicsClearAcceleration);
            world.module_call(us, p, ModuleCall::PhysicsScrubVelocity2D(0.0));
        }
        physics
    }

    /// `beginSlowDeath(damageInfo)`: the undeath, unless the death is real.
    fn begin_slow_death(&mut self, world: &mut World, me: ModuleHandle) {
        if self.is_real_death {
            // "If a real death, we aren't allowed to do anything"
            self.base.begin_slow_death(world, me);
            return;
        }
        // "We can intercept and do our extra slow death if this is not from a real onDie"
        let us = me.object;
        let d = self.data.clone();
        self.is_in_first_death = true;
        self.ground_check_frame = world.frame().wrapping_add(if crate::logic::modules::plant(2172) { 0 } else { GROUND_CHECK_DELAY });
        // "First do the special effects"
        Self::fx_and_ocl(world, us, &d.fx_start_undeath, &d.ocl_start_undeath);
        // "Then stop what we were doing", "Then stop physically"
        if let Some(p) = Self::idle_and_stop(world, us) {
            // "Then get chucked in the air"
            let throw_force = Coord3D { x: 0.0, y: 0.0, z: d.throw_force };
            world.module_call(us, p, ModuleCall::PhysicsApplyShock(throw_force));
            if !crate::logic::modules::plant(2173) {
                world.module_call(us, p, ModuleCall::PhysicsApplyRandomRotation);
            }
        }
        // "And finally hit those inside for some damage"
        if let Some(contain) = world.get_contain(us) {
            world.module_call(us, contain, ModuleCall::ProcessDamageToContained(d.percent_damage_to_passengers));
        }
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
    }

    /// The part of `update()` that is not the base's: `None` for the real death (the base's update).
    fn update_undeath(&mut self, world: &mut World, me: ModuleHandle) -> Option<UpdateSleepTime> {
        let us = me.object;
        let d = self.data.clone();
        let now = world.frame();
        if self.is_in_first_death {
            // "First death means we are doing our throw up in the air conversion"
            if self.ground_check_frame < now && !world.is_above_terrain(us) {
                // "We're done since we hit the ground"
                self.is_in_first_death = false;
                Self::fx_and_ocl(world, us, &d.fx_hit_ground, &d.ocl_hit_ground);
                world.set_model_condition_state(us, MODELCONDITION_SECOND_LIFE);
                // "And stop us forever"
                Self::idle_and_stop(world, us);
                if !crate::logic::modules::plant(2174) {
                    world.body_set_disabled(us, DISABLED_HELD);
                }
                // "We can only sleep if we don't have to watch out for being empty."
                return Some(if d.empty_hulk_destruction_delay == 0 { UPDATE_SLEEP_FOREVER } else { UPDATE_SLEEP_NONE });
            }
            return Some(UPDATE_SLEEP_NONE);
        }
        if self.is_real_death {
            return None;
        }
        // "If neither death is active, we must be awake to check for emptiness"
        let Some(contain) = world.get_contain(us) else {
            return Some(UPDATE_SLEEP_FOREVER);
        };
        let count = world.module_call(us, contain, ModuleCall::GetContainCount).as_int(0);
        if self.penalty_death_frame != 0 {
            // "We have been timed for death, see if we have re-filled first"
            if count > 0 && !crate::logic::modules::plant(2175) {
                self.penalty_death_frame = 0;
                Some(update_sleep(EMPTY_HULK_CHECK_DELAY))
            } else if now > self.penalty_death_frame || (crate::logic::modules::plant(2176) && now == self.penalty_death_frame) {
                // "No salvation, see if the timer is up to be killed."
                if world.get_body_module(us).is_none() {
                    // `kill` reads `getBodyModule()->getMaxHealth()` unchecked
                    world.cuts.module_errors.push((me, IniError::undefined(Undefined::NullBodyModule)));
                    return Some(UPDATE_SLEEP_FOREVER);
                }
                world.kill(us, DAMAGE_PENALTY, if crate::logic::modules::plant(2177) { DEATH_EXTRA_4 - 1 } else { DEATH_EXTRA_4 });
                Some(UPDATE_SLEEP_FOREVER)
            } else {
                Some(update_sleep(EMPTY_HULK_CHECK_DELAY))
            }
        } else {
            // "We are not marked for death, so see if we should be"
            if count == 0 {
                self.penalty_death_frame = now.wrapping_add(d.empty_hulk_destruction_delay);
            }
            Some(update_sleep(if crate::logic::modules::plant(2178) { 1 } else { EMPTY_HULK_CHECK_DELAY }))
        }
    }
}

impl UpdateBehavior for BattleBusSlowDeathBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.base.update_base()
    }
    /// `update()`: the first death's, the base's for a real one, else the empty-hulk watch.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.update_undeath(world, me) {
            Some(sleep) => sleep,
            None => UpdateBehavior::update(&mut self.base, world, me),
        }
    }
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.base.construct(world, me);
    }
    /// `OnDie` and `BeginSlowDeath` are this class's; the slow death interface's other calls are the base's.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDie(info) => {
                // "Set beforehand because onDie could result in picking us to beginSlowDeath", "and clear this incase we died while in the
                // alternate death"
                self.is_real_death = !crate::logic::modules::plant(2179);
                self.is_in_first_death = false;
                crate::logic::modules::lend_clone(world, me, self, UpdateBody::BattleBusSlowDeathBehavior, Self::from_body, |w, m| m.base.on_die(w, me, &info));
                ModuleReply::Done
            }
            ModuleCall::BeginSlowDeath(_) => {
                crate::logic::modules::lend_clone(world, me, self, UpdateBody::BattleBusSlowDeathBehavior, Self::from_body, |w, m| m.begin_slow_death(w, me));
                ModuleReply::Done
            }
            _ => self.base.other_call(world, me, call),
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::BattleBusSlowDeathBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::BattleBusSlowDeathBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for BattleBusSlowDeathBehavior {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.crc(xfer)
    }
    /// `xfer`: see [`BattleBusSlowDeathBehavior::xfer_with`].
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
        let d = BattleBusSlowDeathBehaviorModuleData::default();
        assert_eq!((d.throw_force, d.percent_damage_to_passengers, d.empty_hulk_destruction_delay, d.slow.probability_modifier), (1.0, 0.0, 0, 10));
        let m = BattleBusSlowDeathBehavior::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words()[5..], [0, 0, 0, 0]);
    }
}
