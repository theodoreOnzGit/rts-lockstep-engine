// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpecialPowerModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/SpecialPowerModule.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! What every special power module shares: `SpecialPowerModule` (the layer-5 state and arithmetic in `logic::module::special_power_module`) made to
//! act on the `World`. The base class is concrete upstream but never registered with the module factory (`OCLSpecialPower` and the other powers
//! derive from it); [`SpecialPowerBehavior`] is the contract a power has to the functions here, with the base's four `doSpecialPower...`, its
//! `markSpecialPowerTriggered` and `startPowerRecharge` as the defaults a power overrides and calls back ([`base_do_special_power`] and its
//! siblings).
//!
//! # What is looked up when
//!
//! A module's data proc has no store to look the power's template up in, so [`SpecialPowerModuleData`](crate::logic::module::special_power_module::SpecialPowerModuleData)
//! keeps the name that was written ([`special_power_template_name`](crate::logic::module::special_power_module::SpecialPowerModuleData::special_power_template_name)) and
//! the module finds the template in `world.statics.special_power_store` whenever it needs it. A name that is no template is `NULL` upstream and
//! every function that follows it dereferences it: [`Undefined::NullSpecialPowerTemplate`].
//!
//! # The player's shared timers
//!
//! The ready frame of a `SharedSyncedTimer` power belongs to the player (`Player::getOrStartSpecialPowerReadyFrame` and the two siblings the
//! module calls). The `World` does not implement the player host yet (layer 12), so the module works on the player's timer list directly through
//! the list functions of `player_ops` (`special_power_timers_*`), the very ones the host-generic player functions are built from.
//!
//! # Deviation: the destructor
//!
//! `~SpecialPowerModule` tells `TheInGameUI` to remove the superweapon the constructor added. The model has no hook for a module's destruction, so
//! that client event is not sent when the object is deleted (the client would show a timer for an object that is gone); open until the object's
//! destruction runs its modules' destructors.
//!
//! `p1w11-sonnet`, 2026-10-21.

use crate::common::base_type::Coord3D;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{IniError, Undefined};
use crate::common::kind_of::KINDOF_STRUCTURE;
use crate::common::module::ObjectId;
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_UNDER_CONSTRUCTION};
use crate::common::special_power_type::*;
use crate::logic::module::special_power_module::{
    RechargeAction, SpecialPowerContext, SpecialPowerModule, SpecialPowerTiming,
};
use crate::logic::modules::lend_clone;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::{EvaSuperweapon, EvaWhose, ObjectClientEvent};
use crate::logic::world::player_ops::{
    special_power_timers_express, special_power_timers_get_or_start,
    special_power_timers_reset_or_start,
};
use crate::logic::world::update_body::UpdateBody;

