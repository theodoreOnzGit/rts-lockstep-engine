// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/GameLogic.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp
//   (`setDefaults`, `reset`, `destroyAllObjectsImmediate`, `processDestroyList`,
//   `getFirstObject`, `allocateObjectID`, `registerObject`, `destroyObject`,
//   `friend_awakenUpdateModule`, `loadPostProcess`, and the parts of `update`
//   that this wave owns)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! [`World`]: the one context that replaces `TheGameLogic`, `ThePlayerList`,
//! `TheTeamFactory`, `TheTerrainLogic` and the client singletons for the
//! logic layers. See `docs/world-design.md` for the design; this file is its
//! translation of the object-lifetime part of `GameLogic`.
//!
//! # What is translated here
//!
//! | upstream | here |
//! |---|---|
//! | `getFrame()`, `m_frame++` | [`World::frame`], [`World::end_of_frame`] |
//! | `allocateObjectID()` | [`World::allocate_object_id`] |
//! | `registerObject(obj)` | [`World::register_object`] |
//! | `findObjectByID(id)` | [`World::find_object`] |
//! | `getFirstObject()` / `getNextObject()` | [`World::first_object`] / [`World::next_object`] |
//! | `destroyObject(obj)` | [`World::destroy_object`] |
//! | `processDestroyList()` | [`World::process_destroy_list`] |
//! | `destroyAllObjectsImmediate()` | [`World::destroy_all_objects_immediate`] |
//! | `friend_awakenUpdateModule` | [`World::awaken_update_module`] |
//! | the sleepy loop of `update()` | [`World::run_sleepy_updates`] |
//! | the end-of-frame block of `update()` | [`World::end_of_frame`] |
//! | `setDefaults`, `reset` | [`World::set_defaults`], [`World::reset`] |
//! | `loadPostProcess` (ids and the update queue) | [`World::load_post_process`] |
//!
//! # Hook points
//!
//! Where upstream's function calls into a later layer the call is a
//! `hook_*` method below, a no-op today whose doc names what it will do, and
//! which the owning wave implements **in place**. Their call order is
//! pinned by a test ([`World::hook_log`]).
//!
//! # Differences from upstream, all deliberate
//!
//! - **`reset()` destroys with the id table intact.** Upstream empties
//!   `m_objVector` *first*, so during teardown no object can be found by id,
//!   and `removeObjectFromLookupTable` then writes `m_objVector[id] = NULL`
//!   beyond the 8192 entries for any id above that (undefined behaviour). The
//!   world is discarded, so what teardown code sees is not simulation state.
//! - **An object queued twice is deleted once.** `destroyObject` runs the
//!   modules' `onDestroy` *before* it sets the destroyed bit, so a module
//!   that destroys its own object from `onDestroy` queues it twice; upstream
//!   then unlinks and deletes a freed object. Here the second entry finds
//!   nothing and is skipped.
//! - **`destroyObject(NULL)`** is a no-op (an assert upstream).

use crate::common::disabled_types::DisabledMaskType;
use crate::common::object_status_types::OBJECT_STATUS_DESTROYED;
use crate::common::state_machine::{ObjectRef, StateContext};
use crate::logic::client_event::ClientEvent;
use crate::logic::module::update_module::UPDATE_SLEEP_NONE;
use crate::logic::terrain_logic::TerrainLogic;
use crate::logic::world::client_reads::ClientReads;
use crate::logic::world::events::ClientEventQueue;
use crate::logic::world::ids::{ObjectId, INVALID_ID};
use crate::logic::world::object::{ArenaError, Object, ObjectArena};
// --- p1w6-object ---
use crate::logic::world::object_cuts::ObjectCuts;
// --- end p1w6-object ---
use crate::logic::world::player::PlayerArena;
use crate::logic::world::sleepy::{SleepyScheduler, UpdateHandle, UpdateTraceEntry};
use crate::logic::world::team::TeamArena;
use crate::logic::world::update_body::UpdateBody;
// --- p1w6-partition ---
use crate::logic::world::partition::PartitionManager;
// --- end p1w6-partition ---

/// What the world refuses (each is a crash or undefined behaviour upstream).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldError {
    /// The arena refused the object.
    Arena(ArenaError),
    /// `friend_awakenUpdateModule`: "sleepy update module illegal index"
    /// (`RELEASE_CRASH`).
    SleepyIndexIllegal(UpdateHandle),
    /// `friend_awakenUpdateModule`: "sleepy update module index mismatch"
    /// (`RELEASE_CRASH`).
    SleepyIndexMismatch(UpdateHandle),
}

impl From<ArenaError> for WorldError {
    fn from(e: ArenaError) -> Self {
        WorldError::Arena(e)
    }
}

/// The calls out to later layers, in the order `destroyObject` and its
/// neighbours make them. Recorded in [`World::hook_log`] when it is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hook {
    /// `for each behaviour: getDestroy()->onDestroy()` (layer 9).
    DestroyModulesOnDestroy(ObjectId),
    /// `ai->setLocomotorGoalNone(); ai->destroyPath()` (layer 10).
    StopAi(ObjectId),
    /// `obj->onDestroy()` (layer 8).
    ObjectOnDestroy(ObjectId),
    /// `TheAI->pathfinder()->removeWallPiece(obj)` for `KINDOF_WALK_ON_TOP_OF_WALL` (layer 10).
    RemoveWallPiece(ObjectId),
    /// `TheControlBar->markUIDirty()` for a special-power object of the local player (layer 7).
    SpecialPowerUi(ObjectId),
    /// `~Object()`, run by `friend_deleteInstance` (layer 8).
    DeleteInstance(ObjectId),
    /// `obj->checkDisabledStatus()` (layer 8).
    CheckDisabledStatus(ObjectId),
}

