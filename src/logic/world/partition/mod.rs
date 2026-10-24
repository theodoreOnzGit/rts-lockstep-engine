// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/PartitionManager.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/PartitionManager.cpp
//   GeneralsMD/Code/GameEngine/Source/GameClient/Line2D.cpp (two functions)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, September 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PartitionManager` (layer 8): the spatial grid of the world. Every object's
//! footprint is registered in a grid of cells; the grid answers "what is near
//! here" (closest object, every object in range with a sort order and filters,
//! potential collisions), runs the collision pass of each logic frame, and
//! holds the logic side of the **shroud and fog of war** and the **threat and
//! value maps** per player.
//!
//! It is a field of [`World`](crate::logic::world::World) (`world.partition`),
//! checked out for its own `update` as the terrain is: see
//! `docs/world-design.md`. Objects are referenced by `ObjectId`; their
//! position, geometry and status, and the answers of the filters, come from a
//! *host* ([`PartitionHost`], [`PartitionFilterHost`]); the module docs of
//! `host` say why and what each method replaces.
//!
//! | file | holds |
//! |---|---|
//! | `collide` | the nine shape tests, `geomCollidesWithGeom`, the four distance procs |
//! | `cell` | `PartitionCell`, `ShroudLevel`, `SightingInfo` |
//! | `data` | `CellAndObjectIntersection`, `PartitionData` |
//! | `manager` | the grid, registration, the dirty list, the rasterisers, `update`, the contact list |
//! | `shroud` | looking, shrouding, the unlook queue, shrouded status, threat/value maps, store/restore |
//! | `query` | closest/range/all/potential-collision queries, distances, cell walks, LOS, `findPositionAround` |
//! | `filters` | the 33 `PartitionFilter` subclasses as an enum |
//! | `xfer` | CRC and save/load |
//! | `scenario` | a table-driven host for tests (and the differential test) |
//!
//! # Verification
//!
//! Unit tests in each file; `tests/differential_partition.rs` compiles the
//! unmodified `PartitionManager.cpp` (and `Line2D.cpp`) on stand-ins and
//! compares random worlds and query sequences (its header has the method and
//! the results).

pub mod cell;
pub mod collide;
pub mod data;
pub mod filters;
pub mod host;
pub mod manager;
pub mod query;
pub mod scenario;
pub mod shroud;
pub mod xfer;

#[cfg(test)]
mod tests;

pub use cell::{PartitionCell, ShroudLevel, SightingInfo};
pub use collide::{
    collide_test, dist_calc, geom_collides_with_geom, CollideInfo, CollideLocAndNormal, DistResult,
    DistanceCalculationType, HUGE_DIST, HUGE_DIST_SQR,
};
pub use filters::{filters_allow, PartitionFilter};
pub use host::{
    CanAttackResult, ContainFacts, GhostGeometry, ObjectGeometry, PartitionFilterHost,
    PartitionHost, PathfindCellType, PathfindLayer, StealthFacts, LAYER_GROUND,
};
pub use manager::{PartitionClientEvent, PartitionManager};
pub use query::{fpf, ClosestObject, FindPositionOptions, TerrainExtremes, RANDOM_START_ANGLE};
pub use shroud::{
    ShroudStatusStoreRestore, ValueOrThreat, STORE_DONTTOUCH, STORE_FOG, STORE_PERMANENTLY_REVEALED,
};
