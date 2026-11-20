//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/definition.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/definition.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/definitionclassids.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/editable.h (only `EditableClass`'s base role; see below)
//!
//! `DefinitionClass : PersistClass, EditableClass`: a named, id'd description of a game object that can create it. The
//! subclasses are user types, so this is ONE struct with a [`DefinitionKind`] enum: `Generic` for a definition the
//! caller defines the behaviour of (class id, persist factory chunk id and a `create` function pointer, the pure
//! virtuals `Get_Class_ID`, `Get_Factory` and `Create`) and `Twiddler` for `TwiddlerClass` (twiddler.rs).
//! `EditableClass` (editable.h, with `parameter.cpp`, `parameterlist.h`, `parametertypes.h`, `simpleparameter.h`) is
//! the LevelEdit property-sheet system (`DECLARE_EDITABLE`, `CLASSID_DEFIDLIST_PARAM` ...): editor-only, not
//! translated (see the issue comment); nothing in the saved bytes depends on it.

use super::chunk_save::ChunkSaveClass;
use super::definitionmgr::DefinitionMgrClass;
use super::persistfactory::{PersistObject, PostLoadableClass};
use super::saveload_ids::CHUNKID_TWIDDLER;
use super::twiddler::TwiddlerClass;
use crate::ww::chunkio::ChunkLoad;
use std::sync::{Arc, RwLock};

/// `const int DEF_CLASSID_START = 0x00001000;`
pub const DEF_CLASSID_START: i32 = 0x00001000;
/// `const int DEF_CLASSID_RANGE = 0x00001000;`
pub const DEF_CLASSID_RANGE: i32 = 0x00001000;

/// `NEXT_SUPER_CLASSID(x)`: `DEF_CLASSID_START + (x * DEF_CLASSID_RANGE)`.
pub const fn next_super_classid(x: i32) -> u32 {
    (DEF_CLASSID_START + x * DEF_CLASSID_RANGE) as u32
}

/// The super-class ids (definitionclassids.h).
pub const CLASSID_TERRAIN: u32 = next_super_classid(0);
pub const CLASSID_TILE: u32 = next_super_classid(1);
pub const CLASSID_GAME_OBJECTS: u32 = next_super_classid(2);
pub const CLASSID_LIGHT: u32 = next_super_classid(3);
pub const CLASSID_SOUND: u32 = next_super_classid(4);
pub const CLASSID_WAYPATH: u32 = next_super_classid(5);
pub const CLASSID_ZONE: u32 = next_super_classid(6);
pub const CLASSID_TRANSITION: u32 = next_super_classid(7);
pub const CLASSID_PHYSICS: u32 = next_super_classid(8);
pub const CLASSID_EDITOR_OBJECTS: u32 = next_super_classid(9);
pub const CLASSID_MUNITIONS: u32 = next_super_classid(10);
pub const CLASSID_DUMMY_OBJECTS: u32 = next_super_classid(11);
pub const CLASSID_BUILDINGS: u32 = next_super_classid(12);
pub const CLASSID_TWIDDLERS: u32 = next_super_classid(13);
pub const CLASSID_GLOBAL_SETTINGS: u32 = next_super_classid(14);

/// `SuperClassID_From_ClassID(class_id)`: the start of the id range `class_id` falls in (`int` arithmetic, so a class id
/// below `DEF_CLASSID_START` divides toward zero).
pub fn super_class_id_from_class_id(class_id: u32) -> u32 {
    let delta = (class_id as i32).wrapping_sub(DEF_CLASSID_START);
    let num_ranges = if crate::logic::modules::plant(17408) { ((delta as u32) / DEF_CLASSID_RANGE as u32) as i32 } else { delta / DEF_CLASSID_RANGE };
    (DEF_CLASSID_START + num_ranges * DEF_CLASSID_RANGE) as u32
}

/// Chunk ids (definition.cpp:44): `CHUNKID_VARIABLES = 0x00000100`; micro chunks `VARID_INSTANCEID = 1`,
/// `XXX_VARID_PARENTID` (2), `VARID_NAME` (3).
const CHUNKID_VARIABLES: u32 = 0x00000100;
const VARID_INSTANCEID: u32 = 0x01;
const VARID_NAME: u32 = 0x03;

/// A shared definition (the manager's table, twiddler lists and the persist system all hold these).
pub type DefinitionHandle = Arc<RwLock<DefinitionClass>>;

