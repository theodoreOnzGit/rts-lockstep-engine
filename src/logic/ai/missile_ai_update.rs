// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/MissileAIUpdate.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/MissileAIUpdate.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `MissileAIUpdate` (`p1w13-sonnet`, layer 10, batch A36): a guided missile's flight: pre-launch, launch (falling free for the
//! ignition delay), ignition (armed, exhaust, fuel), attack without turning for its first distance, attack, the lock ("kill",
//! the locomotor cheating straight at the goal), the self-destruction after detonating, dead.
//!
//! The missile's members are [`Missile`], held in [`AiSubclass::Missile`]. [`AiUpdateModule::update`] takes
//! [`missile_pre_update`] (`Some`: return at once) and [`missile_post_update`] around the base's update; the weapon's calls are
//! [`projectile_launch_at_object_or_position`], [`projectile_fire_at_object_or_position`], [`projectile_handle_collision`],
//! [`set_frames_till_countermeasure_diversion_occurs`] and [`projectile_now_jammed`]; `xfer` is version 6.
//!
//! # Upstream facts kept
//!
//! - The lock distance is compared squared, halved first when the missile was fired at a position; the dive distance squared.
//! - The kill-self wait compares `m_stateTimestamp > now - killSelfDelay` unsigned (early frames wrap: no wait).
//! - A launched state that reaches its ignition delay ignites in the same update (the `switch` falls through).
//! - `projectileNowJammed` never sets `m_isJammed` (so a missile is jammed again by each call); its scatter is
//!   `GameLogicRandomValue` of the scatter distance as integers.
//! - The garrison kill loop tests `*it != NULL` before its count: it dereferences the list's end when it gets there (undefined
//!   upstream: refused here, `StateMachineError::Undefined`, before anything is done); likewise the detonation weapon used
//!   unchecked (a collision with an object, the weapon's speed) and a countermeasure that is not there.
//!
//! [`AiUpdateModule::update`]: crate::logic::ai::ai_states::AiUpdateModule::update

