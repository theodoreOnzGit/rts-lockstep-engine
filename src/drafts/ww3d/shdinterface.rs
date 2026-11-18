//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdinterface.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdinterface.h
//!
//! `ShdInterfaceClass` is the abstract base of the per-hardware shader implementations. Its pure virtuals
//! (`Get_Pass_Count`, `Apply_Shared`, `Apply_Instance`, `Copy_Vertex_Stream`, ...: they set D3D render
//! states) are device-only and implemented by the shd*.cpp shaders; not translated here. The base data and the
//! base's default answers (`Greater_Than`, `Similar_Enough`, `Pass_Selection`: all true) are.

use super::shddef::ShdDefClass;
use std::sync::Arc;

/// `class ShdInterfaceClass` (shdinterface.h): holds a reference to its definition (`REF_PTR_SET`, so an
/// `Arc`).
#[derive(Clone, Debug)]
pub struct ShdInterfaceClass {
    definition: Option<Arc<ShdDefClass>>,
    class_id: i32,
}

impl ShdInterfaceClass {
    /// `ShdInterfaceClass::ShdInterfaceClass(def, class_id)` (shdinterface.cpp:34).
    pub fn new(def: Option<Arc<ShdDefClass>>, class_id: i32) -> Self {
        ShdInterfaceClass { definition: def, class_id }
    }

    /// `ShdInterfaceClass::Peek_Definition` (shdinterface.cpp:51).
    pub fn peek_definition(&self) -> Option<&Arc<ShdDefClass>> {
        self.definition.as_ref()
    }

    /// `Greater_Than` (shdinterface.h): the base answer; a derived shader overrides it to sort by state.
    pub fn greater_than(&self, _s: &ShdInterfaceClass, _pass: i32) -> bool {
        true
    }

    /// `Similar_Enough` (shdinterface.h): the base answer.
    pub fn similar_enough(&self, _s: &ShdInterfaceClass, _pass: i32) -> bool {
        true
    }

    /// `Pass_Selection` (shdinterface.h): the base answer (the mesh and render info arguments are unused).
    pub fn pass_selection(&self, _pass: i32) -> bool {
        true
    }

    /// `SHD_MAX_PASSES` (shdinterface.h): "Maximum passes allowed for any shader implementation!".
    pub const SHD_MAX_PASSES: i32 = 4;

    /// `Get_Class_ID` (shdinterface.h).
    pub fn get_class_id(&self) -> i32 {
        self.class_id
    }
}
