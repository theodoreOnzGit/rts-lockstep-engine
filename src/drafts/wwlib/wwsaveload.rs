//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 10,000 random programs of WWSaveLoad operations (test: tests/differential_draft_saveload.rs);
//! not yet wired into the game. `Init` is empty; `Shutdown` is exercised through `Free_Definitions`.
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
    if crate::logic::modules::plant(18301) {
        return;
    }
    DefinitionMgrClass::free_definitions();
}