/// A special power module: a compile-time contract, used as a generic bound by the functions of this file.
pub trait SpecialPowerBehavior: Clone + PartialEq + Sized {
    /// The `SpecialPowerModule` base (its state and data).
    fn power(&self) -> &SpecialPowerModule;
    /// The base, mutably.
    fn power_mut(&mut self) -> &mut SpecialPowerModule;
    /// The module as the variant of [`UpdateBody`] it is.
    fn into_body(self) -> UpdateBody;
    /// The module back out of its variant; `None` for another variant.
    fn from_body(body: UpdateBody) -> Option<Self>;
    /// `doSpecialPower(commandOptions)` (virtual): the base's unless the power overrides it.
    fn do_special_power(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        command_options: u32,
    ) -> Result<(), IniError> {
        base_do_special_power(self, world, me, command_options)
    }
    /// `doSpecialPowerAtObject(obj, commandOptions)` (virtual).
    fn do_special_power_at_object(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        target: ObjectId,
        command_options: u32,
    ) -> Result<(), IniError> {
        base_do_special_power_at_object(self, world, me, target, command_options)
    }
    /// `doSpecialPowerAtLocation(loc, angle, commandOptions)` (virtual).
    fn do_special_power_at_location(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        loc: Coord3D,
        angle: f32,
        command_options: u32,
    ) -> Result<(), IniError> {
        base_do_special_power_at_location(self, world, me, loc, angle, command_options)
    }
    /// `doSpecialPowerUsingWaypoints(way, commandOptions)` (virtual), the waypoint by id.
    fn do_special_power_using_waypoints(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        waypoint: u32,
        command_options: u32,
    ) -> Result<(), IniError> {
        base_do_special_power_using_waypoints(self, world, me, waypoint, command_options)
    }
    /// A call the base does not answer (a power's own interface): [`ModuleReply::Unhandled`] unless it overrides this.
    fn other_call(
        &mut self,
        _world: &mut World,
        _me: ModuleHandle,
        _call: ModuleCall,
    ) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

// ---- what the functions read of the world ----------------------------------------------------------------------------------------------

/// The data's template as an id: the id read with a store, else the name looked up now.
fn template_id(world: &World, power: &SpecialPowerModule) -> Option<u32> {
    let data = power.special_power_module_data();
    data.special_power_template.or_else(|| {
        data.special_power_template_name.as_ref().and_then(|n| {
            world
                .statics
                .special_power_store
                .find_special_power_template(n)
                .map(|t| t.id())
        })
    })
}

/// The reload time and `isSharedNSync()` of template `id`.
fn timing(world: &World, id: u32) -> Option<SpecialPowerTiming> {
    world
        .statics
        .special_power_store
        .find_special_power_template_by_id(id)
        .map(|t| SpecialPowerTiming {
            reload_time: t.reload_time(),
            is_shared_n_sync: t.is_shared_n_sync(),
        })
}

/// The `SpecialPowerContext` of the module on `me`: the frame, the template's timing, whether there is a controlling player, whether the object is
/// disabled. [`Undefined::NullSpecialPowerTemplate`] for a null template with a controlling player.
fn context(
    world: &World,
    me: ModuleHandle,
    power: &SpecialPowerModule,
) -> Result<SpecialPowerContext, IniError> {
    let template = template_id(world, power).and_then(|id| timing(world, id));
    let has_player = world.get_controlling_player(me.object).is_some();
    let disabled = world
        .objects
        .find(me.object)
        .is_some_and(|o| o.is_disabled());
    SpecialPowerContext::new(world.frame(), template, has_player, disabled)
}

/// `Player::getOrStartSpecialPowerReadyFrame(template)` for the object's controlling player (0 for none: `uses_player_ready_frame` makes sure
/// there is one).
fn shared_ready_frame(world: &mut World, me: ModuleHandle, template: Option<u32>) -> u32 {
    let (Some(player), Some(template)) = (world.get_controlling_player(me.object), template) else {
        return 0;
    };
    let now = world.frame();
    world.players.get_nth_mut(player).map_or(0, |p| {
        special_power_timers_get_or_start(&mut p.special_power_timers, template, now)
    })
}

fn null_template() -> IniError {
    IniError::undefined(Undefined::NullSpecialPowerTemplate)
}

// ---- the interface ----------------------------------------------------------------------------------------------------------------------------

/// `isReady()`.
pub fn is_ready<M: SpecialPowerBehavior>(
    m: &M,
    world: &mut World,
    me: ModuleHandle,
) -> Result<bool, IniError> {
    let ctx = context(world, me, m.power())?;
    let t = template_id(world, m.power());
    Ok(m.power()
        .is_ready(&ctx, || shared_ready_frame(world, me, t)))
}

/// `getPercentReady()`.
pub fn get_percent_ready<M: SpecialPowerBehavior>(
    m: &M,
    world: &mut World,
    me: ModuleHandle,
) -> Result<f32, IniError> {
    let ctx = context(world, me, m.power())?;
    let t = template_id(world, m.power());
    Ok(m.power()
        .get_percent_ready(&ctx, || shared_ready_frame(world, me, t)))
}

/// `getReadyFrame() const`.
pub fn get_ready_frame<M: SpecialPowerBehavior>(
    m: &M,
    world: &mut World,
    me: ModuleHandle,
) -> Result<u32, IniError> {
    let ctx = context(world, me, m.power())?;
    let t = template_id(world, m.power());
    Ok(m.power()
        .get_ready_frame(&ctx, || shared_ready_frame(world, me, t)))
}

/// `pauseCountdown(pause)`.
pub fn pause_countdown<M: SpecialPowerBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    pause: bool,
) -> Result<(), IniError> {
    let ctx = context(world, me, m.power())?;
    let t = template_id(world, m.power());
    let mut shared = || shared_ready_frame(world, me, t);
    m.power_mut().pause_countdown(pause, &ctx, &mut shared);
    Ok(())
}

/// `setReadyFrame(frame)`.
pub fn set_ready_frame<M: SpecialPowerBehavior>(m: &mut M, world: &World, frame: u32) {
    let now = world.frame();
    m.power_mut().set_ready_frame(frame, now);
}

