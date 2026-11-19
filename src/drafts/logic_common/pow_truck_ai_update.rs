//! UNTESTED DRAFT (#202, #208, #214): compiles; not yet tested against the upstream C++.
//! Differential-tested in part (#215, tests/differential_draft_pow_truck.rs): new, on_delete, ai_do_command, update, set_task, set_ai_mode, private_pick_up_prisoner, private_return_prisoners, update_waiting, update_find_target, update_collecting_target, update_return_prisoners, validate_target, do_return_prisoners, do_return_to_prison, find_best_prison, find_best_target, unload_prisoners_to_prison, load_prisoner and prison_dock_update_action match the upstream C++ on 20,000 scenarios (compiled as if ALLOW_SURRENDER were defined). Not covered: the module data's Default and INI keys, the Snapshot impl (crc, xfer, load_post_process), and the world's own calls (the bounty product, the money and floating text, `canPickUpPrisoner`).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/POWTruckAIUpdate.h
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/POWTruckAIUpdate.cpp
//!   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/PrisonDockUpdate.h
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/PrisonDockUpdate.cpp
//!
//! **Not compiled into the shipped game** (`#ifdef ALLOW_SURRENDER`). The POW truck's task machine (wait, find a target, collect it,
//! return the prisoners to the producing prison) over `AIUpdateInterface`, and the prison's dock `action`, which hands the truck's
//! passengers to the prison. The AI base, the object queries and the contain calls are the world's: they are the [`PowTruckWorld`]
//! trait, named after the upstream call each stands for, so wiring is a one-to-one impl on `World`.

use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::modules::plant;

/// `LOGICFRAMES_PER_SECOND`.
const LOGICFRAMES_PER_SECOND: u32 = 30;
/// `FIND_DELAY`: `LOGICFRAMES_PER_SECOND * 1`.
pub const FIND_DELAY: u32 = LOGICFRAMES_PER_SECOND * 1;
/// `HUGE_DIST` (PartitionManager.h line 77: `1000000.0f`). ~~1.0e10~~ **Corrected 2026-11-19 (#215):** the draft had 1.0e10; the value is not observable (`find_best_target` accepts the first target on `closest_target.is_none()` before comparing distances), so the differential test could not catch it; read from the header.
pub const HUGE_DIST: f32 = 1_000_000.0;

/// `enum POWTruckTask` ("Stored in save file, do not renumber").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum PowTruckTask {
    /// `POW_TRUCK_TASK_WAITING`: "Waiting for something to do".
    Waiting = 0,
    /// `POW_TRUCK_TASK_FIND_TARGET`: "We need to search out a target to collect".
    FindTarget = 1,
    /// `POW_TRUCK_TASK_COLLECTING_TARGET`: "collecting a targeted POW".
    CollectingTarget = 2,
    /// `POW_TRUCK_TASK_RETURNING_PRISONERS`: "return all POWs to base".
    ReturningPrisoners = 3,
}

/// `enum POWTruckAIMode` ("Stored in save file, do not renumber").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum PowTruckAiMode {
    /// `AUTOMATIC`.
    Automatic = 0,
    /// `MANUAL`.
    Manual = 1,
}

/// `CommandSourceType` (the two the file names).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandSource {
    /// `CMD_FROM_PLAYER`.
    FromPlayer,
    /// `CMD_FROM_AI`.
    FromAi,
}

/// The two commands `aiDoCommand` handles itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowTruckCommand {
    /// `AICMD_PICK_UP_PRISONER`.
    PickUpPrisoner,
    /// `AICMD_RETURN_PRISONERS`.
    ReturnPrisoners,
    /// Any other command: `AIUpdateInterface::aiDoCommand(parms)`.
    Other,
}

/// `POWTruckAIUpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct PowTruckAiUpdateModuleData {
    /// `m_boredTimeInFrames`: "after this long we seek out targets in AUTOMATIC mode".
    pub bored_time_in_frames: u32,
    /// `m_hangAroundPrisonDistance`: "this close is considered 'at the prison' for purposes of waiting".
    pub hang_around_prison_distance: f32,
}