use crate::common::base_type::Coord3D;
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID, LAYER_GROUND};
use crate::common::kind_of::KindOfMaskType;
use crate::common::model_state::MODELCONDITION_JAMMED;
use crate::common::object_status_types::{OBJECT_STATUS_BRAKING, OBJECT_STATUS_MISSILE_KILLING_SELF, OBJECT_STATUS_NO_COLLISIONS};
use crate::common::state_machine::{SmResult, StateContext, StateMachine, StateMachineError};
use crate::common::xfer::{Xfer, XferMode, XferResult, XferVersion};
use crate::logic::ai::ai_commands as commands;
use crate::logic::ai::ai_states::{ai, AiState, AiStateHost, DistanceKind};
use crate::logic::ai::ai_update::AiSubclass;
use crate::logic::ai::ai_update_types::CMD_FROM_AI;
use crate::logic::ai::path::PATHFIND_CELL_SIZE_F;
use crate::logic::damage::{DamageInfo, DAMAGE_UNRESISTABLE, DEATH_DETONATED};
use crate::logic::module::body_module::BODY_PRISTINE;
use crate::logic::module::update_module::{UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::plant;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

type Sm<H> = StateMachine<AiState<H>>;

/// `BIGNUM`.
const BIGNUM: f32 = 99999.0;
/// `APPROACH_HEIGHT`.
const APPROACH_HEIGHT: f32 = 10.0;

/// `MissileStateType` ("Stored in save file, don't renumber.").
pub const PRELAUNCH: u32 = 0;
/// "released from launcher, falling".
pub const LAUNCH: u32 = 1;
/// "engines ignite".
pub const IGNITION: u32 = 2;
/// "fly toward victim" (not yet turning).
pub const ATTACK_NOTURN: u32 = 3;
/// "fly toward victim".
pub const ATTACK: u32 = 4;
/// `DEAD`.
pub const DEAD: u32 = 5;
/// "Hit victim (cheat)."
pub const KILL: u32 = 6;
/// "Destroy self."
pub const KILL_SELF: u32 = 7;

/// `MissileAIUpdateModuleData`'s own fields (the FX lists and templates as the host's handles).
#[derive(Clone, Debug, PartialEq)]
pub struct MissileData {
    /// `TryToFollowTarget`: "if true, attack object, not pos".
    pub try_to_follow_target: bool,
    /// `FuelLifetime` (frames; 0 is for ever).
    pub fuel_lifetime: u32,
    /// `IgnitionDelay` (frames).
    pub ignition_delay: u32,
    /// `InitialVelocity`.
    pub initial_vel: f32,
    /// `DistanceToTravelBeforeTurning`.
    pub initial_dist: f32,
    /// `DistanceToTargetBeforeDiving`.
    pub dive_distance: f32,
    /// `IgnitionFX`.
    pub ignition_fx: Option<u32>,
    /// `UseWeaponSpeed`.
    pub use_weapon_speed: bool,
    /// `DetonateOnNoFuel`.
    pub detonate_on_no_fuel: bool,
    /// `GarrisonHitKillCount`.
    pub garrison_hit_kill_count: i32,
    /// `GarrisonHitKillRequiredKindOf`.
    pub garrison_hit_kill_kindof: KindOfMaskType,
    /// `GarrisonHitKillForbiddenKindOf`.
    pub garrison_hit_kill_kindof_not: KindOfMaskType,
    /// `GarrisonHitKillFX`.
    pub garrison_hit_kill_fx: Option<u32>,
    /// `DistanceScatterWhenJammed` (default 75).
    pub distance_scatter_when_jammed: f32,
    /// `DistanceToTargetForLock` (default 75).
    pub lock_distance: f32,
    /// `DetonateCallsKill`.
    pub detonate_calls_kill: bool,
    /// `KillSelfDelay` (frames; default 3).
    pub kill_self_delay: i32,
}

impl Default for MissileData {
    /// `MissileAIUpdateModuleData()`.
    fn default() -> Self {
        MissileData {
            try_to_follow_target: true,
            fuel_lifetime: 0,
            ignition_delay: 0,
            initial_vel: 0.0,
            initial_dist: 0.0,
            dive_distance: 0.0,
            ignition_fx: None,
            use_weapon_speed: false,
            detonate_on_no_fuel: false,
            garrison_hit_kill_count: 0,
            garrison_hit_kill_kindof: KindOfMaskType::default(),
            garrison_hit_kill_kindof_not: KindOfMaskType::default(),
            garrison_hit_kill_fx: None,
            distance_scatter_when_jammed: 75.0,
            lock_distance: 75.0,
            detonate_calls_kill: false,
            kill_self_delay: 3,
        }
    }
}

/// `MissileAIUpdate`'s members, with its module data.
#[derive(Clone, Debug, PartialEq)]
pub struct Missile {
    /// The module data's own fields.
    pub data: MissileData,
    /// `m_state` (a `MissileStateType`; as loaded, any value).
    pub state: u32,
    /// `m_stateTimestamp`.
    pub state_timestamp: u32,
    /// `m_nextTargetTrackTime` (never changed after the constructor).
    pub next_target_track_time: u32,
    /// `m_launcherID`.
    pub launcher_id: ObjectId,
    /// `m_victimID`.
    pub victim_id: ObjectId,
    /// `m_fuelExpirationDate`.
    pub fuel_expiration_date: u32,
    /// `m_noTurnDistLeft`.
    pub no_turn_dist_left: f32,
    /// `m_maxAccel`.
    pub max_accel: f32,
    /// `m_originalTargetPos`.
    pub original_target_pos: Coord3D,
    /// `m_prevPos`.
    pub prev_pos: Coord3D,
    /// `m_extraBonusFlags`.
    pub extra_bonus_flags: u32,
    /// `m_detonationWeaponTmpl` (the host's handle).
    pub detonation_weapon: Option<u32>,
    /// `m_exhaustSysTmpl` (the host's handle).
    pub exhaust_sys: Option<u32>,
    /// `m_exhaustID` (`INVALID_PARTICLE_SYSTEM_ID`, 0: none).
    pub exhaust_id: u32,
    /// `m_framesTillDecoyed`.
    pub frames_till_decoyed: u32,
    /// `m_isTrackingTarget`.
    pub is_tracking_target: bool,
    /// `m_isArmed`.
    pub is_armed: bool,
    /// `m_noDamage`.
    pub no_damage: bool,
    /// `m_isJammed`.
    pub is_jammed: bool,
}

impl Missile {
    /// `MissileAIUpdate(thing, moduleData)`: pre-launch now, at the object's position.
    pub fn new(data: MissileData, now: u32, pos: Coord3D) -> Self {
        Missile {
            state: PRELAUNCH,
            state_timestamp: now,
            // "so that we never recalc target pos, by default"
            next_target_track_time: 0x7fff_ffff,
            launcher_id: INVALID_ID,
            victim_id: INVALID_ID,
            is_armed: false,
            fuel_expiration_date: 0,
            no_turn_dist_left: data.initial_dist,
            prev_pos: pos,
            max_accel: BIGNUM,
            detonation_weapon: None,
            exhaust_sys: None,
            is_tracking_target: false,
            exhaust_id: 0,
            extra_bonus_flags: 0,
            original_target_pos: Coord3D::default(),
            frames_till_decoyed: 0,
            no_damage: false,
            is_jammed: false,
            data,
        }
    }
}

/// The running AI's `MissileAIUpdate` members.
fn ms<H: AiStateHost>(h: &mut H) -> &mut Missile {
    match &mut ai(h).subclass {
        AiSubclass::Missile(m) => m,
        _ => unreachable!("a MissileAIUpdate"),
    }
}

/// The current locomotor, if any.
fn cur_loco<H: AiStateHost>(h: &mut H) -> Option<&mut crate::logic::locomotor::Locomotor> {
    let a = ai(h);
    a.cur_locomotor.and_then(|c| a.locomotor_set.get_mut(c))
}

/// `setMaxAcceleration(accel)`, `setMaxTurnRate(turn)` of the current locomotor, if any.
fn set_loco_accel_turn<H: AiStateHost>(h: &mut H, accel: f32, turn: f32) {
    if let Some(l) = cur_loco(h) {
        l.set_max_acceleration(accel);
        l.set_max_turn_rate(turn);
    }
}

/// `switchToState(s)`: the state and its time, when it changes.
fn switch_to_state<H: AiStateHost>(h: &mut H, s: u32) {
    let now = StateContext::frame(h);
    let m = ms(h);
    if m.state != s {
        m.state = s;
        m.state_timestamp = now;
    }
}

/// `tossExhaust()`: the exhaust's particle system destroyed.
fn toss_exhaust<H: AiStateHost>(h: &mut H) {
    let id = ms(h).exhaust_id;
    if id != 0 {
        h.particle_system_destroy(id);
        ms(h).exhaust_id = 0;
    }
}

/// `projectileLaunchAtObjectOrPosition(...)`: the launcher's id and bonus, the projectile positioned at the launcher's barrel, then
/// fired.
#[allow(clippy::too_many_arguments)]
pub fn projectile_launch_at_object_or_position<H: AiStateHost>(
    sm: &mut Sm<H>,
    h: &mut H,
    victim: ObjectId,
    victim_pos: &Coord3D,
    launcher: ObjectId,
    wslot: i32,
    specific_barrel_to_use: i32,
    det_weap: Option<u32>,
    exhaust_sys_override: Option<u32>,
) -> SmResult<()> {
    let launcher = if h.object_exists(launcher) { launcher } else { INVALID_ID };
    let bonus = if launcher != INVALID_ID { h.object_weapon_bonus_condition(launcher) } else { 0 };
    let m = ms(h);
    m.launcher_id = launcher;
    m.detonation_weapon = det_weap;
    m.extra_bonus_flags = bonus;
    h.position_projectile_for_launch(launcher, wslot, specific_barrel_to_use);
    projectile_fire_at_object_or_position(sm, h, victim, victim_pos, det_weap, exhaust_sys_override)
}

/// `projectileFireAtObjectOrPosition(...)`: the initial push along the object's facing (raised toward a higher target), the facing
/// set to it, launched; toward the victim (followed) or the position (raised for the lock).
pub fn projectile_fire_at_object_or_position<H: AiStateHost>(
    sm: &mut Sm<H>,
    h: &mut H,
    victim: ObjectId,
    victim_pos: &Coord3D,
    det_weap: Option<u32>,
    exhaust_sys_override: Option<u32>,
) -> SmResult<()> {
    let d = ms(h).data.clone();
    let victim = if h.object_exists(victim) { victim } else { INVALID_ID };
    {
        let m = ms(h);
        m.exhaust_sys = exhaust_sys_override;
        m.detonation_weapon = det_weap;
    }
    let mut initial_vel_to_use = d.initial_vel;
    if d.use_weapon_speed {
        let Some(det) = det_weap else {
            return Err(StateMachineError::Undefined("a missile's weapon speed without its weapon"));
        };
        let weapon_speed = h.weapon_template_speed(det);
        initial_vel_to_use = weapon_speed;
        if cur_loco(h).is_some() {
            ms(h).max_accel = weapon_speed;
            let max_accel = ms(h).max_accel;
            let l = cur_loco(h).expect("the locomotor");
            l.set_max_speed(weapon_speed);
            l.set_max_acceleration(max_accel);
        }
    }
    let pos = h.position();
    let delta_z = victim_pos.z - pos.z;
    let dx = victim_pos.x - pos.x;
    let dy = victim_pos.y - pos.y;
    let mut xy_dist = (dx * dx + dy * dy).sqrt();
    if xy_dist < 1.0 {
        xy_dist = 1.0;
    }
    let z_factor = if delta_z > 0.0 { delta_z / xy_dist } else { 0.0 };
    let mut dir = h.transform_matrix().get_x_vector();
    dir.normalize();
    dir.z += 2.0 * z_factor;
    dir.normalize();
    if h.has_physics() && initial_vel_to_use > 0.0 {
        let force_mag = h.mass() * initial_vel_to_use;
        let force = Coord3D { x: force_mag * dir.x, y: force_mag * dir.y, z: force_mag * dir.z };
        h.apply_motive_force(&force);
    }
    let obj_pos = Vector3 { x: pos.x, y: pos.y, z: pos.z };
    let mut new_xform = Matrix3D::default();
    new_xform.build_transform_matrix(&obj_pos, &dir);
    h.set_transform_matrix(&new_xform);
    switch_to_state(h, LAUNCH);
    ms(h).is_tracking_target = false;
    if victim != INVALID_ID && d.try_to_follow_target {
        let vpos = h.object_position(victim);
        sm.set_goal_position(Some(&vpos));
        commands::ai_move_to_object(sm, h, victim, CMD_FROM_AI)?;
        let m = ms(h);
        m.original_target_pos = vpos;
        // "Remember that I was originally shot at a moving object"
        m.is_tracking_target = true;
        m.victim_id = victim;
    } else {
        let mut initial_pos = *victim_pos;
        ms(h).original_target_pos = *victim_pos;
        if d.lock_distance > 0.0 {
            initial_pos.z += APPROACH_HEIGHT;
        }
        commands::ai_move_to_position(sm, h, &initial_pos, CMD_FROM_AI)?;
        ms(h).victim_id = INVALID_ID;
    }
    // "extending access to the victim via the parent class"
    crate::logic::ai::ai_states::with_ai(h, |a, h| a.set_current_victim(h, victim));
    ms(h).prev_pos = h.position();
    Ok(())
}

/// `projectileHandleCollision(other)`: unarmed, nothing; the ground well above the goal, nothing; an object the weapon does not
/// collide with, nothing; a garrisoned building, its garrison killed (up to the count) and the missile destroyed; else
/// detonated, and collisions off. (`true`: "Returns true if the physics collide should apply the force.")
pub fn projectile_handle_collision<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, other: ObjectId) -> SmResult<bool> {
    let d = ms(h).data.clone();
    if !ms(h).is_armed {
        return Ok(true);
    }
    let other = if h.object_exists(other) { other } else { INVALID_ID };
    if other == INVALID_ID {
        let goal = *sm.goal_position();
        let pos = h.position();
        let delta = Coord3D { x: pos.x - goal.x, y: pos.y - goal.y, z: pos.z - goal.z };
        if delta.z > PATHFIND_CELL_SIZE_F && delta.length() > 3.0 * PATHFIND_CELL_SIZE_F {
            return Ok(true);
        }
    } else {
        let launcher_id = ms(h).launcher_id;
        let launcher = if h.object_exists(launcher_id) { launcher_id } else { INVALID_ID };
        let Some(det) = ms(h).detonation_weapon else {
            return Err(StateMachineError::Undefined("a missile's collision without its weapon"));
        };
        let victim_id = ms(h).victim_id;
        if !h.weapon_template_should_projectile_collide_with(det, launcher, other, victim_id) {
            return Ok(true);
        }
        if d.garrison_hit_kill_count > 0 && h.object_garrison_hit_killable(other) {
            let items = h.contain_items(other);
            // (the loop tests `*it != NULL` first: it must end on the count before the list's end)
            let mut num_killed = 0;
            let mut ended = false;
            for (k, &thing) in items.iter().enumerate() {
                if !h.object_is_effectively_dead(thing)
                    && h.object_is_kind_of_multi(thing, &d.garrison_hit_kill_kindof, &d.garrison_hit_kill_kindof_not)
                {
                    num_killed += 1;
                    if num_killed >= d.garrison_hit_kill_count && k + 1 < items.len() {
                        ended = true;
                        break;
                    }
                }
            }
            // (the stand-in's empty list is none: no loop)
            if !items.is_empty() && !ended {
                return Err(StateMachineError::Undefined("a missile's garrison kill past the list's end"));
            }
            let mut num_killed = 0;
            for &thing in &items {
                if num_killed >= d.garrison_hit_kill_count {
                    break;
                }
                if !h.object_is_effectively_dead(thing)
                    && h.object_is_kind_of_multi(thing, &d.garrison_hit_kill_kindof, &d.garrison_hit_kill_kindof_not)
                {
                    if launcher != INVALID_ID && !plant(3670) {
                        h.object_score_the_kill(launcher, thing);
                    }
                    h.object_kill(thing);
                    num_killed += 1;
                }
            }
            if num_killed > 0 {
                h.fx_do_obj(d.garrison_hit_kill_fx, other);
                h.destroy_object();
                return Ok(true);
            }
        }
    }
    detonate(h);
    h.set_status_bits(&[OBJECT_STATUS_NO_COLLISIONS], true);
    Ok(true)
}

