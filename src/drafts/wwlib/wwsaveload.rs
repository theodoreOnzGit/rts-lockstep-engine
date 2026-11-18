//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/wwsaveload.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/wwsaveload.cpp
//!
//! `class WWSaveLoad`: `Init` is empty upstream; `Shutdown` frees the definitions.

use super::definitionmgr::DefinitionMgrClass;

/// `WWSaveLoad::Init()`: nothing to do (the body is empty upstream).
pub fn init() {}

/// `WWSaveLoad::Shutdown()`: `_TheDefinitionMgr.Free_Definitions()`.
pub fn shutdown() {
    DefinitionMgrClass::free_definitions();
}
