//! UNTESTED DRAFT (#202, #208): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameClient/Smudge.h
//!   GeneralsMD/Code/GameEngine/Source/GameClient/System/Smudge.cpp
//!
//! The smudge (heat-haze) system's bookkeeping: a manager owns sets, a set owns smudges, and both are recycled through free lists
//! (`DLListClass`, intrusive lists in upstream; here indices in `VecDeque`s with the same head/tail operations). The smudge free
//! list is `static` upstream (shared by every set), so [`SmudgePool`] holds it once. Drawing the smudges is the W3D device's
//! (`W3DSmudge.cpp`, not in this area).

use std::collections::VecDeque;

use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;

/// `Smudge::smudgeVertex`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SmudgeVertex {
    /// `pos`: world-space position of the vertex.
    pub pos: Vector3,
    /// `uv`.
    pub uv: Vector2,
}

/// `struct Smudge`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Smudge {
    /// `m_pos`: position of smudge center.
    pub pos: Vector3,
    /// `m_offset`: difference in position between "texture" extraction and re-insertion for center vertex.
    pub offset: Vector2,
    /// `m_size`: size in world space.
    pub size: f32,
    /// `m_opacity`: alpha of center vertex, corners are assumed at 0.
    pub opacity: f32,
    /// `m_verts[5]`: counter-clockwise from top-left, ending in the center.
    pub verts: [SmudgeVertex; 5],
}

/// `struct SmudgeSet`'s own lists: `m_usedSmudgeList` and `m_usedSmudgeCount` (smudge indices into the pool).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SmudgeSet {
    /// `m_usedSmudgeList`.
    pub used_smudge_list: VecDeque<usize>,
    /// `m_usedSmudgeCount`.
    pub used_smudge_count: i32,
}

/// `HardwareSmudgeSupport`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareSmudgeSupport {
    /// `SMUDGE_SUPPORT_UNKNOWN`.
    Unknown,
    /// `SMUDGE_SUPPORT_NO`.
    No,
    /// `SMUDGE_SUPPORT_YES`.
    Yes,
}

/// Everything the manager and its sets own: the smudge arena and `SmudgeSet::m_freeSmudgeList` (the static), the set arena and the
/// manager's two set lists.
#[derive(Clone, Debug, PartialEq)]
pub struct SmudgePool {
    /// All smudges ever made (`W3DNEW Smudge`).
    pub smudges: Vec<Smudge>,
    /// `SmudgeSet::m_freeSmudgeList`: "list of unused smudges for use by SmudgeSets".
    pub free_smudge_list: VecDeque<usize>,
    /// All sets ever made.
    pub sets: Vec<SmudgeSet>,
    /// `m_usedSmudgeSetList`.
    pub used_smudge_set_list: VecDeque<usize>,
    /// `m_freeSmudgeSetList`: "unused SmudgeSets ready for re-use".
    pub free_smudge_set_list: VecDeque<usize>,
    /// `m_smudgeCountLastFrame`.
    pub smudge_count_last_frame: i32,
    /// `m_hardwareSupportStatus`.
    pub hardware_support_status: HardwareSmudgeSupport,
}

impl SmudgePool {
    /// `SmudgeManager::SmudgeManager()`: `m_smudgeCountLastFrame = 0; m_hardwareSupportStatus = SMUDGE_SUPPORT_UNKNOWN`.
    pub fn new() -> SmudgePool {
        SmudgePool {
            smudges: Vec::new(),
            free_smudge_list: VecDeque::new(),
            sets: Vec::new(),
            used_smudge_set_list: VecDeque::new(),
            free_smudge_set_list: VecDeque::new(),
            smudge_count_last_frame: 0,
            hardware_support_status: HardwareSmudgeSupport::Unknown,
        }
    }

