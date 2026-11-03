// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp (`AIUpdateInterface` as one of its object's
//   modules: the constructor, `onObjectCreated`, `update`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I9b-3: `AIUpdateInterface` as a module of the world's objects ([`AiModuleBody`], the update body
//! `UpdateBody::AiUpdate`).
//!
//! **Live with `cuts.live_ai`** (off by default: an AI module stays the untranslated module stub, so every test's objects
//! behave as before). With it, the thing factory makes the module for every AI class (`ModuleDataKind::AiUpdate` data:
//! ~~the base class `AIUpdateInterface`; the subclasses are still stubs~~ **since batch I9b-5** `AIUpdateInterface` and its
//! subclasses, made by [`AiModuleBody::world_construct`] from their data), the world's `world_construct` runs its
//! constructor with the object as [`World::ai_self`], the object's `onObjectCreated` makes its machine, and the scheduler's
//! update runs the machine. The world is the AI's host throughout (batches I9b-1 and I9b-2).
//!
//! **The snapshot** (`xfer`, `loadPostProcess`) needs its host: the world's slot snapshot (`xfer_module_slot`, an
//! object's save and load) runs it with the world as the host ([`World::xfer_ai_slot`]); the update body's own snapshot
//! calls, which have no world, answer `XferStatus::InvalidParameters`. `Object::crc` does not include modules, so the
//! logic's CRC is unaffected. ~~The other `ModuleCall`s an object makes of its AI answer `Unhandled` (they are the AI cut
//! tables until they are routed here; routed so far: `getCurrentVictim`, `isDoingGroundMovement`,
//! `getLastCommandSource`)~~ **Since batch I9b-4** the object's calls to its AI are routed here (see
//! [`AiModuleBody::module_call`] for the few that are not; there were no AI cut tables, the callers took their defaults),
//! ~~and the turrets' AIs (`TurretHost`) are not run~~ **since batch I9b-6** the turrets' AIs are made with the AI and run
//! with it (`world::turret_host_world`).
//!
//! **Open:** upstream's `~AIUpdateInterface` halts its machine (the current state exits) before it deletes the turrets; the
//! World deletes the turrets (batch I9b-6) but does not halt the AI's machine when the object is deleted.
//!
//! `p1w13-sonnet`, integration batches I9b-3 to I9b-5, 2026-11-01.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::logic::ai::ai_update_types::CMD_FROM_AI;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::state_machine::StateMachine;
use crate::logic::ai::ai_commands::{self as commands, *};
use crate::logic::ai::ai_states::AiState;
use crate::logic::ai::ai_update::{AiSubclass, AiUpdate};
use crate::logic::ai::chinook_ai_update::ai_free_to_exit as chinook_ai_free_to_exit;
use crate::logic::locomotor::Locomotor;
use crate::logic::modules::transport_contain::FREE_TO_EXIT;
use crate::logic::weapon::NO_MAX_SHOTS_LIMIT;

/// `AIFreeToExitType`'s `NOT_FREE_TO_EXIT`.
const NOT_FREE_TO_EXIT: i32 = 1;
use crate::common::state_machine::StateMachineError;
use crate::common::xfer::{Xfer, XferResult, XferStatus};
use crate::logic::ai::ai_states::AiUpdateModule;
use crate::logic::ai::ai_update_data::AiUpdateModuleData;
use crate::logic::module::update_module::{UpdateModule, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;

/// `AIUpdateInterface` in its object's module list: its data and, once the world has constructed it, the AI's state and
/// machines.
pub struct AiModuleBody {
    /// The module's class name (`AIUpdateInterface`, `JetAIUpdate`, ...; batch I9b-5).
    pub name: Vec<u8>,
    pub data: Arc<AiUpdateModuleData>,
    /// None until `world_construct`.
    pub module: Option<AiUpdateModule<World>>,
}

impl Clone for AiModuleBody {
    /// A copy of the module's data, unconstructed: the AI's machines cannot be copied (their states hold no `Clone`), and
    /// the world copies no running AI (see the module docs).
    fn clone(&self) -> Self {
        AiModuleBody { name: self.name.clone(), data: self.data.clone(), module: None }
    }
}

impl std::fmt::Debug for AiModuleBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AiModuleBody {{ {}, constructed: {} }}", String::from_utf8_lossy(&self.name), self.module.is_some())
    }
}

impl AiModuleBody {
    /// The module of class `name` as the factory makes it (the constructor runs in `world_construct`).
    pub fn new(name: &[u8], data: Arc<AiUpdateModuleData>) -> Self {
        AiModuleBody { name: name.to_vec(), data, module: None }
    }