/// `detonate()`: the weapon's detonation (and the missile's death, when the weapon says so), else its own death (unless it does no
/// damage); hidden; killing itself.
fn detonate<H: AiStateHost>(h: &mut H) {
    let (det, bonus, no_damage) = {
        let m = ms(h);
        (m.detonation_weapon, m.extra_bonus_flags, m.no_damage)
    };
    if let Some(det) = det {
        h.weapon_store_handle_projectile_detonation(det, bonus, !no_damage);
        if h.weapon_template_die_on_detonate(det) {
            damage_self_detonated(h);
        }
    } else if !no_damage && !plant(3671) {
        damage_self_detonated(h);
    }
    h.set_drawable_hidden(true);
    switch_to_state(h, KILL_SELF);
    h.set_status_bits(&[OBJECT_STATUS_MISSILE_KILLING_SELF], true);
}

/// The detonation's own death: unresistable, "detonated", no source, the body's maximum health.
fn damage_self_detonated<H: AiStateHost>(h: &mut H) {
    let mut damage_info = DamageInfo::default();
    damage_info.input.damage_type = DAMAGE_UNRESISTABLE;
    damage_info.input.death_type = DEATH_DETONATED;
    damage_info.input.source_id = INVALID_ID;
    damage_info.input.amount = h.body_max_health();
    h.attempt_damage(&mut damage_info);
}