/// `SpecialPowerModule::startPowerRecharge()`.
pub fn base_start_power_recharge<M: SpecialPowerBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
) -> Result<(), IniError> {
    // (`DEBUG_CRASH("special power not found")` is compiled out: the template-less module recharges nothing)
    let t = template_id(world, m.power());
    let ctx = match context(world, me, m.power()) {
        Ok(c) => c,
        Err(e) => return if t.is_none() { Ok(()) } else { Err(e) },
    };
    if m.power_mut().start_power_recharge(&ctx) == RechargeAction::ResetPlayerReadyFrame {
        if let (Some(player), Some(tid), Some(tm)) = (
            world.get_controlling_player(me.object),
            t,
            t.and_then(|id| timing(world, id)),
        ) {
            let now = world.frame();
            if let Some(p) = world.players.get_nth_mut(player) {
                special_power_timers_reset_or_start(
                    &mut p.special_power_timers,
                    tid,
                    tm.reload_time,
                    now,
                );
            }
        }
    }
    Ok(())
}

/// `onSpecialPowerCreation()` ("called by a create module to start our countdown").
pub fn on_special_power_creation<M: SpecialPowerBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
) -> Result<(), IniError> {
    base_start_power_recharge(m, world, me)?;
    let t = template_id(world, m.power()).ok_or_else(null_template)?;
    let tm = timing(world, t).ok_or_else(null_template)?;
    if tm.is_shared_n_sync {
        if let Some(player) = world.get_controlling_player(me.object) {
            let now = world.frame();
            if let Some(p) = world.players.get_nth_mut(player) {
                special_power_timers_express(&mut p.special_power_timers, t, now);
            }
            let frame = shared_ready_frame(world, me, Some(t));
            m.power_mut().set_available_on_frame(frame);
        }
    }
    if m.power().special_power_module_data().starts_paused && !crate::logic::modules::plant(848) {
        pause_countdown(m, world, me, true)?;
    }
    add_superweapon_if_public(world, me, t);
    Ok(())
}

/// `if (template->hasPublicTimer() && controlling player && structure) TheInGameUI->addSuperweapon(...)`.
fn add_superweapon_if_public(world: &mut World, me: ModuleHandle, template: u32) {
    let Some(t) = world
        .statics
        .special_power_store
        .find_special_power_template_by_id(template)
    else {
        return;
    };
    let (public, name) = (t.has_public_timer(), t.name().to_vec());
    let structure = world
        .objects
        .find(me.object)
        .is_some_and(|o| o.is_kind_of(KINDOF_STRUCTURE));
    if public && (structure || crate::logic::modules::plant(847)) {
        if let Some(player) = world.get_controlling_player(me.object) {
            world.emit(ObjectClientEvent::UiAddSuperweapon {
                player_index: player,
                power_name: name,
                object_id: me.object,
                template,
            });
        }
    }
}

/// The world half of `SpecialPowerModule::SpecialPowerModule`.
pub fn construct<M: SpecialPowerBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
) -> Result<(), IniError> {
    let under_construction = world
        .objects
        .find(me.object)
        .is_some_and(|o| o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION));
    let t = template_id(world, m.power()).ok_or_else(null_template)?;
    let tm = timing(world, t).ok_or_else(null_template)?;
    if (!under_construction || crate::logic::modules::plant(849)) && !tm.is_shared_n_sync {
        base_start_power_recharge(m, world, me)?;
    }
    if m.power().special_power_module_data().starts_paused {
        pause_countdown(m, world, me, true)?;
    }
    // (`resolveSpecialPower()` is empty)
    if m.power().paused_count() == 0 && tm.is_shared_n_sync {
        add_superweapon_if_public(world, me, t);
    }
    Ok(())
}

/// The world half of `SpecialPowerModule::loadPostProcess`: the superweapon is registered with the UI again.
pub fn load_post_process_world<M: SpecialPowerBehavior>(
    m: &M,
    world: &mut World,
    me: ModuleHandle,
) -> Result<(), IniError> {
    let t = template_id(world, m.power()).ok_or_else(null_template)?;
    let tm = timing(world, t).ok_or_else(null_template)?;
    if m.power().paused_count() == 0 && tm.is_shared_n_sync {
        add_superweapon_if_public(world, me, t);
    }
    Ok(())
}

// ---- doing the power ----------------------------------------------------------------------------------------------------------------------------

