//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 programs (test: tests/differential_draft_ww3d7_shd.rs);
//! not yet wired into the game.
//! Not covered: `unregister_factory` with two registered factories of one class id (the draft identifies a factory by id), a `Load_Shader` of an unregistered id.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shddefmanager.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shddefmanager.h
//!
//! `_FactoryListHead` (a static intrusive list) is the vector in [`ShdDefManagerClass`]. Upstream links new
//! factories at the **head**, so the list is in reverse registration order; `Get_First`/`Get_Next` and the
//! first-match lookups keep that order ([`ShdDefManagerClass::register_factory`] inserts at index 0).
//! `Save_Shader` is not translated (`ChunkSaveClass` is not in the verified port).

use super::shddef::ShdDefClass;
use super::shddeffactory::ShdDefFactoryClass;
use crate::formats::w3d::w3d_file::{W3D_CHUNK_SHDSUBMESH_SHADER_CLASSID, W3D_CHUNK_SHDSUBMESH_SHADER_DEF};
use crate::formats::w3d::W3dResult;
use crate::ww::chunkio::ChunkLoad;

/// What `Load_Shader` asks of a concrete definition: its base data and the `Load` of its own chunk.
pub trait ShdDefLoad {
    /// The `ShdDefClass` base.
    fn base(&self) -> &ShdDefClass;
    /// `shddef->Load(cload)`, the virtual (a derived class loads its own variables after the base's).
    fn load(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool>;
}

/// `class ShdDefManagerClass` (shddefmanager.h).
#[derive(Clone, Debug)]
pub struct ShdDefManagerClass<D> {
    factories: Vec<ShdDefFactoryClass<D>>,
}

impl<D> ShdDefManagerClass<D> {
    /// An empty list (`_FactoryListHead = NULL`).
    pub fn new() -> Self {
        ShdDefManagerClass { factories: Vec::new() }
    }

    /// `ShdDefManagerClass::Find_Factory(class_id)` (shddefmanager.cpp:42): the first factory in list order.
    pub fn find_factory(&self, class_id: u32) -> Option<&ShdDefFactoryClass<D>> {
        self.factories.iter().find(|f| f.get_class_id() == class_id)
    }

    /// `ShdDefManagerClass::Find_Factory(name)` (shddefmanager.cpp:64): case-insensitive (`stricmp`).
    pub fn find_factory_by_name(&self, name: &str) -> Option<&ShdDefFactoryClass<D>> {
        self.factories.iter().find(|f| if crate::logic::modules::plant(19485) { f.get_name() == name } else { f.get_name().eq_ignore_ascii_case(name) })
    }

    /// `Register_Factory` / `Link_Factory` (shddefmanager.cpp:84, 125): links at the head of the list.
    pub fn register_factory(&mut self, factory: ShdDefFactoryClass<D>) {
        if crate::logic::modules::plant(19484) {
            self.factories.push(factory);
        } else {
            self.factories.insert(0, factory);
        }
    }

    /// `Unregister_Factory` / `Unlink_Factory` (shddefmanager.cpp:95, 143): removes the factory with this
    /// class id (a factory is identified by identity upstream; ids are what distinguish them here).
    pub fn unregister_factory(&mut self, class_id: u32) {
        if let Some(pos) = self.factories.iter().position(|f| f.get_class_id() == class_id) {
            if !crate::logic::modules::plant(19487) {
                self.factories.remove(pos);
            }
        }
    }

    /// `Get_First` (shddefmanager.cpp:104): the head.
    pub fn get_first(&self) -> Option<&ShdDefFactoryClass<D>> {
        self.factories.first()
    }

    /// `Get_Next` (shddefmanager.cpp:109): the factory after `current` (an index into the list); with no
    /// current, the head (as upstream's NULL case).
    pub fn get_next(&self, current: Option<usize>) -> Option<&ShdDefFactoryClass<D>> {
        match current {
            Some(i) => self.factories.get(i + 1),
            None => if crate::logic::modules::plant(19486) { None } else { self.factories.first() },
        }
    }

    /// `Create_ShdDefClass_Instance` (shddefmanager.cpp:175): a new definition from the factory for this id, or
    /// None.
    pub fn create_shd_def_class_instance(&self, class_id: u32) -> Option<D> {
        self.find_factory(class_id).map(|f| f.create())
    }
}

impl<D: ShdDefLoad> ShdDefManagerClass<D> {
    /// `ShdDefManagerClass::Load_Shader` (shddefmanager.cpp:211): a chunk holding the class id (a `uint32`), then
    /// a chunk holding the definition. The class id read is not checked and the instance not null-checked upstream
    /// (an unknown id makes `(*shddef)->Load` a null call); here an unknown id is `None` after the second
    /// chunk is still opened and closed, without loading.
    pub fn load_shader(&self, cload: &mut ChunkLoad<'_>) -> W3dResult<Option<D>> {
        cload.open_chunk()?;
        debug_assert!(cload.cur_chunk_id()? == W3D_CHUNK_SHDSUBMESH_SHADER_CLASSID);
        let mut b = [0u8; 4];
        cload.read_into(&mut b)?;
        let id = u32::from_le_bytes(b);
        cload.close_chunk()?;

        let mut shddef = self.create_shd_def_class_instance(id);

        cload.open_chunk()?;
        debug_assert!(cload.cur_chunk_id()? == W3D_CHUNK_SHDSUBMESH_SHADER_DEF);
        if let Some(d) = shddef.as_mut() {
            if !crate::logic::modules::plant(19488) {
                d.load(cload)?;
            }
        }
        cload.close_chunk()?;
        Ok(shddef)
    }
}

impl<D> Default for ShdDefManagerClass<D> {
    fn default() -> Self {
        Self::new()
    }
}
