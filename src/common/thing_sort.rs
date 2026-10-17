// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/ThingSort.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   ThingSort.h: Colin Day, December 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `enum EditorSortingType` and its names (`EditorSortingNames[]`, defined
//! under `DEFINE_EDITOR_SORTING_NAMES`, which `ThingTemplate.cpp` uses for the
//! `EditorSorting` INI field). Editor data, but parsed from every object's INI.

/// `enum EditorSortingType`.
pub type EditorSortingType = i32;
/// `ES_FIRST`.
pub const ES_FIRST: EditorSortingType = 0;
/// `ES_NONE`.
pub const ES_NONE: EditorSortingType = ES_FIRST;
/// `ES_STRUCTURE`.
pub const ES_STRUCTURE: EditorSortingType = 1;
/// `ES_INFANTRY`.
pub const ES_INFANTRY: EditorSortingType = 2;
/// `ES_VEHICLE`.
pub const ES_VEHICLE: EditorSortingType = 3;
/// `ES_SHRUBBERY`.
pub const ES_SHRUBBERY: EditorSortingType = 4;
/// `ES_MISC_MAN_MADE`.
pub const ES_MISC_MAN_MADE: EditorSortingType = 5;
/// `ES_MISC_NATURAL`.
pub const ES_MISC_NATURAL: EditorSortingType = 6;
/// `ES_DEBRIS`.
pub const ES_DEBRIS: EditorSortingType = 7;
/// `ES_SYSTEM`: programmer objects.
pub const ES_SYSTEM: EditorSortingType = 8;
/// `ES_AUDIO`.
pub const ES_AUDIO: EditorSortingType = 9;
/// `ES_TEST`.
pub const ES_TEST: EditorSortingType = 10;
/// `ES_FOR_REVIEW`.
pub const ES_FOR_REVIEW: EditorSortingType = 11;
/// `ES_ROAD`.
pub const ES_ROAD: EditorSortingType = 12;
/// `ES_WAYPOINT`.
pub const ES_WAYPOINT: EditorSortingType = 13;
/// `ES_NUM_SORTING_TYPES`.
pub const ES_NUM_SORTING_TYPES: EditorSortingType = 14;

/// `EditorSortingNames[]`, without the `NULL` terminator.
pub const EDITOR_SORTING_NAMES: [&str; 14] = [
    "NONE",
    "STRUCTURE",
    "INFANTRY",
    "VEHICLE",
    "SHRUBBERY",
    "MISC_MAN_MADE",
    "MISC_NATURAL",
    "DEBRIS",
    "SYSTEM",
    "AUDIO",
    "TEST",
    "FOR_REVIEW",
    "ROAD",
    "WAYPOINT",
];
