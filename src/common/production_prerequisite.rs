// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/ProductionPrerequisite.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/ProductionPrerequisite.cpp
//
// Original copyright notice of those files:
//   Westwood Studios Pacific.
//   Confidential Information
//   Copyright (C) 2001 - All Rights Reserved
//   Project: RTS3
//   Created: Steven Johnson, October 2001
//   (The files carry no GPL header of their own; the repository is released
//   under the GPL-3.0 with the copyright notice above, see NOTICE.md.)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ProductionPrerequisite`: what a thing template needs before it can be
//! built (the `Prerequisites` block of an `Object`): a list of unit
//! templates, each either required in addition to the others or an
//! alternative to the one before it, and a list of sciences.
//!
//! # What is translated
//!
//! Only the part that needs no `Player`: the data, `init`, `resetSciences`,
//! `addSciencePrereq`, `resetUnits`, both `addUnitPrereq`, `resolveNames` and
//! `getAllPossibleBuildFacilityTemplates`. These are what `ThingTemplate`
//! uses while the INI is read and when the template database is resolved.
//!
//! # Translation choices
//!
//! - **`const ThingTemplate*`** is the template's id
//!   ([`ThingTemplate::template_id`](super::thing_template::ThingTemplate::template_id)): `resolveNames` stores what
//!   `TheThingFactory->findTemplate(name)` returns, the top-most template of
//!   that name, whose id never changes.
//! - **`resolveNames`** takes the lookup as a closure instead of reading
//!   `TheThingFactory`.
//!
//! # Cut points: methods that take a `Player` (layer 7)
//!
//! `isSatisfied(player)`, `getRequiresList(player)` (also needs `TheGameText`
//! and the templates' display names) and `getExistingBuildFacilityTemplate(player)`
//! count the objects a player owns (`Player::countObjectsByThingTemplate`) and
//! ask `Player::hasScience`; `calcNumPrereqUnitsOwned` is their helper. They
//! arrive with `Player`.

use super::science::{ScienceType, ScienceVec};

/// `UNIT_OR_WITH_PREV`: "if set, unit is 'or-ed' with prev unit, so that
/// either one's presence satisfies".
pub const UNIT_OR_WITH_PREV: i32 = 0x01;

/// `MAX_PREREQ`: the most unit prerequisites that the player-dependent
/// methods look at (and `getAllPossibleBuildFacilityTemplates`'s caller's
/// array size in `ThingTemplate::resolveNames`).
pub const MAX_PREREQ: usize = 32;

/// `struct PrereqUnitRec`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrereqUnitRec {
    /// `unit`: the template, once `resolveNames` has found it (its id).
    pub unit: Option<u16>,
    /// `flags`.
    pub flags: i32,
    /// `name`: the template's name until `resolveNames` has looked it up.
    pub name: Vec<u8>,
}

/// `class ProductionPrerequisite`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProductionPrerequisite {
    /// `m_prereqUnits`.
    prereq_units: Vec<PrereqUnitRec>,
    /// `m_prereqSciences`.
    prereq_sciences: ScienceVec,
}

impl ProductionPrerequisite {
    /// `ProductionPrerequisite()`: `init()`, no units and no sciences.
    pub fn new() -> Self {
        Self::default()
    }

    /// `init()`: "init to safe default values".
    pub fn init(&mut self) {
        self.prereq_units.clear();
        self.prereq_sciences.clear();
    }

    /// `resetSciences()`.
    pub fn reset_sciences(&mut self) {
        self.prereq_sciences.clear();
    }

    /// `addSciencePrereq(science)`.
    pub fn add_science_prereq(&mut self, science: ScienceType) {
        self.prereq_sciences.push(science);
    }

    /// `resetUnits()`.
    pub fn reset_units(&mut self) {
        self.prereq_units.clear();
    }

    /// `addUnitPrereq(unit, orUnitWithPrevious)`: with `or_unit_with_previous`
    /// the unit is an alternative to the previous one, otherwise it is
    /// required in addition to the others.
    pub fn add_unit_prereq(&mut self, unit: &[u8], or_unit_with_previous: bool) {
        self.prereq_units.push(PrereqUnitRec {
            unit: None,
            flags: if or_unit_with_previous {
                UNIT_OR_WITH_PREV
            } else {
                0
            },
            name: unit.to_vec(),
        });
    }

    /// `addUnitPrereq(units)`: the first is required, the others are
    /// alternatives to the one before.
    pub fn add_unit_prereqs(&mut self, units: &[Vec<u8>]) {
        let mut or_with_previous = false;
        for u in units {
            self.add_unit_prereq(u, or_with_previous);
            or_with_previous = true;
        }
    }

