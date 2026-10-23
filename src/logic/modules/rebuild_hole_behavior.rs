// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/RebuildHoleBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/RebuildHoleBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `RebuildHoleBehavior`: the hole a GLA building leaves (`RebuildHoleExposeDie` makes it), which rebuilds the building. `startRebuildProcess` names
//! the building and the object that died; then, `WorkerRespawnDelay` frames after the last worker went, a worker (`WorkerObjectName`) appears at
//! the hole and its AI starts the reconstruction (or resumes the one under way); the attackers of the hole move to the reconstruction, which the
//! hole produced; the worker is unselectable and the hole masked; sticky bombs on the hole move to the reconstruction. Every frame the hole heals
//! `HoleHealthRegen%PerSecond` of its maximum health; when the reconstruction is finished, it takes the hole's name and the worker and the hole
//! are destroyed. A worker or a reconstruction that is gone starts the wait again. When the hole dies, it takes the worker with it.
//!
//! The calls of the die module and of the world are [`ModuleCall::StartRebuildProcess`] (the template by id) and `OnDie`; the AI's are
//! [`ModuleCall::AiConstruct`] and [`ModuleCall::AiResumeConstruction`], a sticky bomb's [`ModuleCall::StickyBombGetTargetObject`] and
//! [`ModuleCall::StickyBombSetTargetObject`]. The save's template names are resolved against the thing factory after the load
//! ([`RebuildHoleBehavior::check_loaded_templates`]).
//!
//! # Upstream facts kept
//!
//! - The wait is a `Real` number of frames stored in an `UnsignedInt` (undefined outside its range: [`Undefined::RealOutOfRange`]).
//! - The worker's template name is looked up once, the first time a worker is made, and a missing template is `ThingFactory::newObject(NULL)`'s throw.
//! - A reconstruction the AI did not make is read unchecked ([`Undefined::NullObject`]); so is the hole's body ([`Undefined::NullBodyModule`]).
//! - The ids are tested against 0, not `INVALID_ID` (the same value).
//! - A worker found gone is replaced but keeps its place in `update`: the local stays what was found, so a reconstruction that is also gone does not
//!   destroy it again, and no worker is made in the same frame.
//!
//! # Open (deviation)
//!
//! `~RebuildHoleBehavior` (the worker destroyed with the hole) is not sent: there is no hook for a module's destruction.

use std::sync::Arc;