/// `initiateIntentToDoSpecialPower(targetObj, targetPos, way, commandOptions)`: asks the object's special power update modules; true if one took it.
fn initiate_intent<M: SpecialPowerBehavior>(
    m: &M,
    world: &mut World,
    me: ModuleHandle,
    target_object: ObjectId,
    target_position: Option<Coord3D>,
    waypoint: u32,
    command_options: u32,
) -> Result<bool, IniError> {
    let t = template_id(world, m.power());
    let mut valid = false;
    for idx in 0..world
        .objects
        .find(me.object)
        .map_or(0, |o| o.modules.len() as u16)
    {
        // `isModuleForPower(m_specialPowerTemplate)` is asked of THIS module for every update module (upstream's loop) with its own template, so
        // it is true (even for a null template: `NULL == NULL`)
        if !world
            .module_call(
                me.object,
                idx,
                ModuleCall::DoesSpecialPowerUpdatePassScienceTest,
            )
            .as_bool()
        {
            continue;
        }
        let r = world.module_call(
            me.object,
            idx,
            ModuleCall::InitiateIntentToDoSpecialPower {
                template: t.unwrap_or(0),
                target_object,
                target_position,
                waypoint,
                command_options,
            },
        );
        if r.as_bool() {
            valid = true;
            break;
        }
    }
    let player = world
        .get_controlling_player(me.object)
        .ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
    let academy_template = t.ok_or_else(null_template)?;
    if !crate::logic::modules::plant(841) {
        world.cut(CutCall::PlayerAcademyRecordSpecialPowerUsed {
            player,
            template: academy_template,
        });
    }
    Ok(valid)
}

/// `triggerSpecialPower(location)`.
fn trigger_special_power<M: SpecialPowerBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    location: Option<Coord3D>,
) -> Result<(), IniError> {
    about_to_do_special_power(m, world, me, location)?; // do BEFORE recharge
    create_view_object(m, world, me, location)?;
    base_start_power_recharge(m, world, me)
}

/// `markSpecialPowerTriggered(location)` of the base.
pub fn base_mark_special_power_triggered<M: SpecialPowerBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    location: Option<Coord3D>,
) -> Result<(), IniError> {
    trigger_special_power(m, world, me, location)
}

/// The `EvaSuperweapon` a power type announces, if it does.
fn eva_weapon(t: SpecialPowerType) -> Option<EvaSuperweapon> {
    match t {
        SPECIAL_PARTICLE_UPLINK_CANNON
        | SUPW_SPECIAL_PARTICLE_UPLINK_CANNON
        | LAZR_SPECIAL_PARTICLE_UPLINK_CANNON => Some(EvaSuperweapon::ParticleCannon),
        SPECIAL_NEUTRON_MISSILE | NUKE_SPECIAL_NEUTRON_MISSILE | SUPW_SPECIAL_NEUTRON_MISSILE => {
            Some(EvaSuperweapon::Nuke)
        }
        SPECIAL_SCUD_STORM => Some(EvaSuperweapon::ScudStorm),
        SPECIAL_GPS_SCRAMBLER | SLTH_SPECIAL_GPS_SCRAMBLER => Some(EvaSuperweapon::GpsScrambler),
        SPECIAL_SNEAK_ATTACK => Some(EvaSuperweapon::SneakAttack),
        _ => None,
    }
}

/// `aboutToDoSpecialPower(location)`: tells the script engine, the Eva announcer and the audio.
fn about_to_do_special_power<M: SpecialPowerBehavior>(
    m: &M,
    world: &mut World,
    me: ModuleHandle,
    location: Option<Coord3D>,
) -> Result<(), IniError> {
    let tid = template_id(world, m.power()).ok_or_else(null_template)?;
    let (name, power_type, initiate, at_target) = {
        let t = world
            .statics
            .special_power_store
            .find_special_power_template_by_id(tid)
            .ok_or_else(null_template)?;
        (
            t.name().to_vec(),
            t.special_power_type(),
            t.initiate_sound().event_name.clone(),
            t.initiate_at_target_sound().event_name.clone(),
        )
    };
    let player = world
        .get_controlling_player(me.object)
        .ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
    world.cut(CutCall::ScriptNotifyOfTriggeredSpecialPower {
        player,
        name,
        source: me.object,
    });
    if let Some(weapon) = eva_weapon(power_type) {
        let local = world.players.local_index();
        let whose = if local == Some(player) {
            EvaWhose::Own
        } else {
            // `localPlayer->getRelationship(getObject()->getTeam())`: a null local player is dereferenced
            let local = local.ok_or_else(|| IniError::undefined(Undefined::NullLocalPlayer))?;
            let team = world.get_team(me.object);
            let ally = if crate::logic::modules::plant(843) {
                world.cuts.player_relationship_to_team(local, team)
                    == crate::common::game_common::Relationship::Allies
            } else {
                world.cuts.player_relationship_to_team(local, team)
                    != crate::common::game_common::Relationship::Enemies
            };
            if ally {
                EvaWhose::Ally
            } else {
                EvaWhose::Enemy
            }
        };
        world.emit(ObjectClientEvent::EvaSuperweaponLaunched { weapon, whose });
    }
    world.emit(ObjectClientEvent::AudioAddEvent {
        name: initiate,
        object_id: me.object,
        position: None,
        player_index: None,
    });
    if let Some(loc) = location.filter(|_| !crate::logic::modules::plant(846)) {
        world.emit(ObjectClientEvent::AudioAddEvent {
            name: at_target,
            object_id: INVALID_ID,
            position: Some(loc),
            player_index: Some(player),
        });
    }
    Ok(())
}

