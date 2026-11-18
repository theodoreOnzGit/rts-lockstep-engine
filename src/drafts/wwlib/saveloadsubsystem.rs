//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/saveloadsubsystem.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/saveloadsubsystem.cpp
//!
//! `SaveLoadSubSystemClass`: "Each SaveLoadSubSystem will automatically be registered with the SaveLoadSystem at
//! construction time" -- here an enum of the sub-systems (no trait objects) with the explicit
//! [`SaveLoadSubSystemClass::register`] / [`SaveLoadSubSystemClass::unregister`] the constructor and destructor do.
//! Only the definition manager (`_TheDefinitionMgr`) is translated; add a variant per sub-system.

use super::chunk_save::ChunkSaveClass;
use super::definitionmgr::DefinitionMgrClass;
use super::saveload::SaveLoadSystemClass;
use crate::ww::chunkio::ChunkLoad;

/// A registered sub-system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveLoadSubSystemClass {
    /// `DefinitionMgrClass`.
    DefinitionMgr,
}

impl SaveLoadSubSystemClass {
    /// `Chunk_ID()`.
    pub fn chunk_id(&self) -> u32 {
        match self {
            SaveLoadSubSystemClass::DefinitionMgr => DefinitionMgrClass::chunk_id(),
        }
    }

    /// `Contains_Data()` (default `true`).
    pub fn contains_data(&self) -> bool {
        match self {
            SaveLoadSubSystemClass::DefinitionMgr => DefinitionMgrClass::contains_data(),
        }
    }

    /// `Save(csave)`.
    pub fn save(&self, csave: &mut ChunkSaveClass) -> bool {
        match self {
            SaveLoadSubSystemClass::DefinitionMgr => DefinitionMgrClass::save(csave),
        }
    }

    /// `Load(cload)`.
    pub fn load(&self, cload: &mut ChunkLoad<'_>) -> bool {
        match self {
            SaveLoadSubSystemClass::DefinitionMgr => DefinitionMgrClass::load(cload),
        }
    }

    /// `Name()`.
    pub fn name(&self) -> &'static str {
        match self {
            SaveLoadSubSystemClass::DefinitionMgr => "DefinitionMgrClass",
        }
    }

    /// The constructor's `SaveLoadSystemClass::Register_Sub_System(this)`.
    pub fn register(self) {
        SaveLoadSystemClass::register_sub_system(self);
    }

    /// The destructor's `SaveLoadSystemClass::Unregister_Sub_System(this)`.
    pub fn unregister(self) {
        SaveLoadSystemClass::unregister_sub_system(self);
    }
}
