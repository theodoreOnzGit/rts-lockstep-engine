// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/ObjectTypes.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ObjectTypes.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: John McDonald, Jr., September 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ObjectTypes`: a named list of thing-template names (a "set" of object
//! types, for example the units a special power or a script refers to by
//! list name), with the queries the game asks of it.
//!
//! # What is in this file, and what is cut
//!
//! The list itself is complete: the names, their order, membership, counting
//! and the save/load/CRC transfer. Two things upstream reaches for through
//! singletons are **cut points**, passed in as arguments:
//!
//! - `TheThingFactory->findTemplate(name)` (`prepForPlayerCounting`,
//!   `canBuildAny`): the caller supplies `find: Fn(&AsciiString) -> Option<T>`,
//!   returning a handle to the template (`None` for an unknown name, as
//!   `findTemplate` returns `NULL`). `T` is whatever the caller uses for a
//!   template (the layer-5 template type when it lands).
//! - `player->canBuild(templ)` (`canBuildAny`): the caller supplies
//!   `can_build: Fn(T) -> bool`.
//! - `isInSet(const ThingTemplate*)` takes the template's name, because only
//!   `ThingTemplate::getName()` is read; the caller passes it (`None` for a
//!   null template).
//!
//! # Semantics kept from upstream
//!
//! - **Order.** The names are a `std::vector`: insertion order, duplicates
//!   refused by `addObjectType`. `prepForPlayerCounting` and `canBuildAny`
//!   visit them in that order.
//! - **`getNthInList`** returns the empty string for any index outside the
//!   list, including a negative index (upstream compares the `Int` with an
//!   unsigned size, so a negative index is huge and fails the test).
//! - **`prepForPlayerCounting` appends.** It does not clear `templates`, and
//!   returns the size of the whole vector (not just what it added), and
//!   resizes `counts` to that size, keeping the values already there.
//! - **Transfer.** Version 1: the version, the list name, the count (a
//!   `UnsignedShort`: a list of 65,536 or more names saves its low 16 bits,
//!   as upstream), then the names. Upstream's `else` branch covers **load and
//!   CRC** modes alike: a non-empty list is an error in both (upstream throws
//!   `SC_INVALID_DATA` after a `DEBUG_CRASH`; here it is
//!   [`XferStatus::ListNotEmpty`], the same condition in this crate's error
//!   type). The CRC, `crc()`, is empty upstream and here.
//! - **`removeObjectType` of a name not in the list** is `DEBUG_CRASH` in
//!   upstream (a no-op in release builds): here it returns `false` and changes
//!   nothing.
//!
//! # Not translated
//!
//! The `Snapshot` trait impl: its methods take `&mut dyn Xfer`, which this
//! crate does not use. [`ObjectTypes::xfer`] and [`ObjectTypes::crc`] are
//! generic over the transfer instead, and the owner (the Snapshot holder)
//! calls them. `MemoryPoolObject` glue: nothing to translate.

use crate::common::ascii_string::AsciiString;
use crate::common::xfer::{Xfer, XferMode, XferResult, XferStatus, XferVersion};

/// `class ObjectTypes`: a list name and the template names in it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectTypes {
    /// `m_listName`: "which list is this again?"
    list_name: AsciiString,
    /// `m_objectTypes`: the template names, in insertion order, no duplicates.
    object_types: Vec<AsciiString>,
}

impl ObjectTypes {
    /// `ObjectTypes()`: an empty list with no name.
    pub fn new() -> Self {
        Self::default()
    }

    /// `ObjectTypes(const AsciiString& listName)`: an empty list with a name.
    pub fn with_list_name(list_name: AsciiString) -> Self {
        ObjectTypes { list_name, object_types: Vec::new() }
    }

    /// `addObjectType`: appends the name unless it is already in the list.
    pub fn add_object_type(&mut self, object_type: &AsciiString) {
        if self.is_in_set(object_type) {
            return;
        }
        self.object_types.push(object_type.clone());
    }

    /// `removeObjectType`: removes the first occurrence of the name. A name
    /// that is not there is upstream's `DEBUG_CRASH` (a no-op in release
    /// builds): returns `false` and changes nothing; `true` when removed.
    pub fn remove_object_type(&mut self, object_type: &AsciiString) -> bool {
        match self.object_types.iter().position(|t| t == object_type) {
            Some(i) => {
                self.object_types.remove(i);
                true
            }
            None => false,
        }
    }

