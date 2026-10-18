// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Object.h (the list links and
//   `isInList`/`prependToList`/`removeFromList`, Object.cpp 2690-2800)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp
//   (`m_objList`, `m_objVector`, `addObjectToLookupTable`,
//   `removeObjectFromLookupTable`; GameLogic.h `findObjectByID`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The object arena: the object list (`m_objList`) and the id lookup table
//! (`m_objVector`) of `GameLogic`, and the shell of `Object` that they hold.
//!
//! # The two structures upstream keeps, and the one here
//!
//! Upstream has two views of the same objects:
//!
//! - **`m_objList`**, an intrusive doubly linked list threaded through
//!   `Object::m_prev`/`m_next`. `registerObject` *prepends*, so the list runs
//!   **newest first**. Everything that walks "all objects"
//!   (`getFirstObject()`/`getNextObject()`, 28 calls in the logic, the CRC,
//!   the disabled-status pass at the end of a frame, `TerrainLogic`, script
//!   conditions) sees this order, and so does any result built from it.
//! - **`m_objVector`**, `std::vector<Object*>` indexed by `ObjectID`, 8192
//!   entries to begin with, doubled until the id fits. `findObjectByID` is an
//!   index into it.
//!
//! Here the objects live once, in a slab ([`ObjectArena`]); the list is the
//! same intrusive list, but its links are **ids**, kept next to the object in
//! its slab entry (so [`Object`] need not know it is in a list, and the
//! Object translator cannot forget to maintain the links); and the lookup
//! table is a `Vec<u32>` indexed by id, as upstream's, holding the slab
//! position. Slab positions are internal: nothing observable depends on them.
//!
//! # Semantics kept from upstream (each is relied on by callers)
//!
//! - **A destroyed object is still found.** `destroyObject` only marks the
//!   object and queues it; it stays in the list and the table until
//!   `processDestroyList` at the end of the frame. [`ObjectArena::find`]
//!   therefore returns destroyed objects, and callers check
//!   [`Object::is_destroyed`] (upstream's pattern at the 373 call sites).
//! - **`INVALID_ID` is never found**, and so is an id beyond the table.
//! - **The table grows by doubling**; ids are never reused during a game
//!   (the allocator is a counter).
//!
//! # Where upstream is undefined (Rust: an error, never a crash)
//!
//! - `findObjectByID` with a negative id indexes before the vector: here
//!   `None`.
//! - `addObjectToLookupTable` with a negative id (an `Int` compared with a
//!   `size_t`, so it doubles forever) or with `INVALID_ID`: [`ArenaError::BadId`].
//!   An id of 2^28 or more would need a table of 2 GiB: [`ArenaError::BadId`]
//!   too (a game allocates one id per object; 2^28 is unreachable).
//! - Registering an id that is already registered silently overwrites the
//!   table entry upstream (and corrupts the list): [`ArenaError::DuplicateId`].
//! - `prependToList` of an object already in a list asserts in debug and
//!   corrupts the list in release; same error.
//!
//! # The shell
//!
//! [`Object`] below is **the shell** of `Object.h`: the few members that
//! `GameLogic` itself reads (id, status, disabled flags, the behaviour modules
//! with an update interface). The Object translator (layer 8) grows it in
//! place; the contract it must keep is in `docs/world-design.md`
//! ("What the Object translator must keep").

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::DisabledMaskType;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_DESTROYED};
use crate::logic::damage::DamageInfo;
use crate::logic::module::update_module::UpdateModule;
use crate::logic::world::ids::TeamId;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `OBJ_HASH_SIZE` of `GameLogic.cpp`: the initial size of the lookup table.
pub const OBJ_HASH_SIZE: usize = 8192;

/// Ids of 2^28 and above are refused (see the module docs).
pub const MAX_LOOKUP_ID: ObjectId = 1 << 28;

/// One behaviour module of an object that has an update interface, as the
/// scheduler sees it.
///
/// Upstream's `BehaviorModule*` is a heap object whose `getUpdate()` yields the
/// `UpdateModuleInterface` of the same object. Here the **scheduling half**
/// (`UpdateModule`: the next-call word and the heap index, which the scheduler
/// reads while another module of the same object may be running) stays in the
/// arena at all times, and the **behaviour half** (the concrete module's data)
/// is an [`UpdateBody`] that is *checked out* while it runs (see
/// `docs/world-design.md`, "Re-entrancy").
#[derive(Clone, Debug)]
pub struct BehaviorSlot {
    /// The `UpdateModule` base class.
    pub update: UpdateModule,
    /// The concrete module; `None` while it is running.
    pub body: Option<UpdateBody>,
}

