// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/TurretAI.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/TurretAI.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `TurretAI`: a turret's angle and pitch, and the state machine that aims it (`p1w13-sonnet`, layer 10, batch T1).
//!
//! The machine's states: **idle** (the AI's mood check; to idle scan when its random interval is up), **idle scan** (to a random
//! angle off the natural one, at half speed), **aim** (at the target object, or the bridge's nearer attack point, or a position,
//! sweeping while it fires; pitch toward it, raised for ground units; on target and in range: to fire), **fire**
//! (`AIAttackFireWeaponState`, [`crate::logic::ai::fire_state`]; back to aim when out of range), **hold** (the recenter time) and
//! **recenter** (to the natural angle and pitch at half speed). `setTurretTargetObject` / `setTurretTargetPosition` start or stop
//! the aim; the update runs the machine while the turret is enabled (or recentering), tracks firing for the sweep, and plays the
//! rotation sound.
//!
//! # The shape
//!
//! [`TurretAi`] holds the turret's fields ([`TurretData`]) and its machine. The machine's context is the host ([`TurretHost`]: the
//! owner, its AI, weapons and the world); while the machine runs, the turret's fields are **lent to the host**
//! ([`TurretHost::turret_stack`], the innermost last), so that the states, which upstream reach their turret through the machine,
//! reach them through the context, and the machine itself is the `sm` every callback has. `TurretStateMachine`'s overrides of
//! `setState`, `clear` and `resetToDefaultState` (which wake the turret) are [`tsm_set_state`] and friends; the machine's own
//! transitions do not go through them, as upstream's do not.
//!
//! # Upstream facts kept
//!
//! - `TurretStateMachine::xfer` writes a version and nothing else: the machine's states, current state and goal are not saved.
//! - The aim state nulls its enemy before the pitch: a ground target's pitch is always adjusted for an object target too.
//! - `m_continuousFireExpirationFrame` starts at `-1` (the largest frame).

use std::convert::Infallible;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_common::{normalize_angle, CommandSourceType, WhichTurretType, ATTACK_CONTINUED_TARGET, ATTACK_CONTINUED_TARGET_FORCED, TURRET_INVALID};
use crate::common::game_type::{ObjectId, WeaponSlotType, WEAPONSLOT_COUNT};
use crate::common::kind_of::{KindOfType, KINDOF_BRIDGE};
use crate::common::model_state::{ModelConditionFlagType, MODELCONDITION_TURRET_ROTATE};
use crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use crate::common::state_machine::{
    SmResult, StateBehavior, StateConditionInfo, StateExitType, StateId, StateMachine, StateReturn,
};
use crate::common::trig;
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::common::ini::{FieldParse, Ini, IniResult, MultiIniFieldParse, UserData};
use crate::logic::weapon_set::WEAPON_SLOT_TYPE_NAMES;
use crate::logic::ai::ai_update_types::{MAA_AFFECT_RANGE_IGNORE_ALL, MM_ACTION_IDLE};
use crate::logic::ai::fire_state::{self, FireAttacker, FireHost, SLOTS};
use crate::logic::module::update_module::{update_sleep, UpdateSleepTime, FOREVER, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::plant;
use crate::logic::weapon_host::CanAttackResult;
use crate::logic::world::ids::TeamId;

/// `DEFAULT_TURN_RATE`, `DEFAULT_PITCH_RATE`.
pub const DEFAULT_TURN_RATE: f32 = 0.01;
pub const DEFAULT_PITCH_RATE: f32 = 0.01;

/// `enum TurretStateType`.
pub const TURRETAI_IDLE: StateId = 0;
pub const TURRETAI_IDLESCAN: StateId = 1;
/// "aim turret at GoalObject"
pub const TURRETAI_AIM: StateId = 2;
/// "fire turret at GoalObject"
pub const TURRETAI_FIRE: StateId = 3;
/// "rotate turret back to default position"
pub const TURRETAI_RECENTER: StateId = 4;
/// "hold turret position for a bit before recenter"
pub const TURRETAI_HOLD: StateId = 5;

/// `enum TurretTargetType` (saved as four bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TurretTargetType {
    None = 0,
    Object = 1,
    Position = 2,
}

/// `class TurretAIData`: the `Turret` block's fields (their defaults the constructor's).
#[derive(Clone, Debug, PartialEq)]
pub struct TurretAiData {
    pub turn_rate: f32,
    pub pitch_rate: f32,
    pub natural_turret_angle: f32,
    pub natural_turret_pitch: f32,
    /// "if nonzero, sweep within +/- this angle range while firing"
    pub turret_fire_angle_sweep: [f32; WEAPONSLOT_COUNT as usize],
    /// "While sweeping, change your speed by this"
    pub turret_sweep_speed_modifier: [f32; WEAPONSLOT_COUNT as usize],
    /// "if nonzero, I am on target at this fixed pitch, not when pointing at target"
    pub fire_pitch: f32,
    pub min_pitch: f32,
    pub ground_unit_pitch: f32,
    /// "which WeaponSlots are controlled by this turret"
    pub turret_weapon_slots: u32,
    pub min_idle_scan_angle: f32,
    pub max_idle_scan_angle: f32,
    pub min_idle_scan_interval: u32,
    pub max_idle_scan_interval: u32,
    pub recenter_time: u32,
    pub initially_disabled: bool,
    pub fires_while_turning: bool,
    pub is_allows_pitch: bool,
}

impl Default for TurretAiData {
    /// `TurretAIData()`.
    fn default() -> Self {
        TurretAiData {
            turn_rate: DEFAULT_TURN_RATE,
            pitch_rate: DEFAULT_PITCH_RATE,
            natural_turret_angle: 0.0,
            natural_turret_pitch: 0.0,
            turret_fire_angle_sweep: [0.0; WEAPONSLOT_COUNT as usize],
            turret_sweep_speed_modifier: [1.0; WEAPONSLOT_COUNT as usize],
            fire_pitch: 0.0,
            min_pitch: 0.0,
            ground_unit_pitch: 0.0,
            turret_weapon_slots: 0,
            min_idle_scan_angle: 0.0,
            max_idle_scan_angle: 0.0,
            min_idle_scan_interval: 9_999_999,
            max_idle_scan_interval: 9_999_999,
            recenter_time: 2 * crate::common::game_common::LOGICFRAMES_PER_SECOND as u32,
            initially_disabled: false,
            fires_while_turning: false,
            is_allows_pitch: false,
        }
    }
}