    /// The class's constructor (`AIUpdateInterface(thing, moduleData)`, or the subclass's), with the object as the AI's host's
    /// `this`. A subclass's constructor that fails (its machines' default states, upstream's undefined behaviour) is recorded
    /// in `team_live_errors` and leaves the module unconstructed.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        let data = self.data.clone();
        let subclass = make_subclass(world, me.object, &self.name, &data);
        let made = world.with_ai_self(me.object, |w| match subclass {
            AiSubclass::Plain => Ok(AiUpdateModule::new(w, data)),
            sub => AiUpdateModule::new_subclass(w, data, sub),
        });
        match made {
            Ok(m) => self.module = Some(m),
            Err(e) => world.cuts.team_live_errors.push(format!("{}: constructor: {e:?}", String::from_utf8_lossy(&self.name))),
        }
        // (batch I9b-6: `m_turretAI[i] = newInstance(TurretAI)(...)` for each turret of the data. Upstream makes them before
        // `chooseLocomotorSet(LOCOMOTORSET_NORMAL)` and `setWakeFrame`, which `AiUpdate::new` has run already; neither draws a
        // random value nor reads a turret, so the order is the same to the logic.)
        if self.module.is_some() {
            world.make_turrets(me.object, &self.data);
        }
    }

    /// `update()`: the AI's machine, run with the object as `this`. An error of the machine (upstream's undefined
    /// behaviour) is recorded in `team_live_errors` and the module runs again next frame.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        let Some(m) = self.module.as_mut() else { return UPDATE_SLEEP_FOREVER };
        match world.with_ai_self(me.object, |w| m.update(w)) {
            Ok(sleep) => sleep,
            Err(e) => {
                world.cuts.team_live_errors.push(format!("AIUpdateInterface::update: {e:?}"));
                UPDATE_SLEEP_NONE
            }
        }
    }

    /// The object's calls: `onObjectCreated` makes the machine; ~~the others are not routed here yet~~ (batch I9b-4) the commands
    /// go through `aiDoCommand`, and the AI's and its machine's getters and setters answer here. Not routed (they answer
    /// `Unhandled`): the ones listed at the end of the match.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let Some(m) = self.module.as_mut() else { return ModuleReply::Unhandled };
        // the commands: `AICommandInterface`'s inline builders of `AICommandParms`, then `aiDoCommand` (AI.h)
        if let Some(p) = ai_command_parms(&call) {
            return command(m, world, me, p);
        }
        // (batch I9i: `getSupplyTruckAIInterface()`'s calls, with the AI lent)
        if is_supply_call(&call) {
            return world.with_ai_self(me.object, |w| m.run(w, |_, h| supply_call(h, &call)));
        }
        match call {
            ModuleCall::OnObjectCreated => {
                if let Err(e) = world.with_ai_self(me.object, |w| m.on_object_created(w)) {
                    world.cuts.team_live_errors.push(format!("AIUpdateInterface::onObjectCreated: {e:?}"));
                }
                ModuleReply::Done
            }
            ModuleCall::AiJoinTeam => {
                if let Err(e) = world.with_ai_self(me.object, |w| m.run(w, crate::logic::ai::ai_commands::join_team)) {
                    world.cuts.team_live_errors.push(format!("AIUpdateInterface::joinTeam: {e:?}"));
                }
                ModuleReply::Done
            }
            // (batch I9e: `construct(what, pos, angle, owner, isRebuild)`: a dozer's or worker's; the base's makes nothing)
            // (batch I16: another object's AI's state type, chase flag and desired speed, for the members of a transport, a spawn or a group)
            ModuleCall::AiGetAiStateType => match m.machine.as_ref() {
                Some(sm) => ModuleReply::Int(sm.get_current_state_id() as i32),
                None => ModuleReply::Unhandled,
            },
            ModuleCall::AiSetAllowedToChase(allowed) => {
                m.ai_mut().allowed_to_chase = allowed;
                ModuleReply::Done
            }
            ModuleCall::AiSetDesiredSpeed(speed) => {
                m.ai_mut().desired_speed = speed;
                ModuleReply::Done
            }
            // (batch I20: a script's attack priority list, waypoint queue clear and reinforcement delivery)
            ModuleCall::AiSetAttackInfo(info) => {
                m.ai_mut().attack_info = info;
                ModuleReply::Done
            }
            ModuleCall::AiClearWaypointQueue => {
                m.ai_mut().clear_waypoint_queue();
                ModuleReply::Done
            }
            ModuleCall::AiExecuteWaypointQueue => {
                m.ai_mut().execute_waypoint_queue();
                ModuleReply::Done
            }
            ModuleCall::AiDeliverPayloadViaModuleData(pos) => {
                if !matches!(m.ai.as_ref().map(|a| &a.subclass), Some(crate::logic::ai::ai_update::AiSubclass::DeliverPayload(_))) {
                    return ModuleReply::Unhandled;
                }
                let r = world.with_ai_self(me.object, |w| m.run(w, |sm, h| crate::logic::ai::deliver_payload_ai_update::deliver_payload_via_module_data(sm, h, &pos)));
                if let Err(e) = r {
                    world.cuts.team_live_errors.push(format!("DeliverPayloadAIUpdate::deliverPayloadViaModuleData: {e:?}"));
                }
                ModuleReply::Done
            }
            // (batch I14: `MissileAIUpdate` is a `ProjectileUpdateInterface`; none of its calls reached it, so a missile was never launched.
            // The exhaust override is a client particle system: `None`, its template's own)
            ModuleCall::HasProjectileUpdateInterface => ModuleReply::Bool(m.ai.as_ref().is_some_and(|a| matches!(a.subclass, crate::logic::ai::ai_update::AiSubclass::Missile(_)))),
            ModuleCall::ProjectileGetLauncherId => match m.ai.as_ref().map(|a| &a.subclass) {
                Some(crate::logic::ai::ai_update::AiSubclass::Missile(ms)) => ModuleReply::Id(ms.launcher_id),
                _ => ModuleReply::Unhandled,
            },
            ModuleCall::ProjectileLaunchAtObjectOrPosition { victim, destination, source, wslot, barrel, template, .. } if is_missile(m) => {
                use crate::logic::ai::missile_ai_update::projectile_launch_at_object_or_position as launch;
                let r = world.with_ai_self(me.object, |w| m.run(w, |sm, h| launch(sm, h, victim, &destination, source, wslot, barrel, Some(template as u32), None)));
                if let Err(e) = r {
                    world.cuts.team_live_errors.push(format!("MissileAIUpdate::projectileLaunchAtObjectOrPosition: {e:?}"));
                }
                ModuleReply::Done
            }
            ModuleCall::ProjectileFireAtObjectOrPosition { victim, pos, template, .. } if is_missile(m) => {
                use crate::logic::ai::missile_ai_update::projectile_fire_at_object_or_position as fire;
                let r = world.with_ai_self(me.object, |w| m.run(w, |sm, h| fire(sm, h, victim, &pos, Some(template as u32), None)));
                if let Err(e) = r {
                    world.cuts.team_live_errors.push(format!("MissileAIUpdate::projectileFireAtObjectOrPosition: {e:?}"));
                }
                ModuleReply::Done
            }
            ModuleCall::ProjectileHandleCollision { other } if is_missile(m) => {
                use crate::logic::ai::missile_ai_update::projectile_handle_collision as collide;
                match world.with_ai_self(me.object, |w| m.run(w, |sm, h| collide(sm, h, other))) {
                    Ok(b) => ModuleReply::Bool(b),
                    Err(e) => {
                        world.cuts.team_live_errors.push(format!("MissileAIUpdate::projectileHandleCollision: {e:?}"));
                        ModuleReply::Bool(true)
                    }
                }
            }
            ModuleCall::ProjectileNowJammed if is_missile(m) => {
                if let Err(e) = world.with_ai_self(me.object, |w| m.run(w, crate::logic::ai::missile_ai_update::projectile_now_jammed)) {
                    world.cuts.team_live_errors.push(format!("MissileAIUpdate::projectileNowJammed: {e:?}"));
                }
                ModuleReply::Done
            }
            ModuleCall::ProjectileSetFramesTillCountermeasureDiversion(frames) if is_missile(m) => {
                world.with_ai_self(me.object, |w| m.run(w, |_, h| crate::logic::ai::missile_ai_update::set_frames_till_countermeasure_diversion_occurs(h, frames)));
                ModuleReply::Done
            }
            // (batch I11: the `DeliverPayload` OCL nugget's calls)
            ModuleCall::AiDeliverPayload { move_to, target, data } => {
                if !matches!(m.ai.as_ref().map(|a| &a.subclass), Some(crate::logic::ai::ai_update::AiSubclass::DeliverPayload(_))) {
                    return ModuleReply::Unhandled;
                }
                let r = world.with_ai_self(me.object, |w| m.run(w, |sm, h| crate::logic::ai::deliver_payload_ai_update::deliver_payload(sm, h, &move_to, &target, &data)));
                if let Err(e) = r {
                    world.cuts.team_live_errors.push(format!("DeliverPayloadAIUpdate::deliverPayload: {e:?}"));
                }
                ModuleReply::Done
            }
            ModuleCall::AiGetCurLocomotorMaxSpeedForCondition(state) => {
                if m.ai.as_ref().is_none_or(|a| a.get_cur_locomotor().is_none()) {
                    return ModuleReply::Unhandled;
                }
                ModuleReply::Real(world.with_ai_self(me.object, |w| {
                    m.run(w, |_, h| {
                        use crate::logic::ai::ai_states::AiStateHost;
                        let a = h.ai_stack().pop().expect("a running AI");
                        let speed = a.get_cur_locomotor().map_or(0.0, |l| l.get_max_speed_for_condition(h, state));
                        h.ai_stack().push(a);
                        speed
                    })
                }))
            }
            ModuleCall::AiConstruct { template, pos, angle, owner: _, is_rebuild } => {
                use crate::logic::ai::ai_update::AiSubclass;
                if !matches!(m.ai.as_ref().map(|a| &a.subclass), Some(AiSubclass::Dozer(_) | AiSubclass::Worker(_))) {
                    return ModuleReply::Id(INVALID_ID);
                }
                match world.with_ai_self(me.object, |w| m.run_lent(w, |h| crate::logic::ai::dozer_ai_update::construct(h, template.map(u32::from), &pos, angle, is_rebuild))) {
                    Ok(id) => ModuleReply::Id(id),
                    Err(e) => {
                        world.cuts.team_live_errors.push(format!("DozerAIUpdate::construct: {e:?}"));
                        ModuleReply::Id(INVALID_ID)
                    }
                }
            }
            // (upstream dereferences the `NULL`: undefined behaviour, recorded rather than guessed at)
            ModuleCall::AiAttackPosition { pos: None, .. } => {
                world.cuts.team_live_errors.push("AIUpdateInterface::aiAttackPosition: NULL position".to_string());
                ModuleReply::Done
            }
            ModuleCall::AiDoQuickExit { path } => {
                if let Err(e) = world.with_ai_self(me.object, |w| m.run(w, |sm, h| commands::do_quick_exit(sm, h, &path))) {
                    world.cuts.team_live_errors.push(format!("AIUpdateInterface::doQuickExit: {e:?}"));
                }
                ModuleReply::Done
            }
            // the getters and setters of the AI and its machine
            ModuleCall::AiGetCurrentVictim => {
                let ai = m.ai.take();
                let r = ai.as_ref().map_or(INVALID_ID, |a| world.with_ai_self(me.object, |w| a.get_current_victim(w)));
                m.ai = ai;
                ModuleReply::Id(r)
            }
            ModuleCall::AiGetCurrentVictimPos => {
                let (Some(a), Some(sm)) = (m.ai.as_ref(), m.machine.as_ref()) else { return ModuleReply::Unhandled };
                match world.with_ai_self(me.object, |w| a.get_current_victim_pos(w, sm)) {
                    Some(p) => ModuleReply::Coord(p),
                    None => ModuleReply::Unhandled,
                }
            }
            ModuleCall::AiIsDoingGroundMovement => {
                let ai = m.ai.take();
                let r = ai.as_ref().is_some_and(|a| world.with_ai_self(me.object, |w| a.is_doing_ground_movement(w)));
                m.ai = ai;
                ModuleReply::Bool(r)
            }
            ModuleCall::AiGetLastCommandSource => ModuleReply::Int(m.ai().last_command_source),
            // batch I9c-4: `processCollision(physics, other)`, the AI lent; its commands to itself after
            ModuleCall::AiProcessCollision { other } => {
                let r = lent(m, world, me, |w| w.ai_process_collision(me.object, other));
                for p in std::mem::take(&mut world.collision_self_commands) {
                    command(m, world, me, p);
                }
                ModuleReply::Bool(r)
            }
            // (batch I39: `transferAttack(fromID, toID)`, which the rebuild hole's die and behavior and `CreateObjectDie` send; it
            // reached no arm here, so their attackers kept the dead victim)
            ModuleCall::AiTransferAttack { from, to } => {
                world.with_ai_self(me.object, |w| m.run(w, |sm, h| crate::logic::ai::ai_commands::transfer_attack(sm, h, from, to)));
                ModuleReply::Done
            }
            // (batch I39: `destroyObject`: "We desperately need to stop here, or else the destructor of the statemachine will try to
            // do stopping logic")
            ModuleCall::AiStopForDestroy => {
                world.with_ai_self(me.object, |w| {
                    m.ai_mut().set_locomotor_goal_none(w);
                    m.ai_mut().destroy_path(w);
                });
                ModuleReply::Done
            }
            ModuleCall::AiDoPathfind => {
                world.with_ai_self(me.object, |w| m.do_pathfind(w));
                ModuleReply::Done
            }
            // `getStateMachine()->getGoalObject()`: `findObjectByID` of the goal
            ModuleCall::AiGetGoalObject => {
                let id = m.machine.as_ref().map_or(INVALID_ID, |sm| sm.goal_object_id());
                ModuleReply::Id(if world.objects.find(id).is_some() { id } else { INVALID_ID })
            }
            ModuleCall::AiGetGoalPosition => ModuleReply::Coord(m.machine.as_ref().map(|sm| *sm.goal_position()).unwrap_or_default()),
            ModuleCall::AiIsMoving => machine_bool(m, world, me, |a, sm, w| a.is_moving(sm, w)),
            ModuleCall::AiIsIdle => machine_bool(m, world, me, |a, sm, w| a.is_idle(sm, w)),
            ModuleCall::AiIsBusy => machine_bool(m, world, me, |a, sm, _| a.is_busy(sm)),
            ModuleCall::AiIsAttacking => machine_bool(m, world, me, |a, sm, _| a.is_attacking(sm)),
            ModuleCall::AiIsAiInDeadState => ModuleReply::Bool(m.ai().is_ai_in_dead_state()),
            ModuleCall::AiIsClearingMines => {
                let a = m.ai();
                ModuleReply::Bool(world.with_ai_self(me.object, |w| a.is_clearing_mines(w)))
            }
            ModuleCall::AiGetEnterTarget => {
                let (Some(a), Some(sm)) = (m.ai.as_ref(), m.machine.as_ref()) else { return ModuleReply::Id(INVALID_ID) };
                ModuleReply::Id(world.with_ai_self(me.object, |w| a.get_enter_target(w, sm)))
            }
            ModuleCall::AiGetLocomotorDistanceToGoal => {
                let (Some(a), Some(sm)) = (m.ai.as_mut(), m.machine.as_ref()) else { return ModuleReply::Real(0.0) };
                ModuleReply::Real(world.with_ai_self(me.object, |w| a.get_locomotor_distance_to_goal(w, sm)))
            }
            ModuleCall::AiGetNextMoodTarget { called_by_ai, called_during_idle } => {
                let (Some(a), Some(sm)) = (m.ai.as_mut(), m.machine.as_ref()) else { return ModuleReply::Id(INVALID_ID) };
                ModuleReply::Id(world.with_ai_self(me.object, |w| a.get_next_mood_target(w, sm, called_by_ai, called_during_idle)))
            }
            ModuleCall::AiWakeUpAndAttemptToTarget => {
                if let (Some(a), Some(sm)) = (m.ai.as_mut(), m.machine.as_ref()) {
                    world.with_ai_self(me.object, |w| a.wake_up_and_attempt_to_target(w, sm));
                }
                ModuleReply::Done
            }
            ModuleCall::AiMarkAsDead => {
                let a = m.ai_mut();
                world.with_ai_self(me.object, |w| a.mark_as_dead(w));
                ModuleReply::Done
            }
            ModuleCall::AiEvaluateMoraleBonus => {
                let a = m.ai_mut();
                world.with_ai_self(me.object, |w| a.evaluate_morale_bonus(w));
                ModuleReply::Done
            }
            ModuleCall::AiChooseLocomotorSet(set) => {
                let a = m.ai_mut();
                ModuleReply::Bool(world.with_ai_self(me.object, |w| a.choose_locomotor_set(w, set)))
            }
            ModuleCall::AiSetIgnoreCollisionTime { frames } => {
                let now = world.frame();
                m.ai_mut().set_ignore_collision_time(now, frames);
                ModuleReply::Done
            }
            ModuleCall::AiSetAttitude(tude) => {
                m.ai_mut().set_attitude(tude);
                ModuleReply::Done
            }
            ModuleCall::AiSetIsRecruitable(r) => {
                m.ai_mut().is_recruitable = r;
                ModuleReply::Done
            }
            ModuleCall::AiNotifyCrate(id) => {
                m.ai_mut().notify_crate(id);
                ModuleReply::Done
            }
            ModuleCall::AiIgnoreObstacle { obstacle } => {
                m.ai_mut().ignore_obstacle_id(obstacle);
                ModuleReply::Done
            }
            ModuleCall::AiGetIgnoredObstacleId => ModuleReply::Id(m.ai().get_ignored_obstacle_id()),
            ModuleCall::AiSetPathExtraDistance(d) => {
                m.ai_mut().set_path_extra_distance(d);
                ModuleReply::Done
            }
            ModuleCall::AiGetCurLocomotorSetType => ModuleReply::Int(m.ai().cur_locomotor_set),
            ModuleCall::AiHasLocomotorForSurface(s) => ModuleReply::Bool(m.ai().has_locomotor_for_surface(s as i32)),
            ModuleCall::AiGetPathLastNodePosition => match m.ai().path.as_ref().and_then(|p| p.last_node().map(|i| *p.node(i).position())) {
                Some(p) => ModuleReply::Coord(p),
                None => ModuleReply::Unhandled,
            },
            // `getAiFreeToExit(exiter)`: the base's FREE_TO_EXIT, or the chinook's and the deliverer's overrides
            ModuleCall::AiGetFreeToExit { exiter } => match &m.ai().subclass {
                AiSubclass::Chinook(_) => ModuleReply::Int(world.with_ai_self(me.object, |w| m.run(w, |_, h| chinook_ai_free_to_exit(h, exiter)))),
                AiSubclass::DeliverPayload(d) => {
                    let dead = world.objects.find(me.object).is_none_or(|o| o.is_effectively_dead());
                    ModuleReply::Int(if d.ai_free_to_exit(dead) { FREE_TO_EXIT } else { NOT_FREE_TO_EXIT })
                }
                _ => ModuleReply::Int(FREE_TO_EXIT),
            },
            ModuleCall::AiGetDozerAIInterface => ModuleReply::Bool(matches!(m.ai().subclass, AiSubclass::Dozer(_) | AiSubclass::Worker(_))),
            ModuleCall::HasSupplyTruckAiInterface => ModuleReply::Bool(matches!(m.ai().subclass, AiSubclass::SupplyTruck(_) | AiSubclass::Worker(_))),
            // the current locomotor's
            ModuleCall::AiHasCurLocomotor => ModuleReply::Bool(m.ai().get_cur_locomotor().is_some()),
            ModuleCall::AiGetCurLocomotorPreferredHeight => match m.ai().get_cur_locomotor() {
                Some(l) => ModuleReply::Real(l.get_preferred_height()),
                None => ModuleReply::Unhandled,
            },
            ModuleCall::AiLocomotorSway(which) => match m.ai().get_cur_locomotor() {
                Some(l) => ModuleReply::Real(match which {
                    0 => l.get_pitch_stiffness(),
                    1 => l.get_roll_stiffness(),
                    2 => l.get_pitch_damping(),
                    _ => l.get_roll_damping(),
                }),
                None => ModuleReply::Unhandled,
            },
            ModuleCall::AiLocomotorSetUltraAccurate(v) => locomotor(m, |l| l.set_ultra_accurate(v)),
            ModuleCall::AiLocomotorSetAllowInvalidPosition(v) => locomotor(m, |l| l.set_allow_invalid_position(v)),
            ModuleCall::AiLocomotorSetCloseEnoughDist(v) => locomotor(m, |l| l.set_close_enough_dist(v)),
            ModuleCall::AiLocomotorSetCloseEnoughDist3D(v) => locomotor(m, |l| l.set_close_enough_dist_3d(v)),
            ModuleCall::AiLocomotorSetUsePreciseZPos(v) => locomotor(m, |l| l.set_use_precise_z_pos(v)),
            ModuleCall::AiLocomotorSetMaxLift(v) => locomotor(m, |l| l.set_max_lift(v)),
            ModuleCall::AiLocomotorSetMaxTurnRate(v) => locomotor(m, |l| l.set_max_turn_rate(v)),
            ModuleCall::AiLocomotorSetMaxBraking(v) => locomotor(m, |l| l.set_max_braking(v)),
            // the turrets (batch I9b-6), with the AI lent to the World (the turrets ask their AI)
            ModuleCall::AiSetTurretTargetPosition { turret, pos } => {
                lent(m, world, me, |w| crate::logic::ai::ai_attack_states::set_turret_target_position(w, turret, Some(&pos)));
                ModuleReply::Done
            }
            ModuleCall::AiSetTurretTargetObject { turret, target, force_attacking } => {
                let o = (target != INVALID_ID).then_some(target);
                lent(m, world, me, |w| crate::logic::ai::ai_attack_states::set_turret_target_object(w, turret, o, force_attacking));
                ModuleReply::Done
            }
            ModuleCall::AiGetWhichTurretForCurWeapon => ModuleReply::Int(lent(m, world, me, crate::logic::ai::ai_attack_states::get_which_turret_for_cur_weapon)),
            ModuleCall::AiSetTurretEnabled { turret, enabled } => {
                lent(m, world, me, |w| crate::logic::ai::ai_attack_states::set_turret_enabled(w, turret, enabled));
                ModuleReply::Done
            }
            ModuleCall::AiRecenterTurret { turret } => {
                lent(m, world, me, |w| crate::logic::ai::ai_attack_states::recenter_turret(w, turret));
                ModuleReply::Done
            }
            ModuleCall::AiIsTurretInNaturalPosition { turret } => {
                ModuleReply::Bool(lent(m, world, me, |w| crate::logic::ai::ai_attack_states::is_turret_in_natural_position(w, turret)))
            }
            // `aiGoProne(damageInfo, cmdSource)` (batch I9b-6): the parms' `m_damage` is the World's while the command runs
            ModuleCall::AiGoProne { info, command_source } => {
                let prev = world.ai_prone_damage.replace(info);
                let r = command(m, world, me, parms(AICMD_GO_PRONE, command_source));
                world.ai_prone_damage = prev;
                r
            }
            // (not routed yet: construct, transferAttack (TheAI's), getAttackInfo)
            _ => ModuleReply::Unhandled,
        }
    }

    /// The snapshot without the world (see the module docs).
    pub fn xfer_with(&mut self, _update: &mut UpdateModule, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Err(XferStatus::InvalidParameters)
    }

    pub fn crc_with(&mut self, _update: &mut UpdateModule, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Err(XferStatus::InvalidParameters)
    }

    pub fn load_post_process_with(&mut self, _update: &mut UpdateModule) {}
}