/// The shell of `Object`. See the module docs.
#[derive(Clone, Debug)]
pub struct Object {
    /// `m_id`, set by `allocateObjectID` before the modules are made.
    pub id: ObjectId,
    /// `m_status`.
    pub status: ObjectStatusMaskType,
    /// `m_disabledMask`.
    pub disabled: DisabledMaskType,
    /// `getPosition()`. (The `Thing` base arrives with the Object translator.)
    pub position: Coord3D,
    /// `m_team`, by id (0 = none).
    pub team: TeamId,
    /// `m_behaviors`, the ones with an update interface, in module order.
    pub behaviors: Vec<BehaviorSlot>,
    /// Stand-in for `BodyModule::attemptDamage`: the damage handed to this
    /// object through the cut point, oldest first. Replaced by the body module.
    pub damage_received: Vec<DamageInfo>,
}

impl Object {
    /// A fresh object with the given id: nothing set, no behaviours.
    pub fn new(id: ObjectId) -> Object {
        Object {
            id,
            status: ObjectStatusMaskType::new(),
            disabled: DisabledMaskType::new(),
            position: Coord3D { x: 0.0, y: 0.0, z: 0.0 },
            team: 0,
            behaviors: Vec::new(),
            damage_received: Vec::new(),
        }
    }

    /// `getID()`.
    pub fn get_id(&self) -> ObjectId {
        self.id
    }

    /// `isDestroyed()`: `m_status.test(OBJECT_STATUS_DESTROYED)`.
    pub fn is_destroyed(&self) -> bool {
        self.status.test(OBJECT_STATUS_DESTROYED)
    }

    /// `isDisabled()`: any bit of `m_disabledMask`.
    pub fn is_disabled(&self) -> bool {
        self.disabled.any()
    }

    /// `setStatus(MAKE_OBJECT_STATUS_MASK(bit))`: set that one bit.
    pub fn set_status_bit(&mut self, bit: i32) {
        self.status.set(bit, 1);
    }
}

/// What can go wrong with the arena (each is undefined upstream).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArenaError {
    /// `INVALID_ID`, a negative id or one of 2^28 and above.
    BadId(ObjectId),
    /// The id is already registered.
    DuplicateId(ObjectId),
}

#[derive(Clone, Debug)]
struct Entry {
    object: Object,
    /// `m_prev`: toward the head (newer). `INVALID_ID` for none.
    prev: ObjectId,
    /// `m_next`: toward the tail (older). `INVALID_ID` for none.
    next: ObjectId,
}

/// `m_objList` + `m_objVector`. See the module docs.
#[derive(Clone, Debug)]
pub struct ObjectArena {
    /// `m_objVector`: slab position + 1 per id, 0 for `NULL`.
    lookup: Vec<u32>,
    /// The objects, in no meaningful order (see the module docs).
    slab: Vec<Option<Entry>>,
    /// Free slab positions, most recently freed last (deterministic).
    free: Vec<u32>,
    /// `m_objList`: the head (newest), `INVALID_ID` when empty.
    head: ObjectId,
    count: usize,
}

impl Default for ObjectArena {
    fn default() -> Self {
        ObjectArena::new()
    }
}

impl ObjectArena {
    /// `GameLogic::reset()`'s `m_objVector.resize(OBJ_HASH_SIZE, NULL)` and an
    /// empty list.
    pub fn new() -> ObjectArena {
        ObjectArena { lookup: vec![0; OBJ_HASH_SIZE], slab: Vec::new(), free: Vec::new(), head: INVALID_ID, count: 0 }
    }

    /// The number of objects in the list.
    pub fn len(&self) -> usize {
        self.count
    }

    /// No objects.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The size of the lookup table (`m_objVector.size()`).
    pub fn lookup_len(&self) -> usize {
        self.lookup.len()
    }

    fn entry(&self, id: ObjectId) -> Option<&Entry> {
        if id <= INVALID_ID {
            return None;
        }
        match self.lookup.get(id as usize) {
            Some(&p) if p != 0 => self.slab[(p - 1) as usize].as_ref(),
            _ => None,
        }
    }

    fn entry_mut(&mut self, id: ObjectId) -> Option<&mut Entry> {
        if id <= INVALID_ID {
            return None;
        }
        match self.lookup.get(id as usize) {
            Some(&p) if p != 0 => self.slab[(p - 1) as usize].as_mut(),
            _ => None,
        }
    }

    /// `findObjectByID(id)`: `None` for `INVALID_ID`, an unregistered id or
    /// one beyond the table. Finds destroyed objects (see the module docs).
    pub fn find(&self, id: ObjectId) -> Option<&Object> {
        self.entry(id).map(|e| &e.object)
    }

