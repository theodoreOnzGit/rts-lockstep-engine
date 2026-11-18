//! UNTESTED DRAFT (#202, #208, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/PropagandaCenterBehavior.h
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PropagandaCenterBehavior.cpp
//!
//! **Not compiled into the shipped game** (`#ifdef ALLOW_SURRENDER`). A `PrisonBehavior` that brainwashes its prisoners one at a time:
//! after `BrainwashDuration` frames the current subject leaves under the prison owner's (temporary) control, with its surrender
//! status cleared; when the centre is deleted every brainwashed unit still alive returns to its original team. The prison base
//! ([`super::prison_behavior`]) is held by value; the contain-door calls and object queries are [`PropagandaWorld`].

use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};

use super::prison_behavior::PrisonBehaviorModuleData;

/// `PropagandaCenterBehaviorModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct PropagandaCenterBehaviorModuleData {
    /// The `PrisonBehaviorModuleData` base.
    pub prison: PrisonBehaviorModuleData,
    /// `m_brainwashDuration`: "how long (in frames) it takes to become brainwashed". Default 0.
    pub brainwash_duration: u32,
}

impl Default for PropagandaCenterBehaviorModuleData {
    /// `PropagandaCenterBehaviorModuleData()`.
    fn default() -> Self {
        PropagandaCenterBehaviorModuleData { prison: PrisonBehaviorModuleData::default(), brainwash_duration: 0 }
    }
}

/// `PropagandaCenterBehaviorModuleData::buildFieldParse`'s key after the prison base's: `BrainwashDuration` (`parseDurationUnsignedInt`).
pub const BRAINWASH_DURATION_KEY: &str = "BrainwashDuration";

/// What the behaviour asks of the world.
pub trait PropagandaWorld {
    /// `TheGameLogic->getFrame()`.
    fn get_frame(&self) -> u32;
    /// `TheGameLogic->findObjectByID(id)` is non-NULL.
    fn object_exists(&self, id: ObjectId) -> bool;
    /// `obj->restoreOriginalTeam()`.
    fn restore_original_team(&mut self, obj: ObjectId);
    /// `reserveDoorForExit(obj->getTemplate(), obj)`: whether a door is available (`!= DOOR_NONE_AVAILABLE`); the door is kept for the exit.
    fn reserve_door_for_exit(&mut self, obj: ObjectId) -> bool;
    /// `exitObjectViaDoor(obj, exitDoor)` with the door just reserved.
    fn exit_object_via_door(&mut self, obj: ObjectId);
    /// `player->getDefaultTeam()` of the centre's controlling player then `obj->setTemporaryTeam(team)`; false (nothing done) with no
    /// controlling player (the `DEBUG_ASSERTCRASH` is compiled out of release).
    fn set_temporary_team_to_owners_default(&mut self, us: ObjectId, obj: ObjectId) -> bool;
    /// `ai->setSurrendered(NULL, FALSE)` when the object has AI.
    fn clear_surrendered(&mut self, obj: ObjectId);
    /// `getContainList().front()`: the first contained object, if any.
    fn first_contained(&self, us: ObjectId) -> Option<ObjectId>;
}

/// `class PropagandaCenterBehavior : public PrisonBehavior`: its own members.
#[derive(Clone, Debug, PartialEq)]
pub struct PropagandaCenterBehavior {
    /// `m_brainwashingSubjectID`: "who we're currently brainwashing".
    pub brainwashing_subject_id: ObjectId,
    /// `m_brainwashingSubjectStartFrame`: "frame we started brainwashing".
    pub brainwashing_subject_start_frame: u32,
    /// `m_brainwashedList`: "list of objects we've brainwashed" (`push_front`: index 0 is the newest).
    pub brainwashed_list: Vec<ObjectId>,
    /// The module data.
    pub data: PropagandaCenterBehaviorModuleData,
}