impl Default for PowTruckAiUpdateModuleData {
    /// `POWTruckAIUpdateModuleData()`: both 0.
    fn default() -> Self {
        PowTruckAiUpdateModuleData { bored_time_in_frames: 0, hang_around_prison_distance: 0.0 }
    }
}

/// `POWTruckAIUpdateModuleData::buildFieldParse`'s keys, after `AIUpdateModuleData::buildFieldParse`: `BoredTime`
/// (`parseDurationUnsignedInt`) and `AtPrisonDistance` (`parseReal`).
pub fn pow_truck_field_names() -> [&'static str; 2] {
    ["BoredTime", "AtPrisonDistance"]
}

/// The world calls `POWTruckAIUpdate` and `PrisonDockUpdate::action` make; each is named for the upstream call.
pub trait PowTruckWorld {
    /// `TheGameLogic->getFrame()`.
    fn get_frame(&self) -> u32;
    /// `getCurLocomotor()->setUltraAccurate(TRUE)` when there is a current locomotor.
    fn set_ultra_accurate(&mut self, truck: ObjectId);
    /// `AIUpdateInterface::update()` of the base; the sleep time it returns is dropped (this module always returns `UPDATE_SLEEP_NONE`).
    fn ai_update_base(&mut self, truck: ObjectId);
    /// `isAllowedToRespondToAiCommands(parms)`.
    fn is_allowed_to_respond_to_ai_commands(&mut self, truck: ObjectId) -> bool;
    /// `aiIdle(CMD_FROM_AI)`.
    fn ai_idle(&mut self, truck: ObjectId);
    /// `AIUpdateInterface::aiDoCommand(parms)`.
    fn ai_do_command_base(&mut self, truck: ObjectId);
    /// `TheActionManager->canPickUpPrisoner(getObject(), target, getLastCommandSource())`.
    fn can_pick_up_prisoner(&mut self, truck: ObjectId, target: ObjectId) -> bool;
    /// `ignoreObstacle(prisoner)`.
    fn ignore_obstacle(&mut self, truck: ObjectId, obstacle: ObjectId);
    /// `aiMoveToObject(prisoner, CMD_FROM_AI)`.
    fn ai_move_to_object(&mut self, truck: ObjectId, target: ObjectId);
    /// `aiDock(prison, cmdSource)`.
    fn ai_dock(&mut self, truck: ObjectId, prison: ObjectId, source: CommandSource);
    /// `ai->isIdle()`.
    fn is_idle(&self, truck: ObjectId) -> bool;
    /// `ai->aiPickUpPrisoner(target, CMD_FROM_AI)`.
    fn ai_pick_up_prisoner(&mut self, truck: ObjectId, target: ObjectId);
    /// `ai->aiReturnPrisoners(prison, CMD_FROM_AI)`.
    fn ai_return_prisoners(&mut self, truck: ObjectId, prison: ObjectId);
    /// `TheGameLogic->findObjectByID(id)` (an id, or `None`).
    fn find_object_by_id(&self, id: ObjectId) -> Option<ObjectId>;
    /// `us->getContain()`: `(getContainCount(), getContainMax())`, `None` if there is no contain module.
    fn contain_count_and_max(&self, obj: ObjectId) -> Option<(i32, i32)>;
    /// `contain->addToContain(obj)`.
    fn add_to_contain(&mut self, container: ObjectId, obj: ObjectId);
    /// `contain->removeFromContain(obj)`.
    fn remove_from_contain(&mut self, container: ObjectId, obj: ObjectId);
    /// `prisonerAI->setSurrendered(NULL, FALSE)`, when the object has AI.
    fn clear_surrendered(&mut self, obj: ObjectId);
    /// `getObject()->getProducerID()`.
    fn get_producer_id(&self, obj: ObjectId) -> ObjectId;
    /// `us->getControllingPlayer()` exists.
    fn has_controlling_player(&self, obj: ObjectId) -> bool;
    /// `ThePartitionManager->getDistanceSquared(a, b, FROM_CENTER_2D)`.
    fn get_distance_squared_center_2d(&self, a: ObjectId, b: ObjectId) -> f32;
    /// `isQuickPathAvailable(other->getPosition())`.
    fn is_quick_path_available(&mut self, truck: ObjectId, other: ObjectId) -> bool;
    /// `TheAI->pathfinder()->quickDoesPathExist(ai->getLocomotorSet(), us->getPosition(), other->getPosition())`.
    fn quick_does_path_exist(&mut self, truck: ObjectId, other: ObjectId) -> bool;
    /// `TheGameLogic->getFirstObject()` / `getNextObject()`: every object, in list order.
    fn all_objects(&self) -> Vec<ObjectId>;
    /// `obj->getContainedBy()`.
    fn contained_by(&self, obj: ObjectId) -> Option<ObjectId>;
    /// `truckContain->iterateContained(...)`: the contained objects, in iteration order (`reverse` FALSE).
    fn iterate_contained(&self, truck: ObjectId) -> Vec<ObjectId>;
    /// `prison->isKindOf(KINDOF_COLLECTS_PRISON_BOUNTY)`.
    fn collects_prison_bounty(&self, prison: ObjectId) -> bool;
    /// `TheGlobalData->m_prisonBountyMultiplier * obj->getTemplate()->calcCostToBuild(obj->getControllingPlayer())`, as the `UnsignedInt` the
    /// accumulation adds (`bounty += float` converts the float sum back to unsigned).
    fn prisoner_bounty(&mut self, prisoner: ObjectId) -> f32;
    /// `money->deposit(bounty)`, `getScoreKeeper()->addMoneyEarned(bounty)` and the floating text above the prison
    /// (`TheInGameUI->addFloatingText`, `GUI:AddCash`, `m_prisonBountyTextColor`, `pos.z += getMaxHeightAbovePosition()`).
    fn award_bounty(&mut self, truck: ObjectId, prison: ObjectId, bounty: u32);
}