    /// `SmudgeManager::~SmudgeManager()`: `reset()`, then frees the free sets, then (upstream's bug, kept) loops on the SMUDGE free list
    /// but calls `m_freeSmudgeSetList.Remove_Head()` on the SET list inside it, so the smudge list is never emptied by that loop and,
    /// once the set list is empty, `Remove_Head` on an empty list is undefined upstream (`Head()` stays non-NULL). Here the sets are
    /// freed and the smudge list is cleared; the quirk is recorded, not reproduced.
    pub fn drop_manager(&mut self) {
        self.reset();
        self.free_smudge_set_list.clear();
        self.free_smudge_list.clear();
    }

    /// `SmudgeManager::init()`: empty.
    pub fn init(&mut self) {}

    /// `SmudgeManager::reset()`: every used set is reset (its smudges return to the free list) and goes to the free-set list's tail.
    pub fn reset(&mut self) {
        // Return all smudgeSets back to free pool.
        while let Some(head) = self.used_smudge_set_list.pop_front() {
            self.set_reset(head); // free all smudges.
            self.free_smudge_set_list.push_back(head);
        }
    }

    /// `SmudgeManager::addSmudgeSet()`: the head of the free list if any (moved to the used list's tail), else a new set.
    pub fn add_smudge_set(&mut self) -> usize {
        if let Some(set) = self.free_smudge_set_list.pop_front() {
            self.used_smudge_set_list.push_back(set);
            return set;
        }
        self.sets.push(SmudgeSet::default());
        let set = self.sets.len() - 1;
        self.used_smudge_set_list.push_back(set);
        set
    }

    /// `SmudgeManager::removeSmudgeSet(mySmudge)`: off the used list, onto the free list's HEAD.
    pub fn remove_smudge_set(&mut self, set: usize) {
        if let Some(pos) = self.used_smudge_set_list.iter().position(|&s| s == set) {
            self.used_smudge_set_list.remove(pos);
        }
        self.free_smudge_set_list.push_front(set);
    }

    /// `SmudgeManager::getSmudgeCountLastFrame()`.
    pub fn get_smudge_count_last_frame(&self) -> i32 {
        self.smudge_count_last_frame
    }

    /// `SmudgeManager::setSmudgeCountLastFrame(count)`.
    pub fn set_smudge_count_last_frame(&mut self, count: i32) {
        self.smudge_count_last_frame = count;
    }

    /// `SmudgeManager::getHardwareSupport()`: anything but `SMUDGE_SUPPORT_NO`.
    pub fn get_hardware_support(&self) -> bool {
        self.hardware_support_status != HardwareSmudgeSupport::No
    }

    /// `SmudgeSet::reset()`: every used smudge goes to the free list's head, one by one.
    pub fn set_reset(&mut self, set: usize) {
        while let Some(head) = self.sets[set].used_smudge_list.pop_front() {
            self.free_smudge_list.push_front(head); // add to free list
        }
    }

    /// `SmudgeSet::addSmudgeToSet()`: the free list's head if any, else a new smudge; appended to the set's list; the count goes up.
    pub fn add_smudge_to_set(&mut self, set: usize) -> usize {
        let smudge = match self.free_smudge_list.pop_front() {
            Some(s) => s, // remove from free list
            None => {
                self.smudges.push(Smudge::default());
                self.smudges.len() - 1
            }
        };
        self.sets[set].used_smudge_list.push_back(smudge); // add to used list.
        self.sets[set].used_smudge_count += 1;
        smudge
    }

    /// `SmudgeSet::removeSmudgeFromSet(mySmudge)`: off the set's list, onto the free list's head; the count goes down.
    pub fn remove_smudge_from_set(&mut self, set: usize, smudge: usize) {
        if let Some(pos) = self.sets[set].used_smudge_list.iter().position(|&s| s == smudge) {
            self.sets[set].used_smudge_list.remove(pos);
        }
        self.free_smudge_list.push_front(smudge);
        self.sets[set].used_smudge_count -= 1;
    }
}

impl Default for SmudgePool {
    fn default() -> Self {
        SmudgePool::new()
    }
}