/// The AI's subclass part as the class's constructor makes it from its module data (batch I9b-5): the data the AI keeps, the
/// names it resolves. `LockonCursor` and `RopeName` are thing templates of the World's factory (found or not, as the AI
/// keeps them). `RotorWashParticleSystem` names a client particle system template, which the World has not: never found
/// (a cut point; the rotor wash is the client's). An FX list is kept by an index into `cuts.ai_host.fx_names`, which the FX
/// cut points log. A missile is made at the frame and its object's position. `WanderAIUpdate` and `TransportAIUpdate` have
/// the plain data and their own class.
fn make_subclass(world: &mut World, id: ObjectId, name: &[u8], data: &AiUpdateModuleData) -> AiSubclass {
    use crate::logic::ai::ai_subclass_data::AiSubclassModuleData as D;
    use crate::logic::ai::{
        assault_transport_ai_update as at, chinook_ai_update as ch, deliver_payload_ai_update as dp, deploy_style_ai_update as ds,
        dozer_ai_update as dz, hack_internet_ai_update as hi, jet_ai_update as jt, missile_ai_update as mi, railed_transport_ai_update as rt,
        supply_truck_ai_update as st, worker_ai_update as wk,
    };
    let supply = |s: &crate::logic::ai::ai_subclass_data::SupplyTruckAiModuleData| st::SupplyTruckData {
        max_boxes: s.max_boxes_data,
        center_delay: s.center_delay,
        warehouse_delay: s.warehouse_delay,
        warehouse_scan_distance: s.warehouse_scan_distance,
        has_depleted_voice: !s.supplies_depleted_voice.event_name.is_empty(),
    };
    match &data.subclass {
        D::Plain => match name {
            b"WanderAIUpdate" => AiSubclass::Wander,
            b"TransportAIUpdate" => AiSubclass::Transport,
            _ => AiSubclass::Plain,
        },
        D::Jet(j) => AiSubclass::Jet(jt::Jet::new(jt::JetData {
            out_of_ammo_damage_per_second: j.out_of_ammo_damage_per_second,
            needs_runway: j.needs_runway,
            keeps_parking_space_when_airborne: j.keeps_parking_space_when_airborne,
            takeoff_dist_for_max_lift: j.takeoff_dist_for_max_lift,
            min_height: j.min_height,
            parking_offset: j.parking_offset,
            sneaky_offset_when_attacking: j.sneaky_offset_when_attacking,
            takeoff_pause: j.takeoff_pause,
            attacking_loco: j.attacking_loco,
            returning_loco: j.returning_loco,
            attack_loco_persist_time: j.attack_loco_persist_time,
            attackers_miss_persist_time: j.attackers_miss_persist_time,
            lockon_time: j.lockon_time,
            lockon_cursor: world.thing_factory_find_template(&j.lockon_cursor).is_some(),
            lockon_initial_dist: j.lockon_initial_dist,
            lockon_freq: j.lockon_freq,
            lockon_angle_spin: j.lockon_angle_spin,
            lockon_blinky: j.lockon_blinky,
            return_to_base_idle_time: j.return_to_base_idle_time,
        })),
        D::Missile(m) => {
            let ignition_fx = world.ai_fx_handle(&m.ignition_fx);
            let garrison_hit_kill_fx = world.ai_fx_handle(&m.garrison_hit_kill_fx);
            let pos = world.objects.find(id).map(|o| o.get_position()).unwrap_or_default();
            let data = mi::MissileData {
                try_to_follow_target: m.try_to_follow_target,
                fuel_lifetime: m.fuel_lifetime,
                ignition_delay: m.ignition_delay,
                initial_vel: m.initial_vel,
                initial_dist: m.initial_dist,
                dive_distance: m.dive_distance,
                ignition_fx,
                use_weapon_speed: m.use_weapon_speed,
                detonate_on_no_fuel: m.detonate_on_no_fuel,
                garrison_hit_kill_count: m.garrison_hit_kill_count,
                garrison_hit_kill_kindof: m.garrison_hit_kill_kindof,
                garrison_hit_kill_kindof_not: m.garrison_hit_kill_kindof_not,
                garrison_hit_kill_fx,
                distance_scatter_when_jammed: m.distance_scatter_when_jammed,
                lock_distance: m.lock_distance,
                detonate_calls_kill: m.detonate_calls_kill,
                kill_self_delay: m.kill_self_delay,
            };
            AiSubclass::Missile(mi::Missile::new(data, world.frame(), pos))
        }
        D::SupplyTruck(s) => AiSubclass::SupplyTruck(st::SupplyTruck::new(supply(s))),
        D::Chinook(c) => AiSubclass::Chinook(ch::Chinook::new(ch::ChinookData {
            supply: supply(&c.supply),
            rope_template: world.thing_factory_find_template(&c.rope_name).is_some(),
            rotor_wash: false,
            rappel_speed: c.rappel_speed,
            rope_drop_speed: c.rope_drop_speed,
            rope_width: c.rope_width,
            rope_final_height: c.rope_final_height,
            rope_wobble_len: c.rope_wobble_len,
            rope_wobble_amp: c.rope_wobble_amp,
            rope_wobble_rate: c.rope_wobble_rate,
            rope_color: [c.rope_color.red, c.rope_color.green, c.rope_color.blue],
            num_ropes: c.num_ropes,
            per_rope_delay_min: c.per_rope_delay_min,
            per_rope_delay_max: c.per_rope_delay_max,
            min_drop_height: c.min_drop_height,
            wait_for_ropes_to_drop: c.wait_for_ropes_to_drop,
            upgraded_supply_boost: c.upgraded_supply_boost,
        })),
        D::Dozer(z) => AiSubclass::Dozer(dz::Dozer::new(dz::DozerData {
            repair_health_percent_per_second: z.repair_health_percent_per_second,
            bored_time: z.bored_time,
            bored_range: z.bored_range,
        })),
        D::Worker(w) => AiSubclass::Worker(wk::Worker::new(wk::WorkerData {
            dozer: dz::DozerData { repair_health_percent_per_second: w.repair_health_percent_per_second, bored_time: w.bored_time, bored_range: w.bored_range },
            supply: st::SupplyTruckData {
                max_boxes: w.max_boxes_data,
                center_delay: w.center_delay,
                warehouse_delay: w.warehouse_delay,
                warehouse_scan_distance: w.warehouse_scan_distance,
                has_depleted_voice: !w.supplies_depleted_voice.event_name.is_empty(),
            },
            upgraded_supply_boost: w.upgraded_supply_boost,
        })),
        D::DeliverPayload(p) => AiSubclass::DeliverPayload(dp::DeliverPayload::new(dp::DeliverPayloadModuleData {
            door_delay: p.door_delay,
            max_distance_to_target: p.max_distance_to_target,
            max_number_attempts: p.max_number_attempts,
            drop_delay: p.drop_delay,
            drop_offset: p.drop_offset,
            drop_variance: p.drop_variance,
            put_in_container_name: p.put_in_container_name.clone(),
            delivery_decal_template: p.delivery_decal_template.clone(),
            delivery_decal_radius: p.delivery_decal_radius,
        })),
        D::DeployStyle(d) => AiSubclass::DeployStyle(ds::DeployStyle::new(ds::DeployStyleData {
            unpack_time: d.unpack_time,
            pack_time: d.pack_time,
            reset_turret_before_packing: d.reset_turret_before_packing,
            turrets_function_only_when_deployed: d.turrets_function_only_when_deployed,
            turrets_must_center_before_packing: d.turrets_must_center_before_packing,
            manual_deploy_animations: d.manual_deploy_animations,
        })),
        D::HackInternet(h) => AiSubclass::HackInternet(hi::HackInternet::new(hi::HackInternetData {
            unpack_time: h.unpack_time,
            pack_time: h.pack_time,
            cash_update_delay: h.cash_update_delay,
            cash_update_delay_fast: h.cash_update_delay_fast,
            regular_cash_amount: h.regular_cash_amount,
            veteran_cash_amount: h.veteran_cash_amount,
            elite_cash_amount: h.elite_cash_amount,
            heroic_cash_amount: h.heroic_cash_amount,
            xp_per_cash_update: h.xp_per_cash_update,
            pack_unpack_variation_factor: h.pack_unpack_variation_factor,
        })),
        D::AssaultTransport(a) => AiSubclass::AssaultTransport(at::AssaultTransport::new(at::AssaultTransportData {
            members_get_healed_at_life_ratio: a.members_get_healed_at_life_ratio,
            clear_range_required_to_continue_attack_move: a.clear_range_required_to_continue_attack_move,
        })),
        D::RailedTransport { path_prefix_name } => AiSubclass::RailedTransport(rt::RailedTransport::new(rt::RailedTransportData {
            path_prefix_name: String::from_utf8_lossy(path_prefix_name).into_owned(),
        })),
    }
}