    /// `getListName()`.
    pub fn list_name(&self) -> &AsciiString {
        &self.list_name
    }

    /// `setListName(listName)`.
    pub fn set_list_name(&mut self, list_name: AsciiString) {
        self.list_name = list_name;
    }

    /// `isInSet(const AsciiString&)`: whether the name is in the list.
    pub fn is_in_set(&self, object_type: &AsciiString) -> bool {
        self.object_types.iter().any(|t| t == object_type)
    }

    /// `isInSet(const ThingTemplate*)`: `template_name` is the template's
    /// `getName()`, or `None` for a null template (`FALSE`).
    pub fn is_in_set_of_template(&self, template_name: Option<&AsciiString>) -> bool {
        match template_name {
            Some(name) => self.is_in_set(name),
            None => false,
        }
    }

    /// `getListSize()`: the number of names.
    pub fn list_size(&self) -> usize {
        self.object_types.len()
    }

    /// `getNthInList(index)`: the name at `index`, or the empty string for an
    /// index outside the list (a negative index included).
    pub fn get_nth_in_list(&self, index: i32) -> AsciiString {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.object_types.get(i))
            .cloned()
            .unwrap_or_default()
    }

    /// `prepForPlayerCounting(templates, counts)`: appends every name that
    /// `find` resolves to `templates`, in list order, then resizes `counts` to
    /// the length of `templates` (keeping the values it had). Returns that
    /// length. `find` is `TheThingFactory->findTemplate`.
    pub fn prep_for_player_counting<T: Copy>(
        &self,
        find: impl Fn(&AsciiString) -> Option<T>,
        templates: &mut Vec<T>,
        counts: &mut Vec<i32>,
    ) -> i32 {
        for name in &self.object_types {
            if let Some(templ) = find(name) {
                templates.push(templ);
            }
        }
        let ret_val = templates.len() as i32;
        counts.resize(ret_val as usize, 0);
        ret_val
    }

    /// `canBuildAny(player)`: whether any resolvable name in the list is
    /// buildable. `find` is `TheThingFactory->findTemplate` and `can_build`
    /// is `player->canBuild`, which upstream calls only for a template that
    /// exists.
    pub fn can_build_any<T: Copy>(
        &self,
        find: impl Fn(&AsciiString) -> Option<T>,
        can_build: impl Fn(T) -> bool,
    ) -> bool {
        self.object_types.iter().any(|name| match find(name) {
            Some(templ) => can_build(templ),
            None => false,
        })
    }

    /// `ObjectTypes::xfer`, version 1. See the module docs for the layout and
    /// for the load and CRC branch.
    pub fn xfer<X: Xfer + ?Sized>(&mut self, xfer: &mut X) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;

        // list name
        let mut name = self.list_name.str().to_vec();
        xfer.xfer_ascii_string(&mut name)?;
        if xfer.get_xfer_mode() != XferMode::Save {
            self.list_name = AsciiString::try_from_bytes(&name).map_err(|_| XferStatus::StringError)?;
        }

        // size of object types vector: an UnsignedShort, so the count wraps
        let mut count = self.object_types.len() as u16;
        xfer.xfer_unsigned_short(&mut count)?;

        if xfer.get_xfer_mode() == XferMode::Save {
            for object_type in &self.object_types {
                let mut bytes = object_type.str().to_vec();
                xfer.xfer_ascii_string(&mut bytes)?;
            }
        } else {
            // sanity: the vector must be empty when loading (and in CRC mode,
            // which upstream's `else` branch also serves)
            if !self.object_types.is_empty() {
                return Err(XferStatus::ListNotEmpty);
            }
            for _ in 0..count {
                let mut bytes = Vec::new();
                xfer.xfer_ascii_string(&mut bytes)?;
                self.object_types
                    .push(AsciiString::try_from_bytes(&bytes).map_err(|_| XferStatus::StringError)?);
            }
        }
        Ok(())
    }

    /// `ObjectTypes::crc`: empty upstream and here.
    pub fn crc<X: Xfer + ?Sized>(&mut self, _xfer: &mut X) -> XferResult<()> {
        Ok(())
    }

    /// `ObjectTypes::loadPostProcess`: empty upstream and here.
    pub fn load_post_process(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;

    fn name(s: &str) -> AsciiString {
        AsciiString::try_from_bytes(s.as_bytes()).unwrap()
    }

    fn sample() -> ObjectTypes {
        let mut t = ObjectTypes::with_list_name(name("Ground"));
        for n in ["Tank", "Jeep", "Tank", "Gun"] {
            t.add_object_type(&name(n));
        }
        t
    }

    #[test]
    fn add_keeps_insertion_order_and_refuses_duplicates() {
        let t = sample();
        assert_eq!(t.list_size(), 3);
        assert_eq!(t.get_nth_in_list(0), name("Tank"));
        assert_eq!(t.get_nth_in_list(1), name("Jeep"));
        assert_eq!(t.get_nth_in_list(2), name("Gun"));
        assert!(t.is_in_set(&name("Jeep")));
        assert!(!t.is_in_set(&name("jeep")));
    }

    #[test]
    fn get_nth_outside_the_list_is_empty() {
        let t = sample();
        assert!(t.get_nth_in_list(3).is_empty());
        assert!(t.get_nth_in_list(-1).is_empty());
        assert!(t.get_nth_in_list(i32::MIN).is_empty());
    }

    #[test]
    fn remove_takes_the_first_and_refuses_a_missing_name() {
        let mut t = sample();
        assert!(t.remove_object_type(&name("Jeep")));
        assert_eq!(t.list_size(), 2);
        assert_eq!(t.get_nth_in_list(1), name("Gun"));
        assert!(!t.remove_object_type(&name("Jeep")));
        assert_eq!(t.list_size(), 2);
    }

    #[test]
    fn null_template_is_not_in_the_set() {
        let t = sample();
        assert!(!t.is_in_set_of_template(None));
        assert!(t.is_in_set_of_template(Some(&name("Gun"))));
    }

    #[test]
    fn prep_appends_and_returns_the_whole_length() {
        let t = sample();
        let find = |n: &AsciiString| match n.str() {
            b"Tank" => Some(10u32),
            b"Gun" => Some(30u32),
            _ => None,
        };
        let mut templates = vec![99u32];
        let mut counts = vec![7, 8, 9, 10, 11];
        let n = t.prep_for_player_counting(find, &mut templates, &mut counts);
        assert_eq!(templates, vec![99, 10, 30]);
        assert_eq!(n, 3);
        assert_eq!(counts, vec![7, 8, 9]);
    }

    #[test]
    fn can_build_any_asks_only_resolvable_names() {
        let t = sample();
        let find = |n: &AsciiString| if n.str() == b"Gun" { Some(30u32) } else { None };
        assert!(t.can_build_any(find, |x| x == 30));
        assert!(!t.can_build_any(find, |_| false));
    }

    /// The bytes of a save of `t`, through the open/close protocol.
    fn save_of(t: &ObjectTypes) -> Vec<u8> {
        let mut save = XferSave::new();
        save.open(b"list").unwrap();
        t.clone().xfer(&mut save).unwrap();
        save.close().unwrap();
        save.take_bytes()
    }

    /// A load of `bytes` into `t`, through the open/close protocol.
    fn load_into(t: &mut ObjectTypes, bytes: Vec<u8>) -> XferResult<()> {
        let mut load = XferLoad::new(bytes);
        load.open(b"list").unwrap();
        let r = t.xfer(&mut load);
        load.close().unwrap();
        r
    }

    #[test]
    fn save_then_load_round_trips_the_list() {
        let t = sample();
        let mut loaded = ObjectTypes::new();
        load_into(&mut loaded, save_of(&t)).unwrap();
        assert_eq!(loaded, t);
    }

    #[test]
    fn crc_and_load_post_process_leave_the_list_alone() {
        let mut t = sample();
        let before = t.clone();
        let mut save = XferSave::new();
        save.open(b"crc").unwrap();
        t.crc(&mut save).unwrap();
        save.close().unwrap();
        assert!(save.take_bytes().is_empty());
        t.load_post_process();
        assert_eq!(t, before);
    }

    #[test]
    fn loading_into_a_non_empty_list_is_refused() {
        let mut busy = sample();
        assert_eq!(load_into(&mut busy, save_of(&ObjectTypes::new())), Err(XferStatus::ListNotEmpty));
    }
}