/// What kind of definition this is (the concrete subclass).
#[derive(Clone)]
pub enum DefinitionKind {
    /// A user-defined definition subclass.
    Generic {
        /// `Get_Class_ID()`.
        class_id: u32,
        /// `Get_Factory().Chunk_ID()`.
        factory_chunk_id: u32,
        /// `Create()`: the object this definition makes.
        create: fn(&DefinitionClass) -> Option<PersistObject>,
    },
    /// `TwiddlerClass`.
    Twiddler(TwiddlerClass),
}

/// `class DefinitionClass`.
#[derive(Clone)]
pub struct DefinitionClass {
    /// The `PostLoadableClass` base.
    pub post_loadable: PostLoadableClass,
    /// `m_DefinitionMgrLink`: index in the manager's sorted array, -1 when not registered.
    pub(crate) mgr_link: i32,
    m_name: String,
    m_id: u32,
    m_generic_user_data: u32,
    m_save_enabled: bool,
    /// The concrete subclass.
    pub kind: DefinitionKind,
}

impl DefinitionClass {
    /// `DefinitionClass()`: `m_ID(0)`, `m_SaveEnabled(true)`, `m_DefinitionMgrLink(-1)`.
    pub fn new(kind: DefinitionKind) -> Self {
        DefinitionClass { post_loadable: PostLoadableClass::new(), mgr_link: -1, m_name: String::new(), m_id: 0, m_generic_user_data: 0, m_save_enabled: true, kind }
    }

    /// The definition as a shared handle.
    pub fn new_handle(kind: DefinitionKind) -> DefinitionHandle {
        Arc::new(RwLock::new(DefinitionClass::new(kind)))
    }

    /// `Get_Class_ID()` (pure virtual; the Twiddler's is `CLASSID_TWIDDLERS`).
    pub fn get_class_id(&self) -> u32 {
        match &self.kind {
            DefinitionKind::Generic { class_id, .. } => *class_id,
            DefinitionKind::Twiddler(_) => CLASSID_TWIDDLERS,
        }
    }

    /// `Get_Factory()`'s chunk id.
    pub fn get_factory_chunk_id(&self) -> u32 {
        match &self.kind {
            DefinitionKind::Generic { factory_chunk_id, .. } => *factory_chunk_id,
            DefinitionKind::Twiddler(_) => CHUNKID_TWIDDLER,
        }
    }

    /// `Get_ID()`.
    pub fn get_id(&self) -> u32 {
        self.m_id
    }

    /// `Get_Name()`.
    pub fn get_name(&self) -> &str {
        &self.m_name
    }

    /// `Set_Name(new_name)`.
    pub fn set_name(&mut self, new_name: &str) {
        self.m_name = new_name.to_string();
    }

    /// `Is_Valid_Config(message)`: always true.
    pub fn is_valid_config(&self, _message: &mut String) -> bool {
        true
    }

    /// `Get_User_Data()`.
    pub fn get_user_data(&self) -> u32 {
        self.m_generic_user_data
    }

    /// `Set_User_Data(data)`.
    pub fn set_user_data(&mut self, data: u32) {
        self.m_generic_user_data = data;
    }

    /// `Is_Save_Enabled()`.
    pub fn is_save_enabled(&self) -> bool {
        self.m_save_enabled
    }

    /// `Enable_Save(onoff)`.
    pub fn enable_save(&mut self, onoff: bool) {
        self.m_save_enabled = onoff;
    }

    /// `Create()` (pure virtual): a Twiddler makes whatever a randomly picked definition makes (`random_int(lo, hi)`
    /// is `RandomClass::operator()(lo, hi)`, seeded upstream from `TIMEGETTIME()`).
    pub fn create(&self, random_int: &mut dyn FnMut(i32, i32) -> i32) -> Option<PersistObject> {
        match &self.kind {
            DefinitionKind::Generic { create, .. } => create(self),
            DefinitionKind::Twiddler(t) => {
                // Pick a random definition, then indirect the creation to it
                let definition = t.twiddle(random_int)?;
                let d = definition.read().unwrap();
                d.create(random_int)
            }
        }
    }