/// `AICommandParms(cmd, cmdSource)` with the source as the call carries it.
fn parms(cmd: AiCommandType, command_source: i32) -> AiCommandParms {
    AiCommandParms::new(cmd, command_source)
}

/// The parms with `m_obj`.
pub(crate) fn with_obj(cmd: AiCommandType, obj: ObjectId, command_source: i32) -> AiCommandParms {
    let mut p = parms(cmd, command_source);
    p.obj = obj;
    p
}

/// The parms with `m_pos`.
fn with_pos(cmd: AiCommandType, pos: Coord3D, command_source: i32) -> AiCommandParms {
    let mut p = parms(cmd, command_source);
    p.pos = pos;
    p
}

/// Runs `f` with the module's AI and machine lent to the World (on `ai_stacks.ai` and `ai_stacks.machine`) and the object as
/// `this`, as `AIUpdateInterface::update` lends them around its turrets (batch I9b-6).
fn lent<R>(m: &mut AiUpdateModule<World>, world: &mut World, me: ModuleHandle, f: impl FnOnce(&mut World) -> R) -> R {
    world.ai_stacks.ai.push(m.ai.take().expect("the AI (not lent)"));
    world.ai_stacks.machine.push(m.machine.take().expect("the AI's machine"));
    let r = world.with_ai_self(me.object, f);
    m.machine = Some(world.ai_stacks.machine.pop().expect("the lent machine"));
    m.ai = Some(world.ai_stacks.ai.pop().expect("the lent AI"));
    r
}