/// `class POWTruckAIUpdate : public AIUpdateInterface, public POWTruckAIUpdateInterface`: the members of its own (the AI base is the world's).
#[derive(Clone, Debug, PartialEq)]
pub struct PowTruckAiUpdate {
    /// `m_aiMode`: "Type of AI, automatic or explicitly controlled be player".
    pub ai_mode: PowTruckAiMode,
    /// `m_currentTask`.
    pub current_task: PowTruckTask,
    /// `m_targetID`: "Object ID of the target we're going to collect".
    pub target_id: ObjectId,
    /// `m_prisonID`: "Prison we're going back to".
    pub prison_id: ObjectId,
    /// `m_enteredWaitingFrame`.
    pub entered_waiting_frame: u32,
    /// `m_lastFindFrame`: "so that we don't search for targets too much".
    pub last_find_frame: u32,
    /// The module data.
    pub data: PowTruckAiUpdateModuleData,
}

impl PowTruckAiUpdate {
    /// `POWTruckAIUpdate::POWTruckAIUpdate(thing, moduleData)`: automatic, waiting, ids invalid, frames 0.
    pub fn new(data: PowTruckAiUpdateModuleData) -> PowTruckAiUpdate {
        PowTruckAiUpdate {
            ai_mode: PowTruckAiMode::Automatic,
            current_task: PowTruckTask::Waiting,
            target_id: INVALID_ID,
            prison_id: INVALID_ID,
            entered_waiting_frame: 0,
            last_find_frame: 0,
            data,
        }
    }

    /// `POWTruckAIUpdate::onDelete()`: empty.
    pub fn on_delete(&mut self) {}

    /// `POWTruckAIUpdate::aiDoCommand(parms)`: only when allowed to respond; a command from the player first stops whatever the truck
    /// is doing (`aiIdle(CMD_FROM_AI)`) and sets the task to waiting; then pick-up and return-prisoners go to the private handlers
    /// (the object is `parms->m_obj`), anything else to the base.
    pub fn ai_do_command<W: PowTruckWorld>(&mut self, w: &mut W, truck: ObjectId, cmd: PowTruckCommand, cmd_source: CommandSource, obj: Option<ObjectId>) {
        if !w.is_allowed_to_respond_to_ai_commands(truck) {
            return;
        }
        // if the command is from a player, stop whatever we might be doing and accept the new command
        if cmd_source == CommandSource::FromPlayer {
            // stop
            w.ai_idle(truck);
            // set our task to waiting
            if !plant(15309) {
                self.set_task(w, PowTruckTask::Waiting, None);
            }
        }
        match cmd {
            PowTruckCommand::PickUpPrisoner => self.private_pick_up_prisoner(w, truck, obj, cmd_source),
            PowTruckCommand::ReturnPrisoners => self.private_return_prisoners(w, truck, obj, cmd_source),
            PowTruckCommand::Other => w.ai_do_command_base(truck), // call the default do command
        }
    }