impl PropagandaCenterBehavior {
    /// `PropagandaCenterBehavior::PropagandaCenterBehavior`: no subject, start frame 0.
    pub fn new(data: PropagandaCenterBehaviorModuleData) -> PropagandaCenterBehavior {
        PropagandaCenterBehavior { brainwashing_subject_id: INVALID_ID, brainwashing_subject_start_frame: 0, brainwashed_list: Vec::new(), data }
    }

    /// `PropagandaCenterBehavior::onDelete()` after `PrisonBehavior::onDelete()` (the caller runs the base): every brainwashed unit still
    /// alive goes back to its original team; the list is cleared.
    pub fn on_delete<W: PropagandaWorld>(&mut self, w: &mut W) {
        // go through our list of brainwashed objects, and if they are still under our control, return them to their original owners
        for &id in self.brainwashed_list.iter() {
            if w.object_exists(id) {
                // return this object under the control of the original owner
                w.restore_original_team(id);
            }
        }
        // clear the list
        self.brainwashed_list.clear();
    }

    /// `PropagandaCenterBehavior::update()` after `PrisonBehavior::update()` (the caller runs the base). A subject that has been inside
    /// for `BrainwashDuration` frames (unsigned difference) and can leave by a reserved door is put under the owner's default team,
    /// loses its surrender status, is added to the brainwashed list unless already there, and exits. With no subject, the first
    /// contained unit becomes the subject, starting this frame. Always `UPDATE_SLEEP_NONE`.
    pub fn update<W: PropagandaWorld>(&mut self, w: &mut W, us: ObjectId) {
        // if we have a prisoner inside, continue the brainwashing on them (one at a time)
        if self.brainwashing_subject_id != INVALID_ID {
            if w.object_exists(self.brainwashing_subject_id) {
                let subject = self.brainwashing_subject_id;
                // if we've been in here long enough, we come out brainwashed
                if w.get_frame().wrapping_sub(self.brainwashing_subject_start_frame) >= self.data.brainwash_duration {
                    // only can exit if the prison allows us to
                    if w.reserve_door_for_exit(subject) {
                        // place this object under the control of the player
                        w.set_temporary_team_to_owners_default(us, subject);
                        // remove any surrender status from this object
                        w.clear_surrendered(subject);
                        // add this object to our brainwashed list if we're not already in it
                        if !self.brainwashed_list.iter().any(|&i| i == subject) {
                            self.brainwashed_list.insert(0, subject);
                        }
                        // exit the prison
                        w.exit_object_via_door(subject);
                    }
                }
            }
        }
        // if we have no brainwashing subject, hook one up if we have people inside us
        if self.brainwashing_subject_id == INVALID_ID {
            // find the first object in our containment list
            if let Some(obj) = w.first_contained(us) {
                // assign brainwashing subject on this frame
                self.brainwashing_subject_id = obj;
                self.brainwashing_subject_start_frame = w.get_frame();
            }
        }
    }

    /// `PropagandaCenterBehavior::onRemoving(obj)` before `PrisonBehavior::onRemoving(obj)`: the departing subject clears the subject.
    pub fn on_removing(&mut self, obj: ObjectId) {
        // if we're removing the brainwashing subject, NULL the pointer
        if self.brainwashing_subject_id == obj {
            self.brainwashing_subject_id = INVALID_ID;
            self.brainwashing_subject_start_frame = 0;
        }
    }
}

impl Snapshot for PropagandaCenterBehavior {
    /// `PropagandaCenterBehavior::crc`: the base's.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(()) // wire: PrisonBehavior::crc
    }

    /// `PropagandaCenterBehavior::xfer`: version 1; the prison base (wire); the subject id; its start frame; the brainwashed list
    /// (`xferSTLObjectIDList`: a count, then the ids).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        // extend base class: wire PrisonBehavior::xfer
        xfer.xfer_object_id(&mut self.brainwashing_subject_id)?;
        xfer.xfer_unsigned_int(&mut self.brainwashing_subject_start_frame)?;
        xfer.xfer_stl_object_id_list(&mut self.brainwashed_list)?;
        Ok(())
    }

    /// `PropagandaCenterBehavior::loadPostProcess`: the base's.
    fn load_post_process(&mut self) {}
}
