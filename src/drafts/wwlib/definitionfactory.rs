//! DIFFERENTIAL-TESTED (#202, #215): the registration, lookup and unregistration match the upstream C++ on 20,000 random programs
//! (test: tests/differential_draft_deffactory.rs); not yet wired into the game. `Create` and `Is_Displayed` are not exercised (the test's
//! factories return NULL). Corrected 2026-11-20 (#215): a factory is identified by an instance id (upstream: its address), not its class id;
//! two factories of one class id made `Get_Next` loop forever and `Unregister` remove the wrong one.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/definitionfactory.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/definitionfactory.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/simpledefinitionfactory.h
//!
//! `DefinitionFactoryClass` ("virtual constructors for object definitions") and
//! `SimpleDefinitionFactoryClass<T, class_id, name>` as one struct: the class id, the name and a constructor function
//! pointer. Upstream's constructor registers the factory with `DefinitionFactoryMgrClass` and the destructor removes it:
//! here [`DefinitionFactoryClass::register`] / [`DefinitionFactoryClass::unregister`]. `DECLARE_DEFINITION_FACTORY` is
//! `DefinitionFactoryClass::new` with the displayed flag defaulting to true.

use super::definition::DefinitionHandle;
use super::definitionfactorymgr::DefinitionFactoryMgrClass;

/// `class DefinitionFactoryClass`.
#[derive(Clone, Copy)]
pub struct DefinitionFactoryClass {
    /// `Get_Class_ID()`.
    pub class_id: u32,
    /// `Get_Name()`.
    pub name: &'static str,
    /// `Is_Displayed()` (`SimpleDefinitionFactoryClass(bool is_displayed = true)`).
    pub is_displayed: bool,
    /// `Create()`: `W3DNEW T`.
    pub create: fn() -> DefinitionHandle,
    /// The factory's identity (upstream: its address): two factories may share a class id, and `Unregister_Factory` and `Get_Next`
    /// act on the one object. Assigned by `new`; copies of a factory share it.
    instance: u32,
}

static NEXT_INSTANCE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

impl DefinitionFactoryClass {
    /// `SimpleDefinitionFactoryClass(is_displayed)` for `T` made by `create`.
    pub fn new(class_id: u32, name: &'static str, is_displayed: bool, create: fn() -> DefinitionHandle) -> Self {
        DefinitionFactoryClass { class_id, name, is_displayed, create, instance: NEXT_INSTANCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed) }
    }

    /// `Create()`.
    pub fn create(&self) -> DefinitionHandle {
        (self.create)()
    }

    /// Whether `other` is the same factory object (a copy of it).
    pub fn same_as(&self, other: &DefinitionFactoryClass) -> bool {
        self.instance == other.instance
    }

    /// `Get_Name()`.
    pub fn get_name(&self) -> &'static str {
        self.name
    }

    /// `Get_Class_ID()`.
    pub fn get_class_id(&self) -> u32 {
        self.class_id
    }

    /// `Is_Displayed()`.
    pub fn is_displayed(&self) -> bool {
        self.is_displayed
    }

    /// The constructor's `DefinitionFactoryMgrClass::Register_Factory(this)`.
    pub fn register(&self) {
        DefinitionFactoryMgrClass::register_factory(*self);
    }

    /// The destructor's `DefinitionFactoryMgrClass::Unregister_Factory(this)`.
    pub fn unregister(&self) {
        DefinitionFactoryMgrClass::unregister_factory(self);
    }
}
