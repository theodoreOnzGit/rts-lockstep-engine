// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/MobMemberSlavedUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/MobMemberSlavedUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `MobMemberSlavedUpdate`: a member of an angry mob (the slave of the mob's nexus, a spawn behaviour). Every 16 frames it keeps up with the
//! nexus: far from it (`MustCatchUpRadius`) it runs (or slows when ahead) to the nexus's goal, and past three times the radius for longer than
//! `CatchUpCrisisBailTime` it dies; travelling with it, it changes its gait at random; standing with it, it idles when the nexus idles, picks
//! its own targets when the spawn behaviour lets it (`Squirrelliness`), or attacks the nexus's last victim. A nexus that is gone kills it.
//! Every frame a member whose drawable shows the player-upgrade weapon set loses its weapon-A firing conditions.
//!
//! # Upstream facts kept
//!
//! - The constructor draws `m_framesToWait` (0 to 20) and the three channels of `m_personalColor` (0.2 to 0.4) from the logic random values.
//! - `MIN(MAX_SQUIRRELLINESS, MAX(0, ratio))` at `onObjectCreated`; `sqr` of the `Int` radii is `Int` arithmetic
//!   (`Undefined::SignedIntOverflow` past `Int`); the bail time's third is unsigned division.
//! - `doCatchUpLogic` and the mob state are never used by `update`; the state is saved.
//! - `xfer`: the version, the update base, the slaver, the countdown, the mob state (its raw word), the colour, the primary victim, the
//!   squirrelliness, self-tasking and the crisis timer.
//!
//! `p1w13-sonnet`, batch R58, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::{Coord3D, RGBColor};
use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::{
    MODELCONDITION_BETWEEN_FIRING_SHOTS_A, MODELCONDITION_FIRING_A, MODELCONDITION_PREATTACK_A, MODELCONDITION_RELOADING_A, MODELCONDITION_USING_WEAPON_A,
    MODELCONDITION_WEAPONSET_PLAYER_UPGRADE,
};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::OBJECT_STATUS_UNSELECTABLE;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object::ModuleIndex;
use crate::logic::world::object_state::mask_of;
use crate::logic::world::partition::DistanceCalculationType;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `MAX_SQUIRRELLINESS`.
const MAX_SQUIRRELLINESS: f32 = 1.0;
/// `DEFAULT_MUST_CATCH_UP_RADIUS` (`CATCH_UP_RADIUS_MAX`), `DEFAULT_NO_NEED_TO_CATCH_UP_RADIUS` (`CATCH_UP_RADIUS_MIN`).
const DEFAULT_MUST_CATCH_UP_RADIUS: i32 = 50;
const DEFAULT_NO_NEED_TO_CATCH_UP_RADIUS: i32 = 25;
/// `LOCOMOTORSET_NORMAL`, `LOCOMOTORSET_WANDER`, `LOCOMOTORSET_PANIC`.
const LOCOMOTORSET_NORMAL: i32 = 0;
const LOCOMOTORSET_WANDER: i32 = 3;
const LOCOMOTORSET_PANIC: i32 = 4;
/// `PATHFIND_CELL_SIZE_F`.
const PATHFIND_CELL_SIZE_F: f32 = 10.0;

/// `MobStates`.
pub const MOB_STATE_NONE: i32 = 0;

/// `class MobMemberSlavedUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct MobMemberSlavedUpdateModuleData {
    pub update: UpdateModuleData,
    /// `m_mustCatchUpRadius`, `m_noNeedToCatchUpRadius`.
    pub must_catch_up_radius: i32,
    pub no_need_to_catch_up_radius: i32,
    /// `m_squirrellinessRatio`.
    pub squirrelliness_ratio: f32,
    /// `m_catchUpCrisisBailTime`.
    pub catch_up_crisis_bail_time: u32,
}

