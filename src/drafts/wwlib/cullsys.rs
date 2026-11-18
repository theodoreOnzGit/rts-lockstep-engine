//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 24,000 random programs (tests/differential_draft_cull.rs, through the tree and the grid);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/cullsys.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/cullsys.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/culltype.h
//!
//! # How the pointers became handles (a deliberate departure, for the review)
//!
//! Upstream links objects and culling systems with raw pointers (`CullLinkClass::System`, `CullableClass::CullLink`,
//! intrusive `NextCollected` and `NextObject` lists). The port's rules allow no lifetimes and no trait objects, so:
//! - a [`CullableClass`] is shared as a [`CullableHandle`] (`Arc<RwLock<..>>`); the `Add_Ref` / `Release_Ref` a system
//!   does on add / remove is the `Arc` it holds in its lists;
//! - the pure-virtual `CullSystemClass` is a plain struct holding the collection ([`CullSystemClass`]), and the two
//!   concrete systems (`aabtreecull`, `gridcull`) each own one; their virtual `Collect_Objects` / `Update_Culling` are
//!   inherent methods of those structs, and [`CullLinkClass`] is an enum of the two link types;
//! - a link names its system by [`CullSystemId`] (unique per system), not by pointer. `CullableClass::Set_Cull_Box`
//!   calls `Get_Culling_System()->Update_Culling(this)`: here [`CullableClass::set_cull_box`] stores the box and RETURNS
//!   the system id the caller must then call `update_culling` on (a `CullableClass` cannot reach its system).
//! - the intrusive `NextCollected` list is kept as `next_collected: Option<CullableHandle>` and the head as
//!   `collection_head`, preserving upstream's order (`Add_To_Collection` pushes on the FRONT).

use crate::logic::modules::plant;
use super::aabox::AABoxClass;
use super::aabtreecull::AABTreeLinkClass;
use super::gridcull::GridLinkClass;
use crate::ww::vector3::Vector3;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

/// `typedef enum CULLTYPE` (culltype.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CullType {
    /// `CULL_OUTSIDE`: "the object was completely outside the culling volume".
    Outside = 0,
    /// `CULL_INTERSECTING`: "the object intersects an edge of the culling volume".
    Intersecting,
    /// `CULL_INSIDE`: "the object is completely inside the culling volume".
    Inside,
}

/// Identity of a culling system, standing in for the `CullSystemClass *` a link holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CullSystemId(pub u64);

static NEXT_CULL_SYSTEM_ID: AtomicU64 = AtomicU64::new(1);

