//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 10,000 random programs of WWSaveLoad operations (test: tests/differential_draft_saveload.rs);
//! not yet wired into the game. Not covered: `Twiddle()` (the reference's stand-in returns the definition itself, as the draft does for a non-twiddler), `definition_mgr_set_id`, `same_definition`.
//! Equal ids after a `Load` that appends to a non-empty table are not compared (`qsort`'s order of ties is the C library's). The reference walks `Count()` entries in
//! `Find_Typed_Definition`'s cache loop; upstream walks `Length()` (the capacity) and reads uninitialised slots (an upstream defect, reported on #211).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/definitionmgr.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/definitionmgr.cpp
//!
//! `DefinitionMgrClass` (the global `_TheDefinitionMgr`): the table of all definitions sorted by id, searched by
//! binary search, saved and loaded as a save-load sub-system. All members are static upstream: the table, its counts and
//! the lower-cased-name hash are one process-wide [`DefinitionMgrState`]. Upstream notes kept: the binary search /
//! insertion windows are copied verbatim (`index = upper_index / 2`, the two-entry special case); a duplicate id on
//! `Register_Definition` is silently not registered ("//WWASSERT (is_valid)"); `Load_Objects` sorts with `qsort`, which
//! is not stable (equal ids may come out in any order; `sort_unstable_by` here); `Find_Typed_Definition` returns the
//! cached definition whose class OR super-class matches OR any twiddler (`twiddle && ClassID == TWIDDLERS`), whatever
//! its name when read from the hash; the hash is keyed by the lower-cased name. The `Twiddle` random draws come from the
//! caller's `random_int(lo, hi)` (upstream seeds a `RandomClass` from `TIMEGETTIME()`).

use super::chunk_save::ChunkSaveClass;
use super::definition::{definition_set_id, super_class_id_from_class_id, DefinitionHandle, DefinitionKind, CLASSID_TWIDDLERS, DEF_CLASSID_START};
use super::persistfactory::PersistObject;
use super::saveload::SaveLoadSystemClass;
use super::saveload_ids::CHUNKID_SAVELOAD_DEFMGR;
use crate::logic::modules::plant;
use crate::ww::chunkio::ChunkLoad;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

/// `DEFINTION_LIST_GROW_SIZE = 1000` (a `Vec` grows on its own).
pub const DEFINTION_LIST_GROW_SIZE: i32 = 1000;
/// `IDRANGE_PER_CLASS = 10000`.
pub const IDRANGE_PER_CLASS: u32 = 10000;

/// `CHUNKID_VARIABLES = 0x00000100`, `CHUNKID_OBJECTS` (0x101), `CHUNKID_OBJECT` (0x102); `VARID_NEXTDEFID = 1`.
const CHUNKID_VARIABLES: u32 = 0x00000100;
const CHUNKID_OBJECTS: u32 = 0x00000101;
const VARID_NEXTDEFID: u32 = 0x01;

/// `DefinitionMgrClass::ID_TYPE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdType {
    /// `ID_CLASS = 1`.
    Class,
    /// `ID_SUPERCLASS`.
    SuperClass,
}

/// The static members: `_SortedDefinitionArray` (its length is `_DefinitionCount`) and `DefinitionHash`.
#[derive(Default)]
pub struct DefinitionMgrState {
    sorted: Vec<DefinitionHandle>,
    hash: Option<HashMap<String, Vec<DefinitionHandle>>>,
}

static STATE: LazyLock<Mutex<DefinitionMgrState>> = LazyLock::new(|| Mutex::new(DefinitionMgrState::default()));

fn def_id(d: &DefinitionHandle) -> u32 {
    d.read().unwrap().get_id()
}

fn def_class_id(d: &DefinitionHandle) -> u32 {
    d.read().unwrap().get_class_id()
}

/// `((TwiddlerClass *)definition)->Twiddle()` if `definition` is a twiddler.
fn twiddle_definition(definition: DefinitionHandle, random_int: &mut dyn FnMut(i32, i32) -> i32) -> Option<DefinitionHandle> {
    let twiddled = match &definition.read().unwrap().kind {
        DefinitionKind::Twiddler(t) => Some(t.twiddle(random_int)),
        _ => None,
    };
    match twiddled {
        Some(result) => result,
        None => Some(definition),
    }
}