/// `doKillSelfState()`: after the kill-self delay, killed (or destroyed) when it has a weapon; dead.
fn do_kill_self_state<H: AiStateHost>(h: &mut H) {
    let now = StateContext::frame(h);
    let m = ms(h);
    if m.state_timestamp > now.wrapping_sub(m.data.kill_self_delay as u32) {
        return;
    }
    if m.detonation_weapon.is_some() {
        if m.data.detonate_calls_kill {
            // "kill it (vs destroying it) so that its Die modules are called"
            h.kill_self();
        } else {
            h.destroy_object();
        }
    }
    switch_to_state(h, DEAD);
}

/// `doLaunchState()`: no motive power until the ignition delay has passed.
fn do_launch_state<H: AiStateHost>(h: &mut H) {
    set_loco_accel_turn(h, 0.0, 0.0);
    let now = StateContext::frame(h);
    let m = ms(h);
    if now.wrapping_sub(m.state_timestamp) >= m.data.ignition_delay {
        switch_to_state(h, IGNITION);
    }
}

/// `doIgnitionState()`: power on (no turning yet), the ignition FX and exhaust, armed, the fuel's end.
fn do_ignition_state<H: AiStateHost>(h: &mut H) {
    let max_accel = ms(h).max_accel;
    set_loco_accel_turn(h, max_accel, 0.0);
    let d = ms(h).data.clone();
    h.fx_do_obj(d.ignition_fx, h.object_id());
    if let Some(t) = ms(h).exhaust_sys {
        let id = h.particle_system_create_attached(t);
        ms(h).exhaust_id = id;
    }
    let now = StateContext::frame(h);
    let m = ms(h);
    m.is_armed = true;
    m.fuel_expiration_date = if d.fuel_lifetime != 0 { now.wrapping_add(d.fuel_lifetime) } else { 0x7fff_ffff };
    switch_to_state(h, ATTACK_NOTURN);
}