impl Default for MobMemberSlavedUpdateModuleData {
    /// `MobMemberSlavedUpdateModuleData()`.
    fn default() -> Self {
        MobMemberSlavedUpdateModuleData {
            update: UpdateModuleData::new(),
            must_catch_up_radius: DEFAULT_MUST_CATCH_UP_RADIUS,
            no_need_to_catch_up_radius: DEFAULT_NO_NEED_TO_CATCH_UP_RADIUS,
            squirrelliness_ratio: 0.0,
            catch_up_crisis_bail_time: 999_999,
        }
    }
}

static FIELD_PARSE: [FieldParse<MobMemberSlavedUpdateModuleData>; 5] = [
    FieldParse::new("MustCatchUpRadius", |i, d, _| { d.must_catch_up_radius = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("CatchUpCrisisBailTime", |i, d, _| { d.catch_up_crisis_bail_time = i.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("NoNeedToCatchUpRadius", |i, d, _| { d.no_need_to_catch_up_radius = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("Squirrelliness", |i, d, _| { d.squirrelliness_ratio = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl MobMemberSlavedUpdateModuleData {
    /// `buildFieldParse(p)`: the update base's (nothing), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, MobMemberSlavedUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<MobMemberSlavedUpdateModuleData> {
        let mut data = MobMemberSlavedUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, MobMemberSlavedUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `MobMemberSlavedUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::MobMemberSlavedUpdate(Arc::new(MobMemberSlavedUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `sqr` of an `Int`: `Int` arithmetic.
fn sqr_int(x: i32) -> Result<f32, Undefined> {
    x.checked_mul(x).map(|v| v as f32).ok_or(Undefined::SignedIntOverflow)
}

/// `class MobMemberSlavedUpdate : public UpdateModule, public SlavedUpdateInterface`.
#[derive(Clone, Debug)]
pub struct MobMemberSlavedUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getMobMemberSlavedUpdateModuleData()`.
    data: Arc<MobMemberSlavedUpdateModuleData>,
    /// `m_slaver`.
    slaver: ObjectId,
    /// `m_framesToWait`.
    frames_to_wait: i32,
    /// `m_mobState`.
    mob_state: i32,
    /// `m_personalColor`.
    personal_color: RGBColor,
    /// `m_primaryVictimID`.
    primary_victim_id: ObjectId,
    /// `m_squirrellinessRatio`.
    squirrelliness_ratio: f32,
    /// `m_isSelfTasking`.
    is_self_tasking: bool,
    /// `m_catchUpCrisisTimer`.
    catch_up_crisis_timer: u32,
}

impl PartialEq for MobMemberSlavedUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl MobMemberSlavedUpdate {
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data); the random draws
    /// are the world half's ([`UpdateBehavior::construct`]).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<MobMemberSlavedUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(MobMemberSlavedUpdate {
            update,
            data,
            slaver: INVALID_ID,
            frames_to_wait: 0,
            mob_state: MOB_STATE_NONE,
            personal_color: RGBColor::default(),
            primary_victim_id: INVALID_ID,
            squirrelliness_ratio: 0.0,
            is_self_tasking: false,
            catch_up_crisis_timer: 0,
        })
    }

    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    pub fn module_data(&self) -> &Arc<MobMemberSlavedUpdateModuleData> {
        &self.data
    }

    /// The slaver, the countdown, the mob state, the colour (bits), the primary victim, the squirrelliness (bits), self-tasking, the crisis timer.
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            self.slaver as u32,
            self.frames_to_wait as u32,
            self.mob_state as u32,
            self.personal_color.red.to_bits(),
            self.personal_color.green.to_bits(),
            self.personal_color.blue.to_bits(),
            self.primary_victim_id as u32,
            self.squirrelliness_ratio.to_bits(),
            u32::from(self.is_self_tasking),
            self.catch_up_crisis_timer,
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

    fn ai(world: &World, id: ObjectId) -> Option<ModuleIndex> {
        world.objects.find(id).and_then(|o| o.ai)
    }

    fn position(world: &World, id: ObjectId) -> Coord3D {
        world.objects.find(id).map(|o| o.get_position()).unwrap_or_default()
    }

    /// `ai->getGoalPosition()` (the origin when the AI answers none).
    fn goal_position(world: &mut World, id: ObjectId, ai: ModuleIndex) -> Coord3D {
        match world.module_call(id, ai, ModuleCall::AiGetGoalPosition) {
            ModuleReply::Coord(c) => c,
            _ => Coord3D::default(),
        }
    }

    /// `update()`.
    fn run_update(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        if world.objects.find(us).is_none() {
            return Ok(());
        }
        let master = self.slaver;
        if world.objects.find(master).is_none() {
            self.stop_slaved_effects(world, us);
            world.kill(us, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
            return Ok(());
        }
        let (Some(my_ai), Some(master_ai)) = (Self::ai(world, us), Self::ai(world, master)) else { return Ok(()) };
        if !world.has_drawable(us) || !world.has_drawable(master) {
            return Ok(());
        }
        let flags = world.reads.drawable_model_condition_flags(us);
        if flags.test(MODELCONDITION_WEAPONSET_PLAYER_UPGRADE) {
            let clear = [MODELCONDITION_RELOADING_A, MODELCONDITION_BETWEEN_FIRING_SHOTS_A, MODELCONDITION_PREATTACK_A, MODELCONDITION_FIRING_A, MODELCONDITION_USING_WEAPON_A];
            world.contain_clear_model_condition_flags(us, &clear);
        }
        self.frames_to_wait += 1;
        if self.frames_to_wait < if crate::logic::modules::plant(2281) { 8 } else { 16 } {
            return Ok(());
        }
        self.frames_to_wait = 0;
        if !world.module_call(us, my_ai, ModuleCall::AiHasCurLocomotor).as_bool() {
            return Ok(());
        }
        let victim_of = |world: &mut World, id: ObjectId, ai: ModuleIndex| Some(world.module_call(id, ai, ModuleCall::AiGetCurrentVictim).as_id()).filter(|&v| world.objects.find(v).is_some());
        let mut victim = victim_of(world, us, my_ai);
        if let Some(mv) = victim_of(world, master, master_ai) {
            self.primary_victim_id = mv;
        }
        let primary_victim = (self.primary_victim_id != INVALID_ID && world.objects.find(self.primary_victim_id).is_some()).then_some(self.primary_victim_id);
        let master_path_dist_to_goal = world.module_call(master, master_ai, ModuleCall::AiGetLocomotorDistanceToGoal).as_real(0.0);
        let my_path_dist_to_goal = world.module_call(us, my_ai, ModuleCall::AiGetLocomotorDistanceToGoal).as_real(0.0);
        let catch_up_radius_squared = world.partition_distance_squared(us, master, DistanceCalculationType::FromCenter3D);
        let master_pos = Self::position(world, master);
        if catch_up_radius_squared > sqr_int(d.must_catch_up_radius)? {
            if world.module_call(master, master_ai, ModuleCall::AiIsMoving).as_bool() {
                // "master is on the move"
                let set = if master_path_dist_to_goal > my_path_dist_to_goal { LOCOMOTORSET_WANDER } else { LOCOMOTORSET_PANIC };
                world.module_call(us, my_ai, ModuleCall::AiChooseLocomotorSet(set));
                let nu_pos = Self::goal_position(world, master, master_ai);
                if nu_pos.length() < 1.0 {
                    // "if a nasty error has sent me to map origin"
                    world.module_call(us, my_ai, ModuleCall::AiMoveToPosition { pos: master_pos, command_source: CMD_FROM_AI });
                } else {
                    let mut goal_delta = Self::goal_position(world, us, my_ai);
                    goal_delta.sub(&nu_pos);
                    if goal_delta.length() > 5.0 * PATHFIND_CELL_SIZE_F && !crate::logic::modules::plant(2282) {
                        // "Whither thou goest..."
                        world.module_call(us, my_ai, ModuleCall::AiMoveToPosition { pos: nu_pos, command_source: CMD_FROM_AI });
                    }
                }
            } else {
                // "master is still, so let's re group in a hurry"
                world.module_call(us, my_ai, ModuleCall::AiChooseLocomotorSet(LOCOMOTORSET_PANIC));
                world.module_call(us, my_ai, ModuleCall::AiMoveToPosition { pos: master_pos, command_source: CMD_FROM_AI });
            }
            let triple = d.must_catch_up_radius.checked_mul(3).ok_or(Undefined::SignedIntOverflow)?;
            if catch_up_radius_squared > sqr_int(triple)? {
                // "I am critically far, now!"
                self.catch_up_crisis_timer = self.catch_up_crisis_timer.wrapping_add(1);
                if self.catch_up_crisis_timer > d.catch_up_crisis_bail_time {
                    world.kill(us, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
                    return Ok(());
                } else if self.catch_up_crisis_timer > d.catch_up_crisis_bail_time / if crate::logic::modules::plant(2283) { 2 } else { 3 } {
                    world.module_call(us, my_ai, ModuleCall::AiMoveToPosition { pos: master_pos, command_source: CMD_FROM_AI });
                }
            }
        } else if world.module_call(us, my_ai, ModuleCall::AiIsMoving).as_bool() {
            // "we're all on a trip, together"
            self.catch_up_crisis_timer = 0;
            let seed = world.random.game_logic_random_value(0, 10);
            let set = match seed {
                1 => Some(LOCOMOTORSET_WANDER),
                2 => Some(LOCOMOTORSET_PANIC),
                3 => Some(if crate::logic::modules::plant(2284) { LOCOMOTORSET_WANDER } else { LOCOMOTORSET_NORMAL }),
                _ => None,
            };
            if let Some(set) = set {
                world.module_call(us, my_ai, ModuleCall::AiChooseLocomotorSet(set));
            }
        } else {
            // "give me something to do while I'm standing here..."
            self.catch_up_crisis_timer = if crate::logic::modules::plant(2285) { self.catch_up_crisis_timer } else { 0 };
            if let Some(spawner) = world.get_spawn_behavior_interface(master) {
                if world.module_call(master, master_ai, ModuleCall::AiIsIdle).as_bool() {
                    // "if controlling player has pressed stop, we stop! That's it!"
                    world.module_call(us, my_ai, ModuleCall::AiIdleFrom { command_source: CMD_FROM_AI });
                    self.primary_victim_id = INVALID_ID;
                    return Ok(());
                }
                if world.module_call(master, spawner, ModuleCall::SpawnMaySpawnSelfTaskAi(self.squirrelliness_ratio)).as_bool()
                    && world.module_call(us, my_ai, ModuleCall::AiGetLastCommandSource).as_int(0) != CMD_FROM_AI
                {
                    // "I may have been told to attack directly more recently"
                    let new_target = Some(world.module_call(us, my_ai, ModuleCall::AiGetNextMoodTarget { called_by_ai: false, called_during_idle: false }).as_id())
                        .filter(|&t| world.objects.find(t).is_some());
                    if let Some(t) = new_target.filter(|&t| Some(t) != victim) {
                        // "if there is someone else around to attack"
                        victim = Some(t);
                        world.module_call(us, my_ai, ModuleCall::AiAttackObject { victim: t, max_shots: 999, command_source: CMD_FROM_AI });
                        self.is_self_tasking = true;
                    }
                }
                if victim.is_none() {
                    // "If I still don't have anyone to shoot at"
                    if let Some(p) = primary_victim.filter(|_| !crate::logic::modules::plant(2286)) {
                        world.module_call(us, my_ai, ModuleCall::AiAttackObject { victim: p, max_shots: 999, command_source: CMD_FROM_AI });
                    } else {
                        // `else if( ! masterAI->isAttacking() ) { }`
                        world.module_call(master, master_ai, ModuleCall::AiIsAttacking);
                    }
                    self.is_self_tasking = false;
                }
            }
        }
        Ok(())
    }

    /// `startSlavedEffects(slaver)`.
    fn start_slaved_effects(&mut self, world: &World, slaver: ObjectId) {
        if world.objects.find(slaver).is_none() {
            return;
        }
        self.slaver = slaver;
    }

    /// `stopSlavedEffects()`.
    fn stop_slaved_effects(&mut self, world: &mut World, us: ObjectId) {
        self.slaver = INVALID_ID;
        if !crate::logic::modules::plant(2287) {
            world.body_set_status(us, mask_of(OBJECT_STATUS_UNSELECTABLE), false);
        }
        world.body_clear_disabled(us, DISABLED_HELD);
    }
}

impl UpdateBehavior for MobMemberSlavedUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(e) = self.run_update(world, me.object) {
            world.cuts.module_errors.push((me, IniError::undefined(e)));
        }
        UPDATE_SLEEP_NONE
    }
    /// The constructor's world half: the countdown and the personal colour.
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        self.frames_to_wait = world.random.game_logic_random_value(0, 20);
        self.personal_color.red = world.random.game_logic_random_value_real(0.2, 0.4);
        self.personal_color.green = world.random.game_logic_random_value_real(0.2, 0.4);
        self.personal_color.blue = world.random.game_logic_random_value_real(0.2, 0.4);
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_object_id(&mut self.slaver)?;
        xfer.xfer_int(&mut self.frames_to_wait)?;
        let mut b = self.mob_state.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        self.mob_state = i32::from_le_bytes(b);
        xfer.xfer_rgb_color(&mut self.personal_color)?;
        xfer.xfer_object_id(&mut self.primary_victim_id)?;
        xfer.xfer_real(&mut self.squirrelliness_ratio)?;
        xfer.xfer_bool(&mut self.is_self_tasking)?;
        if crate::logic::modules::plant(2288) {
            return Ok(());
        }
        xfer.xfer_unsigned_int(&mut self.catch_up_crisis_timer)
    }
    /// The slaved interface (`onEnslave`, `onSlaverDie`, `onSlaverDamage`, `getSlaverID`) and `onObjectCreated`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnObjectCreated => {
                // `MIN(MAX_SQUIRRELLINESS, MAX(0, ratio))`
                let r = self.data.squirrelliness_ratio;
                let lo = if 0.0 > r { 0.0 } else { r };
                self.squirrelliness_ratio = if MAX_SQUIRRELLINESS < lo { MAX_SQUIRRELLINESS } else { lo };
                ModuleReply::Done
            }
            ModuleCall::SlavedOnEnslave { slaver } => {
                self.start_slaved_effects(world, slaver);
                ModuleReply::Done
            }
            ModuleCall::SlavedOnSlaverDie => {
                self.stop_slaved_effects(world, me.object);
                ModuleReply::Done
            }
            ModuleCall::SlavedOnSlaverDamage(info) => {
                if let Some(ai) = Self::ai(world, me.object) {
                    world.module_call(me.object, ai, ModuleCall::AiGoProne { info, command_source: CMD_FROM_AI });
                }
                ModuleReply::Done
            }
            ModuleCall::GetSlaverId => ModuleReply::Id(self.slaver),
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::MobMemberSlavedUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::MobMemberSlavedUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for MobMemberSlavedUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base and the fields.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = MobMemberSlavedUpdateModuleData::default();
        assert_eq!((d.must_catch_up_radius, d.no_need_to_catch_up_radius, d.catch_up_crisis_bail_time), (50, 25, 999_999));
        let m = MobMemberSlavedUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words()[0], INVALID_ID as u32);
    }
}
