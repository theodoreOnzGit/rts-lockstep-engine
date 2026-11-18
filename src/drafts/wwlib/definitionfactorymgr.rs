//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/definitionfactorymgr.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/definitionfactorymgr.cpp
//!
//! `DefinitionFactoryMgrClass`: the static list of definition factories (`_FactoryListHead`), pushed on the FRONT by
//! `Link_Factory`, so enumeration and lookups run newest-first. The doubly linked list is a `Vec` (index 0 is the head).
//! `stricmp` is an ASCII case-insensitive comparison.

use super::definition::super_class_id_from_class_id;
use super::definitionfactory::DefinitionFactoryClass;
use std::sync::{LazyLock, Mutex};

static FACTORIES: LazyLock<Mutex<Vec<DefinitionFactoryClass>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// `class DefinitionFactoryMgrClass` (all members static).
pub struct DefinitionFactoryMgrClass;

impl DefinitionFactoryMgrClass {
    /// `Find_Factory(class_id)`.
    pub fn find_factory(class_id: u32) -> Option<DefinitionFactoryClass> {
        FACTORIES.lock().unwrap().iter().find(|f| f.class_id == class_id).copied()
    }

    /// `Find_Factory(name)`: `stricmp`.
    pub fn find_factory_by_name(name: &str) -> Option<DefinitionFactoryClass> {
        FACTORIES.lock().unwrap().iter().find(|f| f.name.eq_ignore_ascii_case(name)).copied()
    }

    /// `Get_First(superclass_id)`: the first factory whose class belongs to the super-class.
    pub fn get_first_in_superclass(superclass_id: u32) -> Option<DefinitionFactoryClass> {
        FACTORIES.lock().unwrap().iter().find(|f| super_class_id_from_class_id(f.class_id) == superclass_id).copied()
    }

    /// `Get_Next(current, superclass_id)`: the next one after `current` in the super-class. `current` is found by its class
    /// id; nothing if it is not registered.
    pub fn get_next_in_superclass(current: &DefinitionFactoryClass, superclass_id: u32) -> Option<DefinitionFactoryClass> {
        let list = FACTORIES.lock().unwrap();
        let pos = list.iter().position(|f| f.class_id == current.class_id)?;
        list[pos + 1..].iter().find(|f| super_class_id_from_class_id(f.class_id) == superclass_id).copied()
    }

    /// `Get_First()`: the head of the list.
    pub fn get_first() -> Option<DefinitionFactoryClass> {
        FACTORIES.lock().unwrap().first().copied()
    }

    /// `Get_Next(current)`: "simply return the next factory in the chain".
    pub fn get_next(current: &DefinitionFactoryClass) -> Option<DefinitionFactoryClass> {
        let list = FACTORIES.lock().unwrap();
        let pos = list.iter().position(|f| f.class_id == current.class_id)?;
        list.get(pos + 1).copied()
    }

    /// `Register_Factory(factory)` -> `Link_Factory`: "Adding this factory in front of the current head of the list".
    pub fn register_factory(factory: DefinitionFactoryClass) {
        FACTORIES.lock().unwrap().insert(0, factory);
    }

    /// `Unregister_Factory(factory)` -> `Unlink_Factory`.
    pub fn unregister_factory(class_id: u32) {
        let mut list = FACTORIES.lock().unwrap();
        if let Some(i) = list.iter().position(|f| f.class_id == class_id) {
            list.remove(i);
        }
    }
}