/// `parseTWS` (`ControlledWeaponSlots`): each named slot's bit OR'd in (at least one name).
fn parse_tws(ini: &mut Ini<'_>, d: &mut TurretAiData, _: &UserData) -> IniResult<()> {
    let mut token = Some(ini.get_next_token(None)?);
    while let Some(t) = token {
        let wslot = Ini::scan_index_list(&t, &WEAPON_SLOT_TYPE_NAMES)?;
        if plant(3186) {
            d.turret_weapon_slots = 1 << wslot;
        } else {
            d.turret_weapon_slots |= 1 << wslot;
        }
        token = ini.get_next_token_or_null(None);
    }
    Ok(())
}

/// `parseTurretSweep` (`TurretFireAngleSweep`): a slot, then its sweep as an angle.
fn parse_turret_sweep(ini: &mut Ini<'_>, d: &mut TurretAiData, _: &UserData) -> IniResult<()> {
    let wslot = Ini::scan_index_list(&ini.get_next_token(None)?, &WEAPON_SLOT_TYPE_NAMES)? as usize;
    d.turret_fire_angle_sweep[wslot] = ini.parse_angle_real()?;
    Ok(())
}

/// `parseTurretSweepSpeed` (`TurretSweepSpeedModifier`): a slot, then its modifier.
fn parse_turret_sweep_speed(ini: &mut Ini<'_>, d: &mut TurretAiData, _: &UserData) -> IniResult<()> {
    let wslot = Ini::scan_index_list(&ini.get_next_token(None)?, &WEAPON_SLOT_TYPE_NAMES)? as usize;
    d.turret_sweep_speed_modifier[wslot] = ini.parse_real()?;
    Ok(())
}