/// `doAttackState(turnOK)`: out of fuel, detonate or coast; the lock (the goal close enough: the kill state); the dive; turning
/// once the first distance is flown; the tracked target gone.
fn do_attack_state<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, turn_ok: bool) -> SmResult<()> {
    let d = ms(h).data.clone();
    let now = StateContext::frame(h);
    if now >= ms(h).fuel_expiration_date {
        if d.detonate_on_no_fuel {
            detonate(h);
            return Ok(());
        }
        set_loco_accel_turn(h, 0.0, 0.0);
        toss_exhaust(h);
    } else {
        let max_accel = ms(h).max_accel;
        set_loco_accel_turn(h, max_accel, if turn_ok { BIGNUM } else { 0.0 });
    }
    if d.lock_distance > 0.0 {
        let mut lock_distance_squared = d.lock_distance;
        let tracking = ms(h).is_tracking_target;
        let goal_object = sm.goal_object(h).map(|o| o.id);
        let distance_to_target_squared = match goal_object {
            Some(g) if tracking => h.distance_squared_to_object(g, DistanceKind::Center2d),
            _ => {
                let goal = *sm.goal_position();
                h.distance_squared_to_position_2d(&goal)
            }
        };
        if lock_distance_squared > 0.0 {
            if !tracking {
                lock_distance_squared *= 0.5;
            }
            lock_distance_squared *= lock_distance_squared;
            if distance_to_target_squared < lock_distance_squared && !plant(3672) {
                if !tracking {
                    let original = ms(h).original_target_pos;
                    commands::ai_move_to_position(sm, h, &original, CMD_FROM_AI)?;
                }
                switch_to_state(h, KILL);
                return Ok(());
            }
        }
    }
    if cur_loco(h).is_some_and(|l| l.get_preferred_height() > 0.0) {
        let goal = *sm.goal_position();
        let distance_to_target_squared = h.distance_squared_to_position_2d(&goal);
        let mut dive_distance_squared = d.dive_distance;
        dive_distance_squared *= dive_distance_squared;
        if distance_to_target_squared < dive_distance_squared {
            cur_loco(h).expect("the locomotor").set_use_precise_z_pos(true);
        }
    }
    if ms(h).no_turn_dist_left <= 0.0 {
        switch_to_state(h, ATTACK);
    }
    if ms(h).is_tracking_target && sm.goal_object(h).is_none() {
        airborne_target_gone(h);
    }
    Ok(())
}