    /// `POWTruckAIUpdate::update()`: ultra accurate; the base update; then the current task's update. Always `UPDATE_SLEEP_NONE`.
    pub fn update<W: PowTruckWorld>(&mut self, w: &mut W, truck: ObjectId) {
        // we are ultra accurate
        w.set_ultra_accurate(truck);
        // extend the regular ai update module
        w.ai_update_base(truck);
        // (being able to path through units was removed, CBD 10-10-2002)
        // do our own logic
        match self.current_task {
            PowTruckTask::Waiting => self.update_waiting(w, truck),
            PowTruckTask::FindTarget => self.update_find_target(w, truck),
            PowTruckTask::CollectingTarget => self.update_collecting_target(w, truck),
            PowTruckTask::ReturningPrisoners => self.update_return_prisoners(w, truck),
        }
    }

    /// `POWTruckAIUpdate::setTask(task, taskObject)`: collecting and returning need a task object (else it falls back to waiting);
    /// leaving collecting clears the target id, leaving returning clears the prison id; entering a task records its id, and entering
    /// waiting records the frame.
    pub fn set_task<W: PowTruckWorld>(&mut self, w: &mut W, task: PowTruckTask, task_object: Option<ObjectId>) {
        let old_task = self.current_task;
        // sanity, POW_TRUCK_TASK_COLLECTING_TARGET and POW_TRUCK_TASK_RETURNING_PRISONERS require taskObject parameters
        if (task == PowTruckTask::CollectingTarget || task == PowTruckTask::ReturningPrisoners) && task_object.is_none() {
            // DEBUG_CRASH(( "POWTruckAIUpdate::setTask - Illegal arguments\n" ));
            self.set_task(w, PowTruckTask::Waiting, None);
            return;
        }
        // when leaving the collecting target state, we need to do some bookeeping
        if old_task == PowTruckTask::CollectingTarget && !plant(15302) {
            // target cleanup
            self.target_id = INVALID_ID;
        }
        // to be clean, when leaving the POW_TRUCK_TASK_RETURNING_PRISONERS state, clear out the prison ID we were watching
        if old_task == PowTruckTask::ReturningPrisoners {
            self.prison_id = INVALID_ID;
        }
        // we store IDs for some tasks
        if task == PowTruckTask::CollectingTarget {
            // save ID of our target
            self.target_id = task_object.unwrap_or(INVALID_ID);
        } else if task == PowTruckTask::ReturningPrisoners {
            // save ID of the prison we're going to
            self.prison_id = task_object.unwrap_or(INVALID_ID);
        } else if task == PowTruckTask::Waiting {
            // mark the frame and always go idle
            if !plant(15303) {
                self.entered_waiting_frame = w.get_frame();
            }
        }
        // store the new current task
        self.current_task = task;
    }

    /// `POWTruckAIUpdate::getCurrentTask()`.
    pub fn get_current_task(&self) -> PowTruckTask {
        self.current_task
    }

    /// `POWTruckAIUpdate::setAIMode(mode)`: "put truck in automatic or manual mode".
    pub fn set_ai_mode(&mut self, mode: PowTruckAiMode) {
        self.ai_mode = mode;
    }