/// `createViewObject(location)`: "an object that looks at the target for a while".
fn create_view_object<M: SpecialPowerBehavior>(
    m: &M,
    world: &mut World,
    me: ModuleHandle,
    location: Option<Coord3D>,
) -> Result<(), IniError> {
    let Some(tid) = template_id(world, m.power()) else {
        return Ok(());
    };
    let Some(t) = world
        .statics
        .special_power_store
        .find_special_power_template_by_id(tid)
    else {
        return Ok(());
    };
    let (range, duration) = (t.view_object_range(), t.view_object_duration());
    if range == 0.0 || duration == 0 {
        return Ok(()); // we don't want a view object at all
    }
    let object_name = world.cuts.special_power_view_object_name.clone();
    if object_name.is_empty() {
        return Ok(());
    }
    let Some(template) = world.thing_factory_find_template(&object_name) else {
        return Ok(());
    };
    let player = world
        .get_controlling_player(me.object)
        .ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
    let team = world.players.get_nth(player).and_then(|p| p.default_team);
    let Ok(view) = world.thing_factory_new_object(template, team, ObjectStatusMaskType::default())
    else {
        return Ok(());
    };
    let loc = location.ok_or_else(|| IniError::undefined(Undefined::NullLocation))?;
    world.set_position(view, &loc);
    if !crate::logic::modules::plant(844) {
        world.set_shroud_clearing_range(view, range);
    }
    if let Some(idx) = world.find_module(view, b"DeletionUpdate") {
        world.module_call(
            view,
            idx,
            ModuleCall::SetLifetimeRange {
                min_frames: duration,
                max_frames: duration,
            },
        );
    }
    Ok(())
}

/// `SpecialPowerModule::doSpecialPower(commandOptions)`.
pub fn base_do_special_power<M: SpecialPowerBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    command_options: u32,
) -> Result<(), IniError> {
    let ctx = context(world, me, m.power())?;
    if m.power().do_special_power_is_blocked(&ctx) {
        return Ok(());
    }
    initiate_intent(m, world, me, INVALID_ID, None, 0, command_options)?;
    if m.power().special_power_module_data().triggers_immediately() {
        trigger_special_power(m, world, me, None)?; // location-less trigger
    }
    Ok(())
}

/// `SpecialPowerModule::doSpecialPowerAtObject(obj, commandOptions)`.
pub fn base_do_special_power_at_object<M: SpecialPowerBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    target: ObjectId,
    command_options: u32,
) -> Result<(), IniError> {
    let ctx = context(world, me, m.power())?;
    if m.power().do_special_power_is_blocked(&ctx) {
        return Ok(());
    }
    initiate_intent(m, world, me, target, None, 0, command_options)?;
    if m.power().special_power_module_data().triggers_immediately() {
        // `obj->getPosition()`: a null object is dereferenced
        let mut pos = world
            .objects
            .find(target)
            .map(|o| o.get_position())
            .ok_or_else(|| IniError::undefined(Undefined::NullLocation))?;
        if crate::logic::modules::plant(852) {
            pos = world
                .objects
                .find(me.object)
                .map_or(pos, |o| o.get_position());
        }
        trigger_special_power(m, world, me, Some(pos))?;
    }
    Ok(())
}

/// `SpecialPowerModule::doSpecialPowerAtLocation(loc, angle, commandOptions)`.
pub fn base_do_special_power_at_location<M: SpecialPowerBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    loc: Coord3D,
    _angle: f32,
    command_options: u32,
) -> Result<(), IniError> {
    let ctx = context(world, me, m.power())?;
    if m.power().do_special_power_is_blocked(&ctx) {
        return Ok(());
    }
    initiate_intent(m, world, me, INVALID_ID, Some(loc), 0, command_options)?;
    if m.power().special_power_module_data().triggers_immediately() {
        trigger_special_power(m, world, me, Some(loc))?;
    }
    Ok(())
}