/// `aiDoCommand(&parms)` with the object as `this`; an error of the machine is recorded in `team_live_errors`.
/// `AICommandInterface`'s inline builders (AI.h): the `AICommandParms` of a command call, `None` for any other call (batch I9h:
/// shared by the module and by a running AI's own commands, [`World::lent_ai_call`]). `aiForceAttackObject` and
/// `aiAttackMoveToPosition` (AI.h:571, :604) were not routed before.
pub(crate) fn ai_command_parms(call: &ModuleCall) -> Option<AiCommandParms> {
    let with_shots = |mut p: AiCommandParms, max_shots: i32| {
        p.int_value = max_shots;
        p
    };
    let with_path = |mut p: AiCommandParms, path: &[Coord3D]| {
        p.coords = path.to_vec();
        p
    };
    Some(match call {
        ModuleCall::AiIdle => AiCommandParms::new(AICMD_IDLE, CMD_FROM_AI),
        ModuleCall::AiIdleFrom { command_source } => parms(AICMD_IDLE, *command_source),
        ModuleCall::AiBusy { command_source } => parms(AICMD_BUSY, *command_source),
        ModuleCall::AiHunt { command_source } => parms(AICMD_HUNT, *command_source),
        ModuleCall::AiHackInternet { command_source } => parms(AICMD_HACK_INTERNET, *command_source),
        ModuleCall::AiEvacuate { expose_stealth_units, command_source } => {
            with_shots(parms(AICMD_EVACUATE, *command_source), i32::from(*expose_stealth_units))
        }
        ModuleCall::AiEvacuateInstantly { expose_stealth_units } => {
            with_shots(AiCommandParms::new(AICMD_EVACUATE_INSTANTLY, CMD_FROM_AI), i32::from(*expose_stealth_units))
        }
        ModuleCall::AiGuardRetaliate { victim, pos } => {
            let mut p = AiCommandParms::new(AICMD_GUARD_RETALIATE, CMD_FROM_AI);
            p.obj = *victim;
            p.pos = *pos;
            p.int_value = NO_MAX_SHOTS_LIMIT;
            p
        }
        ModuleCall::AiExit { container, command_source } => with_obj(AICMD_EXIT, *container, *command_source),
        ModuleCall::AiExitInstantly { container, command_source } => with_obj(AICMD_EXIT_INSTANTLY, *container, *command_source),
        ModuleCall::AiMoveToObject { target, command_source } => with_obj(AICMD_MOVE_TO_OBJECT, *target, *command_source),
        ModuleCall::AiFaceObject { target, command_source } => with_obj(AICMD_FACE_OBJECT, *target, *command_source),
        ModuleCall::AiGetHealed { target, command_source } => with_obj(AICMD_GET_HEALED, *target, *command_source),
        ModuleCall::AiEnter { target, command_source } => with_obj(AICMD_ENTER, *target, *command_source),
        ModuleCall::AiResumeConstruction { object, command_source } => with_obj(AICMD_RESUME_CONSTRUCTION, *object, *command_source),
        ModuleCall::AiMoveToPosition { pos, command_source } => with_pos(AICMD_MOVE_TO_POSITION, *pos, *command_source),
        ModuleCall::AiFacePosition { pos, command_source } => with_pos(AICMD_FACE_POSITION, *pos, *command_source),
        ModuleCall::AiAttackObject { victim, max_shots, command_source } => with_shots(with_obj(AICMD_ATTACK_OBJECT, *victim, *command_source), *max_shots),
        ModuleCall::AiForceAttackObject { victim, max_shots, command_source } => {
            with_shots(with_obj(AICMD_FORCE_ATTACK_OBJECT, *victim, *command_source), *max_shots)
        }
        ModuleCall::AiAttackPosition { pos: Some(pos), max_shots, command_source } => with_shots(with_pos(AICMD_ATTACK_POSITION, *pos, *command_source), *max_shots),
        ModuleCall::AiAttackMoveToPosition { pos, max_shots, command_source } => {
            with_shots(with_pos(AICMD_ATTACKMOVE_TO_POSITION, *pos, *command_source), *max_shots)
        }
        ModuleCall::AiFollowPath { path, ignore_object, command_source } => with_path(with_obj(AICMD_FOLLOW_PATH, *ignore_object, *command_source), path),
        ModuleCall::AiFollowExitProductionPath { path, ignore_object, command_source } => {
            with_path(with_obj(AICMD_FOLLOW_EXITPRODUCTION_PATH, *ignore_object, *command_source), path)
        }
        ModuleCall::AiFollowWaypointPath { waypoint, command_source } => {
            let mut p = parms(AICMD_FOLLOW_WAYPOINT_PATH, *command_source);
            p.waypoint = u32::try_from(*waypoint).ok();
            p
        }
        ModuleCall::AiDoCommandParms(p) => p.clone(),
        ModuleCall::AiDoCommand { cmd, command_source, obj, pos } => {
            let mut p = parms(*cmd, *command_source);
            p.obj = *obj;
            p.pos = *pos;
            p
        }
        _ => return None,
    })
}

/// Batch I9i: one of `SupplyTruckAIInterface`'s calls (SupplyTruckAIUpdate.h:166-182).
/// Whether the module's AI is a `MissileAIUpdate` (integration batch I14).
fn is_missile(m: &crate::logic::ai::ai_states::AiUpdateModule<World>) -> bool {
    m.ai.as_ref().is_some_and(|a| matches!(a.subclass, crate::logic::ai::ai_update::AiSubclass::Missile(_)))
}

fn is_supply_call(call: &ModuleCall) -> bool {
    matches!(
        call,
        ModuleCall::GetSupplyTruckPreferredDockId
            | ModuleCall::GetSupplyTruckWarehouseScanDistance
            | ModuleCall::SupplyTruckGetNumberBoxes
            | ModuleCall::SupplyTruckIsAvailableForSupplying
            | ModuleCall::SupplyTruckLoseOneBox
            | ModuleCall::SupplyTruckGainOneBox { .. }
            | ModuleCall::SupplyTruckGetUpgradedSupplyBoost
            | ModuleCall::SupplyTruckSetForceWantingState(_)
    )
}