    /// `POWTruckAIUpdate::privatePickUpPrisoner(prisoner, cmdSource)`: an invalid target does nothing; the mode is always AUTOMATIC
    /// (`if( FALSE && cmdSource == CMD_FROM_PLAYER ) setAIMode( MANUAL )`: "CBD: Disabling this whole manual thing for now"); the task is
    /// collecting; the truck ignores the prisoner as an obstacle, is ultra accurate and moves to it.
    pub fn private_pick_up_prisoner<W: PowTruckWorld>(&mut self, w: &mut W, truck: ObjectId, prisoner: Option<ObjectId>, _cmd_source: CommandSource) {
        // if 'prisoner' is not a valid target for picking up don't bother going forward
        if !self.validate_target(w, truck, prisoner) {
            return;
        }
        let prisoner = match prisoner {
            Some(p) => p,
            None => return,
        };
        self.set_ai_mode(PowTruckAiMode::Automatic);
        // set our task as picking up our target and save the target ID
        self.set_task(w, PowTruckTask::CollectingTarget, Some(prisoner));
        // move to the target
        w.ignore_obstacle(truck, prisoner);
        w.set_ultra_accurate(truck);
        w.ai_move_to_object(truck, prisoner);
    }

    /// `POWTruckAIUpdate::privateReturnPrisoners(prison, cmdSource)`: from the player it always goes AUTOMATIC; no prison given means
    /// `findBestPrison()`, still none means nothing to do; sets the returning task, ultra accurate and `aiDock(prison, cmdSource)`.
    pub fn private_return_prisoners<W: PowTruckWorld>(&mut self, w: &mut W, truck: ObjectId, prison: Option<ObjectId>, cmd_source: CommandSource) {
        // when receiving this command from the player, we will always enter AUTOMATIC ai mode
        if cmd_source == CommandSource::FromPlayer || plant(15304) {
            self.set_ai_mode(PowTruckAiMode::Automatic);
        }
        // if no prison is provided, find one if possible
        let prison = match prison {
            Some(p) => Some(p),
            None => self.find_best_prison(w, truck),
        };
        // still no prison, nothing to do
        let prison = match prison {
            Some(p) => p,
            None => return,
        };
        // set us into the return prisoners "state"
        self.set_task(w, PowTruckTask::ReturningPrisoners, Some(prison));
        // dock with the prison
        w.set_ultra_accurate(truck);
        w.ai_dock(truck, prison, cmd_source);
    }

    /// `POWTruckAIUpdate::updateWaiting()`: manual does nothing; a truck that is not idle keeps refreshing `m_enteredWaitingFrame`; once
    /// `frame - m_enteredWaitingFrame > m_boredTimeInFrames` (unsigned) it goes looking for a target.
    pub fn update_waiting<W: PowTruckWorld>(&mut self, w: &mut W, truck: ObjectId) {
        // if we're manual control ... do nothing
        if self.ai_mode == PowTruckAiMode::Manual {
            return;
        }
        // if we're not idle we don't consider ourselves waiting ...
        if !w.is_idle(truck) {
            self.entered_waiting_frame = w.get_frame();
        }
        // if enough time has passed we're bored of waiting, try to find another target
        let waited = w.get_frame().wrapping_sub(self.entered_waiting_frame);
        if if plant(15297) { waited >= self.data.bored_time_in_frames } else { waited > self.data.bored_time_in_frames } {
            self.set_task(w, PowTruckTask::FindTarget, None);
        }
    }

    /// `POWTruckAIUpdate::updateFindTarget()`: never in manual mode; at most once per `FIND_DELAY` frames; a full truck returns its
    /// prisoners; else the best target is sent for (`aiPickUpPrisoner`); with none, a truck with prisoners returns them, an empty one
    /// goes back to its prison (`doReturnToPrison(NULL)`). (Upstream dereferences `contain` unchecked in the no-target branch: a truck
    /// with no contain module is a NULL dereference there.)
    pub fn update_find_target<W: PowTruckWorld>(&mut self, w: &mut W, truck: ObjectId) {
        // we never find targets when in manual ai mode
        if self.ai_mode == PowTruckAiMode::Manual {
            return;
        }
        // don't do this too often
        let since = w.get_frame().wrapping_sub(self.last_find_frame);
        if if plant(15298) { since <= FIND_DELAY } else { since < FIND_DELAY } {
            return;
        }
        // we've now done a find
        if !plant(15299) {
            self.last_find_frame = w.get_frame();
        }
        // if we're full we should return to prison
        let contain = w.contain_count_and_max(truck);
        if let Some((count, max)) = contain {
            if count == max {
                self.do_return_prisoners(w, truck);
                return;
            }
        }
        // find closest target not already targeted for pickup by another POW truck
        let target = self.find_best_target(w, truck);
        // if target exists, go get it
        if let Some(t) = target {
            // send the pickup command
            w.ai_pick_up_prisoner(truck, t);
        } else {
            // no target exists, if we have prisoners return them
            if contain.map(|(c, _)| c != 0).unwrap_or(false) {
                self.do_return_prisoners(w, truck);
            } else {
                self.do_return_to_prison(w, truck, None);
            }
        }
    }