/// `SpecialPowerModule::doSpecialPowerUsingWaypoints(way, commandOptions)`.
pub fn base_do_special_power_using_waypoints<M: SpecialPowerBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    waypoint: u32,
    command_options: u32,
) -> Result<(), IniError> {
    let ctx = context(world, me, m.power())?;
    if m.power().do_special_power_is_blocked(&ctx) {
        return Ok(());
    }
    initiate_intent(m, world, me, INVALID_ID, None, waypoint, command_options)?;
    if m.power().special_power_module_data().triggers_immediately() {
        trigger_special_power(m, world, me, None)?; // this type doesn't create view objects
    }
    Ok(())
}

// ---- the answer to Object's calls ------------------------------------------------------------------------------------------------------------

/// The answer of a special power module to one of `Object`'s calls. An error a function returns is upstream's undefined behaviour: it is recorded in
/// `world.cuts.module_errors` and the call is answered as if it did nothing.
pub fn special_power_module_call<M: SpecialPowerBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    call: ModuleCall,
) -> ModuleReply {
    fn record<T>(world: &mut World, me: ModuleHandle, r: Result<T, IniError>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                world.cuts.module_errors.push((me, e));
                None
            }
        }
    }
    match call {
        // (the data's own template may be a name that is looked up now: compare ids)
        ModuleCall::IsModuleForPower { template } => {
            ModuleReply::Bool(template_id(world, m.power()) == Some(template).filter(|t| *t != 0))
        }
        ModuleCall::GetSpecialPowerTemplateId => template_id(world, m.power())
            .map_or(ModuleReply::Unhandled, |id| ModuleReply::Int(id as i32)),
        ModuleCall::GetSpecialPowerType => {
            let t = template_id(world, m.power()).and_then(|id| {
                world
                    .statics
                    .special_power_store
                    .find_special_power_template_by_id(id)
                    .map(|t| t.special_power_type())
            });
            t.map_or(ModuleReply::Unhandled, ModuleReply::Int)
        }
        ModuleCall::IsShortcutPower => {
            let t = template_id(world, m.power()).and_then(|id| {
                world
                    .statics
                    .special_power_store
                    .find_special_power_template_by_id(id)
                    .map(|t| t.is_shortcut_power())
            });
            ModuleReply::Bool(t.unwrap_or(false))
        }
        ModuleCall::IsReady => {
            let r = is_ready(m, world, me);
            ModuleReply::Bool(record(world, me, r).unwrap_or(false))
        }
        ModuleCall::GetPercentReady => {
            let r = get_percent_ready(m, world, me);
            ModuleReply::Real(record(world, me, r).unwrap_or(0.0))
        }
        ModuleCall::GetReadyFrame => {
            let r = get_ready_frame(m, world, me);
            ModuleReply::Int(record(world, me, r).unwrap_or(0) as i32)
        }
        ModuleCall::GetRequiredScience => {
            let s = template_id(world, m.power()).and_then(|id| {
                world
                    .statics
                    .special_power_store
                    .find_special_power_template_by_id(id)
                    .map(|t| t.required_science().0)
            });
            s.map_or(ModuleReply::Unhandled, ModuleReply::Int)
        }
        ModuleCall::IsScriptOnly => ModuleReply::Bool(m.power().is_script_only()),
        ModuleCall::SetReadyFrame { frame } => {
            set_ready_frame(m, world, frame);
            ModuleReply::Done
        }
        ModuleCall::PauseCountdown { disabling } => {
            let r = pause_countdown(m, world, me, disabling);
            record(world, me, r);
            ModuleReply::Done
        }
        ModuleCall::OnSpecialPowerCreation => {
            let r = on_special_power_creation(m, world, me);
            record(world, me, r);
            ModuleReply::Done
        }
        ModuleCall::StartPowerRecharge => {
            let r = base_start_power_recharge(m, world, me);
            record(world, me, r);
            ModuleReply::Done
        }
        ModuleCall::LoadPostProcess => {
            let r = load_post_process_world(m, world, me);
            record(world, me, r);
            ModuleReply::Done
        }
        ModuleCall::MarkSpecialPowerTriggered { location } => {
            let r = lend_clone(world, me, m, M::into_body, M::from_body, |w, m| {
                base_mark_special_power_triggered(m, w, me, location)
            });
            record(world, me, r);
            ModuleReply::Done
        }
        ModuleCall::DoSpecialPower { command_options } => {
            let r = lend_clone(world, me, m, M::into_body, M::from_body, |w, m| {
                m.do_special_power(w, me, command_options)
            });
            record(world, me, r);
            ModuleReply::Done
        }
        ModuleCall::DoSpecialPowerAtObject {
            target,
            command_options,
        } => {
            let r = lend_clone(world, me, m, M::into_body, M::from_body, |w, m| {
                m.do_special_power_at_object(w, me, target, command_options)
            });
            record(world, me, r);
            ModuleReply::Done
        }
        ModuleCall::DoSpecialPowerAtLocation {
            loc,
            angle,
            command_options,
        } => {
            let r = lend_clone(world, me, m, M::into_body, M::from_body, |w, m| {
                m.do_special_power_at_location(w, me, loc, angle, command_options)
            });
            record(world, me, r);
            ModuleReply::Done
        }
        ModuleCall::DoSpecialPowerUsingWaypoints {
            waypoint,
            command_options,
        } => {
            let r = lend_clone(world, me, m, M::into_body, M::from_body, |w, m| {
                m.do_special_power_using_waypoints(w, me, waypoint, command_options)
            });
            record(world, me, r);
            ModuleReply::Done
        }
        other => m.other_call(world, me, other),
    }
}