/// The world. See the module docs and `docs/world-design.md`.
#[derive(Clone, Debug)]
pub struct World {
    /// `m_objList` + `m_objVector`.
    pub objects: ObjectArena,
    /// `m_sleepyUpdates` + `m_curUpdateModule`.
    pub sched: SleepyScheduler,
    /// `ThePlayerList`.
    pub players: PlayerArena,
    /// `TheTeamFactory`.
    pub teams: TeamArena,
    /// `TheTerrainLogic`.
    pub terrain: TerrainLogic,
    /// The client-notification queue.
    pub events: ClientEventQueue,
    /// The values logic reads from the client.
    pub reads: ClientReads,
    // --- p1w6-partition ---
    /// `ThePartitionManager`: the spatial grid and the shroud. Checked out with
    /// `std::mem::take` for its own `update` (see `world::partition`).
    pub partition: PartitionManager,
    // --- end p1w6-partition ---
    /// When `Some`, the hooks append themselves (see [`Hook`]).
    pub hook_log: Option<Vec<Hook>>,
    // --- p1w6-object ---
    /// The cut points of `Object` (layer 8): the tables and logs behind every
    /// call it makes into a unit that is not translated (`object_cuts.rs`).
    pub cuts: ObjectCuts,
    // --- end p1w6-object ---
    // --- p1w7-sonnet ---
    /// `TheCaveSystem`: a tracker per cave index. See `world::cave_system`.
    pub cave_system: crate::logic::world::cave_system::CaveSystem,
    // --- end p1w7-sonnet ---
    // --- p1l9-example ---
    /// The INI stores upstream reads through globals (`TheUpgradeCenter`, ...),
    /// read-only once the game runs. See `world::statics`.
    pub statics: std::sync::Arc<crate::logic::world::statics::GameStatics>,
    /// `TheNameKeyGenerator`: the generator the INI load used (modules assign
    /// keys at run time, so it is not one of the statics).
    pub names: crate::common::name_key_generator::NameKeyGenerator,
    // --- end p1l9-example ---
    /// `m_frame`.
    frame: u32,
    /// `m_nextObjID`.
    next_object_id: ObjectId,
    /// `m_objectsToDestroy`.
    objects_to_destroy: Vec<ObjectId>,
    /// `m_startNewGame`.
    start_new_game: bool,
    /// `m_isInUpdate`.
    in_update: bool,
    /// The object whose module is running, for state-machine callbacks.
    owner: Option<ObjectId>,
}

impl Default for World {
    fn default() -> Self {
        World::new()
    }
}

impl World {
    /// A world as `GameLogic` is after `init()`: frame 0, the first object id
    /// is 1, nothing registered.
    pub fn new() -> World {
        World {
            objects: ObjectArena::new(),
            sched: SleepyScheduler::new(),
            players: PlayerArena::new(),
            teams: TeamArena::new(),
            terrain: TerrainLogic::default(),
            events: ClientEventQueue::new(),
            reads: ClientReads::default(),
            // --- p1w6-partition ---
            partition: PartitionManager::new(),
            // --- end p1w6-partition ---
            hook_log: None,
            // --- p1w6-object ---
            cuts: ObjectCuts::default(),
            // --- end p1w6-object ---
            // --- p1w7-sonnet ---
            cave_system: crate::logic::world::cave_system::CaveSystem::new(),
            // --- end p1w7-sonnet ---
            // --- p1l9-example ---
            statics: std::sync::Arc::new(crate::logic::world::statics::GameStatics::new()),
            names: crate::common::name_key_generator::NameKeyGenerator::new(),
            // --- end p1l9-example ---
            frame: 0,
            next_object_id: 1,
            objects_to_destroy: Vec::new(),
            start_new_game: false,
            in_update: false,
            owner: None,
        }
    }

    // ---- the clock -------------------------------------------------------

    /// `TheGameLogic->getFrame()`.
    pub fn frame(&self) -> u32 {
        self.frame
    }

    /// `xferUnsignedInt(&m_frame)` on a load: the frame a save was taken at.
    /// (`setDefaults` sets it to 0 first, see [`World::set_defaults`].)
    pub fn set_frame(&mut self, frame: u32) {
        self.frame = frame;
    }

    /// `m_startNewGame`: while set, the frame counter does not advance.
    pub fn set_start_new_game(&mut self, v: bool) {
        self.start_new_game = v;
    }

    /// `isInGameLogicUpdate()`.
    pub fn is_in_game_logic_update(&self) -> bool {
        self.in_update
    }

    // ---- ids and lookup --------------------------------------------------

    /// `allocateObjectID()`: the counter, then the counter plus one
    /// (`UnsignedInt` arithmetic).
    pub fn allocate_object_id(&mut self) -> ObjectId {
        let ret = self.next_object_id;
        self.next_object_id = (self.next_object_id as u32).wrapping_add(1) as ObjectId;
        ret
    }

    /// `m_nextObjID` (the id the next allocation returns).
    pub fn next_object_id(&self) -> ObjectId {
        self.next_object_id
    }

    /// `findObjectByID(id)`. Finds destroyed objects, see [`ObjectArena`].
    pub fn find_object(&self, id: ObjectId) -> Option<&Object> {
        self.objects.find(id)
    }