    /// The unit records, in order.
    pub fn prereq_units(&self) -> &[PrereqUnitRec] {
        &self.prereq_units
    }

    /// The sciences, in order.
    pub fn prereq_sciences(&self) -> &[ScienceType] {
        &self.prereq_sciences
    }

    /// `resolveNames()`: "called after all ThingTemplates have been loaded".
    /// Every unit whose name is not empty gets what `find` returns for it
    /// (`TheThingFactory->findTemplate(name)`: the top-most template of that
    /// name, or none), and its name is cleared ("we're done with it"). A unit
    /// with an empty name is left as it is, which is what keeps the templates
    /// of an earlier resolve when an override changes something else.
    pub fn resolve_names(&mut self, mut find: impl FnMut(&[u8]) -> Option<u16>) {
        for rec in &mut self.prereq_units {
            if !rec.name.is_empty() {
                rec.unit = find(&rec.name);
                rec.name.clear();
            }
        }
    }

    /// `getAllPossibleBuildFacilityTemplates(tmpls, maxtmpls)`: the first
    /// unit and the alternatives that follow it (stopping at the next unit
    /// that is not an alternative), at most `maxtmpls`; an unresolved unit
    /// is `None`.
    pub fn all_possible_build_facility_templates(&self, maxtmpls: usize) -> Vec<Option<u16>> {
        let mut out = Vec::new();
        for (i, rec) in self.prereq_units.iter().enumerate() {
            if i > 0 && rec.flags & UNIT_OR_WITH_PREV == 0 {
                break;
            }
            if out.len() >= maxtmpls {
                break;
            }
            out.push(rec.unit);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::name_key_generator::NameKeyType;

    #[test]
    fn units_and_alternatives_are_listed_with_their_flags() {
        let mut p = ProductionPrerequisite::new();
        p.add_unit_prereq(b"A", false);
        p.add_unit_prereqs(&[b"B".to_vec(), b"C".to_vec()]);
        let flags: Vec<i32> = p.prereq_units().iter().map(|r| r.flags).collect();
        assert_eq!(flags, [0, 0, UNIT_OR_WITH_PREV]);
        p.add_science_prereq(ScienceType::from_name_key(NameKeyType(7)));
        assert_eq!(p.prereq_sciences().len(), 1);
        p.reset_sciences();
        assert!(p.prereq_sciences().is_empty());
        p.reset_units();
        assert!(p.prereq_units().is_empty());
        p.add_unit_prereq(b"X", true);
        p.init();
        assert!(p.prereq_units().is_empty() && p.prereq_sciences().is_empty());
    }

    #[test]
    fn resolve_names_looks_each_named_unit_up_once_and_clears_the_name() {
        let mut p = ProductionPrerequisite::new();
        p.add_unit_prereqs(&[b"One".to_vec(), b"Two".to_vec()]);
        p.add_unit_prereq(b"Three", false);
        let mut asked = Vec::new();
        p.resolve_names(|n| {
            asked.push(n.to_vec());
            if n == b"Two" {
                None
            } else {
                Some(n.len() as u16)
            }
        });
        assert_eq!(asked, [b"One".to_vec(), b"Two".to_vec(), b"Three".to_vec()]);
        let units: Vec<Option<u16>> = p.prereq_units().iter().map(|r| r.unit).collect();
        assert_eq!(units, [Some(3), None, Some(5)]);
        assert!(p.prereq_units().iter().all(|r| r.name.is_empty()));
        // a second resolve asks about nothing and keeps what it found
        p.resolve_names(|_| panic!("no name left"));
        assert_eq!(p.prereq_units()[0].unit, Some(3));
    }

    #[test]
    fn build_facility_templates_are_the_first_block_of_alternatives() {
        let mut p = ProductionPrerequisite::new();
        p.add_unit_prereqs(&[b"A".to_vec(), b"B".to_vec(), b"C".to_vec()]);
        p.add_unit_prereq(b"D", false);
        p.resolve_names(|n| Some(u16::from(n[0])));
        assert_eq!(
            p.all_possible_build_facility_templates(32),
            [Some(65), Some(66), Some(67)]
        );
        assert_eq!(
            p.all_possible_build_facility_templates(2),
            [Some(65), Some(66)]
        );
        assert!(ProductionPrerequisite::new()
            .all_possible_build_facility_templates(32)
            .is_empty());
    }
}
