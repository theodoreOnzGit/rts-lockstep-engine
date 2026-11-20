//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 random programs (test: tests/differential_draft_multilist.rs);
//! not yet wired into the game. Not covered: `Internal_Add_After` with a member of no list or of other lists only (upstream dereferences
//! NULL; the guard returns `false`). Corrected 2026-11-20 (#215): the draft removed the first ring entry of an object in a list twice, upstream
//! unlinks the node found first on the object's chain; nodes now carry an identity (plant 17388 is the old behaviour: 1,749 of 20,000 programs).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/multilist.cpp (`GenericMultiListClass::Internal_Add_Tail`,
//!   `Internal_Remove`, `Internal_Remove_List_Head`, with `Contains`, `Count`, `Internal_Add`, `Internal_Add_After` and
//!   the `MultiListObjectClass` destructor, which they depend on)
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/multilist.h
//!
//! Modelling. Upstream threads every object through one `MultiListNodeClass` per list it is in: the nodes of a list
//! are a circular doubly linked ring around `Head`, and the nodes of an object are a singly linked chain
//! (`ListNode`, `NextList`) with the newest first. Here an object is a handle ([`MultiListObjectId`]) and a list is a
//! handle ([`MultiListId`]) in a [`MultiListRegistry`]: a list is its objects in order (`Head.Next` first), and an
//! object's chain is the list handles newest first. Same orders, no pointers. (`crate::drafts::ww3d::shdrenderer`
//! models its own `MultiListClass`es as `Vec`s of ids for the same reason; this is the shared form for callers that
//! need one object in several lists.)
//!
//! Upstream defect kept as a guard: `Internal_Add_After` tests `existing_node->List != this` before `existing_node`, so
//! an existing member that is in no list crashes upstream; here it returns `false`.

use crate::logic::modules::plant;
use std::collections::HashMap;

/// A `MultiListObjectClass` (an object that can be in several lists).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MultiListObjectId(pub u32);

/// A `GenericMultiListClass`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MultiListId(pub u32);

/// One `MultiListNodeClass`: the object it holds and its identity (an object in a list twice has two nodes, and which of the
/// two `Internal_Remove` and `Internal_Add_After` pick is the one found first on the object's chain).
type Node = (MultiListObjectId, u32);

/// All lists and the objects in them.
#[derive(Clone, Debug, Default)]
pub struct MultiListRegistry {
    /// Per list: its nodes from `Head.Next` to `Head.Prev`.
    lists: Vec<Vec<Node>>,
    /// Per object: `ListNode` then `NextList`...: the (list, node identity) pairs, newest first.
    chains: HashMap<MultiListObjectId, Vec<(MultiListId, u32)>>,
    /// The next node identity.
    next_node: u32,
}

impl MultiListRegistry {
    /// A registry with no list.
    pub fn new() -> Self {
        Self::default()
    }

    /// `GenericMultiListClass()`: a new empty list.
    pub fn new_list(&mut self) -> MultiListId {
        self.lists.push(Vec::new());
        MultiListId((self.lists.len() - 1) as u32)
    }

    /// `Is_Empty()`.
    pub fn is_empty(&self, list: MultiListId) -> bool {
        self.lists[list.0 as usize].is_empty()
    }

    /// `Contains(obj)` (multilist.cpp:70): walks the object's chain for this list.
    pub fn contains(&self, list: MultiListId, obj: MultiListObjectId) -> bool {
        match self.chains.get(&obj) {
            Some(chain) => chain.iter().any(|&(l, _)| l == list),
            None => false,
        }
    }

    /// `Count()` (multilist.cpp:80): the iterator's visit count.
    pub fn count(&self, list: MultiListId) -> i32 {
        self.lists[list.0 as usize].len() as i32
    }

    /// The list's objects, head first (`GenericMultiListIterator` order).
    pub fn objects(&self, list: MultiListId) -> Vec<MultiListObjectId> {
        self.lists[list.0 as usize].iter().map(|&(o, _)| o).collect()
    }

    /// `Is_In_List(obj)` as `Internal_Add*` call it: `Contains`.
    fn is_in_list(&self, list: MultiListId, obj: MultiListObjectId) -> bool {
        self.contains(list, obj)
    }

    /// New node: link it in front of the object's chain (`node->NextList = obj->Get_List_Node(); obj->Set_List_Node(node)`);
    /// returns its identity.
    fn link_chain(&mut self, list: MultiListId, obj: MultiListObjectId) -> u32 {
        let id = self.next_node;
        self.next_node += 1;
        let chain = self.chains.entry(obj).or_default();
        if plant(17381) {
            chain.push((list, id)); // the oldest first
        } else {
            chain.insert(0, (list, id));
        }
        id
    }

