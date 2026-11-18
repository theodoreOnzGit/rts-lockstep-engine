//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
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

use std::collections::HashMap;

/// A `MultiListObjectClass` (an object that can be in several lists).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MultiListObjectId(pub u32);

/// A `GenericMultiListClass`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MultiListId(pub u32);

/// All lists and the objects in them.
#[derive(Clone, Debug, Default)]
pub struct MultiListRegistry {
    /// Per list: its objects from `Head.Next` to `Head.Prev`.
    lists: Vec<Vec<MultiListObjectId>>,
    /// Per object: `ListNode` then `NextList`...: the lists it is in, newest first.
    chains: HashMap<MultiListObjectId, Vec<MultiListId>>,
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
            Some(chain) => chain.iter().any(|&l| l == list),
            None => false,
        }
    }

    /// `Count()` (multilist.cpp:80): the iterator's visit count.
    pub fn count(&self, list: MultiListId) -> i32 {
        self.lists[list.0 as usize].len() as i32
    }

    /// The list's objects, head first (`GenericMultiListIterator` order).
    pub fn objects(&self, list: MultiListId) -> &[MultiListObjectId] {
        &self.lists[list.0 as usize]
    }

    /// `Is_In_List(obj)` as `Internal_Add*` call it: `Contains`.
    fn is_in_list(&self, list: MultiListId, obj: MultiListObjectId) -> bool {
        self.contains(list, obj)
    }

    /// New node: link it in front of the object's chain (`node->NextList = obj->Get_List_Node(); obj->Set_List_Node(node)`).
    fn link_chain(&mut self, list: MultiListId, obj: MultiListObjectId) {
        self.chains.entry(obj).or_default().insert(0, list);
    }

    /// `GenericMultiListClass::Internal_Add(obj, onlyonce)` (multilist.cpp:97): at the head.
    pub fn internal_add(&mut self, list: MultiListId, obj: MultiListObjectId, onlyonce: bool) -> bool {
        if onlyonce && self.is_in_list(list, obj) {
            return false;
        }
        self.link_chain(list, obj);
        self.lists[list.0 as usize].insert(0, obj);
        true
    }

    /// `GenericMultiListClass::Internal_Add_Tail(obj, onlyonce)` (multilist.cpp:126): at the tail
    /// (`node->Prev = Head.Prev; node->Next = &Head`).
    pub fn internal_add_tail(&mut self, list: MultiListId, obj: MultiListObjectId, onlyonce: bool) -> bool {
        if onlyonce && self.is_in_list(list, obj) {
            return false;
        }
        self.link_chain(list, obj);
        self.lists[list.0 as usize].push(obj);
        true
    }

    /// `GenericMultiListClass::Internal_Add_After(obj, existing_list_member, onlyonce)` (multilist.cpp:155): the node
    /// of `existing_list_member` that belongs to this list is found by walking its chain; `false` if it is not in this
    /// list. (Upstream dereferences before testing for a missing chain, see the module docs.)
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
        if !self.contains(list, existing_list_member) {
            return false; // "he's not in this list!"
        }
        self.link_chain(list, obj);
        let objects = &mut self.lists[list.0 as usize];
        let pos = objects.iter().position(|&o| o == existing_list_member).unwrap_or(objects.len().saturating_sub(1));
        objects.insert(pos + 1, obj);
        true
    }

    /// `GenericMultiListClass::Internal_Remove(obj)` (multilist.cpp:193): finds the object's node for this list
    /// (newest chain entry first), unlinks it from the list and from the chain; `false` if the object is not in it.
    pub fn internal_remove(&mut self, list: MultiListId, obj: MultiListObjectId) -> bool {
        let chain = match self.chains.get_mut(&obj) {
            Some(c) => c,
            None => return false,
        };
        let at = match chain.iter().position(|&l| l == list) {
            Some(i) => i,
            None => return false,
        };
        chain.remove(at);
        if chain.is_empty() {
            self.chains.remove(&obj);
        }
        // The node that is unlinked is the one in this list's ring. An object in a list twice has two nodes; the
        // chain's newest one is the one added last, which is the one nearest the position it was added at: remove
        // the first match from the head (the ring order does not record which node is which).
        let objects = &mut self.lists[list.0 as usize];
        if let Some(pos) = objects.iter().position(|&o| o == obj) {
            objects.remove(pos);
        }
        true
    }

    /// `GenericMultiListClass::Internal_Remove_List_Head()` (multilist.cpp:227): removes and returns `Head.Next`'s
    /// object; `None` for an empty list.
    pub fn internal_remove_list_head(&mut self, list: MultiListId) -> Option<MultiListObjectId> {
        let obj = *self.lists[list.0 as usize].first()?;
        self.internal_remove(list, obj);
        Some(obj)
    }

    /// `MultiListObjectClass::~MultiListObjectClass` (multilist.cpp:44): removes the object from every list it is in.
    pub fn drop_object(&mut self, obj: MultiListObjectId) {
        while let Some(&list) = self.chains.get(&obj).and_then(|c| c.first()) {
            self.internal_remove(list, obj);
        }
    }
}
