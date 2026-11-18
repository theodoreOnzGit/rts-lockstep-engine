//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/twiddler.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/twiddler.cpp
//!
//! `TwiddlerClass : DefinitionClass`: a definition that, when asked to `Create`, picks one of a list of definition ids
//! at random and indirects to it. It is the [`DefinitionKind::Twiddler`](super::definition::DefinitionKind) variant of
//! [`DefinitionClass`](super::definition::DefinitionClass) (see definition.rs); this file holds its data and its
//! save/load/`Twiddle` behaviour. The editable-parameter registration (`CLASSID_DEFIDLIST_PARAM` "Preset List") is
//! editor-only and not translated. The statics `_TwiddlerFactory` (definition factory, "Twiddler") and
//! `_TwiddlerPersistFactory` (`CHUNKID_TWIDDLER`) are registered by [`register_twiddler_factories`]. `Twiddle` seeds a
//! `RandomClass` from `TIMEGETTIME()` on every call; the caller's `random_int(lo, hi)` replaces it.

use super::chunk_save::ChunkSaveClass;
use super::definition::{DefinitionClass, DefinitionHandle, DefinitionKind, CLASSID_TWIDDLERS};
use super::definitionfactory::DefinitionFactoryClass;
use super::definitionmgr::DefinitionMgrClass;
use super::persistfactory::{simple_persist_factory_load, simple_persist_factory_save, PersistFactoryClass, PersistObject};
use super::saveload::SaveLoadSystemClass;
use super::saveload_ids::CHUNKID_TWIDDLER;
use crate::ww::chunkio::ChunkLoad;
use std::sync::Arc;

/// `CHUNKID_VARIABLES = 0x00000100`, `CHUNKID_BASE_CLASS = 0x00000200`.
pub const CHUNKID_VARIABLES: u32 = 0x00000100;
pub const CHUNKID_BASE_CLASS: u32 = 0x00000200;
/// `VARID_DEFINTION_ID = 0x01`, `VARID_INDIRECT_CLASSID` (2).
const VARID_DEFINTION_ID: u32 = 0x01;
const VARID_INDIRECT_CLASSID: u32 = 0x02;

/// The Twiddler's own data.
#[derive(Clone, Debug, Default)]
pub struct TwiddlerClass {
    /// `m_IndirectClassID`.
    pub indirect_class_id: u32,
    /// `m_DefinitionList`: the definition ids to choose from.
    pub definition_list: Vec<i32>,
}

impl TwiddlerClass {
    /// `TwiddlerClass()`: `m_IndirectClassID(0)`.
    pub fn new() -> Self {
        TwiddlerClass { indirect_class_id: 0, definition_list: Vec::new() }
    }

    /// `Get_Indirect_Class_ID()`.
    pub fn get_indirect_class_id(&self) -> u32 {
        self.indirect_class_id
    }

    /// `Set_Indirect_Class_ID(class_id)`.
    pub fn set_indirect_class_id(&mut self, class_id: u32) {
        self.indirect_class_id = class_id;
    }

    /// `Twiddle()` (twiddler.cpp:84): a random entry of the list; upstream looks it up with
    /// `DefinitionMgrClass::Find_Definition(def_id)` and its default `twiddle = true`, so a twiddler listed in a twiddler
    /// is itself twiddled. `None` when the list is empty or the id is unknown.
    pub fn twiddle(&self, random_int: &mut dyn FnMut(i32, i32) -> i32) -> Option<DefinitionHandle> {
        if !self.definition_list.is_empty() {
            // Get a random index into our definition list
            let index = random_int(0, self.definition_list.len() as i32 - 1);

            // Lookup the definition this entry represents (`def_id` is an `int`, the lookup's id a `uint32`)
            let def_id = self.definition_list[index as usize];
            return DefinitionMgrClass::find_definition(def_id as u32, true, random_int);
        }
        None
    }

    /// `Save_Variables(csave)`: the indirect class id, then one micro chunk per definition id.
    pub fn save_variables(&self, csave: &mut ChunkSaveClass) -> bool {
        csave.write_micro_chunk(VARID_INDIRECT_CLASSID, &self.indirect_class_id.to_le_bytes());

        for def_id in self.definition_list.iter() {
            // Save this definition ID to the chunk
            csave.write_micro_chunk(VARID_DEFINTION_ID, &def_id.to_le_bytes());
        }
        true
    }

    /// `Load_Variables(cload)`: "Start fresh" (the list is cleared), then each micro chunk.
    pub fn load_variables(&mut self, cload: &mut ChunkLoad<'_>) -> bool {
        self.definition_list.clear();

        while cload.open_micro_chunk().unwrap_or(false) {
            match cload.cur_micro_chunk_id() {
                VARID_INDIRECT_CLASSID => {
                    let mut b = [0u8; 4];
                    let _ = cload.read_into(&mut b);
                    self.indirect_class_id = u32::from_le_bytes(b);
                }
                VARID_DEFINTION_ID => {
                    // Read the definition ID from the chunk and add it to our list
                    let mut b = [0u8; 4];
                    let _ = cload.read_into(&mut b);
                    self.definition_list.push(i32::from_le_bytes(b));
                }
                _ => {}
            }
            let _ = cload.close_micro_chunk();
        }
        true
    }
}

/// A new twiddler definition (`new TwiddlerClass`).
pub fn new_twiddler_definition() -> DefinitionHandle {
    DefinitionClass::new_handle(DefinitionKind::Twiddler(TwiddlerClass::new()))
}

/// `_TwiddlerPersistFactory.Load`: `SimplePersistFactoryClass<TwiddlerClass, CHUNKID_TWIDDLER>::Load`, then
/// `Register_Pointer(old_obj, new_obj)`.
fn twiddler_persist_load(cload: &mut ChunkLoad<'_>) -> Option<PersistObject> {
    let (old_obj, new_obj) = simple_persist_factory_load(cload, new_twiddler_definition, |obj, cl| obj.write().unwrap().load(cl));
    SaveLoadSystemClass::register_pointer(old_obj as usize, Arc::as_ptr(&new_obj) as usize);
    Some(PersistObject::Definition(new_obj))
}

/// `_TwiddlerPersistFactory.Save`: the object's address as its id.
fn twiddler_persist_save(csave: &mut ChunkSaveClass, obj: &PersistObject) {
    let PersistObject::Definition(d) = obj;
    let objptr = Arc::as_ptr(d) as usize as u32;
    simple_persist_factory_save(csave, objptr, |cs| d.write().unwrap().save(cs));
}

/// The static registrations of twiddler.cpp: `DECLARE_DEFINITION_FACTORY(TwiddlerClass, CLASSID_TWIDDLERS, "Twiddler")
/// _TwiddlerFactory` and `SimplePersistFactoryClass<TwiddlerClass, CHUNKID_TWIDDLER> _TwiddlerPersistFactory`.
pub fn register_twiddler_factories() {
    DefinitionFactoryClass::new(CLASSID_TWIDDLERS, "Twiddler", true, new_twiddler_definition).register();
    PersistFactoryClass::new(CHUNKID_TWIDDLER, twiddler_persist_load, twiddler_persist_save).register();
}