/// Batch I9i: `getSupplyTruckAIInterface()`'s calls answered for the AI lent on top of `ai_stacks.ai` (by `AiUpdateModule::run`,
/// or already, while it runs and a dock calls back): a supply truck's, a worker's and a chinook's supply part; for any other AI
/// the interface is `NULL` (`Unhandled`). The supply part is worked on as a copy, written back after (its functions take the host,
/// which may ask the lent AI again).
fn supply_call(w: &mut World, call: &ModuleCall) -> ModuleReply {
    use crate::logic::ai::supply_truck_ai_update as st;
    let Some(a) = w.ai_stacks.ai.last() else { return ModuleReply::Unhandled };
    let (mut s, kind) = match &a.subclass {
        AiSubclass::SupplyTruck(s) => (*s, 0),
        AiSubclass::Worker(wk) => (wk.supply, 1),
        AiSubclass::Chinook(c) => (c.supply, 2),
        _ => return ModuleReply::Unhandled,
    };
    let reply = match call {
        ModuleCall::GetSupplyTruckPreferredDockId => ModuleReply::Id(s.preferred_dock),
        ModuleCall::GetSupplyTruckWarehouseScanDistance => ModuleReply::Real(st::warehouse_scan_distance(w, &s)),
        ModuleCall::SupplyTruckGetNumberBoxes => ModuleReply::Int(s.number_boxes),
        ModuleCall::SupplyTruckIsAvailableForSupplying => ModuleReply::Bool(st::is_available_for_supplying(w)),
        ModuleCall::SupplyTruckLoseOneBox => ModuleReply::Bool(st::lose_one_box(w, &mut s)),
        ModuleCall::SupplyTruckGainOneBox { remaining_stock } => ModuleReply::Bool(st::gain_one_box(w, &mut s, *remaining_stock)),
        // (the supply truck's is 0, SupplyTruckAIUpdate.h:220)
        ModuleCall::SupplyTruckGetUpgradedSupplyBoost => ModuleReply::Int(match kind {
            1 => crate::logic::ai::worker_ai_update::upgraded_supply_boost(w),
            2 => crate::logic::ai::chinook_ai_update::upgraded_supply_boost(w),
            _ => 0,
        }),
        ModuleCall::SupplyTruckSetForceWantingState(v) => {
            s.force_pending = *v;
            ModuleReply::Done
        }
        _ => return ModuleReply::Unhandled,
    };
    if let Some(a) = w.ai_stacks.ai.last_mut() {
        match &mut a.subclass {
            AiSubclass::SupplyTruck(x) => *x = s,
            AiSubclass::Worker(wk) => wk.supply = s,
            AiSubclass::Chinook(c) => c.supply = s,
            _ => {}
        }
    }
    reply
}

fn command(m: &mut AiUpdateModule<World>, world: &mut World, me: ModuleHandle, p: AiCommandParms) -> ModuleReply {
    if let Err(e) = world.with_ai_self(me.object, |w| m.ai_do_command(w, &p)) {
        world.cuts.team_live_errors.push(format!("AIUpdateInterface::aiDoCommand({}): {e:?}", p.cmd));
    }
    ModuleReply::Done
}

/// A predicate of the AI that asks its machine; false without a machine.
fn machine_bool(
    m: &mut AiUpdateModule<World>,
    world: &mut World,
    me: ModuleHandle,
    f: impl FnOnce(&AiUpdate, &StateMachine<AiState<World>>, &mut World) -> bool,
) -> ModuleReply {
    let (Some(a), Some(sm)) = (m.ai.as_ref(), m.machine.as_ref()) else { return ModuleReply::Bool(false) };
    ModuleReply::Bool(world.with_ai_self(me.object, |w| f(a, sm, w)))
}

/// `getCurLocomotor()->set...(..)`: nothing without a locomotor (upstream dereferences it; its callers check first).
fn locomotor(m: &mut AiUpdateModule<World>, f: impl FnOnce(&mut Locomotor)) -> ModuleReply {
    if let Some(l) = m.ai_mut().get_cur_locomotor_mut() {
        f(l);
    }
    ModuleReply::Done
}

impl World {
    /// Batch I9h: a call to the AI module of `ai_self` while that AI is running (its states call into the world, which calls back:
    /// a transport's `exitObjectViaDoor` asks the exiting rider's `getCurLocomotor()` and orders it along its exit path). Its body is
    /// out of the slot then, its AI lent on top of `ai_stacks.ai` (every command and update runs through `AiUpdateModule::run`) and
    /// its machine borrowed by the running state. Upstream makes the call in place. Here: what the module answers from the AI alone
    /// is answered from the lent one, and its setters set it; the commands and `wakeUpAndAttemptToTarget`, which need the machine,
    /// are queued for the AI's code to apply ([`crate::logic::ai::ai_states::apply_own_calls`]); a call the module does not handle
    /// (`onDisabledEdge`, `SpyVisionUpdate`'s alone) is unhandled. `None`: any other call, which stays a re-entrant skip.
    pub(crate) fn lent_ai_call(&mut self, id: ObjectId, idx: crate::logic::world::object::ModuleIndex, call: &ModuleCall) -> Option<ModuleReply> {
        use crate::logic::ai::ai_states::OwnCall;
        if id != self.ai_self || self.objects.find(id)?.ai != Some(idx) || self.ai_stacks.ai.is_empty() {
            return None;
        }
        if let Some(p) = ai_command_parms(call) {
            self.ai_own_calls.push((id, OwnCall::Command(p)));
            return Some(ModuleReply::Done);
        }
        // (batch I9i: a dock's calls to the docking truck: its boxes, its preferred dock, its scan distance)
        if is_supply_call(call) {
            return Some(supply_call(self, call));
        }
        let now = self.frame();
        let a = self.ai_stacks.ai.last_mut()?;
        Some(match call {
            ModuleCall::AiWakeUpAndAttemptToTarget => {
                self.ai_own_calls.push((id, OwnCall::WakeUpAndAttemptToTarget));
                ModuleReply::Done
            }
            ModuleCall::AiIgnoreObstacle { obstacle } => {
                a.ignore_obstacle_id(*obstacle);
                ModuleReply::Done
            }
            ModuleCall::AiSetIgnoreCollisionTime { frames } => {
                a.set_ignore_collision_time(now, *frames);
                ModuleReply::Done
            }
            ModuleCall::OnDisabledEdge { .. } => ModuleReply::Unhandled,
            ModuleCall::AiGetLastCommandSource => ModuleReply::Int(a.last_command_source),
            ModuleCall::AiSetAllowedToChase(allowed) => {
                a.allowed_to_chase = *allowed;
                ModuleReply::Done
            }
            ModuleCall::AiSetDesiredSpeed(speed) => {
                a.desired_speed = *speed;
                ModuleReply::Done
            }
            ModuleCall::AiSetAttackInfo(info) => {
                a.attack_info = *info;
                ModuleReply::Done
            }
            ModuleCall::AiClearWaypointQueue => {
                a.clear_waypoint_queue();
                ModuleReply::Done
            }
            ModuleCall::AiExecuteWaypointQueue => {
                a.execute_waypoint_queue();
                ModuleReply::Done
            }
            ModuleCall::AiIsAiInDeadState => ModuleReply::Bool(a.is_ai_in_dead_state()),
            ModuleCall::AiGetIgnoredObstacleId => ModuleReply::Id(a.get_ignored_obstacle_id()),
            ModuleCall::AiGetCurLocomotorSetType => ModuleReply::Int(a.cur_locomotor_set),
            ModuleCall::AiHasLocomotorForSurface(s) => ModuleReply::Bool(a.has_locomotor_for_surface(*s as i32)),
            ModuleCall::AiHasCurLocomotor => ModuleReply::Bool(a.get_cur_locomotor().is_some()),
            ModuleCall::AiGetDozerAIInterface => ModuleReply::Bool(matches!(a.subclass, AiSubclass::Dozer(_) | AiSubclass::Worker(_))),
            ModuleCall::HasSupplyTruckAiInterface => ModuleReply::Bool(matches!(a.subclass, AiSubclass::SupplyTruck(_) | AiSubclass::Worker(_))),
            // (the chinook's and the deliverer's overrides need more than the AI)
            ModuleCall::AiGetFreeToExit { .. } if !matches!(a.subclass, AiSubclass::Chinook(_) | AiSubclass::DeliverPayload(_)) => ModuleReply::Int(FREE_TO_EXIT),
            _ => return None,
        })
    }