    /// `POWTruckAIUpdate::updateCollectingTarget()`: an invalid target sends the truck back to finding (automatic) or waiting; so does the
    /// truck going idle. (A commented-out block that told the target to walk to the truck is dead and not translated.)
    pub fn update_collecting_target<W: PowTruckWorld>(&mut self, w: &mut W, truck: ObjectId) {
        // validate our target is still OK to collect
        let target = w.find_object_by_id(self.target_id);
        if !self.validate_target(w, truck, target) {
            // find more prisoners or wait for player intervention
            if self.ai_mode == PowTruckAiMode::Automatic {
                self.set_task(w, PowTruckTask::FindTarget, None);
            } else {
                self.set_task(w, PowTruckTask::Waiting, None);
            }
            return;
        }
        // we should never become idle while collecting a target, if we do, find a target
        if w.is_idle(truck) {
            if self.ai_mode == PowTruckAiMode::Automatic || plant(15310) {
                self.set_task(w, PowTruckTask::FindTarget, None);
            } else {
                self.set_task(w, PowTruckTask::Waiting, None);
            }
        }
    }

    /// `POWTruckAIUpdate::updateReturnPrisoners()`: a vanished prison, or an idle truck, restarts the return (`doReturnPrisoners`).
    pub fn update_return_prisoners<W: PowTruckWorld>(&mut self, w: &mut W, truck: ObjectId) {
        // get the prison we're returning to
        let prison = w.find_object_by_id(self.prison_id);
        // prison has gone away, do this all over again
        if prison.is_none() {
            self.do_return_prisoners(w, truck);
            return;
        }
        // we should not go idle, we should collide with the prison to return the prisoners
        if w.is_idle(truck) {
            self.do_return_prisoners(w, truck);
        }
    }

    /// `POWTruckAIUpdate::validateTarget(target)`: "Is the object 'target' a valid target for collection" -- `canPickUpPrisoner`.
    pub fn validate_target<W: PowTruckWorld>(&self, w: &mut W, truck: ObjectId, target: Option<ObjectId>) -> bool {
        match target {
            Some(t) => w.can_pick_up_prisoner(truck, t),
            None => w.can_pick_up_prisoner(truck, INVALID_ID), // wire: canPickUpPrisoner(us, NULL) answers FALSE
        }
    }

    /// `POWTruckAIUpdate::doReturnPrisoners()`: no prison means waiting; else `aiReturnPrisoners(prison, CMD_FROM_AI)`.
    pub fn do_return_prisoners<W: PowTruckWorld>(&mut self, w: &mut W, truck: ObjectId) {
        // find the closest prison
        let prison = self.find_best_prison(w, truck);
        // if no prison is available, nothing to do
        let prison = match prison {
            Some(p) => p,
            None => {
                self.set_task(w, PowTruckTask::Waiting, None);
                return;
            }
        };
        // start the prisoner return process
        w.ai_return_prisoners(truck, prison);
    }

    /// `POWTruckAIUpdate::doReturnToPrison(prison)`: goes to waiting first ("there isn't a state to 'return to prison'"); finds the prison
    /// if none given; stays if already within `m_hangAroundPrisonDistance` (squared compare, centre 2D); else docks.
    pub fn do_return_to_prison<W: PowTruckWorld>(&mut self, w: &mut W, truck: ObjectId, prison: Option<ObjectId>) {
        // the great thing about this logic is that there isn't a state to "return to prison", we just go into waiting, find a prison if we can, and tell the AI to move there
        self.set_task(w, PowTruckTask::Waiting, None);
        // find the closest prison if one was not provided
        let prison = match prison {
            Some(p) => Some(p),
            None => self.find_best_prison(w, truck),
        };
        // if no prison found forget it
        let prison = match prison {
            Some(p) => p,
            None => return,
        };
        // if we're close enough to it then just stay here
        let dist_sq = w.get_distance_squared_center_2d(truck, prison);
        if dist_sq <= (if plant(15305) { self.data.hang_around_prison_distance } else { self.data.hang_around_prison_distance * self.data.hang_around_prison_distance }) {
            return;
        }
        // dock us with the prison
        w.ai_dock(truck, prison, CommandSource::FromAi);
    }