/// `class DefinitionMgrClass : public SaveLoadSubSystemClass`.
pub struct DefinitionMgrClass;

impl DefinitionMgrClass {
    /// `Chunk_ID()`: `CHUNKID_SAVELOAD_DEFMGR`.
    pub fn chunk_id() -> u32 {
        CHUNKID_SAVELOAD_DEFMGR
    }

    /// `Contains_Data()`: "TODO: check if we have any definitions...".
    pub fn contains_data() -> bool {
        true
    }

    /// `Name()`.
    pub fn name() -> &'static str {
        "DefinitionMgrClass"
    }

    /// `Find_Definition(id, twiddle)` (definitionmgr.cpp:92): binary search by id; a found twiddler is replaced by its
    /// random pick when `twiddle`.
    pub fn find_definition(id: u32, twiddle: bool, random_int: &mut dyn FnMut(i32, i32) -> i32) -> Option<DefinitionHandle> {
        let mut definition: Option<DefinitionHandle> = None;
        {
            let st = STATE.lock().unwrap();
            let count = st.sorted.len() as i32;
            let mut lower_index = 0i32;
            let mut upper_index = count - 1;
            let mut index = upper_index / 2;
            let mut keep_going = count > 0;

            // Binary search the list until we've found the definition
            while keep_going {
                let curr_def = &st.sorted[index as usize];

                // Is this the definition we are looking for?
                if def_id(curr_def) == id {
                    definition = Some(curr_def.clone());
                    keep_going = false;
                } else if upper_index <= lower_index + 1 {
                    // When the window get's too small, our divide by two won't catch both entries, so just go ahead and
                    // do them both now.
                    keep_going = false;
                    if def_id(&st.sorted[lower_index as usize]) == id {
                        definition = Some(st.sorted[lower_index as usize].clone());
                    } else if def_id(&st.sorted[upper_index as usize]) == id {
                        definition = Some(st.sorted[upper_index as usize].clone());
                    }
                } else {
                    // Cut our 'window' in half
                    if id > def_id(curr_def) {
                        lower_index = index;
                        index += (upper_index - index) / 2;
                    } else {
                        upper_index = index;
                        index -= (index - lower_index) / 2;
                    }
                }
            }
        }

        // Should we twiddle this definition? (Twiddling refers to our randomizing framework for definitions)
        if twiddle {
            if let Some(d) = &definition {
                if def_class_id(d) == CLASSID_TWIDDLERS {
                    return twiddle_definition(d.clone(), random_int);
                }
            }
        }
        definition
    }

    /// `Find_Named_Definition(name, twiddle)`: linear search, `stricmp`.
    pub fn find_named_definition(name: &str, twiddle: bool, random_int: &mut dyn FnMut(i32, i32) -> i32) -> Option<DefinitionHandle> {
        let definition = {
            let st = STATE.lock().unwrap();
            st.sorted.iter().find(|d| if plant(18291) { d.read().unwrap().get_name() == name } else { d.read().unwrap().get_name().eq_ignore_ascii_case(name) }).cloned()
        };

        if twiddle {
            if let Some(d) = &definition {
                if def_class_id(d) == CLASSID_TWIDDLERS {
                    return twiddle_definition(d.clone(), random_int);
                }
            }
        }
        definition
    }

    /// `Find_Typed_Definition(name, class_id, twiddle)` (definitionmgr.cpp:183): the hash of lower-cased names first (built
    /// as definitions are asked for), then a linear search that fills the hash. `None` when the hash does not exist yet
    /// (upstream prints "failed due to a NULL DefinitionHash" and returns NULL).
    pub fn find_typed_definition(name: &str, class_id: u32, twiddle: bool, random_int: &mut dyn FnMut(i32, i32) -> i32) -> Option<DefinitionHandle> {
        let mut definition: Option<DefinitionHandle> = None;
        {
            let mut st = STATE.lock().unwrap();
            // Sanity check
            st.hash.as_ref()?;

            let lower_case_name = if plant(18293) { name.to_string() } else { name.to_ascii_lowercase() };

            // Check the hash table first.
            if let Some(defs) = st.hash.as_ref().unwrap().get(&lower_case_name) {
                for curr_def in defs.iter() {
                    let curr_class_id = def_class_id(curr_def);
                    if curr_class_id == class_id || (super_class_id_from_class_id(curr_class_id) == class_id && !plant(18292)) || (twiddle && curr_class_id == CLASSID_TWIDDLERS) {
                        definition = Some(curr_def.clone());
                        break;
                    }
                }
            }

            // Loop through all the definitions and see if we can find the one with the requested name
            if definition.is_none() {
                let found = st
                    .sorted
                    .iter()
                    .find(|curr_def| {
                        // Is this the correct class of definition?
                        let curr_class_id = def_class_id(curr_def);
                        (curr_class_id == class_id || (super_class_id_from_class_id(curr_class_id) == class_id && !plant(18292)) || (twiddle && curr_class_id == CLASSID_TWIDDLERS))
                            && curr_def.read().unwrap().get_name().eq_ignore_ascii_case(name) // Is this the definition we were looking for?
                    })
                    .cloned();
                if let Some(f) = found {
                    // Add the definition to the hash table, so that it can be quickly accessed the next time it is needed.
                    st.hash.as_mut().unwrap().entry(lower_case_name).or_default().push(f.clone());
                    definition = Some(f);
                }
            }
        }

        if twiddle {
            if let Some(d) = &definition {
                if def_class_id(d) == CLASSID_TWIDDLERS {
                    return twiddle_definition(d.clone(), random_int);
                }
            }
        }
        definition
    }

    /// `List_Available_Definitions()`: the names (upstream prints them with `WWDEBUG_SAY`).
    pub fn list_available_definitions() -> Vec<String> {
        STATE.lock().unwrap().sorted.iter().map(|d| d.read().unwrap().get_name().to_string()).collect()
    }

    /// `List_Available_Definitions(superclass_id)`.
    pub fn list_available_definitions_in_superclass(superclass_id: u32) -> Vec<String> {
        let mut names = Vec::new();
        let mut definition = Self::get_first_of(superclass_id, IdType::SuperClass);
        while let Some(d) = definition {
            names.push(d.read().unwrap().get_name().to_string());
            definition = Self::get_next_of(&d, superclass_id, IdType::SuperClass);
        }
        names
    }

    /// `Get_First()`: the lowest id.
    pub fn get_first() -> Option<DefinitionHandle> {
        STATE.lock().unwrap().sorted.first().cloned()
    }

    fn matches(curr_def: &DefinitionHandle, id: u32, type_: IdType) -> bool {
        let type_ = if plant(18299) { if type_ == IdType::Class { IdType::SuperClass } else { IdType::Class } } else { type_ };
        let class_id = def_class_id(curr_def);
        (type_ == IdType::SuperClass && super_class_id_from_class_id(class_id) == id) || (type_ == IdType::Class && class_id == id)
    }

    /// `Get_First(id, type)`: the first definition of the class (or super-class).
    pub fn get_first_of(id: u32, type_: IdType) -> Option<DefinitionHandle> {
        STATE.lock().unwrap().sorted.iter().find(|d| Self::matches(d, id, type_)).cloned()
    }

    /// `Get_Next(curr_def, id, type)`: from the index after `curr_def`'s manager link.
    pub fn get_next_of(curr_def: &DefinitionHandle, id: u32, type_: IdType) -> Option<DefinitionHandle> {
        let start = curr_def.read().unwrap().mgr_link + 1;
        let st = STATE.lock().unwrap();
        let mut index = start;
        while index < st.sorted.len() as i32 {
            let d = &st.sorted[index as usize];
            if Self::matches(d, id, type_) {
                return Some(d.clone());
            }
            index += 1;
        }
        None
    }

    /// `Get_Next(curr_def)`.
    pub fn get_next(curr_def: &DefinitionHandle) -> Option<DefinitionHandle> {
        let index = curr_def.read().unwrap().mgr_link + 1;
        let st = STATE.lock().unwrap();
        if index < st.sorted.len() as i32 {
            Some(st.sorted[index as usize].clone())
        } else {
            None
        }
    }

    /// `Free_Definitions()`: clears the hash and releases every definition.
    pub fn free_definitions() {
        let mut st = STATE.lock().unwrap();
        if !plant(18300) {
            st.hash = None;
        }
        st.sorted.clear();
    }

    /// `Prepare_Definition_Array()`: "if (!DefinitionHash) DefinitionHash = new HashTemplateClass" (the array grows by
    /// itself).
    fn prepare_definition_array(st: &mut DefinitionMgrState) {
        if st.hash.is_none() {
            st.hash = Some(HashMap::new());
        }
    }

    /// `Register_Definition(definition)` (definitionmgr.cpp:485): inserted at its sorted position; not registered if it is
    /// already, has id 0, or another definition has the same id.
    pub fn register_definition(definition: &DefinitionHandle) {
        let (link, id) = {
            let d = definition.read().unwrap();
            (d.mgr_link, d.get_id())
        };
        if link == -1 && id != 0 {
            let mut st = STATE.lock().unwrap();
            // Make sure the definition array is large enough
            Self::prepare_definition_array(&mut st);

            // Calculate where in the list we should insert this definition
            let count = st.sorted.len() as i32;
            let mut lower_index = 0i32;
            let mut upper_index = count - 1;
            let mut index = upper_index / 2;
            let mut insert_index = count;
            let mut keep_going = count > 0;
            let mut is_valid = true;

            while keep_going {
                let curr_id = def_id(&st.sorted[index as usize]);

                // Check to make sure we aren't trying to register a definition that has the same ID as one already in
                // the list.
                if curr_id == id && !plant(18294) {
                    insert_index = index;
                    keep_going = false;
                    is_valid = false;
                } else {
                    // Cut our 'window' in half
                    if id > curr_id {
                        lower_index = index;
                        index += (upper_index - index) / 2;
                    } else {
                        upper_index = index;
                        index -= (index - lower_index) / 2;
                    }

                    // If we've narrowed down the window to 2 entries, then quick check the different possibilities.
                    if upper_index <= lower_index + 1 {
                        if def_id(&st.sorted[upper_index as usize]) <= id {
                            insert_index = upper_index + 1;
                        } else if def_id(&st.sorted[lower_index as usize]) <= id {
                            insert_index = upper_index;
                        } else {
                            insert_index = lower_index;
                        }
                        keep_going = false;
                    }
                }
            }

            if is_valid {
                // Insert this definition into the list and re-index all the definitions that got bumped one cell
                st.sorted.insert(insert_index as usize, definition.clone());
                for i in insert_index as usize..st.sorted.len() {
                    st.sorted[i].write().unwrap().mgr_link = i as i32;
                }
            }
        }
    }

    /// `Unregister_Definition(definition)`.
    pub fn unregister_definition(definition: &DefinitionHandle) {
        let link = definition.read().unwrap().mgr_link;
        if link != -1 {
            let mut st = STATE.lock().unwrap();
            if plant(18295) && link as usize >= st.sorted.len() {
                return;
            }
            // Re-index the definitions that come after this definition in the list
            st.sorted.remove(link as usize);
            for i in link as usize..st.sorted.len() {
                st.sorted[i].write().unwrap().mgr_link = i as i32;
            }
            if !plant(18295) {
                definition.write().unwrap().mgr_link = -1;
            }
        }
    }

    /// `Save(csave)`: the variables chunk, then the objects chunk.
    pub fn save(csave: &mut ChunkSaveClass) -> bool {
        let mut retval = true;

        // Create a chunk to contain the class variables we need to serialize.
        csave.begin_chunk(CHUNKID_VARIABLES);
        Self::save_variables(csave);
        csave.end_chunk();

        // Have the base class write the objects to their own chunk.
        csave.begin_chunk(CHUNKID_OBJECTS);
        retval &= Self::save_objects(csave);
        csave.end_chunk();
        retval
    }

    /// `Load(cload)`.
    pub fn load(cload: &mut ChunkLoad<'_>) -> bool {
        let mut retval = true;
        while cload.open_chunk().unwrap_or(false) {
            match cload.cur_chunk_id().unwrap_or(0) {
                CHUNKID_VARIABLES => retval &= Self::load_variables(cload),
                CHUNKID_OBJECTS => retval &= Self::load_objects(cload),
                _ => {}
            }
            let _ = cload.close_chunk();
        }
        retval
    }

    /// `Save_Objects(csave)`: each save-enabled definition in a chunk of its factory's id.
    fn save_objects(csave: &mut ChunkSaveClass) -> bool {
        let retval = true;
        let defs: Vec<DefinitionHandle> = STATE.lock().unwrap().sorted.clone();
        for definition in defs.iter() {
            let (enabled, chunk_id) = {
                let d = definition.read().unwrap();
                (d.is_save_enabled(), d.get_factory_chunk_id())
            };
            if enabled || plant(18297) {
                // Save this definition object
                csave.begin_chunk(chunk_id);
                if let Some(factory) = SaveLoadSystemClass::find_persist_factory(chunk_id) {
                    (factory.save)(csave, &PersistObject::Definition(definition.clone()));
                }
                csave.end_chunk();
            }
        }
        retval
    }

    /// `Save_Variables(csave)`: nothing.
    fn save_variables(_csave: &mut ChunkSaveClass) -> bool {
        true
    }

    /// `Load_Objects(cload)`: each chunk with a registered factory is loaded, appended, then the array is sorted by id
    /// and every definition gets its manager link.
    fn load_objects(cload: &mut ChunkLoad<'_>) -> bool {
        let retval = true;

        while cload.open_chunk().unwrap_or(false) {
            // Load this definition from the chunk (if possible)
            if let Some(factory) = SaveLoadSystemClass::find_persist_factory(cload.cur_chunk_id().unwrap_or(0)) {
                if let Some(PersistObject::Definition(definition)) = (factory.load)(cload) {
                    // Add this definition to our array
                    let mut st = STATE.lock().unwrap();
                    Self::prepare_definition_array(&mut st);
                    st.sorted.push(definition);
                }
            }
            let _ = cload.close_chunk();
        }

        // Sort the definitions (by id) and assign a mgr link to each
        let mut st = STATE.lock().unwrap();
        if !st.sorted.is_empty() && !plant(18298) {
            st.sorted.sort_unstable_by(|a, b| def_id(a).cmp(&def_id(b)));
        }
        for i in 0..st.sorted.len() {
            st.sorted[i].write().unwrap().mgr_link = i as i32;
        }
        retval
    }

    /// `Load_Variables(cload)`: `VARID_NEXTDEFID` is read and ignored.
    fn load_variables(cload: &mut ChunkLoad<'_>) -> bool {
        while cload.open_micro_chunk().unwrap_or(false) {
            if cload.cur_micro_chunk_id() == VARID_NEXTDEFID {
                // (nothing)
            }
            let _ = cload.close_micro_chunk();
        }
        true
    }

    /// `Get_New_ID(class_id)` (definitionmgr.cpp:508): the first free id in the class's range of `IDRANGE_PER_CLASS`.
    pub fn get_new_id(class_id: u32) -> u32 {
        let idrange_start = (class_id.wrapping_sub(DEF_CLASSID_START as u32)).wrapping_mul(IDRANGE_PER_CLASS);
        let idrange_end = idrange_start.wrapping_add(IDRANGE_PER_CLASS);
        let mut new_id = idrange_start + 1;

        let st = STATE.lock().unwrap();
        let count = st.sorted.len();
        // Try to find the first empty slot in this ID range
        for index in 0..count {
            let curr_id = def_id(&st.sorted[index]);

            // Is this id in the range we are looking for?
            if curr_id >= idrange_start && curr_id < idrange_end {
                let mut is_ok = false;
                if index < count - 1 {
                    // Check to see if the next definition in our array leaves a hole in the ID range.
                    if if plant(18296) { def_id(&st.sorted[index + 1]) >= curr_id + 1 } else { def_id(&st.sorted[index + 1]) > curr_id + 1 } {
                        is_ok = true;
                    }
                } else {
                    is_ok = true;
                }

                // Return the new ID
                if is_ok {
                    new_id = curr_id + 1;
                    break;
                }
            }
        }
        new_id
    }
}

/// `_TheDefinitionMgr`'s constructor / destructor: registration as a sub-system and `Free_Definitions` on exit.
pub fn definition_mgr_set_id(definition: &DefinitionHandle, id: u32) {
    definition_set_id(definition, id);
}

/// Whether two handles are the same definition (`def1 == def2`).
pub fn same_definition(a: &DefinitionHandle, b: &DefinitionHandle) -> bool {
    Arc::ptr_eq(a, b)
}