    /// `findObjectByID(id)` for a caller that changes the object.
    pub fn find_object_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.find_mut(id)
    }

    /// `getFirstObject()`: the newest object's id, `INVALID_ID` if none.
    pub fn first_object(&self) -> ObjectId {
        self.objects.first_id()
    }

    /// `obj->getNextObject()`.
    pub fn next_object(&self, id: ObjectId) -> ObjectId {
        self.objects.next_id(id)
    }

    /// The objects waiting for `processDestroyList`, in the order queued.
    pub fn objects_to_destroy(&self) -> &[ObjectId] {
        &self.objects_to_destroy
    }

    // ---- registration ----------------------------------------------------

    /// `registerObject(obj)`: prepend to the object list, add to the lookup
    /// table, then queue each update module: a module that never set its wake
    /// frame is due now (frame 1 at frame 0), and every module goes on the
    /// sleepy heap.
    pub fn register_object(&mut self, obj: Object) -> Result<(), WorldError> {
        let id = obj.id;
        self.objects.insert_at_head(obj)?;
        self.queue_update_modules(id);
        Ok(())
    }

    // --- p1w6-object ---
    /// The second half of `registerObject(obj)` for an object that is already
    /// in the lookup table (`Object::Object` adds it with `setID`, long before
    /// it registers itself): prepend it to the object list, then queue its
    /// update modules.
    pub fn link_registered_object(&mut self, id: ObjectId) -> Result<(), WorldError> {
        self.objects.link_at_head(id)?;
        self.queue_update_modules(id);
        Ok(())
    }

    /// The loop of `registerObject`: a module that never set its wake frame is
    /// due now (frame 1 at frame 0), and every update module goes on the
    /// sleepy heap.
    fn queue_update_modules(&mut self, id: ObjectId) {
        let modules = self.objects.find(id).map_or(0, |o| o.behaviors.len());
        let mut now = self.frame;
        if now == 0 {
            now = 1;
        }
        for index in 0..modules {
            let u = UpdateHandle { object: id, index: index as u16 };
            if let Some(m) = self.objects.update_module_mut(u) {
                // "note that 'when' can be zero here for any update module that didn't bother
                // to call setWakeFrame() in its ctor. this is legal."
                if m.friend_get_next_call_frame() == 0 {
                    m.friend_set_next_call_frame(now);
                }
            }
            self.sched.push(&mut self.objects, u);
        }
    }

    /// `setID` of a queued object: the destroy list holds the new id.
    pub(crate) fn rename_in_destroy_list(&mut self, old: ObjectId, new: ObjectId) {
        for id in &mut self.objects_to_destroy {
            if *id == old {
                *id = new;
            }
        }
    }
    // --- end p1w6-object ---

    // ---- destruction -----------------------------------------------------

    fn note(&mut self, h: Hook) {
        if let Some(log) = &mut self.hook_log {
            log.push(h);
        }
    }

    /// `destroyObject(obj)`: the only way to destroy an object. Ignored for a
    /// missing or already destroyed object. Runs, in upstream's order: the
    /// modules' `onDestroy`; marks the object destroyed; stops its AI; queues
    /// it for the end of the frame; `Object::onDestroy`; wall-piece removal;
    /// the special-power UI refresh. The object stays in the list and the table
    /// until [`World::process_destroy_list`].
    pub fn destroy_object(&mut self, id: ObjectId) {
        match self.objects.find(id) {
            Some(o) if !o.is_destroyed() => {}
            _ => return,
        }
        self.hook_destroy_modules_on_destroy(id);
        // --- p1w6-object ---
        // `setStatus( MAKE_OBJECT_STATUS_MASK( OBJECT_STATUS_DESTROYED ) )`, with its reactions
        self.set_status(id, crate::logic::world::object_state::mask_of(OBJECT_STATUS_DESTROYED), true);
        // --- end p1w6-object ---
        self.hook_stop_ai(id);
        // "add to end of destruction list, in case something is being destroyed and trying to
        // destroy subobjects"
        self.objects_to_destroy.push(id);
        self.hook_object_on_destroy(id);
        self.hook_remove_wall_piece(id);
        self.hook_special_power_ui(id);
    }

    /// `processDestroyList()`: for each queued object, in order, and including
    /// those queued while this runs: erase its sleepy updates, unlink it from
    /// the list, remove it from the table, delete it. Then clear the queue.
    pub fn process_destroy_list(&mut self) {
        let mut i = 0;
        while i < self.objects_to_destroy.len() {
            let id = self.objects_to_destroy[i];
            self.sched.erase_object_updates(&mut self.objects, id);
            if let Some(obj) = self.objects.remove(id) {
                self.hook_delete_instance(obj);
            }
            i += 1;
        }
        self.objects_to_destroy.clear();
    }

    /// `destroyAllObjectsImmediate()`: destroy every object (newest first,
    /// each next pointer read before the object is destroyed), then process
    /// the list.
    pub fn destroy_all_objects_immediate(&mut self) {
        for id in self.objects.ids_in_order() {
            self.destroy_object(id);
        }
        self.process_destroy_list();
    }

    // ---- the update queue ------------------------------------------------

    /// `friend_awakenUpdateModule(obj, u, whenToWakeUp)`: what `setWakeFrame`
    /// calls. Ignored for the module that is running (its return value wins),
    /// for a wake at the frame it already has, and for "wake next frame" when
    /// the module is due this frame. Otherwise the module's frame changes and
    /// the heap is rebalanced around its index.
    ///
    /// An object that is not registered is not in the arena, so there is
    /// nothing to change: the constructor of such a module sets its own frame
    /// before `registerObject` (which upstream's `else` branch does through
    /// the pointer).
    pub fn awaken_update_module(&mut self, obj: ObjectId, u: UpdateHandle, when: u32) -> Result<(), WorldError> {
        let now = self.frame;
        if self.sched.current() == Some(u) {
            // "You should not call setWakeFrame() from inside your update(), because it will
            // be ignored, in favor of the return code from update."
            return Ok(());
        }
        let (next_call, idx) = match self.objects.update_module(u) {
            Some(m) => (m.friend_get_next_call_frame(), m.friend_get_index_in_logic()),
            None => return Ok(()),
        };
        if when == next_call {
            return Ok(());
        }
        if now > 0 && next_call == now && when == now.wrapping_add(1) {
            // already awake: do not push "as soon as possible" past this frame
            return Ok(());
        }
        if self.objects.is_in_list(obj) {
            if idx < 0 || idx as usize >= self.sched.len() {
                return Err(WorldError::SleepyIndexIllegal(u));
            }
            if self.sched.heap()[idx as usize] != u {
                return Err(WorldError::SleepyIndexMismatch(u));
            }
            if let Some(m) = self.objects.update_module_mut(u) {
                m.friend_set_next_call_frame(when);
            }
            self.sched.rebalance(&mut self.objects, idx);
        } else {
            if idx != -1 {
                return Err(WorldError::SleepyIndexMismatch(u));
            }
            if let Some(m) = self.objects.update_module_mut(u) {
                m.friend_set_next_call_frame(when);
            }
        }
        Ok(())
    }

    /// Runs `f` on the behaviour half of module `u`, checked out of its slot
    /// (see `docs/world-design.md`, "Re-entrancy"). `None` if the module is
    /// missing or already checked out, in which case `f` does not run. The
    /// behaviour is put back afterwards.
    pub fn with_body<R>(&mut self, u: UpdateHandle, f: impl FnOnce(&mut UpdateBody, &mut World) -> R) -> Option<R> {
        let mut body = self.objects.take_body(u)?;
        let r = f(&mut body, self);
        self.objects.put_body(u, body);
        Some(r)
    }

    /// The object whose module is running, or whom the caller named with
    /// [`World::with_owner`]: `getMachineOwner()` for a state callback.
    pub fn owner(&self) -> Option<ObjectId> {
        self.owner
    }

    /// Runs `f` with [`World::owner`] set to `id`, restoring the previous
    /// owner afterwards.
    pub fn with_owner<R>(&mut self, id: ObjectId, f: impl FnOnce(&mut World) -> R) -> R {
        let prev = self.owner.replace(id);
        let r = f(self);
        self.owner = prev;
        r
    }

    /// The module loop of `GameLogic::update`: while the top of the heap is
    /// due (`nextCallFrame <= now`), pop nothing, call it (unless its object
    /// is disabled and the module does not run disabled), set its next call to
    /// `now + the returned sleep` (a return below 1 is
    /// `UPDATE_SLEEP_NONE`) and rebalance **index 0**, whichever module now
    /// sits there.
    pub fn run_sleepy_updates(&mut self) {
        let now = self.frame;
        while let Some(u) = self.sched.peek() {
            let (priority, next_call, types) = {
                let m = self.objects.update_module(u).expect("sleepy heap entry names an update module in the arena");
                (m.friend_get_priority(), m.friend_get_next_call_frame(), m.get_disabled_types_to_process())
            };
            // "we're done, everyone else is sleeping. break from the loop BEFORE we pop this item off."
            if next_call > now {
                break;
            }
            let mut sleep_len = UPDATE_SLEEP_NONE;
            let dis: DisabledMaskType = self.objects.find(u.object).map_or(DisabledMaskType::new(), |o| o.disabled);
            if !dis.any() || dis.any_intersection_with(&types) {
                if let Some(trace) = &mut self.sched.trace {
                    trace.push(UpdateTraceEntry { frame: now, handle: u, priority });
                }
                self.sched.set_current(Some(u));
                let prev_owner = self.owner.replace(u.object);
                if let Some(s) = self.with_body(u, |b, w| b.update(w, u)) {
                    sleep_len = s;
                    // "you may not return 0 from update"; the enum is signed
                    if (sleep_len.0 as i32) < 1 {
                        sleep_len = UPDATE_SLEEP_NONE;
                    }
                }
                self.owner = prev_owner;
                self.sched.set_current(None);
            }
            // else defer it till next frame and re-push it
            let wake = now.wrapping_add(sleep_len.0);
            if let Some(m) = self.objects.update_module_mut(u) {
                m.friend_set_next_call_frame(wake);
            }
            self.sched.rebalance(&mut self.objects, 0);
        }
    }

    // ---- the frame -------------------------------------------------------

    /// The end-of-frame block of `GameLogic::update`: destroy everything
    /// queued, then (layer 12 adds the command list reset and the store
    /// updates here) re-check the disabled status of every disabled object in
    /// list order, then advance the frame unless a new game is starting.
    pub fn end_of_frame(&mut self) {
        self.process_destroy_list();
        for id in self.objects.ids_in_order() {
            let disabled = self.objects.find(id).is_some_and(|o| o.is_disabled());
            if disabled {
                self.hook_check_disabled_status(id);
            }
        }
        if !self.start_new_game {
            self.frame = self.frame.wrapping_add(1);
        }
    }

    /// The part of `GameLogic::update` this wave owns, in upstream's order:
    /// the camera freeze (the frame does not advance), the module loop, the
    /// end of frame. Layer 12 places the script engine, terrain, CRC, AI,
    /// partition and the rest around these calls. Returns whether the frame ran.
    pub fn run_logic_frame(&mut self) -> bool {
        let was = self.in_update;
        self.in_update = true;
        let ran = if self.reads.camera_freezes_time() {
            false
        } else {
            self.run_sleepy_updates();
            self.end_of_frame();
            true
        };
        self.in_update = was;
        ran
    }

    // ---- reset and load --------------------------------------------------

    /// `setDefaults(loadingSaveGame)`: frame 0, no objects, no sleepy
    /// updates, and the id counter back to 1 unless a save is being loaded.
    pub fn set_defaults(&mut self, loading_save_game: bool) {
        self.frame = 0;
        self.sched.clear(&mut self.objects);
        self.objects.clear();
        if !loading_save_game {
            self.next_object_id = 1;
        }
    }

    /// `reset()`'s world part: destroy every object immediately, ids from 1.
    /// (The subsystems `reset()` also resets are the other layers' to call.)
    pub fn reset(&mut self) {
        self.destroy_all_objects_immediate();
        self.next_object_id = 1;
    }

    /// `loadPostProcess`'s world part: the id counter becomes the highest id
    /// in the list plus one (0 if none), the sleepy list is rebuilt from the
    /// objects in list order (a module whose frame is 0, "legacy save files",
    /// is due now), and the heap is built in one pass.
    pub fn load_post_process(&mut self) {
        let mut next = INVALID_ID;
        for id in self.objects.ids_in_order() {
            if id >= next {
                next = (id as u32).wrapping_add(1) as ObjectId;
            }
        }
        self.next_object_id = next;
        self.sched.clear(&mut self.objects);
        let mut now = self.frame;
        if now == 0 {
            now = 1;
        }
        let mut handles = Vec::new();
        for id in self.objects.ids_in_order() {
            let n = self.objects.find(id).map_or(0, |o| o.behaviors.len());
            for index in 0..n {
                handles.push(UpdateHandle { object: id, index: index as u16 });
            }
        }
        for u in handles {
            if let Some(m) = self.objects.update_module_mut(u) {
                if m.friend_get_next_call_frame() == 0 {
                    m.friend_set_next_call_frame(now);
                }
            }
            self.sched.push_unsorted(&mut self.objects, u);
        }
        self.sched.remake(&mut self.objects);
    }

    // ---- the cut points --------------------------------------------------

    /// Moves the terrain's pending visual events onto the client queue. Call
    /// after anything that calls into [`World::terrain`].
    pub fn flush_terrain_events(&mut self) {
        let ev = self.terrain.take_events();
        self.events.extend(ev.into_iter().map(ClientEvent::Terrain));
    }

    // --- p1w6-partition ---
    /// Moves the partition manager's pending client calls (shroud writes, ghost
    /// object calls) onto the client queue, in order.
    pub fn flush_partition_events(&mut self) {
        let ev = self.partition.take_client_events();
        self.events.extend(ev.into_iter().map(ClientEvent::Partition));
    }
    // --- end p1w6-partition ---

    /// The client's call after each logic frame: every pending event.
    pub fn take_client_events(&mut self) -> Vec<ClientEvent> {
        self.flush_terrain_events();
        // --- p1w6-partition ---
        self.flush_partition_events();
        // --- end p1w6-partition ---
        self.events.drain()
    }

    // ---- hooks (filled in by later layers) ---------------------------------

    /// `destroyObject`: each behaviour's `DestroyModuleInterface::onDestroy()`.
    /// Layer 9.
    pub fn hook_destroy_modules_on_destroy(&mut self, id: ObjectId) {
        self.note(Hook::DestroyModulesOnDestroy(id));
        // --- p1w6-object ---
        self.call_destroy_modules(id);
        // --- end p1w6-object ---
    }

    /// `destroyObject`: `ai->setLocomotorGoalNone(); ai->destroyPath()`.
    /// Layer 10.
    pub fn hook_stop_ai(&mut self, id: ObjectId) {
        self.note(Hook::StopAi(id));
        // --- p1w6-object ---
        if self.objects.find(id).is_some_and(|o| o.ai.is_some()) {
            // "We desperately need to stop here, or else the destructor of the statemachine will try to do
            // stopping logic" (`setLocomotorGoalNone`, `destroyPath`)
            self.cut(crate::logic::world::object_cuts::CutCall::AiStopForDestroy { object: id });
        }
        // --- end p1w6-object ---
    }

    /// `destroyObject`: `obj->onDestroy()`. Layer 8.
    pub fn hook_object_on_destroy(&mut self, id: ObjectId) {
        self.note(Hook::ObjectOnDestroy(id));
        // --- p1w6-object ---
        self.on_destroy(id);
        // --- end p1w6-object ---
    }

    /// `destroyObject`: `if (obj->isKindOf(KINDOF_WALK_ON_TOP_OF_WALL))
    /// TheAI->pathfinder()->removeWallPiece(obj)`. Layer 10.
    pub fn hook_remove_wall_piece(&mut self, id: ObjectId) {
        self.note(Hook::RemoveWallPiece(id));
        // --- p1w6-object ---
        if self.objects.find(id).is_some_and(|o| o.is_kind_of(crate::common::kind_of::KINDOF_WALK_ON_TOP_OF_WALL)) {
            self.cut(crate::logic::world::object_cuts::CutCall::PathfinderRemoveWallPiece { object: id });
        }
        // --- end p1w6-object ---
    }

    /// `destroyObject`: `if (obj->hasAnySpecialPower() && local player ==
    /// controlling player) TheControlBar->markUIDirty()`. Layer 7 (a
    /// [`ClientEvent`]).
    pub fn hook_special_power_ui(&mut self, id: ObjectId) {
        self.note(Hook::SpecialPowerUi(id));
        // --- p1w6-object ---
        // "Clean up special power shortcut bars"
        if self.objects.find(id).is_some_and(|o| o.has_any_special_power()) && self.players.local_index() == self.get_controlling_player(id) {
            self.emit(crate::logic::world::object_events::ObjectClientEvent::ControlBarMarkUiDirty);
        }
        // --- end p1w6-object ---
    }

    /// `processDestroyList`: `friend_deleteInstance()`, i.e. `~Object()`, which
    /// among much else calls `sendObjectDestroyed` (the client destroys the
    /// drawable). The object is already out of the list and the table. Layer 8
    /// extends the body; the event stays.
    pub fn hook_delete_instance(&mut self, obj: Object) {
        self.note(Hook::DeleteInstance(obj.id));
        // --- p1w6-object ---
        // `~Object`, which sends `ObjectDestroyed` where `sendObjectDestroyed` is called
        self.delete_instance(obj);
        // --- end p1w6-object ---
    }

    /// End of frame: `obj->checkDisabledStatus()` for a disabled object.
    /// Layer 8.
    pub fn hook_check_disabled_status(&mut self, id: ObjectId) {
        self.note(Hook::CheckDisabledStatus(id));
        // --- p1w6-object ---
        self.check_disabled_status(id);
        // --- end p1w6-object ---
    }
}