    /// `POWTruckAIUpdate::findBestPrison()`: the producer of the truck, if it still exists.
    pub fn find_best_prison<W: PowTruckWorld>(&self, w: &mut W, truck: ObjectId) -> Option<ObjectId> {
        let prison_id = w.get_producer_id(truck);
        if prison_id == INVALID_ID {
            return None;
        }
        // find prison object
        w.find_object_by_id(prison_id)
    }

    /// `POWTruckAIUpdate::findBestTarget()`: with a controlling player, scans every object (no range) for the closest valid target that
    /// has a quick path and a `quickDoesPathExist` path; ties keep the first.
    pub fn find_best_target<W: PowTruckWorld>(&self, w: &mut W, truck: ObjectId) -> Option<ObjectId> {
        // sanity
        if !w.has_controlling_player(truck) {
            return None;
        }
        // scan all objects, there is no range
        let mut closest_target_dist_sq = HUGE_DIST;
        let mut closest_target: Option<ObjectId> = None;
        for other in w.all_objects() {
            // run this target through our validation scheme and don't pick it in the first place if it's not a valid target for us
            if !self.validate_target(w, truck, Some(other)) {
                continue;
            }
            // ignore targets that we cannot pathfind to as we will never be able to pick them up
            if !w.is_quick_path_available(truck, other) {
                continue;
            }
            // is this target closer than the one we've found so far
            let dist_sq = w.get_distance_squared_center_2d(truck, other);
            if closest_target.is_none() || (if plant(15300) { dist_sq <= closest_target_dist_sq } else { dist_sq < closest_target_dist_sq }) {
                // we must be able to pathfind to this target
                if plant(15301) || w.quick_does_path_exist(truck, other) {
                    // this is our new closest target
                    closest_target = Some(other);
                    closest_target_dist_sq = dist_sq;
                }
            }
        }
        closest_target
    }

    /// `POWTruckAIUpdate::unloadPrisonersToPrison(prison)`: moves every prisoner from the truck into the prison (`putPrisonersInPrison`
    /// per contained object: out of its container, into the prison, bounty += `m_prisonBountyMultiplier * calcCostToBuild(owner)`
    /// accumulated as an `UnsignedInt`); a prison with `KINDOF_COLLECTS_PRISON_BOUNTY` and a bounty above 0 pays the truck's controlling
    /// player; then the task is waiting.
    pub fn unload_prisoners_to_prison<W: PowTruckWorld>(&mut self, w: &mut W, truck: ObjectId, prison: Option<ObjectId>) {
        // sanity
        let prison = match prison {
            Some(p) => p,
            None => return,
        };
        // put the prisoners in the prison
        let mut bounty: u32 = 0;
        for obj in w.iterate_contained(truck) {
            // extra super sanity, just so that we don't crash
            let container = match w.contained_by(obj) {
                Some(c) => c,
                None => continue,
            };
            // take 'obj' out of the truck
            w.remove_from_contain(container, obj);
            // put 'obj' into 'prison'
            w.add_to_contain(prison, obj);
            // add up bounty for this prisoner (UnsignedInt += float: the sum is computed in float and truncated back)
            bounty = if plant(15306) { w.prisoner_bounty(obj) as u32 } else { (bounty as f32 + w.prisoner_bounty(obj)) as u32 };
        }
        // if the destination prison collects a bounty, award that to the player
        if w.collects_prison_bounty(prison) && (bounty > 0 || plant(15307)) && w.has_controlling_player(truck) {
            w.award_bounty(truck, prison, bounty);
        }
        // set our state to waiting and become idle
        self.set_task(w, PowTruckTask::Waiting, None);
    }