    /// `findObjectByID(id)` for a caller that changes the object.
    pub fn find_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.entry_mut(id).map(|e| &mut e.object)
    }

    /// The scheduling half of the module `h` names: `None` if the object is
    /// not registered or has no such behaviour.
    pub fn update_module(&self, h: UpdateHandle) -> Option<&UpdateModule> {
        self.find(h.object)?.behaviors.get(h.index as usize).map(|b| &b.update)
    }

    /// [`ObjectArena::update_module`] for a caller that changes it.
    pub fn update_module_mut(&mut self, h: UpdateHandle) -> Option<&mut UpdateModule> {
        self.find_mut(h.object)?.behaviors.get_mut(h.index as usize).map(|b| &mut b.update)
    }

    /// Checks the behaviour half of module `h` out of its slot. `None` if
    /// there is no such module or it is already checked out (it is running:
    /// see `docs/world-design.md`, "Re-entrancy").
    pub fn take_body(&mut self, h: UpdateHandle) -> Option<UpdateBody> {
        self.find_mut(h.object)?.behaviors.get_mut(h.index as usize)?.body.take()
    }

    /// Puts a checked-out behaviour back. Dropped if the module is gone.
    pub fn put_body(&mut self, h: UpdateHandle, body: UpdateBody) {
        if let Some(slot) = self.find_mut(h.object).and_then(|o| o.behaviors.get_mut(h.index as usize)) {
            slot.body = Some(body);
        }
    }

    /// `getFirstObject()`: the newest object, `INVALID_ID` if none.
    pub fn first_id(&self) -> ObjectId {
        self.head
    }

    /// `obj->getNextObject()`: the next older object; `INVALID_ID` at the end
    /// or for an object that is not in the list.
    pub fn next_id(&self, id: ObjectId) -> ObjectId {
        self.entry(id).map_or(INVALID_ID, |e| e.next)
    }

    /// `obj->getPrevObject()` (toward the newest).
    pub fn prev_id(&self, id: ObjectId) -> ObjectId {
        self.entry(id).map_or(INVALID_ID, |e| e.prev)
    }

    /// The ids in list order (newest first): the snapshot a caller takes when
    /// upstream's loop caches `getNextObject()` before it acts on the current
    /// object (`destroyAllObjectsImmediate`).
    pub fn ids_in_order(&self) -> Vec<ObjectId> {
        let mut v = Vec::with_capacity(self.count);
        let mut id = self.head;
        while id != INVALID_ID {
            v.push(id);
            id = self.next_id(id);
        }
        v
    }

    /// `obj->isInList(&m_objList)`: `m_prev || m_next || head == obj`.
    pub fn is_in_list(&self, id: ObjectId) -> bool {
        match self.entry(id) {
            Some(e) => e.prev != INVALID_ID || e.next != INVALID_ID || self.head == id,
            None => false,
        }
    }

    /// `addObjectToLookupTable` + `prependToList`, in `registerObject`'s order:
    /// the object becomes the new head of the list and findable by id.
    pub fn insert_at_head(&mut self, object: Object) -> Result<(), ArenaError> {
        let id = object.id;
        if id <= INVALID_ID || id >= MAX_LOOKUP_ID {
            return Err(ArenaError::BadId(id));
        }
        if self.entry(id).is_some() {
            return Err(ArenaError::DuplicateId(id));
        }
        // prependToList
        let old_head = self.head;
        let entry = Entry { object, prev: INVALID_ID, next: old_head };
        let pos = match self.free.pop() {
            Some(p) => {
                self.slab[p as usize] = Some(entry);
                p
            }
            None => {
                self.slab.push(Some(entry));
                (self.slab.len() - 1) as u32
            }
        };
        if old_head != INVALID_ID {
            if let Some(h) = self.entry_mut(old_head) {
                h.prev = id;
            }
        }
        self.head = id;
        // addObjectToLookupTable: "while (newID >= size) resize(size * 2)"
        while (id as usize) >= self.lookup.len() {
            let n = self.lookup.len() * 2;
            self.lookup.resize(n, 0);
        }
        self.lookup[id as usize] = pos + 1;
        self.count += 1;
        Ok(())
    }

    /// `removeFromList(&m_objList)` + `removeObjectFromLookupTable`, in
    /// `processDestroyList`'s order. Returns the object (upstream then calls
    /// `friend_deleteInstance`), or `None` if it is not registered.
    pub fn remove(&mut self, id: ObjectId) -> Option<Object> {
        let (prev, next) = {
            let e = self.entry(id)?;
            (e.prev, e.next)
        };
        // removeFromList: if (m_next) m_next->m_prev = m_prev; if (m_prev) m_prev->m_next = m_next; else head = m_next
        if next != INVALID_ID {
            if let Some(n) = self.entry_mut(next) {
                n.prev = prev;
            }
        }
        if prev != INVALID_ID {
            if let Some(p) = self.entry_mut(prev) {
                p.next = next;
            }
        } else {
            self.head = next;
        }
        // removeObjectFromLookupTable
        let pos = self.lookup[id as usize] - 1;
        self.lookup[id as usize] = 0;
        let entry = self.slab[pos as usize].take();
        self.free.push(pos);
        self.count -= 1;
        entry.map(|e| e.object)
    }

    /// `GameLogic::reset`'s `m_objVector.clear(); resize(OBJ_HASH_SIZE)` and
    /// `m_objList = NULL`, after the objects are destroyed.
    pub fn clear(&mut self) {
        *self = ObjectArena::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(id: ObjectId) -> Object {
        Object::new(id)
    }

    #[test]
    fn the_list_is_newest_first_and_find_uses_the_id() {
        let mut a = ObjectArena::new();
        for id in [1, 2, 3] {
            a.insert_at_head(obj(id)).unwrap();
        }
        assert_eq!(a.ids_in_order(), [3, 2, 1]);
        assert_eq!(a.first_id(), 3);
        assert_eq!(a.next_id(3), 2);
        assert_eq!(a.prev_id(2), 3);
        assert_eq!(a.next_id(1), INVALID_ID);
        assert_eq!(a.find(2).map(|o| o.id), Some(2));
        assert!(a.find(4).is_none());
        assert!(a.find(INVALID_ID).is_none());
        assert!(a.find(-5).is_none(), "undefined upstream: None here");
        assert!(a.find(i32::MAX).is_none());
        assert_eq!(a.len(), 3);
    }

    #[test]
    fn remove_relinks_like_remove_from_list() {
        let mut a = ObjectArena::new();
        for id in 1..=5 {
            a.insert_at_head(obj(id)).unwrap();
        }
        // head, middle, tail
        assert_eq!(a.remove(5).unwrap().id, 5);
        assert_eq!(a.first_id(), 4);
        assert_eq!(a.prev_id(4), INVALID_ID);
        assert_eq!(a.remove(3).unwrap().id, 3);
        assert_eq!(a.ids_in_order(), [4, 2, 1]);
        assert_eq!(a.remove(1).unwrap().id, 1);
        assert_eq!(a.ids_in_order(), [4, 2]);
        assert!(a.remove(1).is_none());
        assert!(a.find(3).is_none());
        assert_eq!(a.remove(4).unwrap().id, 4);
        assert_eq!(a.remove(2).unwrap().id, 2);
        assert!(a.is_empty());
        assert_eq!(a.first_id(), INVALID_ID);
    }

    #[test]
    fn is_in_list_is_prev_or_next_or_head() {
        let mut a = ObjectArena::new();
        assert!(!a.is_in_list(1));
        a.insert_at_head(obj(1)).unwrap();
        assert!(a.is_in_list(1), "a sole member is the head");
        a.insert_at_head(obj(2)).unwrap();
        assert!(a.is_in_list(1) && a.is_in_list(2));
        a.remove(1);
        assert!(!a.is_in_list(1));
    }

    #[test]
    fn the_table_doubles_until_the_id_fits() {
        let mut a = ObjectArena::new();
        assert_eq!(a.lookup_len(), OBJ_HASH_SIZE);
        a.insert_at_head(obj(OBJ_HASH_SIZE as i32 - 1)).unwrap();
        assert_eq!(a.lookup_len(), OBJ_HASH_SIZE);
        a.insert_at_head(obj(OBJ_HASH_SIZE as i32)).unwrap();
        assert_eq!(a.lookup_len(), 2 * OBJ_HASH_SIZE);
        a.insert_at_head(obj(5 * OBJ_HASH_SIZE as i32 + 3)).unwrap();
        assert_eq!(a.lookup_len(), 8 * OBJ_HASH_SIZE);
        assert!(a.find(5 * OBJ_HASH_SIZE as i32 + 3).is_some());
    }

    #[test]
    fn undefined_ids_and_duplicates_are_errors() {
        let mut a = ObjectArena::new();
        assert_eq!(a.insert_at_head(obj(0)), Err(ArenaError::BadId(0)));
        assert_eq!(a.insert_at_head(obj(-3)), Err(ArenaError::BadId(-3)));
        assert_eq!(a.insert_at_head(obj(MAX_LOOKUP_ID)), Err(ArenaError::BadId(MAX_LOOKUP_ID)));
        a.insert_at_head(obj(7)).unwrap();
        assert_eq!(a.insert_at_head(obj(7)), Err(ArenaError::DuplicateId(7)));
        assert_eq!(a.len(), 1);
        a.clear();
        assert!(a.is_empty() && a.find(7).is_none());
    }

    #[test]
    fn destroyed_objects_are_still_found() {
        let mut a = ObjectArena::new();
        a.insert_at_head(obj(1)).unwrap();
        a.find_mut(1).unwrap().set_status_bit(OBJECT_STATUS_DESTROYED);
        assert!(a.find(1).unwrap().is_destroyed());
        assert!(!a.find(1).unwrap().is_disabled());
    }
}
