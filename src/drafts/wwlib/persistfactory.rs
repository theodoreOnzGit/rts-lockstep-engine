//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 10,000 random programs of WWSaveLoad operations (test: tests/differential_draft_saveload.rs);
//! not yet wired into the game. Not covered: `PersistObject::on_post_load` (the draft's has no body to observe). Corrected 2026-11-20 (#215): a persist factory has an
//! instance id (upstream: its address) and `Unregister_Persist_Factory` removes that factory, not the first with its chunk id.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/persist.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/postloadable.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/persistfactory.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/persistfactory.cpp
//!
//! # How the class hierarchy became data (a deliberate departure, for the review)
//!
//! Upstream: `PersistClass` is an abstract base, each concrete type has a static `PersistFactoryClass` (a virtual
//! constructor keyed by chunk id) that registers itself with `SaveLoadSystemClass` at static-construction time, and a
//! saved object is identified by its 32-bit address. The port allows no trait objects, so:
//! - the objects the factories construct are the [`PersistObject`] enum (one variant per persistent type in these
//!   drafts: only [`DefinitionClass`](super::definition::DefinitionHandle) so far; add a variant per type translated);
//! - a [`PersistFactoryClass`] is a chunk id and two plain function pointers (`load`, `save`) over [`PersistObject`];
//!   registration is the explicit [`PersistFactoryClass::register`] (upstream does it in the constructor and undoes it in
//!   the destructor: [`PersistFactoryClass::unregister`]);
//! - an object's "old address" (`(uint32)obj`) is a caller-supplied `u32` object id (`PointerRemapClass` of
//!   `pointer_remap.rs` already works on integer ids).
//! `SimplePersistFactoryClass<T,CHUNKID>` (its `Load` / `Save` templates) is [`simple_persist_factory_save`] /
//! [`simple_persist_factory_load`].

use super::chunk_save::ChunkSaveClass;
use super::definition::DefinitionHandle;
use super::saveload::SaveLoadSystemClass;
use crate::logic::modules::plant;
use crate::ww::chunkio::ChunkLoad;
use std::sync::atomic::{AtomicU32, Ordering};

/// `SIMPLEFACTORY_CHUNKID_OBJPOINTER = 0x00100100`, `SIMPLEFACTORY_CHUNKID_OBJDATA` (the next value).
pub const SIMPLEFACTORY_CHUNKID_OBJPOINTER: u32 = 0x00100100;
pub const SIMPLEFACTORY_CHUNKID_OBJDATA: u32 = 0x00100101;

/// A persistent object: what `PersistClass *` points at.
#[derive(Clone)]
pub enum PersistObject {
    /// A `DefinitionClass` (and its subclasses, `TwiddlerClass` among them).
    Definition(DefinitionHandle),
}

impl PersistObject {
    /// `PersistClass::Get_Factory().Chunk_ID()`: the chunk id of the object's factory.
    pub fn factory_chunk_id(&self) -> u32 {
        match self {
            PersistObject::Definition(d) => d.read().unwrap().get_factory_chunk_id(),
        }
    }

    /// `PersistClass::Save(csave)` (the default is `return true`).
    pub fn save(&self, csave: &mut ChunkSaveClass) -> bool {
        match self {
            PersistObject::Definition(d) => d.write().unwrap().save(csave),
        }
    }

    /// `PostLoadableClass::On_Post_Load()` (the default does nothing).
    pub fn on_post_load(&self) {
        match self {
            PersistObject::Definition(_) => {}
        }
    }

    /// `Is_Post_Load_Registered()`.
    pub fn is_post_load_registered(&self) -> bool {
        match self {
            PersistObject::Definition(d) => d.read().unwrap().post_loadable.is_post_load_registered(),
        }
    }

    /// `Set_Post_Load_Registered(onoff)`.
    pub fn set_post_load_registered(&self, onoff: bool) {
        match self {
            PersistObject::Definition(d) => d.write().unwrap().post_loadable.set_post_load_registered(onoff),
        }
    }

    /// Whether two handles are the same object (`obj1 == obj2`).
    pub fn same_object(&self, other: &PersistObject) -> bool {
        match (self, other) {
            (PersistObject::Definition(a), PersistObject::Definition(b)) => std::sync::Arc::ptr_eq(a, b),
        }
    }
}

/// `class PostLoadableClass`: the registered-for-post-load flag (the `On_Post_Load` virtual is
/// [`PersistObject::on_post_load`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct PostLoadableClass {
    is_post_load_registered: bool,
}

impl PostLoadableClass {
    /// `PostLoadableClass()`: `IsPostLoadRegistered(false)`.
    pub fn new() -> Self {
        PostLoadableClass { is_post_load_registered: false }
    }

    /// `Is_Post_Load_Registered()`.
    pub fn is_post_load_registered(&self) -> bool {
        self.is_post_load_registered
    }