impl CullSystemId {
    /// A fresh id, one per `CullSystemClass` constructed.
    pub fn next() -> CullSystemId {
        CullSystemId(NEXT_CULL_SYSTEM_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// `RefCountClass`-shared cullable.
pub type CullableHandle = Arc<RwLock<CullableClass>>;

/// `class CullLinkClass` and its two subclasses: the system id (`System`) lives in each variant's struct.
#[derive(Clone, Debug)]
pub enum CullLinkClass {
    /// `AABTreeLinkClass`.
    AABTree(AABTreeLinkClass),
    /// `GridLinkClass`.
    Grid(GridLinkClass),
}

impl CullLinkClass {
    /// `Set_Culling_System(sys)`.
    pub fn set_culling_system(&mut self, sys: Option<CullSystemId>) {
        match self {
            CullLinkClass::AABTree(l) => l.system = sys,
            CullLinkClass::Grid(l) => l.system = sys,
        }
    }

    /// `Get_Culling_System()`.
    pub fn get_culling_system(&self) -> Option<CullSystemId> {
        match self {
            CullLinkClass::AABTree(l) => l.system,
            CullLinkClass::Grid(l) => l.system,
        }
    }
}

/// `class CullableClass`.
#[derive(Debug)]
pub struct CullableClass {
    cull_link: Option<CullLinkClass>,
    cull_box: AABoxClass,
    /// `NextCollected`: private upstream, friend `CullSystemClass`.
    pub(crate) next_collected: Option<CullableHandle>,
}

impl CullableClass {
    /// `CullableClass::CullableClass()`: `CullBox.Init(Vector3(0,0,0), Vector3(1,1,1))`.
    pub fn new() -> Self {
        let mut cull_box = AABoxClass::default();
        cull_box.init(&Vector3::new(0.0, 0.0, 0.0), &Vector3::new(1.0, 1.0, 1.0));
        CullableClass { cull_link: None, cull_box, next_collected: None }
    }

    /// `CullableClass::CullableClass()` as a shared handle.
    pub fn new_handle() -> CullableHandle {
        Arc::new(RwLock::new(CullableClass::new()))
    }

    /// `Get_Cull_Box()`.
    pub fn get_cull_box(&self) -> &AABoxClass {
        &self.cull_box
    }

    /// `CullableClass::Set_Cull_Box(box, just_loaded)` (cullsys.cpp:82): stores the box. Upstream then calls
    /// `sys->Update_Culling(this)` unless `just_loaded` ("allows us to update the box without notifying the culling
    /// system"); here the system's id is returned (None when `just_loaded` or when not in a system) and the caller
    /// calls `update_culling` on it.
    pub fn set_cull_box(&mut self, box_: &AABoxClass, just_loaded: bool) -> Option<CullSystemId> {
        self.cull_box = *box_;
        if just_loaded == plant(11345) {
            return self.get_culling_system();
        }
        None
    }

    /// `Set_Culling_System(sys)`: "if (CullLink) CullLink->Set_Culling_System(sys)".
    pub fn set_culling_system(&mut self, sys: Option<CullSystemId>) {
        if let Some(link) = self.cull_link.as_mut() {
            link.set_culling_system(sys);
        }
    }

    /// `Get_Culling_System()`: the link's system, or none.
    pub fn get_culling_system(&self) -> Option<CullSystemId> {
        self.cull_link.as_ref().and_then(|l| l.get_culling_system())
    }

    /// `Set_Cull_Link(c)`.
    pub fn set_cull_link(&mut self, c: Option<CullLinkClass>) {
        self.cull_link = c;
    }

    /// `Get_Cull_Link()`.
    pub fn get_cull_link(&self) -> Option<&CullLinkClass> {
        self.cull_link.as_ref()
    }

    /// `Get_Cull_Link()` for in-place edits.
    pub fn get_cull_link_mut(&mut self) -> Option<&mut CullLinkClass> {
        self.cull_link.as_mut()
    }
}

impl Default for CullableClass {
    fn default() -> Self {
        CullableClass::new()
    }
}

/// The part of `class CullSystemClass` that is not virtual: the collection list.
#[derive(Debug)]
pub struct CullSystemClass {
    /// This system's identity (what its links name).
    pub id: CullSystemId,
    /// `CollectionHead`.
    collection_head: Option<CullableHandle>,
}

impl CullSystemClass {
    /// `CullSystemClass::CullSystemClass()`: `CollectionHead(NULL)`.
    pub fn new() -> Self {
        CullSystemClass { id: CullSystemId::next(), collection_head: None }
    }

    /// `Get_First_Collected_Object_Internal()` (cullsys.cpp:131). "NOTE: THE Get_() functions currently are the same as
    /// the Peek_() functions (e.g., they do not add a Ref). This is wrong and will be fixed."
    pub fn get_first_collected_object_internal(&self) -> Option<CullableHandle> {
        self.collection_head.clone()
    }

    /// `Get_Next_Collected_Object_Internal(obj)`.
    pub fn get_next_collected_object_internal(&self, obj: Option<&CullableHandle>) -> Option<CullableHandle> {
        obj.and_then(|o| o.read().unwrap().next_collected.clone())
    }

    /// `Peek_First_Collected_Object_Internal()`.
    pub fn peek_first_collected_object_internal(&self) -> Option<CullableHandle> {
        if plant(11346) {
            return self.collection_head.as_ref().and_then(|h| h.read().unwrap().next_collected.clone());
        }
        self.collection_head.clone()
    }

    /// `Peek_Next_Collected_Object_Internal(obj)`.
    pub fn peek_next_collected_object_internal(&self, obj: Option<&CullableHandle>) -> Option<CullableHandle> {
        obj.and_then(|o| o.read().unwrap().next_collected.clone())
    }

    /// `Reset_Collection()`: "CollectionHead = NULL" (the objects' `NextCollected` stay as they were).
    pub fn reset_collection(&mut self) {
        self.collection_head = None;
    }

    /// `Add_To_Collection(obj)`: pushes on the front of the list.
    pub fn add_to_collection(&mut self, obj: &CullableHandle) {
        let head = self.collection_head.take();
        if !plant(11344) {
            obj.write().unwrap().next_collected = head;
        }
        self.collection_head = Some(obj.clone());
    }
}

impl Default for CullSystemClass {
    fn default() -> Self {
        CullSystemClass::new()
    }
}