    /// `DefinitionClass::Save(csave)` (definition.cpp:55) -- the base class's part, which a Twiddler's `Save` calls.
    fn save_base(&self, csave: &mut ChunkSaveClass) -> bool {
        csave.begin_chunk(CHUNKID_VARIABLES);
        let retval = self.save_variables(csave);
        csave.end_chunk();
        retval
    }

    /// `Save(csave)`: the subclass's `Save` (for a Twiddler, its variables chunk then its base-class chunk).
    pub fn save(&mut self, csave: &mut ChunkSaveClass) -> bool {
        match &self.kind {
            DefinitionKind::Generic { .. } => self.save_base(csave),
            DefinitionKind::Twiddler(t) => {
                let mut retval = true;
                csave.begin_chunk(super::twiddler::CHUNKID_VARIABLES);
                retval &= t.save_variables(csave);
                csave.end_chunk();

                csave.begin_chunk(super::twiddler::CHUNKID_BASE_CLASS);
                retval &= self.save_base(csave);
                csave.end_chunk();
                retval
            }
        }
    }

    /// `DefinitionClass::Load(cload)` (definition.cpp:70): the base class's part. (Upstream ignores the result of
    /// `Load_Variables` and returns true.)
    fn load_base(&mut self, cload: &mut ChunkLoad<'_>) -> bool {
        let retval = true;
        while cload.open_chunk().unwrap_or(false) {
            if cload.cur_chunk_id().unwrap_or(0) == CHUNKID_VARIABLES {
                self.load_variables(cload);
            }
            let _ = cload.close_chunk();
        }
        retval
    }

    /// `Load(cload)`: the subclass's `Load` (a Twiddler reads its variables and base-class chunks).
    pub fn load(&mut self, cload: &mut ChunkLoad<'_>) -> bool {
        match &self.kind {
            DefinitionKind::Generic { .. } => self.load_base(cload),
            DefinitionKind::Twiddler(_) => {
                let mut retval = true;
                while cload.open_chunk().unwrap_or(false) {
                    match cload.cur_chunk_id().unwrap_or(0) {
                        super::twiddler::CHUNKID_VARIABLES => {
                            if let DefinitionKind::Twiddler(t) = &mut self.kind {
                                retval &= t.load_variables(cload);
                            }
                        }
                        super::twiddler::CHUNKID_BASE_CLASS => retval &= self.load_base(cload),
                        _ => {}
                    }
                    let _ = cload.close_chunk();
                }
                retval
            }
        }
    }

    /// `Save_Variables(csave)`: the id and the name (with its NUL) as micro chunks.
    fn save_variables(&self, csave: &mut ChunkSaveClass) -> bool {
        csave.write_micro_chunk(VARID_INSTANCEID, &self.m_id.to_le_bytes());
        csave.write_micro_chunk_string(VARID_NAME, &self.m_name);
        true
    }

    /// `Load_Variables(cload)`: `READ_MICRO_CHUNK(VARID_INSTANCEID)` and `READ_MICRO_CHUNK_WWSTRING(VARID_NAME)`.
    fn load_variables(&mut self, cload: &mut ChunkLoad<'_>) -> bool {
        while cload.open_micro_chunk().unwrap_or(false) {
            match cload.cur_micro_chunk_id() {
                VARID_INSTANCEID => {
                    let mut b = [0u8; 4];
                    let _ = cload.read_into(&mut b);
                    self.m_id = u32::from_le_bytes(b);
                }
                VARID_NAME => {
                    let len = cload.cur_micro_chunk_length() as usize;
                    let mut buf = vec![0u8; len];
                    let _ = cload.read(&mut buf, len as u32);
                    if let Some(nul) = buf.iter().position(|&c| c == 0) {
                        buf.truncate(nul);
                    }
                    self.m_name = String::from_utf8_lossy(&buf).into_owned();
                }
                _ => {}
            }
            let _ = cload.close_micro_chunk();
        }
        true
    }
}

/// `DefinitionClass::Set_ID(id)` (definition.cpp:152): "If we are registered with the definition manager, then we need
/// to re-link ourselves back into the list". A free function over the handle (the manager holds the handle).
pub fn definition_set_id(definition: &DefinitionHandle, id: u32) {
    let registered = {
        let mut d = definition.write().unwrap();
        d.m_id = id;
        d.mgr_link != -1
    };
    if registered {
        DefinitionMgrClass::unregister_definition(definition);
        DefinitionMgrClass::register_definition(definition);
    }
}