/// `doKillState()`: out of fuel, detonate or give up; braking (the locomotor's cheat), full power and turning; idle at the goal
/// object (close enough: there, detonated; else after it again), or without one, detonated.
fn do_kill_state<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<()> {
    let now = StateContext::frame(h);
    if now >= ms(h).fuel_expiration_date {
        if ms(h).data.detonate_on_no_fuel {
            detonate(h);
            return Ok(());
        }
        airborne_target_gone(h);
        return Ok(());
    }
    h.set_status_bits(&[OBJECT_STATUS_BRAKING], true);
    let max_accel = ms(h).max_accel;
    set_loco_accel_turn(h, max_accel, BIGNUM);
    let idle = crate::logic::ai::ai_states::with_ai(h, |a, h| a.is_idle(&*sm, h));
    if idle {
        if let Some(goal) = sm.goal_object(h).map(|o| o.id) {
            let mut close_enough = 1.0f32;
            if ai(h).cur_locomotor.is_some() {
                let a = h.ai_stack().pop().expect("a running AI");
                let l = a.locomotor_set.get(a.cur_locomotor.expect("the locomotor")).expect("the locomotor");
                close_enough = l.get_max_speed_for_condition(h, BODY_PRISTINE);
                h.ai_stack().push(a);
            }
            let distance_to_target_sq = h.distance_squared_to_object(goal, DistanceKind::BoundingSphere3d);
            if distance_to_target_sq < close_enough * close_enough {
                let pos = h.object_position(goal);
                h.set_position(&pos);
                detonate(h);
            } else {
                commands::ai_move_to_object(sm, h, goal, CMD_FROM_AI)?;
            }
        } else {
            detonate(h);
        }
    }
    if ms(h).is_tracking_target && sm.goal_object(h).is_none() {
        airborne_target_gone(h);
    }
    Ok(())
}

