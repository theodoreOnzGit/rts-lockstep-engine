//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdinterface.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdinterface.h
//!
//! `ShdInterfaceClass` is the abstract base of the per-hardware shader implementations (its virtual
//! `Apply_Shared`, `Apply_Instance`, `Greater_Than` and the rest are the device side, implemented in shd*.cpp and
//! not in this base). Only the base data is here: the definition it was made from and its class id.

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

    /// `Get_Class_ID` (shdinterface.h).
    pub fn get_class_id(&self) -> i32 {
        self.class_id
    }
}
