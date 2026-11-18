//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shddeffactory.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shddeffactory.h
//!
//! Upstream's factories register themselves in the constructor on an intrusive doubly linked static list
//! (`NextFactory`/`PrevFactory`) and unregister in the destructor. Here a factory is a plain value
//! ([`ShdDefFactoryClass`]: class id, name and the function that creates the definition) and the registry,
//! [`super::shddefmanager::ShdDefManagerClass`], holds them in a vector; registering and unregistering are
//! explicit calls on it (the constructor/destructor side effect is not reproducible without a global).

use super::shddef::ShdDefClass;

/// `template <class T> class ShdDefFactory` / `ShdDefFactoryClass` (shddeffactory.h): maps a class id and name
/// to the creation of a definition (`Create` returns a new definition of the concrete type; the `D` is the
/// owner's definition type, for example an enum over the shd*.cpp definitions).
#[derive(Clone, Debug)]
pub struct ShdDefFactoryClass<D> {
    class_id: u32,
    name: String,
    create: fn() -> D,
    /// `Get_Class_ID` of the definition, for the lookup by id.
    pub base_of: fn(&D) -> &ShdDefClass,
}

impl<D> ShdDefFactoryClass<D> {
    /// `ShdDefFactoryClass::ShdDefFactoryClass` (shddeffactory.cpp:34), without the self-registration.
    pub fn new(class_id: u32, name: &str, create: fn() -> D, base_of: fn(&D) -> &ShdDefClass) -> Self {
        ShdDefFactoryClass { class_id, name: name.to_string(), create, base_of }
    }

    /// `Get_Class_ID`.
    pub fn get_class_id(&self) -> u32 {
        self.class_id
    }

    /// `Get_Name`.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// `Create`.
    pub fn create(&self) -> D {
        (self.create)()
    }
}
