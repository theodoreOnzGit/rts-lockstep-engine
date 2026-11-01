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
//! **Live with `cuts.live_ai`** (off by default: an `AIUpdateInterface` stays the untranslated module stub, so every
//! test's objects behave as before). With it, the thing factory makes the module (`ModuleDataKind::AiUpdate` data, the
//! base class `AIUpdateInterface`; the subclasses are still stubs), the world's `world_construct` runs its constructor with
//! the object as [`World::ai_self`], the object's `onObjectCreated` makes its machine, and the scheduler's update runs the
//! machine. The world is the AI's host throughout (batches I9b-1 and I9b-2).
//!
//! **The snapshot** (`xfer`, `loadPostProcess`) needs its host: the world's slot snapshot (`xfer_module_slot`, an
//! object's save and load) runs it with the world as the host ([`World::xfer_ai_slot`]); the update body's own snapshot
//! calls, which have no world, answer `XferStatus::InvalidParameters`. `Object::crc` does not include modules, so the
//! logic's CRC is unaffected. The
//! other `ModuleCall`s an object makes of its AI answer `Unhandled` (they are the AI cut tables until they are routed
//! here; routed so far: `getCurrentVictim`, `isDoingGroundMovement`, `getLastCommandSource`), and the turrets' AIs
//! (`TurretHost`) are not run.
//!
//! `p1w13-sonnet`, integration batch I9b-3, 2026-11-01.

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
    pub data: Arc<AiUpdateModuleData>,
    /// None until `world_construct`.
    pub module: Option<AiUpdateModule<World>>,
}

impl Clone for AiModuleBody {
    /// A copy of the module's data, unconstructed: the AI's machines cannot be copied (their states hold no `Clone`), and
    /// the world copies no running AI (see the module docs).
    fn clone(&self) -> Self {
        AiModuleBody { data: self.data.clone(), module: None }
    }
}

impl std::fmt::Debug for AiModuleBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AiModuleBody {{ constructed: {} }}", self.module.is_some())
    }
}

impl AiModuleBody {
    /// The module as the factory makes it (the constructor runs in `world_construct`).
    pub fn new(data: Arc<AiUpdateModuleData>) -> Self {
        AiModuleBody { data, module: None }
    }

    /// `AIUpdateInterface(thing, moduleData)`, with the object as the AI's host's `this`.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        let data = self.data.clone();
        self.module = Some(world.with_ai_self(me.object, |w| AiUpdateModule::new(w, data)));
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