use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KINDOF_MINE;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::object_status_types::{OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_UNSELECTABLE};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_HEALING, DEATH_NONE};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::crate_collide::crate_destroy_object;
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_state::mask_of;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class RebuildHoleBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct RebuildHoleBehaviorModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_workerRespawnDelay`: "delay in frames from death of object till respawn of worker".
    pub worker_respawn_delay: f32,
    /// `m_holeHealthRegenPercentPerSecond`: "the hole recovers this % of the max hit points per second".
    pub hole_health_regen_percent_per_second: f32,
    /// `m_workerTemplateName`.
    pub worker_template_name: Vec<u8>,
}

impl Default for RebuildHoleBehaviorModuleData {
    /// `RebuildHoleBehaviorModuleData()`: no delay, 10% a second, no worker name.
    fn default() -> Self {
        RebuildHoleBehaviorModuleData { update: UpdateModuleData::default(), worker_respawn_delay: 0.0, hole_health_regen_percent_per_second: 0.1, worker_template_name: Vec::new() }
    }
}

/// `RebuildHoleBehaviorModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<RebuildHoleBehaviorModuleData>; 4] = [
    FieldParse::new("WorkerObjectName", |ini, d, _| { d.worker_template_name = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("WorkerRespawnDelay", |ini, d, _| { d.worker_respawn_delay = ini.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("HoleHealthRegen%PerSecond", |ini, d, _| { d.hole_health_regen_percent_per_second = ini.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl RebuildHoleBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, RebuildHoleBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<RebuildHoleBehaviorModuleData> {
        let mut data = RebuildHoleBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, RebuildHoleBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// A `const ThingTemplate *`: the template's id and its name (the name is what the save holds).
pub type TemplateRef = (u16, Vec<u8>);

/// The two template names a load read (the worker's, the building's), each `None` when the load stopped before it.
type LoadedNames = (Option<Vec<u8>>, Option<Vec<u8>>);

/// `class RebuildHoleBehavior : public UpdateModule, public DieModuleInterface, public RebuildHoleBehaviorInterface`.
#[derive(Clone, Debug)]
pub struct RebuildHoleBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getRebuildHoleBehaviorModuleData()`.
    data: Arc<RebuildHoleBehaviorModuleData>,
    /// `m_workerID`: "id of the worker that will rebuild us".
    worker_id: ObjectId,
    /// `m_reconstructingID`: "ID of the object we're reconstructing".
    reconstructing_id: ObjectId,
    /// `m_spawnerObjectID`: "Object that was killed and therefore the object that created this hole".
    spawner_object_id: ObjectId,
    /// `m_workerWaitCounter`: "when this reaches zero we spawn a worker after death".
    worker_wait_counter: u32,
    /// `m_workerTemplate`: "template of the worker to make".
    worker_template: Option<TemplateRef>,
    /// `m_rebuildTemplate`: "what we are rebuilding".
    rebuild_template: Option<TemplateRef>,
    /// The template names a load read, waiting for the thing factory ([`RebuildHoleBehavior::check_loaded_templates`]): the worker's, then the
    /// building's, each `None` when the load stopped before it.
    loaded_names: Option<LoadedNames>,
}

impl PartialEq for RebuildHoleBehavior {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.state_words() == other.state_words()
            && self.worker_template == other.worker_template
            && self.rebuild_template == other.rebuild_template
            && self.loaded_names == other.loaded_names
    }
}

/// `UnsignedInt = Real`: undefined outside (-1, 2^32).
fn real_to_unsigned(x: f32) -> Option<u32> {
    if x > -1.0 && x < 4_294_967_296.0 {
        Some(x as u32)
    } else {
        None
    }
}

impl RebuildHoleBehavior {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// `RebuildHoleBehavior(thing, moduleData)`: no worker, no reconstruction, no spawner, no wait, no templates.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<RebuildHoleBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(RebuildHoleBehavior {
            update,
            data,
            worker_id: INVALID_ID,
            reconstructing_id: INVALID_ID,
            spawner_object_id: INVALID_ID,
            worker_wait_counter: 0,
            worker_template: None,
            rebuild_template: None,
            loaded_names: None,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getRebuildHoleBehaviorModuleData()`).
    pub fn module_data(&self) -> &Arc<RebuildHoleBehaviorModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's: the worker, the reconstruction, the spawner, the wait.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.worker_id as u32, self.reconstructing_id as u32, self.spawner_object_id as u32, self.worker_wait_counter]
    }

    /// `getSpawnerID()`.
    pub fn get_spawner_id(&self) -> ObjectId {
        self.spawner_object_id
    }

    /// `getReconstructedBuildingID()`.
    pub fn get_reconstructed_building_id(&self) -> ObjectId {
        self.reconstructing_id
    }

    /// `getRebuildTemplate()`: the name of the building to rebuild (`None`: `NULL`).
    pub fn get_rebuild_template(&self) -> Option<&[u8]> {
        self.rebuild_template.as_ref().map(|t| t.1.as_slice())
    }

    /// The worker's template name, as the save holds it (`None`: `NULL`).
    pub fn get_worker_template(&self) -> Option<&[u8]> {
        self.worker_template.as_ref().map(|t| t.1.as_slice())
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

    /// `xfer` with the scheduling half the slot holds: the version, that half, then the fields.
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

    /// The end of a load (`xfer`'s two `findTemplate` look-ups, which need the thing factory): each name read is found again, or the load throws
    /// `SC_INVALID_DATA` (reported as [`XferStatus::ListNotEmpty`]) there, before anything after it was read; an empty name is `NULL`. `r` is
    /// what the rest of the xfer returned.
    pub fn check_loaded_templates(&mut self, world: &World, r: XferResult<()>) -> XferResult<()> {
        let Some((worker, rebuild)) = self.loaded_names.take() else { return r };
        let find = |name: &Vec<u8>| world.thing_factory_find_template(name).map(|id| (id, name.clone()));
        if let Some(name) = worker {
            if name.is_empty() {
                self.worker_template = None;
            } else {
                match find(&name).filter(|_| !crate::logic::modules::plant(1786)) {
                    Some(t) => self.worker_template = Some(t),
                    None => return Err(XferStatus::ListNotEmpty),
                }
            }
        }
        r?;
        if let Some(name) = rebuild {
            if name.is_empty() {
                self.rebuild_template = None;
            } else {
                match find(&name) {
                    Some(t) => self.rebuild_template = Some(t),
                    None => return Err(XferStatus::ListNotEmpty),
                }
            }
        }
        Ok(())
    }

    /// `newWorkerRespawnProcess(existingWorker)`: "start the worker respawn process (again if existingWorker is non NULL)". An error is upstream's
    /// undefined behaviour.
    fn new_worker_respawn_process(&mut self, world: &mut World, me: ObjectId, existing_worker: Option<ObjectId>) -> Result<(), IniError> {
        if let Some(w) = existing_worker {
            crate_destroy_object(world, w);
        }
        self.worker_id = INVALID_ID;
        self.worker_wait_counter = real_to_unsigned(self.data.worker_respawn_delay).ok_or_else(|| IniError::undefined(Undefined::RealOutOfRange))?;
        if !crate::logic::modules::plant(1781) {
            world.crate_mask_object(me, false);
        }
        Ok(())
    }

    /// `startRebuildProcess(rebuild, spawnerID)`: the building and the spawner, then a new wait ("Kill the worker if we have one").
    fn start_rebuild_process(&mut self, world: &mut World, me: ObjectId, rebuild: Option<TemplateRef>, spawner: ObjectId) -> Result<(), IniError> {
        self.rebuild_template = rebuild;
        self.spawner_object_id = spawner;
        let worker = world.objects.find(self.worker_id).map(|_| self.worker_id);
        self.new_worker_respawn_process(world, me, worker)
    }

    /// `transferBombs(reconstruction)`: every mine whose sticky bomb sits on the hole moves to the reconstruction.
    fn transfer_bombs(world: &mut World, me: ObjectId, reconstruction: ObjectId) {
        for obj in world.objects.ids_in_order() {
            if !world.objects.find(obj).is_some_and(|o| o.is_kind_of(KINDOF_MINE)) {
                continue;
            }
            let Some(update) = world.find_module(obj, b"StickyBombUpdate") else { continue };
            let target = world.module_call(obj, update, ModuleCall::StickyBombGetTargetObject).as_id();
            // `getTargetObject()`: the target if it is still an object
            let target = if world.objects.find(target).is_some() { target } else { INVALID_ID };
            if target == me && !crate::logic::modules::plant(1782) {
                world.module_call(obj, update, ModuleCall::StickyBombSetTargetObject(reconstruction));
            }
        }
    }

    /// The body of `update()`. An error is upstream's undefined behaviour (or `newObject`'s throw).
    fn run(&mut self, world: &mut World, hole: ObjectId) -> Result<(), IniError> {
        let mut reconstructing: Option<ObjectId> = None;
        let mut worker: Option<ObjectId> = None;
        // "make sure the worker is still alive"
        if self.worker_id != 0 {
            worker = world.objects.find(self.worker_id).map(|_| self.worker_id);
            if worker.is_none() {
                self.new_worker_respawn_process(world, hole, None)?;
            }
        }
        // "make sure the reconstructing object is still alive"
        if self.reconstructing_id != 0 {
            reconstructing = world.objects.find(self.reconstructing_id).map(|_| self.reconstructing_id);
            if reconstructing.is_none() {
                self.new_worker_respawn_process(world, hole, worker)?;
                self.reconstructing_id = INVALID_ID;
            }
        }
        // "check for worker respawn"
        if worker.is_none() && self.worker_wait_counter > 0 {
            self.worker_wait_counter -= 1;
            if self.worker_wait_counter == 0 || crate::logic::modules::plant(1783) {
                if self.worker_template.is_none() {
                    let name = self.data.worker_template_name.clone();
                    self.worker_template = world.thing_factory_find_template(&name).map(|id| (id, name));
                }
                // `ThingFactory::newObject(NULL)` throws
                let template = self.worker_template.as_ref().map(|t| t.0).ok_or_else(|| IniError::new(IniErrorKind::BadArg))?;
                let team = world.get_team(hole);
                let w = world.crate_new_object(template, team);
                if w != INVALID_ID {
                    worker = Some(w);
                    let pos = world.objects.find(hole).map(|o| o.get_position()).unwrap_or_default();
                    world.set_position(w, &pos);
                    self.worker_id = w;
                    if let Some(ai) = world.objects.find(w).and_then(|o| o.ai) {
                        // "start the reconstruction or resume it if we're replacing a dead worker"
                        let r = match reconstructing {
                            None => {
                                let (pos, angle) = world.objects.find(hole).map(|o| (o.get_position(), o.thing.orientation())).unwrap_or_default();
                                let owner = world.get_controlling_player(hole);
                                let template = self.rebuild_template.as_ref().map(|t| t.0);
                                world.module_call(w, ai, ModuleCall::AiConstruct { template, pos, angle, owner, is_rebuild: true }).as_id()
                            }
                            Some(r) => {
                                world.module_call(w, ai, ModuleCall::AiResumeConstruction { object: r, command_source: CMD_FROM_AI });
                                r
                            }
                        };
                        // the reconstruction is read unchecked from here on
                        if world.objects.find(r).is_none() {
                            return Err(IniError::undefined(Undefined::NullObject));
                        }
                        reconstructing = Some(r);
                        // "transfer any attackers on the hole to the new building"
                        for obj in world.objects.ids_in_order() {
                            let Some(oai) = world.objects.find(obj).and_then(|o| o.ai) else { continue };
                            if !crate::logic::modules::plant(1784) {
                                world.module_call(obj, oai, ModuleCall::AiTransferAttack { from: hole, to: r });
                            }
                        }
                        self.reconstructing_id = r;
                        // "the reconstructing object is "produced" by the hole"
                        world.set_producer(r, hole);
                        // "the worker is not selectable"
                        world.set_status(w, mask_of(OBJECT_STATUS_UNSELECTABLE), true);
                        // "the hole is now masked"
                        world.crate_mask_object(hole, true);
                        // "transfer bombs to the new building"
                        Self::transfer_bombs(world, hole, r);
                    }
                }
            }
        }
        // "holes heal"
        let body = world.objects.find(hole).and_then(|o| o.body).ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
        let health = world.module_call(hole, body, ModuleCall::GetHealth).as_real(0.0);
        let max_health = world.module_call(hole, body, ModuleCall::GetMaxHealth).as_real(0.0);
        if health < max_health {
            let mut healing = DamageInfo::default();
            healing.input.amount = (self.data.hole_health_regen_percent_per_second / LOGICFRAMES_PER_SECOND as f32) * max_health;
            healing.input.source_id = hole;
            healing.input.damage_type = DAMAGE_HEALING;
            healing.input.death_type = DEATH_NONE;
            if !crate::logic::modules::plant(1785) {
                world.module_call(hole, body, ModuleCall::AttemptHealing(healing));
            }
        }
        // "if the reconstruction is complete, we have served our purpose"
        if let Some(r) = reconstructing {
            if !world.objects.find(r).is_some_and(|o| o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION)) {
                let name = world.objects.find(hole).map(|o| o.name.clone()).unwrap_or_default();
                world.cut(crate::logic::world::object_cuts::CutCall::ScriptEngineTransferObjectName { name, object: r });
                if let Some(w) = worker {
                    crate_destroy_object(world, w);
                }
                crate_destroy_object(world, hole);
            }
        }
        Ok(())
    }

    /// `onDie(damageInfo)`: the worker goes, then the hole.
    fn on_die(&mut self, world: &mut World, me: ObjectId) {
        if self.worker_id != INVALID_ID && world.objects.find(self.worker_id).is_some() && !crate::logic::modules::plant(1787) {
            crate_destroy_object(world, self.worker_id);
            self.worker_id = INVALID_ID;
        }
        crate_destroy_object(world, me);
    }
}

impl UpdateBehavior for RebuildHoleBehavior {
    /// "1: Initial version, 2: Added spawner id".
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(e) = self.run(world, me.object) {
            world.cuts.module_errors.push((me, e));
        }
        UPDATE_SLEEP_NONE
    }
    /// The fields after the base: the worker, the reconstruction, the spawner (version 2), the wait, the two template names (resolved by
    /// [`RebuildHoleBehavior::check_loaded_templates`] after a load).
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        let load = xfer.get_xfer_mode() == XferMode::Load;
        if load {
            self.loaded_names = Some((None, None));
        }
        xfer.xfer_object_id(&mut self.worker_id)?;
        xfer.xfer_object_id(&mut self.reconstructing_id)?;
        if version >= 2 {
            xfer.xfer_object_id(&mut self.spawner_object_id)?;
        }
        xfer.xfer_unsigned_int(&mut self.worker_wait_counter)?;
        let mut worker_name = self.worker_template.as_ref().map_or_else(Vec::new, |t| t.1.clone());
        xfer.xfer_ascii_string(&mut worker_name)?;
        if load {
            self.loaded_names = Some((Some(worker_name), None));
        }
        let mut rebuild_name = self.rebuild_template.as_ref().map_or_else(Vec::new, |t| t.1.clone());
        xfer.xfer_ascii_string(&mut rebuild_name)?;
        if load {
            if let Some(n) = &mut self.loaded_names {
                n.1 = Some(rebuild_name);
            }
        }
        Ok(())
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::StartRebuildProcess { template, spawner } => {
                let rebuild = world.statics.thing_factory.find_by_template_id(template).map(|t| (template, t.get_final_override().data.get_name().to_vec()));
                if let Err(e) = self.start_rebuild_process(world, me.object, rebuild, spawner) {
                    world.cuts.module_errors.push((me, e));
                }
                ModuleReply::Done
            }
            ModuleCall::OnDie(_) => {
                self.on_die(world, me.object);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::RebuildHoleBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::RebuildHoleBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for RebuildHoleBehavior {
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

/// The factory's `NewModuleDataProc` for `RebuildHoleBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::RebuildHoleBehavior(Arc::new(RebuildHoleBehaviorModuleData::friend_new_module_data(ini)?)))
}