    /// The AI module's `xfer` with the world as its host: the slot's `UpdateModule` base lent as `ai_update_base` (the
    /// AI's `xfer` saves it first, through the host), the waypoints' names and ids from the terrain. An error of the AI's
    /// snapshot is its `XferStatus`, or `ErrorUnknown` for `SC_INVALID_DATA` (a throw code, not an `XferStatus`); an
    /// unconstructed module, `InvalidParameters`.
    pub(crate) fn xfer_ai_slot(&mut self, id: ObjectId, update: &mut Option<UpdateModule>, m: &mut AiModuleBody, xfer: &mut dyn Xfer) -> XferResult<()> {
        let Some(module) = m.module.as_mut() else { return Err(XferStatus::InvalidParameters) };
        let ways: Vec<(u32, Vec<u8>)> =
            self.terrain.waypoint_handles().into_iter().filter_map(|h| self.terrain.waypoint(h)).map(|w| (w.id() as u32, w.name().str().to_vec())).collect();
        self.ai_update_base = update.take();
        let r = self.with_ai_self(id, |w| {
            module.xfer(
                w,
                xfer,
                |i| ways.iter().find(|x| x.0 == i).map(|x| x.1.clone()).unwrap_or_default(),
                |n| ways.iter().find(|x| x.1 == n).map(|x| x.0),
            )
        });
        *update = self.ai_update_base.take();
        r.map_err(|e| match e {
            StateMachineError::Xfer(s) => s,
            _ => XferStatus::ErrorUnknown,
        })
    }

    /// The AI module's `loadPostProcess` with the world as its host (its base's through the host, as for `xfer`).
    pub(crate) fn load_post_process_ai_slot(&mut self, id: ObjectId, update: &mut Option<UpdateModule>, m: &mut AiModuleBody) {
        let Some(module) = m.module.as_mut() else { return };
        self.ai_update_base = update.take();
        self.with_ai_self(id, |w| module.load_post_process(w));
        *update = self.ai_update_base.take();
    }
}

#[cfg(test)]
mod tests {
    //! The AI itself is checked against the compiled C++ by `tests/differential_ai_update.rs`; this checks it as a module of
    //! a world's object.
    use super::*;
    use crate::common::object_status_types::ObjectStatusMaskType;
    use crate::logic::ai::ai_update_types::AI_IDLE;
    use crate::logic::world::player_host_world::tests::world;
    use crate::logic::world::update_body::UpdateBody;

    fn scout(w: &mut World) -> crate::common::game_type::ObjectId {
        let t = World::thing_factory_find_template(w, b"Scout").unwrap();
        World::thing_factory_new_object(w, t, None, ObjectStatusMaskType::default()).unwrap()
    }

    fn ai_state(w: &World, id: crate::common::game_type::ObjectId) -> Option<u32> {
        let index = w.find_object(id)?.ai?;
        match w.peek_module_body(ModuleHandle { object: id, index })? {
            UpdateBody::AiUpdate(m) => Some(m.module.as_ref()?.machine.as_ref()?.get_current_state_id()),
            _ => None,
        }
    }

    /// A scout with an `AIUpdateInterface`. Without `live_ai` its AI is the untranslated module stub, as before. With it,
    /// the factory makes the world's AI module: constructed, its machine made by `onObjectCreated` and idle, the object's
    /// `getAI()` it. Three frames of the world's update run the machine (the scheduler wakes the AI module), which stays
    /// idle, and no live error is recorded.
    #[test]
    fn the_ai_is_a_module_of_its_object() {
        let (mut w, _) = world();
        let s = scout(&mut w);
        assert_eq!(ai_state(&w, s), None);
        w.cuts.live_ai = true;
        let s = scout(&mut w);
        assert_eq!(ai_state(&w, s), Some(AI_IDLE));
        // (the getters routed to the module: `AIUpdateInterface`'s constructor's values)
        let index = w.find_object(s).unwrap().ai.unwrap();
        assert_eq!(w.module_call(s, index, ModuleCall::AiGetLastCommandSource).as_int(-1), crate::logic::ai::ai_update_types::CMD_FROM_AI);
        assert_eq!(w.module_call(s, index, ModuleCall::AiGetCurrentVictim).as_id(), crate::common::game_type::INVALID_ID);
        assert!(!w.module_call(s, index, ModuleCall::AiIsDoingGroundMovement).as_bool());
        for _ in 0..3 {
            w.logic_update().unwrap();
        }
        assert_eq!(ai_state(&w, s), Some(AI_IDLE));
        assert!(w.ai_stacks.ai.is_empty() && w.ai_stacks.machine.is_empty());
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }

    /// The scout's AI module saved (`xfer_module_slot`, the world its host): its `UpdateModule` base and the AI's state.
    /// Loaded into a second scout's AI module and saved again, it gives the same bytes. The update body's own snapshot (no
    /// world) refuses (`InvalidParameters`).
    #[test]
    fn the_ais_snapshot_has_the_world_as_its_host() {
        use crate::common::xfer::Xfer;
        use crate::common::xfer_load::XferLoad;
        use crate::common::xfer_save::XferSave;
        let (mut w, _) = world();
        w.cuts.live_ai = true;
        let (a, b) = (scout(&mut w), scout(&mut w));
        w.logic_update().unwrap();
        let idx = w.find_object(a).unwrap().ai.unwrap() as usize;
        let mut save = XferSave::new();
        save.open(b"ai").unwrap();
        w.xfer_module_slot(a, idx, &mut save).unwrap();
        let bytes = save.bytes().to_vec();
        assert!(!bytes.is_empty());
        let mut load = XferLoad::new(bytes.clone());
        load.open(b"ai").unwrap();
        w.xfer_module_slot(b, idx, &mut load).unwrap();
        let mut again = XferSave::new();
        again.open(b"ai").unwrap();
        w.xfer_module_slot(b, idx, &mut again).unwrap();
        assert_eq!(again.bytes(), &bytes[..]);
        let h = ModuleHandle { object: a, index: idx as u16 };
        let mut body = w.take_module_body(h).unwrap();
        let mut save = XferSave::new();
        save.open(b"ai").unwrap();
        assert!(matches!(crate::common::xfer::Snapshot::xfer(&mut body, &mut save), Err(XferStatus::InvalidParameters)));
        w.put_module_body(h, body);
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
    /// The object's calls routed to its AI module. A player's `aiMoveToPosition` takes the idle machine out of its idle
    /// state and sets its goal position; `aiIdle` returns it to idle, which `isIdle` and `isBusy` then report. The
    /// setters reach the AI (`setIsRecruitable`), the base's `getAiFreeToExit` is FREE_TO_EXIT, a scout is no dozer, and
    /// no live error is recorded.
    #[test]
    fn the_objects_calls_reach_its_ai() {
        let (mut w, _) = world();
        w.cuts.live_ai = true;
        let s = scout(&mut w);
        let index = w.find_object(s).unwrap().ai.unwrap();
        let goal = Coord3D { x: 40.0, y: 30.0, z: 0.0 };
        assert!(w.module_call(s, index, ModuleCall::AiIsIdle).as_bool());
        w.module_call(s, index, ModuleCall::AiMoveToPosition { pos: goal, command_source: 0 });
        assert_ne!(ai_state(&w, s), Some(AI_IDLE));
        assert!(!w.module_call(s, index, ModuleCall::AiIsIdle).as_bool());
        match w.module_call(s, index, ModuleCall::AiGetGoalPosition) {
            ModuleReply::Coord(c) => assert_eq!((c.x, c.y), (goal.x, goal.y)),
            r => panic!("{r:?}"),
        }
        assert_eq!(w.module_call(s, index, ModuleCall::AiGetLastCommandSource).as_int(-1), 0);
        w.module_call(s, index, ModuleCall::AiIdleFrom { command_source: 0 });
        assert_eq!(ai_state(&w, s), Some(AI_IDLE));
        assert!(w.module_call(s, index, ModuleCall::AiIsIdle).as_bool());
        assert!(!w.module_call(s, index, ModuleCall::AiIsBusy).as_bool());
        w.module_call(s, index, ModuleCall::AiSetIsRecruitable(false));
        let h = ModuleHandle { object: s, index };
        match w.peek_module_body(h) {
            Some(UpdateBody::AiUpdate(m)) => assert!(!m.module.as_ref().unwrap().ai().is_recruitable),
            _ => panic!("no AI module"),
        }
        assert_eq!(w.module_call(s, index, ModuleCall::AiGetFreeToExit { exiter: s }).as_int(-1), FREE_TO_EXIT);
        assert!(!w.module_call(s, index, ModuleCall::AiGetDozerAIInterface).as_bool());
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
    /// batch I9b-5: the subclasses. A dozer's `DozerAIUpdate` block is read into its data (2% a second, 5000 ms of boredom
    /// as 150 frames, a range of 150), and with `live_ai` the World makes its AI as a dozer: the subclass with that data,
    /// the dozer's own machine made, the AI's machine idle, `getDozerAIInterface` answered. A wanderer's `WanderAIUpdate`
    /// has the plain data and the wanderer's class. Three frames run both (on `Player1`'s team) without a live error.
    #[test]
    fn the_subclasses_are_made_from_their_data() {
        use crate::logic::ai::ai_update::AiSubclass;
        let (mut w, team) = world();
        w.cuts.live_ai = true;
        w.cuts.live_teams = true;
        // (on `Player1`'s team, the teams live: a dozer's idle state asks its player, and upstream every object has a team)
        let make = |w: &mut World, name: &[u8]| {
            let t = World::thing_factory_find_template(w, name).unwrap();
            World::thing_factory_new_object(w, t, Some(team), ObjectStatusMaskType::default()).unwrap()
        };
        let d = make(&mut w, b"Dozer");
        let wd = make(&mut w, b"Wanderer");
        let body = |w: &World, id| {
            let index = w.find_object(id).unwrap().ai.unwrap();
            match w.peek_module_body(ModuleHandle { object: id, index }) {
                Some(UpdateBody::AiUpdate(m)) => (m.name.clone(), m.module.as_ref().map(|m| (m.ai().subclass.clone(), m.dozer_machine.is_some()))),
                _ => panic!("no AI module"),
            }
        };
        let (name, made) = body(&w, d);
        assert_eq!(name, b"DozerAIUpdate");
        match made {
            Some((AiSubclass::Dozer(z), true)) => {
                assert_eq!(z.data.repair_health_percent_per_second, 0.02);
                assert_eq!(z.data.bored_time, 150.0);
                assert_eq!(z.data.bored_range, 150.0);
            }
            m => panic!("{m:?}"),
        }
        assert!(matches!(body(&w, wd).1, Some((AiSubclass::Wander, false))));
        assert_eq!(ai_state(&w, d), Some(AI_IDLE));
        let index = w.find_object(d).unwrap().ai.unwrap();
        assert!(w.module_call(d, index, ModuleCall::AiGetDozerAIInterface).as_bool());
        for _ in 0..3 {
            w.logic_update().unwrap();
        }
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
    /// batch I9b-6: the turrets. A gun's AI (a `Turret` block: turn rate 60, the primary slot) has its turret made with it,
    /// kept by the World: in its natural position, enabled. The object's turret calls reach it with the AI lent:
    /// `setTurretEnabled(0, false)` disables it, `isTurretInNaturalPosition(0)` holds. A hundred frames of the World's update run
    /// the AI and its turret (past the 60-frame mood check, which asks the AI for a mood target) without a live error, with the
    /// stacks empty after. Destroying the gun deletes its turret.
    #[test]
    fn the_turrets_run_with_their_ai() {
        let (mut w, team) = world();
        w.cuts.live_ai = true;
        w.cuts.live_teams = true;
        let t = World::thing_factory_find_template(&w, b"Gun").unwrap();
        let g = World::thing_factory_new_object(&mut w, t, Some(team), ObjectStatusMaskType::default()).unwrap();
        let d = w.turret_peek(g, 0).expect("the turret is made with the AI").clone();
        assert!(d.enabled);
        assert_eq!(d.angle, 0.0);
        assert!(w.turret_peek(g, 1).is_none());
        let index = w.find_object(g).unwrap().ai.unwrap();
        assert!(w.module_call(g, index, ModuleCall::AiIsTurretInNaturalPosition { turret: 0 }).as_bool());
        w.module_call(g, index, ModuleCall::AiSetTurretEnabled { turret: 0, enabled: false });
        assert!(!w.turret_peek(g, 0).unwrap().enabled);
        w.module_call(g, index, ModuleCall::AiSetTurretEnabled { turret: 0, enabled: true });
        for _ in 0..100 {
            w.logic_update().unwrap();
        }
        assert!(w.turret_peek(g, 0).is_some());
        assert!(w.ai_stacks.ai.is_empty() && w.ai_stacks.machine.is_empty() && w.ai_stacks.turret.is_empty());
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        w.destroy_object(g);
        w.process_destroy_list();
        assert!(w.turret_peek(g, 0).is_none() && w.turret_ais.map.is_empty());
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
    /// batch I9b-6: `aiGoProne(damageInfo, cmdSource)` reaches the object's `ProneUpdate` with the damage the command carries:
    /// 10 dealt at a ratio of 2 is 20 prone frames. (The prone effects need a drawable, which the test's objects have not: that
    /// refusal is the prone module's, recorded in `module_errors`.)
    #[test]
    fn going_prone_carries_the_damage() {
        let (mut w, team) = world();
        w.cuts.live_ai = true;
        w.cuts.live_teams = true;
        let t = World::thing_factory_find_template(&w, b"Infantry").unwrap();
        let i = World::thing_factory_new_object(&mut w, t, Some(team), ObjectStatusMaskType::default()).unwrap();
        let mut info = crate::logic::damage::DamageInfo::default();
        info.out.actual_damage_dealt = 10.0;
        let index = w.find_object(i).unwrap().ai.unwrap();
        w.module_call(i, index, ModuleCall::AiGoProne { info, command_source: 2 });
        let prone = w.find_module(i, b"ProneUpdate").unwrap();
        match w.peek_module_body(ModuleHandle { object: i, index: prone }) {
            Some(UpdateBody::ProneUpdate(p)) => assert_eq!(p.prone_frames, 20),
            _ => panic!("no prone module"),
        }
        assert!(w.ai_prone_damage.is_none());
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
    /// batch I9c-1: a unit pathfinds and moves. A runner (a ground locomotor of speed 30, physics) at (50, 50) on the
    /// pathfinder's map is told by its player to move to (250, 250). Its AI queues for a path; the World's `AI::update` runs
    /// the pathfinder's queue, where the AI's `doPathfind` finds the path; its locomotor then moves it. After 150 frames it has
    /// moved toward the goal (both coordinates up by more than 50), with no live error. (Measured: about 1 unit a frame along
    /// the diagonal, at (177, 177) after 135 frames. A first fixture with a `FOUR_WHEELS` locomotor and the wheels' fields at
    /// their defaults turned about 2e-6 radians a frame and drove straight along x; with `TREADS` it turns at its turn rate.
    /// The locomotor is verified against the C++ by `differential_locomotor`; ~~the wheels' case was not looked into further~~
    /// **explained, batch I9f:** a wheeled vehicle turns `forward speed / max(MinTurnSpeed, max speed / 4)` of its turn rate,
    /// and `LocomotorTemplate`'s `MinTurnSpeed` defaults to `BIGNUM` (Locomotor.cpp); that fixture set no `MinTurnSpeed`, so
    /// it turned about 1e-5 of its rate, upstream as here.)
    #[test]
    fn a_unit_pathfinds_and_moves() {
        use crate::common::base_type::Coord3D;
        use crate::logic::world::pathfinder_host_world::PfOp;
        let (mut w, team) = world();
        w.cuts.live_ai = true;
        w.cuts.live_teams = true;
        w.cuts.live_pathfinder = true;
        let r = crate::logic::world::player_host_world::tests::make(&mut w, b"Runner", team, Coord3D { x: 50.0, y: 50.0, z: 0.0 });
        assert!(w.pathfinder_op(PfOp::NewMap));
        let index = w.find_object(r).unwrap().ai.unwrap();
        w.module_call(r, index, ModuleCall::AiMoveToPosition { pos: Coord3D { x: 250.0, y: 250.0, z: 0.0 }, command_source: 0 });
        for _ in 0..150 {
            w.logic_update().unwrap();
        }
        let p = w.find_object(r).unwrap().get_position();
        assert!(p.x > 100.0 && p.y > 100.0, "the runner is at {p:?}");
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
    /// batch I9c-1: the AI in the logic's CRC (`xferSnapshot(TheAI)`). The runner's game run twice gives the same CRC at every
    /// one of 30 frames. The AI's part is in it: the same game with the AI's part left out of the CRC (`live_ai` off while the
    /// CRC is taken: the cut point) gives a different CRC at every frame. (This shows the part is included and deterministic,
    /// not that each of its fields is right: the pathfinder's and the AI data's CRCs are the verified ones of their batches.)
    #[test]
    fn the_ai_is_in_the_logics_crc() {
        use crate::common::base_type::Coord3D;
        use crate::logic::world::game_logic_update::CRC_RECALC;
        use crate::logic::world::pathfinder_host_world::PfOp;
        let run = |crc_ai: bool| {
            let (mut w, team) = world();
            w.cuts.live_ai = true;
            w.cuts.live_teams = true;
            w.cuts.live_pathfinder = true;
            let r = crate::logic::world::player_host_world::tests::make(&mut w, b"Runner", team, Coord3D { x: 50.0, y: 50.0, z: 0.0 });
            assert!(w.pathfinder_op(PfOp::NewMap));
            let index = w.find_object(r).unwrap().ai.unwrap();
            w.module_call(r, index, ModuleCall::AiMoveToPosition { pos: Coord3D { x: 250.0, y: 250.0, z: 0.0 }, command_source: 0 });
            let mut crcs = Vec::new();
            for _ in 0..30 {
                w.logic_update().unwrap();
                w.cuts.live_ai = crc_ai;
                crcs.push(w.logic_crc(CRC_RECALC, None).unwrap());
                w.cuts.live_ai = true;
            }
            assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
            crcs
        };
        let (a, b, without) = (run(true), run(true), run(false));
        assert_eq!(a, b);
        assert!(a.iter().zip(&without).all(|(x, y)| x != y), "{a:?} {without:?}");
    }
}