impl StateContext for World {
    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32 {
        self.frame
    }

    /// `TheGameLogic->findObjectByID(id)`, as the state machine sees it.
    fn find_object(&self, id: ObjectId) -> Option<ObjectRef> {
        self.objects.find(id).map(|o| ObjectRef { id: o.id, position: o.position })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::disabled_types::DISABLED_HELD;
    use crate::logic::module::update_module::SleepyUpdatePhase;
    use crate::logic::module::update_module::SleepyUpdatePhase::{Final, Initial, Normal, Physics};
    use crate::logic::world::update_body::{probe_object, ProbeAction, ProbeModuleSpec, ProbeStep};

    fn step(sleep: u32, actions: Vec<ProbeAction>) -> ProbeStep {
        ProbeStep { actions, sleep }
    }

    fn spec(phase: SleepyUpdatePhase, initial: u32, script: Vec<ProbeStep>) -> ProbeModuleSpec {
        ProbeModuleSpec { phase, initial_call_frame: initial, script }
    }

    fn add(w: &mut World, specs: &[ProbeModuleSpec]) -> ObjectId {
        let id = w.allocate_object_id();
        w.register_object(probe_object(id, specs)).unwrap();
        id
    }

    fn traced() -> World {
        let mut w = World::new();
        w.sched.trace = Some(Vec::new());
        w
    }

    fn order(w: &World) -> Vec<(u32, ObjectId, u16)> {
        w.sched.trace.as_ref().unwrap().iter().map(|e| (e.frame, e.handle.object, e.handle.index)).collect()
    }

    fn heap_is_valid(w: &World) -> bool {
        let h = w.sched.heap();
        h.iter().enumerate().all(|(i, u)| {
            let m = w.objects.update_module(*u).unwrap();
            let ok_index = m.friend_get_index_in_logic() == i as i32;
            let ok_order = i == 0 || w.objects.update_module(h[i.div_ceil(2) - 1]).unwrap().friend_get_priority() <= m.friend_get_priority();
            ok_index && ok_order
        })
    }

    #[test]
    fn ids_count_up_from_one_and_wrap_like_unsigned() {
        let mut w = World::new();
        assert_eq!((w.allocate_object_id(), w.allocate_object_id()), (1, 2));
        assert_eq!(w.next_object_id(), 3);
        w.next_object_id = -1;
        assert_eq!(w.allocate_object_id(), -1);
        assert_eq!(w.next_object_id(), 0, "UnsignedInt 0xffffffff + 1");
    }

    #[test]
    fn registration_prepends_and_queues_every_module_due_now() {
        let mut w = World::new();
        let a = add(&mut w, &[spec(Normal, 0, vec![]), spec(Initial, 0, vec![])]);
        let b = add(&mut w, &[spec(Normal, 7, vec![])]);
        assert_eq!((w.first_object(), w.next_object(b)), (b, a));
        assert_eq!(w.next_object(a), INVALID_ID);
        assert_eq!(w.sched.len(), 3);
        // frame 0 counts as frame 1; a module that set a frame keeps it
        let frame_of = |w: &World, o: ObjectId, i: u16| w.objects.update_module(UpdateHandle { object: o, index: i }).unwrap().friend_get_next_call_frame();
        assert_eq!((frame_of(&w, a, 0), frame_of(&w, a, 1), frame_of(&w, b, 0)), (1, 1, 7));
        // the Initial-phase module of a has the smallest key: (1 << 2) | 0
        assert_eq!(w.sched.peek(), Some(UpdateHandle { object: a, index: 1 }));
        assert!(heap_is_valid(&w));
        let again = probe_object(a, &[]);
        assert_eq!(w.register_object(again), Err(WorldError::Arena(ArenaError::DuplicateId(a))));
    }

    #[test]
    fn updates_run_in_key_order_and_sleep_by_their_return() {
        let mut w = traced();
        let a = add(&mut w, &[spec(Final, 0, vec![step(2, vec![])])]);
        let b = add(&mut w, &[spec(Initial, 0, vec![step(1, vec![])])]);
        let c = add(&mut w, &[spec(Normal, 0, vec![step(5, vec![])])]);
        for _ in 0..4 {
            assert!(w.run_logic_frame());
        }
        // Registration at frame 0 makes everything due at frame 1, so frame 0 runs nothing.
        // Frame 1: by phase (Initial b, Normal c, Final a). Frame 2: b. Frame 3: b (phase 0) before a (phase 3).
        assert_eq!(order(&w), [(1, b, 0), (1, c, 0), (1, a, 0), (2, b, 0), (3, b, 0), (3, a, 0)]);
        assert_eq!(w.frame(), 4);
        assert!(heap_is_valid(&w));
    }

    #[test]
    fn a_return_below_one_is_sleep_none_and_forever_is_clipped() {
        let mut w = traced();
        let a = add(&mut w, &[spec(Normal, 0, vec![step(0, vec![]), step(u32::MAX, vec![]), step(0x7fff_ffff, vec![])])]);
        for _ in 0..5 {
            w.run_logic_frame();
        }
        // call 1 returns 0 -> next frame; call 2 returns -1 (as a signed enum) -> next frame; call 3 sleeps ~forever
        assert_eq!(order(&w), [(1, a, 0), (2, a, 0), (3, a, 0)]);
        let m = w.objects.update_module(UpdateHandle { object: a, index: 0 }).unwrap();
        assert_eq!(m.friend_get_next_call_frame(), 0x3fff_ffff);
    }

    #[test]
    fn a_disabled_object_is_skipped_and_retried_next_frame() {
        let mut w = traced();
        let a = add(&mut w, &[spec(Normal, 0, vec![step(3, vec![])])]);
        // (p1w6-object: the end-of-frame pass is `checkDisabledStatus` now, which clears a type
        // whose frame has passed, so the type is disabled until a frame far away, as `setDisabled` does)
        w.set_disabled(a, DISABLED_HELD);
        w.run_logic_frame();
        w.run_logic_frame();
        assert!(order(&w).is_empty());
        assert_eq!(w.sched.len(), 1, "still queued");
        w.find_object_mut(a).unwrap().disabled.clear();
        w.run_logic_frame();
        assert_eq!(order(&w), [(2, a, 0)]);
    }

    #[test]
    fn destroy_runs_its_steps_in_upstream_order_and_defers_removal() {
        let mut w = World::new();
        w.hook_log = Some(Vec::new());
        let a = add(&mut w, &[spec(Normal, 0, vec![])]);
        w.destroy_object(a);
        assert_eq!(
            w.hook_log.take().unwrap(),
            [Hook::DestroyModulesOnDestroy(a), Hook::StopAi(a), Hook::ObjectOnDestroy(a), Hook::RemoveWallPiece(a), Hook::SpecialPowerUi(a)]
        );
        assert!(w.find_object(a).unwrap().is_destroyed(), "marked, still found");
        assert_eq!(w.objects_to_destroy(), [a]);
        assert_eq!(w.sched.len(), 1);
        w.hook_log = Some(Vec::new());
        w.destroy_object(a);
        w.destroy_object(INVALID_ID);
        w.destroy_object(999);
        assert!(w.hook_log.take().unwrap().is_empty(), "already destroyed / missing: ignored");
        w.process_destroy_list();
        assert!(w.find_object(a).is_none());
        assert!(w.sched.is_empty() && w.objects.is_empty() && w.objects_to_destroy().is_empty());
        assert_eq!(w.take_client_events(), [ClientEvent::ObjectDestroyed { object_id: a }]);
    }

    #[test]
    fn an_object_destroyed_during_the_frame_dies_at_its_end_not_before() {
        let mut w = traced();
        let victim = add(&mut w, &[spec(Final, 0, vec![step(1, vec![])])]);
        let killer = add(&mut w, &[spec(Initial, 0, vec![step(9, vec![ProbeAction::Destroy(victim)])])]);
        w.run_logic_frame(); // frame 0: nothing is due before frame 1
        w.run_logic_frame(); // frame 1: the killer destroys the victim, which is due this frame too
        assert_eq!(order(&w), [(1, killer, 0), (1, victim, 0)], "the victim still ran in the frame it was destroyed in");
        assert!(w.find_object(victim).is_none(), "gone after the end-of-frame purge");
        assert!(w.find_object(killer).is_some());
        assert_eq!(w.sched.len(), 1);
        assert!(heap_is_valid(&w));
    }

    #[test]
    fn processing_the_destroy_list_visits_entries_queued_while_it_runs() {
        // An object queued twice (a module destroyed its own object from onDestroy, before the
        // destroyed bit is set) is deleted once, and an entry added to the end is reached in the same pass.
        let mut w = World::new();
        let a = add(&mut w, &[]);
        let b = add(&mut w, &[]);
        w.destroy_object(a);
        w.objects_to_destroy.push(a);
        w.objects_to_destroy.push(b);
        w.process_destroy_list();
        assert!(w.find_object(a).is_none() && w.find_object(b).is_none());
        assert!(w.objects_to_destroy().is_empty());
        assert_eq!(w.take_client_events().len(), 2, "one deletion per object");
    }

    #[test]
    fn destroy_all_destroys_every_object_and_empties_the_world() {
        let mut w = World::new();
        for _ in 0..5 {
            add(&mut w, &[spec(Normal, 0, vec![]), spec(Physics, 0, vec![])]);
        }
        assert_eq!(w.sched.len(), 10);
        w.destroy_all_objects_immediate();
        assert!(w.objects.is_empty() && w.sched.is_empty());
        assert_eq!(w.take_client_events().len(), 5);
        w.reset();
        assert_eq!(w.next_object_id(), 1);
    }

    #[test]
    fn awaken_follows_the_four_rules() {
        let mut w = World::new();
        let a = add(&mut w, &[spec(Normal, 50, vec![])]);
        let u = UpdateHandle { object: a, index: 0 };
        let frame = |w: &World| w.objects.update_module(u).unwrap().friend_get_next_call_frame();
        // a change moves the module and keeps the heap valid
        w.awaken_update_module(a, u, 20).unwrap();
        assert_eq!(frame(&w), 20);
        // the same frame: nothing
        w.awaken_update_module(a, u, 20).unwrap();
        // "already awake, asked to wake next frame": not moved (only once frame > 0)
        w.frame = 20;
        w.awaken_update_module(a, u, 21).unwrap();
        assert_eq!(frame(&w), 20);
        w.frame = 0;
        w.awaken_update_module(a, u, 21).unwrap();
        assert_eq!(frame(&w), 21, "at frame 0 the exception does not apply");
        // the running module's request is ignored
        w.sched.set_current(Some(u));
        w.awaken_update_module(a, u, 99).unwrap();
        assert_eq!(frame(&w), 21);
        w.sched.set_current(None);
        // an object that is not registered: nothing to change
        let ghost = UpdateHandle { object: 77, index: 0 };
        assert_eq!(w.awaken_update_module(77, ghost, 5), Ok(()));
        // a corrupted index is the RELEASE_CRASH
        w.objects.update_module_mut(u).unwrap().friend_set_index_in_logic(9);
        assert_eq!(w.awaken_update_module(a, u, 30), Err(WorldError::SleepyIndexIllegal(u)));
        w.objects.update_module_mut(u).unwrap().friend_set_index_in_logic(-1);
        assert_eq!(w.awaken_update_module(a, u, 30), Err(WorldError::SleepyIndexIllegal(u)));
    }

    #[test]
    fn a_module_can_wake_another_and_a_spawn_joins_the_queue() {
        let mut w = traced();
        let sleeper = add(&mut w, &[spec(Normal, 100, vec![step(1, vec![])])]);
        let target = UpdateHandle { object: sleeper, index: 0 };
        let spawn = vec![spec(Initial, 0, vec![step(50, vec![])])];
        let waker = add(&mut w, &[spec(Normal, 0, vec![step(40, vec![ProbeAction::Awaken { target, delay: 1 }, ProbeAction::Spawn(spawn)])])]);
        for _ in 0..3 {
            w.run_logic_frame();
        }
        // frame 1: the waker wakes the sleeper for frame 2 and spawns an object (id 3) due now (frame 1 at frame 1... = now)
        let spawned = sleeper + 2;
        assert_eq!(order(&w), [(1, waker, 0), (1, spawned, 0), (2, sleeper, 0)]);
        assert!(heap_is_valid(&w));
        assert_eq!(w.first_object(), spawned, "newest first");
    }

    #[test]
    fn the_disabled_pass_visits_disabled_objects_newest_first() {
        let mut w = World::new();
        let a = add(&mut w, &[]);
        let _b = add(&mut w, &[]);
        let c = add(&mut w, &[]);
        w.find_object_mut(a).unwrap().disabled.set(DISABLED_HELD, 1);
        w.find_object_mut(c).unwrap().disabled.set(DISABLED_HELD, 1);
        w.hook_log = Some(Vec::new());
        w.end_of_frame();
        assert_eq!(w.hook_log.take().unwrap(), [Hook::CheckDisabledStatus(c), Hook::CheckDisabledStatus(a)]);
        assert_eq!(w.frame(), 1);
        w.set_start_new_game(true);
        w.end_of_frame();
        assert_eq!(w.frame(), 1, "no increment while m_startNewGame");
    }

    #[test]
    fn the_camera_freeze_stops_the_frame() {
        let mut w = traced();
        add(&mut w, &[spec(Normal, 0, vec![])]);
        w.reads.camera_time_frozen = true;
        w.reads.camera_movement_finished = false;
        assert!(!w.run_logic_frame());
        assert_eq!(w.frame(), 0);
        assert!(order(&w).is_empty());
        w.reads.camera_movement_finished = true;
        assert!(w.run_logic_frame());
        assert!(!w.is_in_game_logic_update());
    }

    #[test]
    fn load_post_process_recovers_the_id_counter_and_rebuilds_the_queue() {
        let mut w = World::new();
        for i in 0..6u32 {
            add(&mut w, &[spec(Normal, 10 + i % 3, vec![]), spec(Final, 0, vec![])]);
        }
        w.destroy_object(2);
        w.process_destroy_list();
        // a legacy save: the Final modules have no frame
        for id in w.objects.ids_in_order() {
            w.objects.update_module_mut(UpdateHandle { object: id, index: 1 }).unwrap().friend_set_next_call_frame(0);
        }
        w.frame = 4;
        w.next_object_id = 99;
        w.load_post_process();
        assert_eq!(w.next_object_id(), 7);
        assert_eq!(w.sched.len(), 10);
        assert!(heap_is_valid(&w));
        // the unset Final modules were made due at the current frame (4)
        let top = w.objects.update_module(w.sched.peek().unwrap()).unwrap();
        assert_eq!(top.friend_get_next_call_frame(), 4);
        let mut empty = World::new();
        empty.load_post_process();
        assert_eq!(empty.next_object_id(), 0, "an empty list leaves INVALID_ID (0)");
    }

    #[test]
    fn set_defaults_keeps_the_id_counter_when_loading() {
        let mut w = World::new();
        add(&mut w, &[spec(Normal, 0, vec![])]);
        w.frame = 12;
        w.set_defaults(true);
        assert_eq!((w.frame(), w.next_object_id(), w.objects.len(), w.sched.len()), (0, 2, 0, 0));
        w.set_defaults(false);
        assert_eq!(w.next_object_id(), 1);
    }

    #[test]
    fn the_state_machine_context_is_the_world() {
        let mut w = World::new();
        let a = add(&mut w, &[]);
        w.find_object_mut(a).unwrap().position = crate::common::base_type::Coord3D { x: 1.0, y: 2.0, z: 3.0 };
        w.frame = 9;
        assert_eq!(StateContext::frame(&w), 9);
        let r = StateContext::find_object(&w, a).unwrap();
        assert_eq!((r.id, r.position.y), (a, 2.0));
        assert!(StateContext::find_object(&w, a + 1).is_none());
        assert_eq!(w.owner(), None);
        assert_eq!(w.with_owner(a, |w| w.owner()), Some(a));
        assert_eq!(w.owner(), None);
    }

    #[test]
    fn terrain_events_nest_into_the_client_queue_in_order() {
        use crate::logic::terrain_visual::TerrainVisualEvent;
        let mut w = World::new();
        w.events.push(ClientEvent::ObjectDestroyed { object_id: 1 });
        let ev = w.take_client_events();
        assert_eq!(ev, [ClientEvent::ObjectDestroyed { object_id: 1 }]);
        assert!(w.take_client_events().is_empty());
        let _ = TerrainVisualEvent::EnableWaterGrid { enable: true };
    }

    #[test]
    fn damage_through_the_cut_point_is_recorded_on_the_target() {
        let mut w = World::new();
        let victim = add(&mut w, &[]);
        let shooter = add(&mut w, &[spec(Normal, 0, vec![step(5, vec![ProbeAction::Damage { target: victim, amount: 7.5 }, ProbeAction::Sound(b"Boom".to_vec())])])]);
        w.run_logic_frame();
        w.run_logic_frame();
        let got = &w.find_object(victim).unwrap().damage_received;
        assert_eq!((got.len(), got[0].input.amount, got[0].input.source_id), (1, 7.5, shooter));
        assert_eq!(w.take_client_events(), [ClientEvent::Audio { event_name: b"Boom".to_vec(), object_id: shooter, player_index: -1 }]);
    }

    #[test]
    fn a_body_that_is_running_is_not_available_a_second_time() {
        let mut w = World::new();
        let a = add(&mut w, &[spec(Normal, 0, vec![])]);
        let u = UpdateHandle { object: a, index: 0 };
        let inner = w.with_body(u, |_, w| w.with_body(u, |_, _| 1));
        assert_eq!(inner, Some(None), "the second checkout finds the slot empty");
        assert!(w.with_body(u, |_, _| 2) == Some(2), "and it is back afterwards");
        assert!(w.with_body(UpdateHandle { object: a, index: 5 }, |_, _| 3).is_none());
    }
}
