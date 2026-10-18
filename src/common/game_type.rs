// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/GameType.h (the id enums, time
//   of day, weather, scorches, weapon slots, pathfind layers)
//   GeneralsMD/Code/GameEngine/Source/Common/System/GameType.cpp
//   (TimeOfDayNames, WeatherNames)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   GameType.h: Michael S. Booth, April 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The basic game-wide identifiers and enumerations of `GameType.h`.
//!
//! Upstream's `ObjectID`, `DrawableID` and `FormationID` are enums used as
//! 32-bit integers (`FORCE_*_TO_LONG_SIZE` only sizes them); here they are
//! `i32` aliases (the ones `common::xfer` moves). Their invalid value is 0.

/// `DEFAULT_WORLD_WIDTH`.
pub const DEFAULT_WORLD_WIDTH: i32 = 64;
/// `DEFAULT_WORLD_HEIGHT`.
pub const DEFAULT_WORLD_HEIGHT: i32 = 64;

/// `enum ObjectID`: a unique identifier of an `Object`. An `int`-sized
/// enum upstream, used as a counter; `INVALID_ID` is 0.
pub type ObjectId = i32;
/// `INVALID_ID`.
pub const INVALID_ID: ObjectId = 0;

/// `enum DrawableID`: a unique identifier of a `Drawable` (client side).
pub type DrawableId = i32;
/// `INVALID_DRAWABLE_ID`.
pub const INVALID_DRAWABLE_ID: DrawableId = 0;

/// `enum FormationID`: a player-specified formation.
pub type FormationId = i32;
/// `NO_FORMATION_ID`: the unit is not in a formation.
pub const NO_FORMATION_ID: FormationId = 0;

/// `FORCE_OBJECTID_TO_LONG_SIZE` (and the `DrawableID`/`FormationID` ones):
/// `0x7ffffff`, with seven `f`s. **Upstream quirk:** it is 2^27 - 1, not
/// the 2^31 - 1 the name suggests; it only sizes the enum, it does not cap
/// the ids.
pub const FORCE_ID_TO_LONG_SIZE: i32 = 0x07ff_ffff;

/// `#define INVALID_ANGLE -100.0f`.
pub const INVALID_ANGLE: f32 = -100.0;

/// `enum TimeOfDay` (keep in sync with [`TIME_OF_DAY_NAMES`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum TimeOfDay {
    Invalid = 0,
    Morning = 1,
    Afternoon = 2,
    Evening = 3,
    Night = 4,
}
/// `TIME_OF_DAY_FIRST` (= `TIME_OF_DAY_MORNING`).
pub const TIME_OF_DAY_FIRST: i32 = 1;
/// `TIME_OF_DAY_COUNT`.
pub const TIME_OF_DAY_COUNT: i32 = 5;

/// `TimeOfDayNames[]` (GameType.cpp), without the `NULL` terminator.
pub const TIME_OF_DAY_NAMES: [&str; 5] = ["NONE", "MORNING", "AFTERNOON", "EVENING", "NIGHT"];

/// `enum Weather`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum Weather {
    Normal = 0,
    Snowy = 1,
}
/// `WEATHER_COUNT`.
pub const WEATHER_COUNT: i32 = 2;

/// `WeatherNames[]` (GameType.cpp), without the `NULL` terminator.
pub const WEATHER_NAMES: [&str; 2] = ["NORMAL", "SNOWY"];

/// `enum Scorches`: `SCORCH_1`..`SCORCH_4` are 0..3, `SHADOW_SCORCH` 4 (the
/// craters and decals after it are commented out upstream).
pub const SCORCH_1: i32 = 0;
/// `SCORCH_2`.
pub const SCORCH_2: i32 = 1;
/// `SCORCH_3`.
pub const SCORCH_3: i32 = 2;
/// `SCORCH_4`.
pub const SCORCH_4: i32 = 3;
/// `SHADOW_SCORCH`.
pub const SHADOW_SCORCH: i32 = 4;
/// `SCORCH_COUNT`.
pub const SCORCH_COUNT: i32 = 5;

/// `enum WeaponSlotType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum WeaponSlotType {
    Primary = 0,
    Secondary = 1,
    Tertiary = 2,
}
/// `WEAPONSLOT_COUNT`.
pub const WEAPONSLOT_COUNT: i32 = 3;

/// `enum PathfindLayerEnum`: 1 is the ground; 2 up to `LAYER_LAST - 1` index
/// bridges in the pathfinder (they have no names); 15 is the wall layer.
pub type PathfindLayer = i32;
/// `LAYER_INVALID`.
pub const LAYER_INVALID: PathfindLayer = 0;
/// `LAYER_GROUND`.
pub const LAYER_GROUND: PathfindLayer = 1;
/// `LAYER_WALL`.
pub const LAYER_WALL: PathfindLayer = 15;
/// `LAYER_LAST`.
pub const LAYER_LAST: PathfindLayer = 15;