    /// The object's calls: `onObjectCreated` makes the machine; the others are not routed here yet.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let Some(m) = self.module.as_mut() else { return ModuleReply::Unhandled };
        match call {
            ModuleCall::OnObjectCreated => {
                if let Err(e) = world.with_ai_self(me.object, |w| m.on_object_created(w)) {
                    world.cuts.team_live_errors.push(format!("AIUpdateInterface::onObjectCreated: {e:?}"));
                }
                ModuleReply::Done
            }
            // the commands: `AICommandInterface`'s inline builders of `AICommandParms`, then `aiDoCommand` (AI.h)
            ModuleCall::AiIdle => command(m, world, me, AiCommandParms::new(AICMD_IDLE, CMD_FROM_AI)),
            ModuleCall::AiIdleFrom { command_source } => command(m, world, me, parms(AICMD_IDLE, command_source)),
            ModuleCall::AiBusy { command_source } => command(m, world, me, parms(AICMD_BUSY, command_source)),
            ModuleCall::AiHunt { command_source } => command(m, world, me, parms(AICMD_HUNT, command_source)),
            ModuleCall::AiHackInternet { command_source } => command(m, world, me, parms(AICMD_HACK_INTERNET, command_source)),
            ModuleCall::AiEvacuate { expose_stealth_units, command_source } => {
                let mut p = parms(AICMD_EVACUATE, command_source);
                p.int_value = i32::from(expose_stealth_units);
                command(m, world, me, p)
            }
            ModuleCall::AiEvacuateInstantly { expose_stealth_units } => {
                let mut p = AiCommandParms::new(AICMD_EVACUATE_INSTANTLY, CMD_FROM_AI);
                p.int_value = i32::from(expose_stealth_units);
                command(m, world, me, p)
            }
            ModuleCall::AiGuardRetaliate { victim, pos } => {
                let mut p = AiCommandParms::new(AICMD_GUARD_RETALIATE, CMD_FROM_AI);
                p.obj = victim;
                p.pos = pos;
                p.int_value = NO_MAX_SHOTS_LIMIT;
                command(m, world, me, p)
            }
            ModuleCall::AiExit { container, command_source } => command(m, world, me, with_obj(AICMD_EXIT, container, command_source)),
            ModuleCall::AiExitInstantly { container, command_source } => command(m, world, me, with_obj(AICMD_EXIT_INSTANTLY, container, command_source)),
            ModuleCall::AiMoveToObject { target, command_source } => command(m, world, me, with_obj(AICMD_MOVE_TO_OBJECT, target, command_source)),
            ModuleCall::AiFaceObject { target, command_source } => command(m, world, me, with_obj(AICMD_FACE_OBJECT, target, command_source)),
            ModuleCall::AiGetHealed { target, command_source } => command(m, world, me, with_obj(AICMD_GET_HEALED, target, command_source)),
            ModuleCall::AiEnter { target, command_source } => command(m, world, me, with_obj(AICMD_ENTER, target, command_source)),
            ModuleCall::AiResumeConstruction { object, command_source } => {
                command(m, world, me, with_obj(AICMD_RESUME_CONSTRUCTION, object, command_source))
            }
            ModuleCall::AiMoveToPosition { pos, command_source } => command(m, world, me, with_pos(AICMD_MOVE_TO_POSITION, pos, command_source)),
            ModuleCall::AiFacePosition { pos, command_source } => command(m, world, me, with_pos(AICMD_FACE_POSITION, pos, command_source)),
            ModuleCall::AiAttackObject { victim, max_shots, command_source } => {
                let mut p = with_obj(AICMD_ATTACK_OBJECT, victim, command_source);
                p.int_value = max_shots;
                command(m, world, me, p)
            }
            ModuleCall::AiAttackPosition { pos: Some(pos), max_shots, command_source } => {
                let mut p = with_pos(AICMD_ATTACK_POSITION, pos, command_source);
                p.int_value = max_shots;
                command(m, world, me, p)
            }
            // (upstream dereferences the `NULL`: undefined behaviour, recorded rather than guessed at)
            ModuleCall::AiAttackPosition { pos: None, .. } => {
                world.cuts.team_live_errors.push("AIUpdateInterface::aiAttackPosition: NULL position".to_string());
                ModuleReply::Done
            }
            ModuleCall::AiFollowPath { path, ignore_object, command_source } => {
                let mut p = with_obj(AICMD_FOLLOW_PATH, ignore_object, command_source);
                p.coords = path;
                command(m, world, me, p)
            }
            ModuleCall::AiFollowExitProductionPath { path, ignore_object, command_source } => {
                let mut p = with_obj(AICMD_FOLLOW_EXITPRODUCTION_PATH, ignore_object, command_source);
                p.coords = path;
                command(m, world, me, p)
            }
            ModuleCall::AiFollowWaypointPath { waypoint, command_source } => {
                let mut p = parms(AICMD_FOLLOW_WAYPOINT_PATH, command_source);
                p.waypoint = u32::try_from(waypoint).ok();
                command(m, world, me, p)
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
            // (not routed yet: aiGoProne (the parms' damage is the host's), construct, the turrets, transferAttack (TheAI's),
            // processCollision, getAttackInfo, getWhichTurretForCurWeapon)
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

/// `AICommandParms(cmd, cmdSource)` with the source as the call carries it.
fn parms(cmd: AiCommandType, command_source: i32) -> AiCommandParms {
    AiCommandParms::new(cmd, command_source)
}

/// The parms with `m_obj`.
fn with_obj(cmd: AiCommandType, obj: ObjectId, command_source: i32) -> AiCommandParms {
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

/// `aiDoCommand(&parms)` with the object as `this`; an error of the machine is recorded in `team_live_errors`.
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
}