    /// `Set_Post_Load_Registered(onoff)`.
    pub fn set_post_load_registered(&mut self, onoff: bool) {
        if plant(18284) && !onoff {
            return;
        }
        self.is_post_load_registered = onoff;
    }
}

/// `class PersistFactoryClass`: "maps a chunk_id to a constructor, a save function, a load function".
#[derive(Clone, Copy)]
pub struct PersistFactoryClass {
    /// `Chunk_ID()`.
    pub chunk_id: u32,
    /// `Load(cload)`: reads the object (and registers its old/new pointer pair).
    pub load: fn(&mut ChunkLoad<'_>) -> Option<PersistObject>,
    /// `Save(csave, obj)`.
    pub save: fn(&mut ChunkSaveClass, &PersistObject),
    /// Which factory this is (upstream: its address). Two factories may share a chunk id; `Unregister_Persist_Factory` removes the one
    /// asked for, not the first with that chunk id. Copies of one factory keep its instance id.
    instance: u32,
}

static NEXT_FACTORY_INSTANCE: AtomicU32 = AtomicU32::new(1);

impl PersistFactoryClass {
    /// `PersistFactoryClass()`: "SaveLoadSystemClass::Register_Persist_Factory(this)" is the explicit [`Self::register`].
    pub fn new(chunk_id: u32, load: fn(&mut ChunkLoad<'_>) -> Option<PersistObject>, save: fn(&mut ChunkSaveClass, &PersistObject)) -> Self {
        PersistFactoryClass { chunk_id, load, save, instance: NEXT_FACTORY_INSTANCE.fetch_add(1, Ordering::Relaxed) }
    }

    /// `Chunk_ID()`.
    pub fn chunk_id(&self) -> u32 {
        self.chunk_id
    }

    /// The constructor's `SaveLoadSystemClass::Register_Persist_Factory(this)`.
    pub fn register(&self) {
        SaveLoadSystemClass::register_persist_factory(*self);
    }

    /// The destructor's `SaveLoadSystemClass::Unregister_Persist_Factory(this)`.
    pub fn unregister(&self) {
        SaveLoadSystemClass::unregister_persist_factory(self.instance, self.chunk_id);
    }

    /// Which factory this is (the address upstream compares).
    pub fn instance(&self) -> u32 {
        self.instance
    }
}

/// `SimplePersistFactoryClass<T,CHUNKID>::Save(csave, obj)`: "objptr" chunk (the object's id as a `uint32`) then the
/// "objdata" chunk written by the object's own `Save`.
pub fn simple_persist_factory_save(csave: &mut ChunkSaveClass, obj_id: u32, save_obj: impl FnOnce(&mut ChunkSaveClass) -> bool) {
    let mut save_obj = Some(save_obj);
    if plant(18281) {
        csave.begin_chunk(SIMPLEFACTORY_CHUNKID_OBJDATA);
        (save_obj.take().unwrap())(csave);
        csave.end_chunk();
    }
    csave.begin_chunk(SIMPLEFACTORY_CHUNKID_OBJPOINTER);
    csave.write(&if plant(18282) { obj_id.to_be_bytes() } else { obj_id.to_le_bytes() });
    csave.end_chunk();

    if let Some(save_obj) = save_obj {
        csave.begin_chunk(SIMPLEFACTORY_CHUNKID_OBJDATA);
        save_obj(csave);
        csave.end_chunk();
    }
}

/// `SimplePersistFactoryClass<T,CHUNKID>::Load(cload)`: the old id, then `new_obj->Load(cload)` in the data chunk;
/// the caller (the factory's `load` function) then does `SaveLoadSystemClass::Register_Pointer(old, new)` with the
/// returned old id. `new_obj` is created by `make` (upstream `W3DNEW T`) before the chunks are read.
pub fn simple_persist_factory_load<T>(cload: &mut ChunkLoad<'_>, make: impl FnOnce() -> T, load_obj: impl FnOnce(&mut T, &mut ChunkLoad<'_>) -> bool) -> (u32, T) {
    let mut new_obj = make();

    let _ = cload.open_chunk();
    assert!(cload.cur_chunk_id().unwrap_or(0) == SIMPLEFACTORY_CHUNKID_OBJPOINTER);
    let mut b = [0u8; 4];
    let _ = cload.read_into(&mut b);
    let old_obj = if plant(18283) { u32::from_le_bytes(b).wrapping_add(1) } else { u32::from_le_bytes(b) };
    let _ = cload.close_chunk();

    let _ = cload.open_chunk();
    assert!(cload.cur_chunk_id().unwrap_or(0) == SIMPLEFACTORY_CHUNKID_OBJDATA);
    load_obj(&mut new_obj, cload);
    let _ = cload.close_chunk();

    (old_obj, new_obj)
}