/// `TurretAIData::buildFieldParse`'s table.
static TURRET_FIELD_PARSE: [FieldParse<TurretAiData>; 19] = [
    FieldParse::new("TurretTurnRate", |i, d, _| { d.turn_rate = i.parse_angular_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("TurretPitchRate", |i, d, _| { d.pitch_rate = i.parse_angular_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("NaturalTurretAngle", |i, d, _| { d.natural_turret_angle = i.parse_angle_real()?; Ok(()) }, UserData::None),
    FieldParse::new("NaturalTurretPitch", |i, d, _| { d.natural_turret_pitch = i.parse_angle_real()?; Ok(()) }, UserData::None),
    FieldParse::new("FirePitch", |i, d, _| { d.fire_pitch = i.parse_angle_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MinPhysicalPitch", |i, d, _| { d.min_pitch = i.parse_angle_real()?; Ok(()) }, UserData::None),
    FieldParse::new("GroundUnitPitch", |i, d, _| { d.ground_unit_pitch = i.parse_angle_real()?; Ok(()) }, UserData::None),
    FieldParse::new("TurretFireAngleSweep", parse_turret_sweep, UserData::None),
    FieldParse::new("TurretSweepSpeedModifier", parse_turret_sweep_speed, UserData::None),
    FieldParse::new("ControlledWeaponSlots", parse_tws, UserData::None),
    FieldParse::new("AllowsPitch", |i, d, _| { d.is_allows_pitch = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("MinIdleScanAngle", |i, d, _| { d.min_idle_scan_angle = i.parse_angle_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxIdleScanAngle", |i, d, _| { d.max_idle_scan_angle = i.parse_angle_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MinIdleScanInterval", |i, d, _| { d.min_idle_scan_interval = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxIdleScanInterval", |i, d, _| { d.max_idle_scan_interval = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("RecenterTime", |i, d, _| { d.recenter_time = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("InitiallyDisabled", |i, d, _| { d.initially_disabled = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("FiresWhileTurning", |i, d, _| { d.fires_while_turning = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl TurretAiData {
    /// `TurretAIData::buildFieldParse(p)`: its own table (no base).
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, TurretAiData>) -> IniResult<()> {
        p.add(&TURRET_FIELD_PARSE)
    }
}

/// `TurretAI`'s fields but its machine.
#[derive(Clone, Debug, PartialEq)]
pub struct TurretData {
    pub data: Arc<TurretAiData>,
    pub which_turret: WhichTurretType,
    pub angle: f32,
    pub pitch: f32,
    /// `m_turretRotOrPitchSound`'s playing handle (0: never played).
    pub sound_handle: u32,
    pub enable_sweep_until: u32,
    /// "The team of the victim at the BEGINNING of the attack."
    pub victim_initial_team: Option<TeamId>,
    pub target: TurretTargetType,
    pub continuous_fire_expiration_frame: u32,
    pub sleep_until: u32,
    pub play_rot_sound: bool,
    pub play_pitch_sound: bool,
    pub positive_sweep: bool,
    pub did_fire: bool,
    pub enabled: bool,
    pub fires_while_turning: bool,
    pub is_force_attacking: bool,
    pub target_was_set_by_idle_mood: bool,
}

/// What the turret asks beyond the fire state: its fields while its machine runs, the owner's model condition and turret
/// reaction, the owner's AI, the attack tests, teams, the bridge's attack points, the partition's geometry, the logic's random
/// values and the rotation sound.
pub trait TurretHost: FireHost {
    /// The turrets whose machines are running, the innermost last.
    fn turret_stack(&mut self) -> &mut Vec<TurretData>;
    /// `setModelConditionState(flag)` / `clearModelConditionState(flag)`.
    fn set_model_condition(&mut self, obj: ObjectId, flag: ModelConditionFlagType, set: bool);
    /// `reactToTurretChange(turret, oldRotation, oldPitch)`.
    fn react_to_turret_change(&mut self, obj: ObjectId, turret: WhichTurretType, old_angle: f32, old_pitch: f32);
    /// `getAIUpdateInterface() != NULL`.
    fn has_ai(&self, obj: ObjectId) -> bool;
    fn ai_next_mood_check_time(&self, obj: ObjectId) -> u32;
    fn ai_reset_next_mood_check_time(&mut self, obj: ObjectId);
    fn ai_turret_sync(&self, obj: ObjectId) -> WhichTurretType;
    fn ai_set_turret_sync(&mut self, obj: ObjectId, turret: WhichTurretType);
    /// `getAI()->getGoalObject()` (its machine's).
    fn ai_goal_object(&self, obj: ObjectId) -> Option<ObjectId>;
    /// `getAI()->getNextMoodTarget(calledByAI, calledDuringIdle)`.
    fn ai_next_mood_target(&mut self, obj: ObjectId, called_by_ai: bool, called_during_idle: bool) -> Option<ObjectId>;
    /// `target->getAI()->addTargeter(attacker, add)`.
    fn ai_add_targeter(&mut self, target: ObjectId, attacker: ObjectId, add: bool);
    fn ai_is_temporarily_preventing_aim_success(&self, obj: ObjectId) -> bool;
    /// `chooseBestWeaponForTarget(target, PREFER_MOST_DAMAGE, CMD_FROM_AI)`.
    fn choose_best_weapon_for_target(&mut self, obj: ObjectId, target: ObjectId);
    fn is_able_to_attack(&self, obj: ObjectId) -> bool;
    fn able_to_attack_specific_object(&self, obj: ObjectId, attack_type: i32, target: ObjectId, source: Option<CommandSourceType>) -> CanAttackResult;
    /// `getTeam()`, as an id (none for `NULL`).
    fn team(&self, obj: ObjectId) -> Option<TeamId>;
    /// `TheTerrainLogic->getBridgeAttackPoints(bridge, &info)`: its two points.
    fn bridge_attack_points(&self, bridge: ObjectId) -> (Coord3D, Coord3D);
    /// `ThePartitionManager->getDistanceSquared(obj, &pos, FROM_BOUNDINGSPHERE_3D)`.
    fn distance_squared_bounding_sphere_3d(&self, obj: ObjectId, pos: &Coord3D) -> f32;
    /// `ThePartitionManager->getRelativeAngle2D(obj, &pos)`.
    fn relative_angle_2d(&self, obj: ObjectId, pos: &Coord3D) -> f32;
    /// `ThePartitionManager->getVectorTo(obj, &pos, FROM_CENTER_3D, v)`.
    fn vector_to_center_3d(&self, obj: ObjectId, pos: &Coord3D) -> Coord3D;
    /// `getGeometryInfo().getMaxHeightAbovePosition()`.
    fn max_height_above_position(&self, obj: ObjectId) -> f32;
    /// `GameLogicRandomValue(lo, hi)`, `GameLogicRandomValueReal(lo, hi)`.
    fn random_value(&mut self, lo: i32, hi: i32) -> i32;
    fn random_value_real(&mut self, lo: f32, hi: f32) -> f32;
    /// `TheAudio->isCurrentlyPlaying(handle)`, `addAudioEvent(&turretMoveLoop of obj)`, `removeAudioEvent(handle)`.
    fn audio_is_playing(&self, handle: u32) -> bool;
    fn audio_add_turret_sound(&mut self, obj: ObjectId) -> u32;
    fn audio_remove(&mut self, handle: u32);
}

/// The turret's machine's states (`TurretAIIdleState` .. `TurretAIHoldTurretState`, and the shared fire state).
pub enum TurretState<H> {
    Idle { next_idle_scan: u32 },
    IdleScan { desired_angle: f32 },
    Aim,
    Fire,
    Recenter,
    Hold { timestamp: u32 },
    #[doc(hidden)]
    _Host(Infallible, PhantomData<fn() -> H>),
}

/// The turret's machine's one condition, `outOfWeaponRangeObject` (fire to aim).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurretCondition {
    OutOfWeaponRangeObject,
}

/// The running turret's fields (the innermost lent to the host).
fn t<H: TurretHost>(h: &mut H) -> &mut TurretData {
    h.turret_stack().last_mut().expect("a running turret")
}

/// `frameToSleepTime(frame1, frame2)`: the earlier frame, as a sleep from now; a frame not after now continues.
fn frame_to_sleep_time<H: TurretHost>(h: &H, frame1: u32, frame2: u32) -> StateReturn {
    let frame1 = frame1.min(frame2).min(FOREVER).min(FOREVER);
    let now = h.frame();
    if frame1 > now {
        StateReturn::sleep(frame1 - now)
    } else {
        StateReturn::CONTINUE // "ignore times that are in the past, since this can frequently happen"
    }
}

/// `TurretStateMachine::setState(id)`: the base's, then the turret woken when the state changed.
pub fn tsm_set_state<H: TurretHost>(sm: &mut StateMachine<TurretState<H>>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let old = sm.get_current_state_id();
    let r = sm.set_state(h, id)?;
    if old != id {
        notify_state_machine_changed(h);
    }
    Ok(r)
}

/// `TurretStateMachine::clear()`.
pub fn tsm_clear<H: TurretHost>(sm: &mut StateMachine<TurretState<H>>, h: &mut H) -> SmResult<()> {
    sm.clear(h)?;
    notify_state_machine_changed(h);
    Ok(())
}

/// `TurretStateMachine::resetToDefaultState()`.
pub fn tsm_reset_to_default_state<H: TurretHost>(sm: &mut StateMachine<TurretState<H>>, h: &mut H) -> SmResult<StateReturn> {
    let r = sm.reset_to_default_state(h)?;
    notify_state_machine_changed(h);
    Ok(r)
}

/// `friend_notifyStateMachineChanged()`: wake now.
fn notify_state_machine_changed<H: TurretHost>(h: &mut H) {
    let now = h.frame();
    t(h).sleep_until = now;
}

/// `friend_turnTowardsAngle(desiredAngle, rateModifier, relThresh)`: one step toward the angle (the rotate condition and sound while
/// turning); whether within `rel_thresh` of it.
fn turn_towards_angle<H: TurretHost>(h: &mut H, desired: f32, rate_modifier: f32, rel_thresh: f32) -> bool {
    let desired = normalize_angle(desired);
    let owner = h.owner();
    let orig = t(h).angle;
    let mut actual = orig;
    let turn_rate = t(h).data.turn_rate * rate_modifier;
    let diff = normalize_angle(desired - actual);
    // "Are we close enough to the desired angle to just snap there?"
    if diff.abs() < turn_rate || (plant(3178) && diff.abs() == turn_rate) {
        actual = desired; // "we are centered"
        h.set_model_condition(owner, MODELCONDITION_TURRET_ROTATE, false);
    } else {
        if diff > 0.0 {
            actual += turn_rate;
        } else {
            actual -= turn_rate;
        }
        h.set_model_condition(owner, MODELCONDITION_TURRET_ROTATE, true);
        t(h).play_rot_sound = true;
    }
    let new = normalize_angle(actual);
    t(h).angle = new;
    if new != orig {
        let (which, pitch) = (t(h).which_turret, t(h).pitch);
        h.react_to_turret_change(owner, which, orig, pitch);
    }
    (new - desired).abs() <= rel_thresh
}

/// `friend_turnTowardsPitch(desiredPitch, rateModifier)`: one step toward the pitch (always there for a turret that does not pitch).
fn turn_towards_pitch<H: TurretHost>(h: &mut H, desired: f32, rate_modifier: f32) -> bool {
    if !t(h).data.is_allows_pitch {
        return true;
    }
    let desired = normalize_angle(desired);
    let mut actual = t(h).pitch;
    let pitch_rate = t(h).data.pitch_rate * rate_modifier;
    let diff = normalize_angle(desired - actual);
    if diff.abs() < pitch_rate {
        actual = desired; // "we are centered"
    } else {
        if diff > 0.0 {
            actual += pitch_rate;
        } else {
            actual -= pitch_rate;
        }
        t(h).play_pitch_sound = true;
    }
    let p = normalize_angle(actual);
    t(h).pitch = p;
    p == desired
}

/// `isWeaponSlotOnTurret(slot)`.
pub fn is_weapon_slot_on_turret(d: &TurretData, slot: WeaponSlotType) -> bool {
    d.data.turret_weapon_slots & (1 << (slot as u32)) != 0
}

/// `isOwnersCurWeaponOnTurret()`.
fn is_owners_cur_weapon_on_turret<H: TurretHost>(h: &mut H) -> bool {
    let owner = h.owner();
    match h.current_weapon(owner) {
        Some(slot) => is_weapon_slot_on_turret(t(h), slot),
        None => false,
    }
}

/// `friend_getTurretTarget(obj, pos, clearDeadTargets = TRUE)`: the target's kind, object and position; a dead or vanished object
/// target is cleared.
fn get_turret_target<H: TurretHost>(sm: &mut StateMachine<TurretState<H>>, h: &mut H) -> (TurretTargetType, Option<ObjectId>, Coord3D) {
    let mut obj = None;
    let mut pos = Coord3D::default();
    match t(h).target {
        TurretTargetType::Object => {
            obj = sm.goal_object(h).map(|o| o.id);
            // "clear it explicitly if null -- could be deceased but holding the old (bogus) objectid internally"
            if obj.is_none_or(|o| h.is_effectively_dead(o)) {
                sm.set_goal_object(None);
                t(h).target = TurretTargetType::None;
                t(h).target_was_set_by_idle_mood = false;
            }
        }
        TurretTargetType::Position => pos = *sm.goal_position(),
        TurretTargetType::None => {}
    }
    (t(h).target, obj, pos)
}

/// `removeSelfAsTargeter()`: an object target's AI forgets this attacker.
fn remove_self_as_targeter<H: TurretHost>(sm: &StateMachine<TurretState<H>>, h: &mut H) {
    if t(h).target == TurretTargetType::Object {
        let me = h.owner();
        if let Some(target) = sm.goal_object(h).map(|o| o.id) {
            if h.has_ai(target) && !plant(3173) {
                h.ai_add_targeter(target, me, false);
            }
        }
    }
}

/// `setTurretTargetObject(victim, forceAttacking)`: see the module's notes.
pub fn set_turret_target_object<H: TurretHost>(sm: &mut StateMachine<TurretState<H>>, h: &mut H, victim: Option<ObjectId>, force_attacking: bool) -> SmResult<()> {
    let owner = h.owner();
    let mut victim = victim;
    if (victim.is_none_or(|v| h.is_effectively_dead(v)) || !is_owners_cur_weapon_on_turret(h)) && !h.ai_are_turrets_linked(owner) {
        victim = None;
    }
    if victim.is_none() {
        // "if nuking the victim, remove self as targeter before doing anything else."
        remove_self_as_targeter(sm, h);
    }
    let r = victim.and_then(|v| h.find_object(v));
    sm.set_goal_object(r.as_ref()); // "could be null! this is OK!"
    t(h).target = if victim.is_some() { TurretTargetType::Object } else { TurretTargetType::None };
    t(h).target_was_set_by_idle_mood = false;
    t(h).is_force_attacking = force_attacking;
    let sid = sm.get_current_state_id();
    if let Some(v) = victim {
        // "if we're already in the aim state, don't call setState"
        if sid != TURRETAI_AIM && sid != TURRETAI_FIRE {
            tsm_set_state(sm, h, TURRETAI_AIM)?;
        }
        let team = h.team(v);
        t(h).victim_initial_team = team;
    } else {
        // "only change states if we are aiming."
        if sid == TURRETAI_AIM || sid == TURRETAI_FIRE {
            tsm_set_state(sm, h, TURRETAI_HOLD)?;
        }
        t(h).victim_initial_team = None;
    }
    Ok(())
}

/// `setTurretTargetPosition(pos)`: see the module's notes.
pub fn set_turret_target_position<H: TurretHost>(sm: &mut StateMachine<TurretState<H>>, h: &mut H, pos: Option<&Coord3D>) -> SmResult<()> {
    let owner = h.owner();
    let mut pos = pos.copied();
    if (pos.is_none() || !is_owners_cur_weapon_on_turret(h)) && !h.ai_are_turrets_linked(owner) {
        pos = None;
    }
    // "remove self as targeter before doing anything else."
    remove_self_as_targeter(sm, h);
    sm.set_goal_object(None);
    if let Some(p) = &pos {
        sm.set_goal_position(Some(p));
    }
    t(h).target = if pos.is_some() { TurretTargetType::Position } else { TurretTargetType::None };
    t(h).target_was_set_by_idle_mood = false;
    let sid = sm.get_current_state_id();
    if pos.is_some() {
        if sid != TURRETAI_AIM && sid != TURRETAI_FIRE {
            tsm_set_state(sm, h, TURRETAI_AIM)?;
        }
    } else if sid == TURRETAI_AIM || sid == TURRETAI_FIRE {
        tsm_set_state(sm, h, TURRETAI_HOLD)?;
    }
    t(h).victim_initial_team = None;
    Ok(())
}

/// `friend_isAnyWeaponInRangeOf(o)`: a weapon of this turret within attack range of the object.
fn is_any_weapon_in_range_of<H: TurretHost>(h: &mut H, o: ObjectId) -> bool {
    let owner = h.owner();
    for slot in SLOTS {
        if !h.has_weapon_in_slot(owner, slot) || !is_weapon_slot_on_turret(t(h), slot) {
            continue;
        }
        if h.weapon_within_range_of_object(owner, slot, o) {
            return true;
        }
    }
    false
}

/// `friend_isSweepEnabled()`.
fn is_sweep_enabled<H: TurretHost>(h: &mut H) -> bool {
    let now = h.frame();
    let until = t(h).enable_sweep_until;
    until != 0 && until > now
}

/// `friend_getNextIdleMoodTargetFrame()`: the AI's next mood check (now without an AI: "ai can be null during object construction").
fn next_idle_mood_target_frame<H: TurretHost>(h: &H) -> u32 {
    let owner = h.owner();
    if h.has_ai(owner) {
        h.ai_next_mood_check_time(owner)
    } else {
        h.frame()
    }
}

/// `friend_checkForIdleMoodTarget()`: the AI's mood target, if the mood allows one, as this turret's target.
fn check_for_idle_mood_target<H: TurretHost>(sm: &mut StateMachine<TurretState<H>>, h: &mut H) -> SmResult<()> {
    let obj = h.owner();
    let mood_adjust = h.ai_mood_action_adjustment(obj, MM_ACTION_IDLE);
    if mood_adjust & MAA_AFFECT_RANGE_IGNORE_ALL != 0 {
        return Ok(());
    }
    // "If we're supposed to attack based on mood, etc, then we will do so."
    if let Some(enemy) = h.ai_next_mood_target(obj, true, true) {
        set_turret_target_object(sm, h, Some(enemy), false)?;
        h.choose_best_weapon_for_target(obj, enemy);
        // "set this magic flag to indicate that this was an independent attack-of-opportunity"
        t(h).target_was_set_by_idle_mood = true;
    }
    Ok(())
}

/// `TurretAIAimTurretState::update()`: see the module's notes.
fn aim_update<H: TurretHost>(sm: &mut StateMachine<TurretState<H>>, h: &mut H) -> SmResult<StateReturn> {
    let obj = h.owner();
    if !h.has_ai(obj) {
        return Ok(StateReturn::FAILURE);
    }
    let mut enemy_has_ai = false;
    let mut enemy_ai_ground = false;
    let mut preventing = false;
    let (target_type, mut enemy, mut enemy_position) = get_turret_target(sm, h);
    let enemy_for_distance_check_only = enemy; // "Do not use this anywhere except for the range check."
    let mut nothing_in_range = false;
    match target_type {
        TurretTargetType::None => return Ok(StateReturn::FAILURE),
        TurretTargetType::Object => {
            let is_primary_enemy = enemy.is_some() && enemy == h.ai_goal_object(obj);
            // "if the enemy is gone, or we're out of range, or it changed teams, the attack is over"
            let mut able = h.is_able_to_attack(obj);
            if able {
                // "since we have already acquired this target, we should use the CONTINUED attack tests"
                let kind = if t(h).is_force_attacking { ATTACK_CONTINUED_TARGET_FORCED } else { ATTACK_CONTINUED_TARGET };
                let source = h.ai_last_command_source(obj);
                let result = h.able_to_attack_specific_object(obj, kind, enemy.expect("an object target"), source);
                able = result == CanAttackResult::Possible || result == CanAttackResult::PossibleAfterMoving;
            }
            let e = enemy.expect("an object target");
            nothing_in_range = !is_any_weapon_in_range_of(h, e);
            let initial_team = t(h).victim_initial_team;
            if !able || (!is_primary_enemy && nothing_in_range) || h.team(e) != initial_team {
                if t(h).target_was_set_by_idle_mood {
                    set_turret_target_object(sm, h, None, false)?;
                }
                return Ok(StateReturn::FAILURE);
            }
            // "aim turret towards enemy (turret angle is relative to its parent object)"
            if h.is_kind_of(e, KINDOF_BRIDGE) {
                // "Special case - bridges have two attackable points at either end."
                let (p1, p2) = h.bridge_attack_points(e);
                let d1 = h.distance_squared_bounding_sphere_3d(obj, &p1);
                enemy_position = if d1 > h.distance_squared_bounding_sphere_3d(obj, &p2) { p2 } else { p1 };
            } else {
                enemy_position = h.position(e);
            }
            enemy_has_ai = h.has_ai(e);
            if enemy_has_ai {
                // "add ourself as a targeter BEFORE calling isTemporarilyPreventingAimSuccess()."
                h.ai_add_targeter(e, obj, true);
                enemy_ai_ground = h.ai_is_doing_ground_movement(e).unwrap_or(false);
            }
            preventing = enemy_has_ai && h.ai_is_temporarily_preventing_aim_success(e);
            // "don't use 'enemy' after this point, just the position."
            enemy = None;
        }
        TurretTargetType::Position => {}
    }
    let Some(slot) = h.current_weapon(obj) else {
        return Ok(StateReturn::FAILURE); // "curWeapon is NULL."
    };
    let mut turn_speed_modifier = 1.0;
    let rel_angle = h.relative_angle_2d(obj, &enemy_position);
    let mut aim_angle = rel_angle;
    let sweep = t(h).data.turret_fire_angle_sweep[slot as usize];
    if sweep > 0.0 && is_sweep_enabled(h) {
        if t(h).positive_sweep {
            aim_angle += sweep;
        } else {
            aim_angle -= sweep;
        }
        turn_speed_modifier = t(h).data.turret_sweep_speed_modifier[slot as usize];
    }
    const REL_THRESH: f32 = 0.035; // "about 2 degrees."
    let mut turn_aligned = turn_towards_angle(h, aim_angle, turn_speed_modifier, REL_THRESH);
    // "this section we do even if sweep is "disabled", so that we can start firing once we get into sweep "range""
    if sweep > 0.0 {
        if turn_aligned {
            let p = t(h).positive_sweep;
            t(h).positive_sweep = !p;
        }
        let angle_diff = normalize_angle(rel_angle - t(h).angle);
        turn_aligned = angle_diff.abs() < sweep;
    }
    let mut pitch_aligned = true;
    if t(h).data.is_allows_pitch {
        let mut desired_pitch;
        let data = t(h).data.clone();
        if data.fire_pitch > 0.0 {
            desired_pitch = data.fire_pitch;
        } else {
            let mut v = h.vector_to_center_3d(obj, &enemy_position);
            v.z -= h.max_height_above_position(obj) / 2.0;
            let len = v.length();
            let actual_pitch = if len > 0.0 { trig::asin(v.z / v.length()) } else { 0.0 };
            desired_pitch = actual_pitch;
            if desired_pitch < data.min_pitch && !plant(3174) {
                desired_pitch = data.min_pitch;
            }
            if data.ground_unit_pitch > 0.0 {
                // (`enemy` is null here: the adjustment is always made; see the module's notes)
                let adjust = enemy.is_none() || enemy_ai_ground;
                if adjust {
                    let mut range = h.weapon_attack_range(obj, slot);
                    let dist = v.length();
                    if range < 1.0 {
                        range = 1.0; // "paranoia. jba."
                    }
                    // "As the unit gets closer, reduce the pitch so we don't shoot over him."
                    let ground_pitch = data.ground_unit_pitch * (dist / range);
                    desired_pitch = actual_pitch + ground_pitch;
                    if desired_pitch < data.min_pitch {
                        desired_pitch = data.min_pitch;
                    }
                }
            }
        }
        pitch_aligned = turn_towards_pitch(h, desired_pitch, 1.0);
    }
    let _ = enemy_has_ai;
    // "we require that we're within range before we can successfully exit the AIM state"
    if turn_aligned && pitch_aligned {
        let in_range = match enemy_for_distance_check_only {
            Some(e) => h.weapon_within_range_of_object(obj, slot, e),
            None => h.weapon_within_range_of_position(obj, slot, &enemy_position),
        };
        if in_range {
            if preventing || nothing_in_range {
                return Ok(StateReturn::CONTINUE);
            }
            return Ok(StateReturn::SUCCESS);
        }
    }
    // "stay in aiming state until directed to do something else"
    Ok(StateReturn::CONTINUE)
}

impl<H: TurretHost> StateBehavior for TurretState<H> {
    type Ctx = H;
    type Condition = TurretCondition;

    fn on_enter(sm: &mut StateMachine<Self>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
        match id {
            TURRETAI_IDLE => {
                let owner = h.owner();
                if h.has_ai(owner) {
                    h.ai_reset_next_mood_check_time(owner);
                    let which = t(h).which_turret;
                    if h.ai_turret_sync(owner) == which {
                        h.ai_set_turret_sync(owner, TURRET_INVALID);
                    }
                } // "ai doesn't exist if the object was just created this frame."
                // (resetIdleScan)
                let now = h.frame();
                let (lo, hi) = (t(h).data.min_idle_scan_interval, t(h).data.max_idle_scan_interval);
                let delay = h.random_value(lo as i32, hi as i32) as u32;
                let next = now.wrapping_add(delay);
                if let Some(TurretState::Idle { next_idle_scan }) = sm.state_mut(id) {
                    *next_idle_scan = next;
                }
                let mood = next_idle_mood_target_frame(h);
                Ok(frame_to_sleep_time(h, mood, next))
            }
            TURRETAI_IDLESCAN => {
                let (min_a, max_a) = (t(h).data.min_idle_scan_angle, t(h).data.max_idle_scan_angle);
                if min_a == 0.0 && max_a == 0.0 {
                    return Ok(StateReturn::SUCCESS);
                }
                let mut desired = min_a + h.random_value_real(0.0, max_a - min_a);
                if h.random_value(0, 1) == 0 {
                    desired = -desired;
                }
                if let Some(TurretState::IdleScan { desired_angle }) = sm.state_mut(id) {
                    *desired_angle = desired;
                }
                Ok(StateReturn::CONTINUE)
            }
            TURRETAI_FIRE => fire_state::fire_on_enter(sm, h),
            TURRETAI_HOLD => {
                let ts = h.frame().wrapping_add(t(h).data.recenter_time).wrapping_add(u32::from(plant(3179)));
                if let Some(TurretState::Hold { timestamp }) = sm.state_mut(id) {
                    *timestamp = ts;
                }
                let mood = next_idle_mood_target_frame(h);
                Ok(frame_to_sleep_time(h, mood, ts))
            }
            _ => Ok(StateReturn::CONTINUE), // (aim, recenter)
        }
    }

    fn on_exit(sm: &mut StateMachine<Self>, h: &mut H, id: StateId, status: StateExitType) -> SmResult<()> {
        if id == TURRETAI_FIRE {
            return fire_state::fire_on_exit(sm, h, status);
        }
        Ok(())
    }

    fn update(sm: &mut StateMachine<Self>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
        match id {
            TURRETAI_IDLE => {
                let now = h.frame();
                let next = match sm.state(id) {
                    Some(TurretState::Idle { next_idle_scan }) => *next_idle_scan,
                    _ => 0,
                };
                if now >= next {
                    return Ok(StateReturn::FAILURE);
                }
                check_for_idle_mood_target(sm, h)?;
                let mood = next_idle_mood_target_frame(h);
                Ok(frame_to_sleep_time(h, mood, next))
            }
            TURRETAI_IDLESCAN => {
                let owner = h.owner();
                if h.test_status(owner, OBJECT_STATUS_UNDER_CONSTRUCTION) {
                    return Ok(StateReturn::CONTINUE); // "so that under-construction base-defenses do not idle-scan"
                }
                let desired = match sm.state(id) {
                    Some(TurretState::IdleScan { desired_angle }) => *desired_angle,
                    _ => 0.0,
                };
                let (na, np) = (t(h).data.natural_turret_angle, t(h).data.natural_turret_pitch);
                let angle_aligned = turn_towards_angle(h, na + desired, 0.5, 0.0);
                let pitch_aligned = turn_towards_pitch(h, np, 0.5);
                Ok(if angle_aligned && pitch_aligned { StateReturn::SUCCESS } else { StateReturn::CONTINUE })
            }
            TURRETAI_AIM => aim_update(sm, h),
            TURRETAI_FIRE => fire_state::fire_update(sm, h),
            TURRETAI_RECENTER => {
                let owner = h.owner();
                if h.test_status(owner, OBJECT_STATUS_UNDER_CONSTRUCTION) {
                    return Ok(StateReturn::CONTINUE); // "so that under-construction base-defenses do not re-center"
                }
                let (na, np) = (t(h).data.natural_turret_angle, t(h).data.natural_turret_pitch);
                let angle_aligned = turn_towards_angle(h, na, if plant(3175) { 1.0 } else { 0.5 }, 0.0);
                let pitch_aligned = turn_towards_pitch(h, np, 0.5);
                Ok(if angle_aligned && pitch_aligned { StateReturn::SUCCESS } else { StateReturn::CONTINUE })
            }
            TURRETAI_HOLD => {
                let ts = match sm.state(id) {
                    Some(TurretState::Hold { timestamp }) => *timestamp,
                    _ => 0,
                };
                if h.frame() >= ts {
                    return Ok(StateReturn::SUCCESS);
                }
                check_for_idle_mood_target(sm, h)?;
                let mood = next_idle_mood_target_frame(h);
                Ok(frame_to_sleep_time(h, mood, ts))
            }
            _ => Ok(StateReturn::CONTINUE),
        }
    }

    fn test(sm: &mut StateMachine<Self>, h: &mut H, _id: StateId, condition: TurretCondition) -> SmResult<bool> {
        match condition {
            TurretCondition::OutOfWeaponRangeObject => Ok(fire_state::out_of_weapon_range_object(sm, h)),
        }
    }

    fn is_attack(&self) -> bool {
        matches!(self, TurretState::Fire)
    }

    /// The states' `crc`: nothing.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// The states' `xfer` (never reached: the turret's machine saves no state; kept as upstream's).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        match self {
            TurretState::Idle { next_idle_scan } => xfer.xfer_unsigned_int(next_idle_scan),
            TurretState::IdleScan { desired_angle } => xfer.xfer_real(desired_angle),
            TurretState::Hold { timestamp } => xfer.xfer_unsigned_int(timestamp),
            _ => Ok(()),
        }
    }

    fn load_post_process(&mut self) {}
}

impl<H: TurretHost> FireAttacker for TurretState<H> {
    /// `TurretAI::notifyFired()`.
    fn notify_fired(_sm: &mut StateMachine<Self>, h: &mut H) -> SmResult<()> {
        t(h).did_fire = true;
        Ok(())
    }
    /// `TurretAI::notifyNewVictimChosen(victim)`.
    fn notify_new_victim_chosen(sm: &mut StateMachine<Self>, h: &mut H, victim: ObjectId) -> SmResult<()> {
        set_turret_target_object(sm, h, Some(victim), false)
    }
    /// `TurretAI::isWeaponSlotOkToFire(slot)`: "If we turrets are linked, ai wants us to fire together, regardless of slot"
    fn is_weapon_slot_ok_to_fire(_sm: &StateMachine<Self>, h: &mut H, slot: WeaponSlotType) -> bool {
        let owner = h.owner();
        if h.ai_are_turrets_linked(owner) {
            return true;
        }
        is_weapon_slot_on_turret(t(h), slot)
    }
    /// `TurretAI::isAttackingObject()`.
    fn is_attacking_object(_sm: &StateMachine<Self>, h: &mut H) -> bool {
        t(h).target == TurretTargetType::Object
    }
    /// `TurretAI::getOriginalVictimPos()`: "yes, we return NULL here"
    fn original_victim_pos(_sm: &StateMachine<Self>, _h: &mut H) -> Option<Coord3D> {
        None
    }
}

/// `class TurretAI`: see the module's notes.
pub struct TurretAi<H: TurretHost> {
    pub data: TurretData,
    pub machine: StateMachine<TurretState<H>>,
}

/// The machine's definitions (`TurretStateMachine::TurretStateMachine`): "order matters: first state is the default state."
fn define_states<H: TurretHost>(sm: &mut StateMachine<TurretState<H>>) -> SmResult<()> {
    let fire_conditions = [StateConditionInfo { test: TurretCondition::OutOfWeaponRangeObject, to_state_id: TURRETAI_AIM }];
    sm.define_state(TURRETAI_IDLE, TurretState::Idle { next_idle_scan: 0 }, TURRETAI_IDLE, TURRETAI_IDLESCAN, &[])?;
    sm.define_state(TURRETAI_IDLESCAN, TurretState::IdleScan { desired_angle: 0.0 }, TURRETAI_HOLD, TURRETAI_HOLD, &[])?;
    sm.define_state(TURRETAI_AIM, TurretState::Aim, TURRETAI_FIRE, TURRETAI_HOLD, &[])?;
    sm.define_state(TURRETAI_FIRE, TurretState::Fire, TURRETAI_AIM, TURRETAI_AIM, &fire_conditions)?;
    sm.define_state(TURRETAI_RECENTER, TurretState::Recenter, TURRETAI_IDLE, TURRETAI_IDLE, &[])?;
    sm.define_state(TURRETAI_HOLD, TurretState::Hold { timestamp: 0 }, TURRETAI_RECENTER, TURRETAI_RECENTER, &[])?;
    Ok(())
}

/// Why a turret cannot be made: `throw INI_INVALID_DATA` ("TurretAI MUST specify controlled weapon slots!").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TurretDataInvalid;

impl<H: TurretHost> TurretAi<H> {
    /// `TurretAI(owner, data, tur)`: at the natural angle and pitch, enabled unless initially disabled, its machine in its default
    /// state (the host's owner is the turret's).
    pub fn new(h: &mut H, data: Arc<TurretAiData>, which: WhichTurretType) -> Result<SmResult<Self>, TurretDataInvalid> {
        if data.turret_weapon_slots == 0 {
            return Err(TurretDataInvalid);
        }
        let d = TurretData {
            which_turret: which,
            angle: data.natural_turret_angle,
            pitch: data.natural_turret_pitch,
            sound_handle: 0,
            enable_sweep_until: 0,
            victim_initial_team: None,
            target: TurretTargetType::None,
            continuous_fire_expiration_frame: u32::MAX,
            sleep_until: 0,
            play_rot_sound: false,
            play_pitch_sound: false,
            positive_sweep: true,
            did_fire: false,
            enabled: !data.initially_disabled,
            fires_while_turning: data.fires_while_turning,
            is_force_attacking: false,
            target_was_set_by_idle_mood: false,
            data,
        };
        let mut turret = TurretAi { data: d, machine: StateMachine::new() };
        let r = define_states(&mut turret.machine).and_then(|_| turret.run(h, |sm, h| sm.init_default_state(h).map(|_| ())));
        Ok(r.map(|_| turret))
    }

    /// Runs `f` on the machine with this turret's fields lent to the host.
    pub fn run<R>(&mut self, h: &mut H, f: impl FnOnce(&mut StateMachine<TurretState<H>>, &mut H) -> R) -> R {
        h.turret_stack().push(self.data.clone());
        let r = f(&mut self.machine, h);
        self.data = h.turret_stack().pop().expect("the lent turret");
        r
    }

    pub fn get_turret_angle(&self) -> f32 {
        self.data.angle
    }
    pub fn get_turret_pitch(&self) -> f32 {
        self.data.pitch
    }
    pub fn is_attacking_object(&self) -> bool {
        self.data.target == TurretTargetType::Object
    }
    pub fn is_force_attacking(&self) -> bool {
        self.data.is_force_attacking
    }
    pub fn is_turret_enabled(&self) -> bool {
        self.data.enabled
    }

    /// `setTurretTargetObject(victim, forceAttacking)`.
    pub fn set_turret_target_object(&mut self, h: &mut H, victim: Option<ObjectId>, force_attacking: bool) -> SmResult<()> {
        self.run(h, |sm, h| set_turret_target_object(sm, h, victim, force_attacking))
    }

    /// `setTurretTargetPosition(pos)`.
    pub fn set_turret_target_position(&mut self, h: &mut H, pos: Option<&Coord3D>) -> SmResult<()> {
        self.run(h, |sm, h| set_turret_target_position(sm, h, pos))
    }

    /// `recenterTurret()`.
    pub fn recenter_turret(&mut self, h: &mut H) -> SmResult<()> {
        self.run(h, |sm, h| tsm_set_state(sm, h, TURRETAI_RECENTER).map(|_| ()))
    }

    /// `isTurretInNaturalPosition()`: under construction counts ("so that under-construction base-defenses do not re-center").
    pub fn is_turret_in_natural_position(&self, h: &H) -> bool {
        if h.test_status(h.owner(), OBJECT_STATUS_UNDER_CONSTRUCTION) {
            return true;
        }
        self.data.data.natural_turret_angle == self.data.angle && self.data.data.natural_turret_pitch == self.data.pitch
    }

    /// `setTurretEnabled(enabled)`: "be sure we wake up!"
    pub fn set_turret_enabled(&mut self, h: &H, enabled: bool) {
        if enabled && !self.data.enabled {
            self.data.sleep_until = h.frame();
        }
        self.data.enabled = enabled;
    }

    /// `isTryingToAimAtTarget(victim)`: aiming at that object.
    pub fn is_trying_to_aim_at_target(&mut self, h: &mut H, victim: ObjectId) -> bool {
        self.run(h, |sm, h| {
            // (the target, which clears a dead one, is asked only while aiming: upstream's `&&` stops first)
            if sm.get_current_state_id() != TURRETAI_AIM && !plant(3177) {
                return false;
            }
            let (kind, obj, _) = get_turret_target(sm, h);
            sm.get_current_state_id() == TURRETAI_AIM && kind == TurretTargetType::Object && obj == Some(victim)
        })
    }

    /// `isWeaponSlotOnTurret(slot)`.
    pub fn is_weapon_slot_on_turret(&self, slot: WeaponSlotType) -> bool {
        is_weapon_slot_on_turret(&self.data, slot)
    }

    /// `isOwnersCurWeaponOnTurret()`.
    pub fn is_owners_cur_weapon_on_turret(&self, h: &H) -> bool {
        h.current_weapon(h.owner()).is_some_and(|s| is_weapon_slot_on_turret(&self.data, s))
    }

    /// `notifyNewVictimChosen(victim)`.
    pub fn notify_new_victim_chosen(&mut self, h: &mut H, victim: ObjectId) -> SmResult<()> {
        self.set_turret_target_object(h, Some(victim), false)
    }

    /// `updateTurretAI()`: see the module's notes; the sleep it asks of its AI.
    pub fn update_turret_ai(&mut self, h: &mut H) -> SmResult<UpdateSleepTime> {
        let now = h.frame();
        if self.data.sleep_until != 0 && now < self.data.sleep_until {
            return Ok(update_sleep(self.data.sleep_until - now));
        }
        let mut sub_machine_sleep = UPDATE_SLEEP_FOREVER; // "assume the best!"
        // "either we don't care about continuous fire stuff, or we care, but time has elapsed"
        if !self.data.fires_while_turning || self.data.continuous_fire_expiration_frame <= now {
            self.data.play_rot_sound = false;
            self.data.play_pitch_sound = false;
        }
        let recentering = self.machine.get_current_state_id() == TURRETAI_RECENTER;
        if self.data.enabled || recentering {
            self.data.did_fire = false;
            // "run the behavior state machine BEFORE doing sound check"
            let st_ret = self.run(h, |sm, h| sm.update_state_machine(h))?;
            if self.data.did_fire {
                // "if we fired, enable sweeping for a few frames."
                const ENABLE_SWEEP_FRAME_COUNT: u32 = 3;
                self.data.enable_sweep_until = now + ENABLE_SWEEP_FRAME_COUNT;
                self.data.continuous_fire_expiration_frame = now + if plant(3176) { 2 } else { ENABLE_SWEEP_FRAME_COUNT };
            }
            if self.data.play_rot_sound || self.data.play_pitch_sound {
                self.start_rot_or_pitch_sound(h);
            } else {
                self.stop_rot_or_pitch_sound(h);
            }
            if st_ret.is_sleep() {
                let frames = st_ret.sleep_frames() as i32;
                if frames < sub_machine_sleep.0 as i32 {
                    sub_machine_sleep = update_sleep(frames as u32);
                }
            } else {
                // "it's STATE_CONTINUE, STATE_SUCCESS, or STATE_FAILURE, any of which will probably require next frame"
                sub_machine_sleep = UPDATE_SLEEP_NONE;
            }
        }
        self.data.sleep_until = now.wrapping_add(sub_machine_sleep.0);
        Ok(sub_machine_sleep)
    }

    /// `startRotOrPitchSound()`.
    fn start_rot_or_pitch_sound(&mut self, h: &mut H) {
        if !h.audio_is_playing(self.data.sound_handle) {
            let owner = h.owner();
            self.data.sound_handle = h.audio_add_turret_sound(owner);
        }
    }

    /// `stopRotOrPitchSound()`.
    fn stop_rot_or_pitch_sound(&mut self, h: &mut H) {
        if h.audio_is_playing(self.data.sound_handle) {
            h.audio_remove(self.data.sound_handle);
        }
    }

    /// `~TurretAI`: the sound stopped, the machine destroyed (its current state exits with `EXIT_RESET`).
    pub fn destroy(mut self, h: &mut H) -> SmResult<()> {
        self.stop_rot_or_pitch_sound(h);
        let TurretAi { data, machine } = self;
        h.turret_stack().push(data);
        let r = machine.destroy(h);
        h.turret_stack().pop();
        r
    }

    /// `crc`: nothing.
    pub fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `xfer` (version 2): the machine (`TurretStateMachine::xfer`: its version alone), the angle and pitch, the sweep frame, the
    /// target (four bytes), the continuous-fire frame, the seven flags, and (version 2) the sleep frame.
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 2;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        {
            // (xferSnapshot(m_turretStateMachine))
            let current: XferVersion = 1;
            let mut v = current;
            xfer.xfer_version(&mut v, current)?;
        }
        let d = &mut self.data;
        xfer.xfer_real(&mut d.angle)?;
        xfer.xfer_real(&mut d.pitch)?;
        xfer.xfer_unsigned_int(&mut d.enable_sweep_until)?;
        let mut target = (d.target as i32).to_le_bytes();
        xfer.xfer_user(&mut target)?;
        d.target = match i32::from_le_bytes(target) {
            1 => TurretTargetType::Object,
            2 => TurretTargetType::Position,
            _ => TurretTargetType::None,
        };
        xfer.xfer_unsigned_int(&mut d.continuous_fire_expiration_frame)?;
        for b in [
            &mut d.play_rot_sound,
            &mut d.play_pitch_sound,
            &mut d.positive_sweep,
            &mut d.did_fire,
            &mut d.enabled,
            &mut d.fires_while_turning,
            &mut d.target_was_set_by_idle_mood,
        ] {
            xfer.xfer_bool(b)?;
        }
        if version >= 2 {
            xfer.xfer_unsigned_int(&mut d.sleep_until)?;
        }
        Ok(())
    }

    /// `loadPostProcess`: the victim's team, when the machine's goal object resolves.
    pub fn load_post_process(&mut self, h: &H) {
        if let Some(victim) = self.machine.goal_object(h) {
            self.data.victim_initial_team = h.team(victim.id);
        }
    }
}

/// (`KindOfType` for the host's doc.)
pub type TurretKindOf = KindOfType;