/// `airborneTargetGone()`: "My airborne target has died": out of fuel now, killing itself.
fn airborne_target_gone<H: AiStateHost>(h: &mut H) {
    ms(h).fuel_expiration_date = StateContext::frame(h);
    switch_to_state(h, KILL_SELF);
}

/// `MissileAIUpdate::update()`'s part before the base's (the AI lent): the distance flown before turning, the countermeasure's
/// diversion, below the ground (destroyed: `Some`, upstream's early return), the state's step.
pub fn missile_pre_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<Option<UpdateSleepTime>> {
    let new_pos = h.position();
    {
        let m = ms(h);
        if m.no_turn_dist_left > 0.0 && m.state >= IGNITION {
            let p = m.prev_pos;
            let dist_this_turn = ((new_pos.x - p.x) * (new_pos.x - p.x) + (new_pos.y - p.y) * (new_pos.y - p.y)
                + (new_pos.z - p.z) * (new_pos.z - p.z))
                .sqrt();
            m.no_turn_dist_left -= dist_this_turn;
            m.prev_pos = new_pos;
        }
    }
    let now = StateContext::frame(h);
    let decoy = ms(h).frames_till_decoyed;
    if decoy != 0 && decoy <= now {
        let m = ms(h);
        m.frames_till_decoyed = 0;
        m.no_damage = true;
        let victim = m.victim_id;
        if h.object_exists(victim) {
            let target_id = h.calculate_countermeasure_to_divert_to(victim);
            if target_id != INVALID_ID && !plant(3673) {
                if !h.object_exists(target_id) {
                    return Err(StateMachineError::Undefined("a missile diverted to a countermeasure that is not there"));
                }
                let vpos = h.object_position(target_id);
                sm.set_goal_position(Some(&vpos));
                commands::ai_move_to_object(sm, h, target_id, CMD_FROM_AI)?;
                let m = ms(h);
                m.original_target_pos = vpos;
                m.is_tracking_target = true;
                m.victim_id = target_id;
            }
        }
    }
    if new_pos.z < 0.0 {
        h.destroy_object();
        return Ok(Some(UPDATE_SLEEP_FOREVER));
    }
    match ms(h).state {
        PRELAUNCH | DEAD => set_loco_accel_turn(h, 0.0, 0.0),
        LAUNCH => {
            do_launch_state(h);
            if ms(h).state == IGNITION {
                do_ignition_state(h);
            }
        }
        IGNITION => do_ignition_state(h),
        ATTACK_NOTURN => do_attack_state(sm, h, false)?,
        ATTACK => do_attack_state(sm, h, true)?,
        KILL => do_kill_state(sm, h)?,
        KILL_SELF => do_kill_self_state(h),
        _ => {}
    }
    Ok(None)
}

/// `MissileAIUpdate::update()`'s part after the base's (the AI lent): the layer the missile is over; an armed missile that leaves a
/// bridge for the ground below it detonates on the bridge.
pub fn missile_post_update<H: AiStateHost>(h: &mut H) -> UpdateSleepTime {
    let old_layer: PathfindLayer = h.layer();
    let pos = h.position();
    let new_layer = h.highest_layer_for_destination(&pos);
    h.object_set_layer(new_layer);
    if ms(h).is_armed && old_layer != LAYER_GROUND && new_layer == LAYER_GROUND && !plant(3674) {
        let mut tmp = h.position();
        tmp.z = 9999.0;
        let test_layer = h.highest_layer_for_destination(&tmp);
        if test_layer == old_layer {
            const FUDGE: f32 = 2.0;
            tmp.z = h.layer_height(tmp.x, tmp.y, test_layer) + FUDGE;
            h.set_position(&tmp);
            detonate(h);
            return UPDATE_SLEEP_NONE;
        }
    }
    UPDATE_SLEEP_NONE
}

