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

use crate::common::game_type::ObjectId;
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
            // the getters routed so far (the other calls of the object to its AI still answer from the AI cut tables)
            ModuleCall::AiGetCurrentVictim => {
                let ai = m.ai.take();
                let r = ai.as_ref().map_or(crate::common::game_type::INVALID_ID, |a| world.with_ai_self(me.object, |w| a.get_current_victim(w)));
                m.ai = ai;
                ModuleReply::Id(r)
            }
            ModuleCall::AiIsDoingGroundMovement => {
                let ai = m.ai.take();
                let r = ai.as_ref().is_some_and(|a| world.with_ai_self(me.object, |w| a.is_doing_ground_movement(w)));
                m.ai = ai;
                ModuleReply::Bool(r)
            }
            ModuleCall::AiGetLastCommandSource => ModuleReply::Int(m.ai().last_command_source),
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
}