    /// `GenericMultiListClass::Internal_Add(obj, onlyonce)` (multilist.cpp:97): at the head.
    pub fn internal_add(&mut self, list: MultiListId, obj: MultiListObjectId, onlyonce: bool) -> bool {
        if onlyonce && self.is_in_list(list, obj) {
            return false;
        }
        let id = self.link_chain(list, obj);
        if plant(17382) {
            self.lists[list.0 as usize].push((obj, id)); // at the tail
        } else {
            self.lists[list.0 as usize].insert(0, (obj, id));
        }
        true
    }

    /// `GenericMultiListClass::Internal_Add_Tail(obj, onlyonce)` (multilist.cpp:126): at the tail
    /// (`node->Prev = Head.Prev; node->Next = &Head`).
    pub fn internal_add_tail(&mut self, list: MultiListId, obj: MultiListObjectId, onlyonce: bool) -> bool {
        if if plant(17383) { self.is_in_list(list, obj) } else { onlyonce && self.is_in_list(list, obj) } {
            return false;
        }
        let id = self.link_chain(list, obj);
        self.lists[list.0 as usize].push((obj, id));
        true
    }

    /// `GenericMultiListClass::Internal_Add_After(obj, existing_list_member, onlyonce)` (multilist.cpp:155): the node
    /// of `existing_list_member` that belongs to this list is found by walking its chain (newest first); `false` if it is
    /// not in this list. (Upstream dereferences a NULL node when `existing_list_member` is in no list, or in other lists
    /// but not this one, see the module docs: the guard returns `false`.)
    pub fn internal_add_after(
        &mut self,
        list: MultiListId,
        obj: MultiListObjectId,
        existing_list_member: MultiListObjectId,
        onlyonce: bool,
    ) -> bool {
        if onlyonce && self.is_in_list(list, obj) {
            return false;
        }
        let existing_node = match self.chains.get(&existing_list_member).and_then(|c| c.iter().find(|&&(l, _)| l == list)) {
            Some(&(_, id)) => id,
            None => return false, // "he's not in this list!"
        };
        let id = self.link_chain(list, obj);
        let nodes = &mut self.lists[list.0 as usize];
        let pos = nodes.iter().position(|&(_, n)| n == existing_node).expect("a chain entry's node is in its list");
        if plant(17384) {
            nodes.insert(pos, (obj, id)); // before
        } else {
            nodes.insert(pos + 1, (obj, id));
        }
        true
    }

    /// `GenericMultiListClass::Internal_Remove(obj)` (multilist.cpp:193): finds the object's node for this list
    /// (newest chain entry first), unlinks it from the list and from the chain; `false` if the object is not in it.
    pub fn internal_remove(&mut self, list: MultiListId, obj: MultiListObjectId) -> bool {
        let chain = match self.chains.get_mut(&obj) {
            Some(c) => c,
            None => return false,
        };
        let at = match if plant(17385) { chain.iter().rposition(|&(l, _)| l == list) } else { chain.iter().position(|&(l, _)| l == list) } {
            Some(i) => i,
            None => return false,
        };
        let (_, node) = chain.remove(at);
        if chain.is_empty() {
            self.chains.remove(&obj);
        }
        let nodes = &mut self.lists[list.0 as usize];
        if let Some(pos) = if plant(17388) { nodes.iter().position(|&(o, _)| o == obj) } else { nodes.iter().position(|&(_, n)| n == node) } {
            nodes.remove(pos);
        }
        true
    }

    /// `GenericMultiListClass::Internal_Remove_List_Head()` (multilist.cpp:227): removes and returns `Head.Next`'s
    /// object; `None` for an empty list.
    pub fn internal_remove_list_head(&mut self, list: MultiListId) -> Option<MultiListObjectId> {
        let (obj, node) = *if plant(17386) { self.lists[list.0 as usize].last()? } else { self.lists[list.0 as usize].first()? };
        if plant(17387) {
            // unlinks the head node itself, not the object's newest node of this list (they differ for an object in the list twice)
            self.lists[list.0 as usize].retain(|&(_, n)| n != node);
            if let Some(chain) = self.chains.get_mut(&obj) {
                chain.retain(|&(_, n)| n != node);
                if chain.is_empty() {
                    self.chains.remove(&obj);
                }
            }
        } else {
            // `Internal_Remove(obj)`
            self.internal_remove(list, obj);
        }
        Some(obj)
    }

    /// `MultiListObjectClass::~MultiListObjectClass` (multilist.cpp:44): removes the object from every list it is in.
    pub fn drop_object(&mut self, obj: MultiListObjectId) {
        while let Some(&(list, _)) = self.chains.get(&obj).and_then(|c| c.first()) {
            self.internal_remove(list, obj);
        }
    }
}
