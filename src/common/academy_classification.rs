// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/AcademyStats.h (lines 71-78:
//   `enum AcademyClassificationType` and its name list declaration)
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/AcademyStats.cpp (lines
//   71-77: `TheAcademyClassificationTypeNames`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Kris Morness, July 2003
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `enum AcademyClassificationType` and its INI names: "a value used by the
//! academy to evaluate advice based on what players do". `Upgrade` and
//! `SpecialPower` templates carry one (`AcademyClassify = ...`). Only the
//! enum and its names are here; the rest of `AcademyStats.h` (the statistics
//! of a `Player`, layer 7) is not translated, and when it is, it should
//! import this module rather than define the enum again.

/// `enum AcademyClassificationType` ("don't forget to update the strings
/// too!").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum AcademyClassificationType {
    /// `ACT_NONE`.
    #[default]
    None = 0,
    /// `ACT_UPGRADE_RADAR`.
    UpgradeRadar = 1,
    /// `ACT_SUPERPOWER`.
    Superpower = 2,
}

/// `TheAcademyClassificationTypeNames[]`, without the `NULL` terminator.
pub const ACADEMY_CLASSIFICATION_TYPE_NAMES: [&str; 3] = ["ACT_NONE", "ACT_UPGRADE_RADAR", "ACT_SUPERPOWER"];

impl AcademyClassificationType {
    /// The enumerator at an index of [`ACADEMY_CLASSIFICATION_TYPE_NAMES`]
    /// (what `INI::parseIndexList` stores), or `None` outside the list.
    pub fn from_index(i: i32) -> Option<Self> {
        match i {
            0 => Some(Self::None),
            1 => Some(Self::UpgradeRadar),
            2 => Some(Self::Superpower),
            _ => Option::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_values_line_up() {
        for (i, _) in ACADEMY_CLASSIFICATION_TYPE_NAMES.iter().enumerate() {
            assert_eq!(AcademyClassificationType::from_index(i as i32).map(|a| a as i32), Some(i as i32));
        }
        assert_eq!(AcademyClassificationType::from_index(3), None);
        assert_eq!(AcademyClassificationType::from_index(-1), None);
        assert_eq!(AcademyClassificationType::default(), AcademyClassificationType::None);
    }
}