/// `getRequiredScience()`: `SpecialPowerTemplate::getRequiredScience()` of the module's template (`None`: a `NULL` template, batch R21).
pub fn template_required_science(
    world: &World,
    power: &SpecialPowerModule,
) -> Option<crate::common::science::ScienceType> {
    let id = template_id(world, power)?;
    world
        .statics
        .special_power_store
        .find_special_power_template_by_id(id)
        .map(|t| t.required_science())
}

/// `SpecialPowerTemplate::getDetectionTime()` of the module's template.
pub fn template_detection_time(world: &World, power: &SpecialPowerModule) -> Option<u32> {
    let id = template_id(world, power)?;
    world
        .statics
        .special_power_store
        .find_special_power_template_by_id(id)
        .map(|t| t.detection_time())
}

/// `SpecialPowerModuleData::buildFieldParse`'s own table, for a derived data class `$t` whose `SpecialPowerModuleData` is the field `$base`. A module's
/// data proc has no `TheSpecialPowerStore`, so the template field keeps its name (see the module docs).
#[macro_export]
macro_rules! special_power_data_field_parse {
    ($t:ty, $base:ident) => {
        [
            $crate::common::ini::FieldParse::<$t>::new(
                "SpecialPowerTemplate",
                |i, d, _| {
                    d.$base.special_power_template_name = Some(i.get_next_token(None)?);
                    Ok(())
                },
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::<$t>::new(
                "UpdateModuleStartsAttack",
                |i, d, _| {
                    d.$base.update_module_starts_attack = i.parse_bool()?;
                    Ok(())
                },
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::<$t>::new(
                "StartsPaused",
                |i, d, _| {
                    d.$base.starts_paused = i.parse_bool()?;
                    Ok(())
                },
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::<$t>::new(
                "InitiateSound",
                |i, d, _| d.$base.initiate_sound.parse(i),
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::<$t>::new(
                "ScriptedSpecialPowerOnly",
                |i, d, _| {
                    d.$base.scripted_special_power_only = i.parse_bool()?;
                    Ok(())
                },
                $crate::common::ini::UserData::None,
            ),
            $crate::common::ini::FieldParse::<$t>::end(),
        ]
    };
}

// --- p1w11-sonnet ---
#[cfg(test)]
mod tests {
    //! The view object's `DeletionUpdate` branch of `createViewObject`, which the differential test does not reach (its `RO_A` has no
    //! modules): the object made from the global view object name gets its lifetime set to the power's view object duration.
    use super::*;
    use crate::common::damage_fx::DamageFxStore;
    use crate::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
    use crate::common::module_factory::ModuleFactory;
    use crate::common::name_key_generator::NameKeyGenerator;
    use crate::common::science::{GameTextFetch, ScienceStore};
    use crate::common::special_power::SpecialPowerStore;
    use crate::common::thing_factory::ThingFactory;
    use crate::common::thing_template::{ObjectTemplateValues, ThingParseEnv, ThingTemplate};
    use crate::common::unicode_string::UnicodeString;
    use crate::logic::armor::ArmorStore;
    use crate::logic::module::special_power_module::SpecialPowerModuleData;
    use crate::logic::modules::special_power_module_impl::SpecialPowerModuleImpl;
    use crate::logic::weapon_set::WeaponTemplateTable;
    use crate::logic::world::object_create::{NewModule, NewObject};
    use crate::logic::world::statics::GameStatics;
    use crate::logic::world::team::Team;
    use std::sync::Arc;

    struct Text;
    impl GameTextFetch for Text {
        fn fetch(&self, _label: &[u8]) -> UnicodeString {
            UnicodeString::new()
        }
    }
    struct Ctx {
        factory: ThingFactory,
        names: NameKeyGenerator,
        modules: ModuleFactory,
        armors: ArmorStore,
        dfx: DamageFxStore,
        weapons: WeaponTemplateTable,
        sciences: ScienceStore,
        store: SpecialPowerStore,
    }
    fn object_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        let mut env = ThingParseEnv {
            names: &mut c.names,
            modules: &mut c.modules,
            armors: &c.armors,
            damage_fx: &c.dfx,
            weapons: &c.weapons,
            sciences: &c.sciences,
            text: &Text,
        };
        c.factory.parse_object_block(ini, &mut env, 0)
    }
    fn power_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.store
            .parse_special_power_definition(ini, &mut c.names, &|_| true)
    }

    #[test]
    fn the_view_object_gets_the_view_duration_as_its_lifetime() {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut modules = ModuleFactory::new();
        modules.init(&mut names).unwrap();
        let mut c = Ctx {
            factory: ThingFactory::new(),
            names,
            modules,
            armors: ArmorStore::new(),
            dfx: DamageFxStore::new(),
            weapons: WeaponTemplateTable::new(),
            sciences: ScienceStore::new(),
            store: SpecialPowerStore::new(),
        };
        static BLOCKS: [BlockParse<Ctx>; 2] = [
            BlockParse {
                token: "Object",
                parse: object_block,
            },
            BlockParse {
                token: "SpecialPower",
                parse: power_block,
            },
        ];
        let text = "Object VIEW\n Behavior = DeletionUpdate ModuleTag_Del\n  MinLifetime = 99000\n  MaxLifetime = 99000\n End\nEnd\n\
                    SpecialPower SP_V\n ReloadTime = 1000\n ViewObjectDuration = 1000\n ViewObjectRange = 50\nEnd\n";
        Ini::load(
            b"Data\\INI\\Test.ini",
            text.as_bytes(),
            IniLoadType::Overwrite,
            None,
            &BLOCKS,
            &mut c,
        )
        .expect("load");

        let mut w = World::new();
        w.names = c.names;
        w.statics = Arc::new(GameStatics {
            special_power_store: c.store,
            thing_factory: c.factory,
            ..Default::default()
        });
        w.cuts.special_power_view_object_name = b"VIEW".to_vec();
        for team in [100u32, 101] {
            w.teams.insert_team(Team::new(team, 0)).unwrap();
        }
        w.players.get_nth_mut(0).unwrap().default_team = Some(101);
        w.cuts.team_players.insert(100, 0);
        w.set_frame(77);
        let mut data = SpecialPowerModuleData::new();
        data.special_power_template_name = Some(b"SP_V".to_vec());
        let mut nm = NewModule::plain(b"SpecialPowerModule", b"ModuleTag_Power");
        nm.interfaces = SpecialPowerModule::get_interface_mask();
        nm.body = Some(UpdateBody::SpecialPowerModule(
            SpecialPowerModuleImpl::new(w.next_object_id(), Some(Arc::new(data))).unwrap(),
        ));
        let template = Arc::new(ThingTemplate::from_object_values(
            &ObjectTemplateValues::default(),
        ));
        let me = w
            .new_object(NewObject {
                template,
                status: ObjectStatusMaskType::default(),
                team: Some(100),
                modules: vec![nm],
            })
            .unwrap();
        let idx = w.find_module(me, b"SpecialPowerModule").unwrap();
        let loc = Coord3D {
            x: 5.0,
            y: 6.0,
            z: 7.0,
        };
        w.module_call(
            me,
            idx,
            ModuleCall::MarkSpecialPowerTriggered {
                location: Some(loc),
            },
        );
        assert!(
            w.cuts.module_errors.is_empty(),
            "{:?}",
            w.cuts.module_errors
        );

        // the object the power made: the next id, at the location, clearing the shroud for the view range, to die in 1000 ms (30 frames) from now
        let view = me + 1;
        let o = w.objects.find(view).expect("the view object was made");
        assert_eq!(o.get_position(), loc);
        assert_eq!(o.shroud_clearing_range, 50.0);
        assert_eq!(w.get_team(view), Some(101));
        let die = o.behaviors.iter().find_map(|b| match b.body.as_ref() {
            Some(UpdateBody::DeletionUpdate(m)) => Some(m.get_die_frame()),
            _ => None,
        });
        assert_eq!(
            die,
            Some(77 + 30),
            "the lifetime is the view object duration, not the 99000 ms of the template"
        );
    }
}
// --- end p1w11-sonnet ---