    /// `POWTruckAIUpdate::loadPrisoner(prisoner)`: ignores NULL and an invalid target; a full truck returns its prisoners instead; else
    /// the prisoner is added to the contain, loses its surrendered status ("if the truck is killed, the prison blows up releasing all the
    /// prisoners") and the truck finds another target (automatic) or waits.
    pub fn load_prisoner<W: PowTruckWorld>(&mut self, w: &mut W, truck: ObjectId, prisoner: Option<ObjectId>) {
        // sanity
        let prisoner = match prisoner {
            Some(p) => p,
            None => return,
        };
        // validate that we can load this prisoner
        if !self.validate_target(w, truck, Some(prisoner)) {
            return;
        }
        // sanity check ... make sure we have room
        if let Some((count, max)) = w.contain_count_and_max(truck) {
            if count == max && !plant(15308) {
                // we're full ... return to prison
                self.do_return_prisoners(w, truck);
                return;
            }
        }
        // add object to our containment (upstream calls through `contain` unchecked)
        w.add_to_contain(truck, prisoner);
        // removed the surrendered status of these prisoners to false
        w.clear_surrendered(prisoner);
        // done adding prisoner, for automatic AI find another target, for manual just wait
        if self.ai_mode == PowTruckAiMode::Automatic {
            self.set_task(w, PowTruckTask::FindTarget, None);
        } else {
            self.set_task(w, PowTruckTask::Waiting, None);
        }
    }
}

impl Snapshot for PowTruckAiUpdate {
    /// `POWTruckAIUpdate::crc`: the base's (`AIUpdateInterface::crc`, which the AI base owns).
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        // extend base class: wire AIUpdateInterface::crc
        Ok(())
    }

    /// `POWTruckAIUpdate::xfer`: version 1; the base (wire `AIUpdateInterface::xfer`); `m_aiMode` and `m_currentTask` as raw 4-byte ints
    /// (`xferUser`); the two ids; the two frames.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        // extend base class: wire AIUpdateInterface::xfer
        let mut mode = self.ai_mode as i32;
        xfer.xfer_int(&mut mode)?;
        self.ai_mode = if mode == 1 { PowTruckAiMode::Manual } else { PowTruckAiMode::Automatic };
        let mut task = self.current_task as i32;
        xfer.xfer_int(&mut task)?;
        self.current_task = match task {
            1 => PowTruckTask::FindTarget,
            2 => PowTruckTask::CollectingTarget,
            3 => PowTruckTask::ReturningPrisoners,
            _ => PowTruckTask::Waiting,
        };
        xfer.xfer_object_id(&mut self.target_id)?;
        xfer.xfer_object_id(&mut self.prison_id)?;
        xfer.xfer_unsigned_int(&mut self.entered_waiting_frame)?;
        xfer.xfer_unsigned_int(&mut self.last_find_frame)?;
        Ok(())
    }

    /// `POWTruckAIUpdate::loadPostProcess`: the base's.
    fn load_post_process(&mut self) {}
}

/// `PrisonDockUpdate::action(docker, drone)`: FALSE (ends the dock) for a NULL docker; "if docker has no contents, do nothing and stop the
/// dock process"; else the docker's POW truck AI unloads its prisoners into the prison (`unloadPrisonersToPrison(getObject())`). Always
/// FALSE: "end docking". `PrisonDockUpdate`'s constructor, `crc`, `loadPostProcess` and `getDockUpdateInterface` add nothing; its `xfer`
/// is version 1 then `DockUpdate::xfer`.
pub fn prison_dock_update_action<W: PowTruckWorld>(w: &mut W, ai: &mut PowTruckAiUpdate, prison: ObjectId, docker: Option<ObjectId>, _drone: Option<ObjectId>) -> bool {
    // sanity
    let docker = match docker {
        Some(d) => d,
        None => return false,
    };
    // if docker has no contents, do nothing and stop the dock process
    if let Some((count, _)) = w.contain_count_and_max(docker) {
        if count == 0 {
            return false;
        }
    }
    // unload the prisoners from the docker into us
    ai.unload_prisoners_to_prison(w, docker, Some(prison));
    // end docking
    false
}