/// `setFramesTillCountermeasureDiversionOccurs(frames)`.
pub fn set_frames_till_countermeasure_diversion_occurs<H: AiStateHost>(h: &mut H, frames: u32) {
    let now = StateContext::frame(h);
    ms(h).frames_till_decoyed = now.wrapping_add(frames);
}

/// `projectileNowJammed()`: "We lose our Object target and scatter to the ground".
pub fn projectile_now_jammed<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<()> {
    if ms(h).is_jammed {
        return Ok(());
    }
    h.set_model_condition_state(MODELCONDITION_JAMMED);
    let tracking = ms(h).is_tracking_target;
    let goal_object = sm.goal_object(h).map(|o| o.id);
    let mut target_position = match goal_object {
        Some(g) if tracking => h.object_position(g),
        _ => *sm.goal_position(),
    };
    let scatter = ms(h).data.distance_scatter_when_jammed;
    target_position.x += h.logic_random_value(-scatter as i32, scatter as i32) as f32;
    target_position.y += h.logic_random_value(-scatter as i32, scatter as i32) as f32;
    let layer = h.highest_layer_for_destination(&target_position);
    target_position.z = h.layer_height(target_position.x, target_position.y, layer);
    sm.set_goal_object(None);
    commands::ai_move_to_position(sm, h, &target_position, CMD_FROM_AI)?;
    let m = ms(h);
    m.is_tracking_target = false;
    m.original_target_pos = target_position;
    m.victim_id = INVALID_ID;
    Ok(())
}

/// `MissileAIUpdate::xfer`'s own members, after the base's (version 6).
pub fn missile_xfer_members<H: AiStateHost>(m: &mut Missile, h: &mut H, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
    let load = xfer.get_xfer_mode() == XferMode::Load;
    if version >= 2 {
        xfer.xfer_coord3d(&mut m.original_target_pos)?;
    }
    let mut b = m.state.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    m.state = u32::from_le_bytes(b);
    xfer.xfer_unsigned_int(&mut m.state_timestamp)?;
    xfer.xfer_unsigned_int(&mut m.next_target_track_time)?;
    xfer.xfer_object_id(&mut m.launcher_id)?;
    xfer.xfer_object_id(&mut m.victim_id)?;
    xfer.xfer_bool(&mut m.is_armed)?;
    xfer.xfer_unsigned_int(&mut m.fuel_expiration_date)?;
    xfer.xfer_real(&mut m.no_turn_dist_left)?;
    xfer.xfer_real(&mut m.max_accel)?;
    let mut weapon_name = m.detonation_weapon.map(|w| h.weapon_template_name(w)).unwrap_or_default();
    xfer.xfer_ascii_string(&mut weapon_name)?;
    if load && !weapon_name.is_empty() && m.detonation_weapon.is_none() {
        m.detonation_weapon = h.weapon_template_by_name(&weapon_name);
    }
    let mut exhaust_name = m.exhaust_sys.map(|t| h.particle_template_name(t)).unwrap_or_default();
    xfer.xfer_ascii_string(&mut exhaust_name)?;
    if load && !exhaust_name.is_empty() && m.exhaust_sys.is_none() {
        m.exhaust_sys = h.particle_template_by_name(&exhaust_name);
    }
    xfer.xfer_bool(&mut m.is_tracking_target)?;
    if version >= 3 {
        xfer.xfer_coord3d(&mut m.prev_pos)?;
    }
    if version >= 4 {
        xfer.xfer_unsigned_int(&mut m.extra_bonus_flags)?;
        let mut b = m.exhaust_id.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        m.exhaust_id = u32::from_le_bytes(b);
    }
    if version >= 5 {
        xfer.xfer_unsigned_int(&mut m.frames_till_decoyed)?;
        xfer.xfer_bool(&mut m.no_damage)?;
    }
    if version >= 6 && !plant(3675) {
        xfer.xfer_bool(&mut m.is_jammed)?;
    }
    Ok(())
}
