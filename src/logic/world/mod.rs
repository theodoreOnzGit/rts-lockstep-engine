// SPDX-License-Identifier: GPL-3.0-only
//! The shared world core for layers 7 and 8: the object arena, the player and
//! team arenas, the update scheduler, the one client-event queue, and the one
//! context ([`World`]) passed to everything that upstream reaches through
//! `TheGameLogic`, `ThePlayerList`, `TheTeamFactory` and the client singletons.
//!
//! The design, the reasons for it, how later layers plug in (with a worked
//! example) and what is left are in `docs/world-design.md`. Each file's
//! module documentation is the reference for what it translates.
//!
//! | file | holds |
//! |---|---|
//! | `ids` | `ObjectId`, `TeamId`, `PlayerIndex` and their invalid values |
//! | `object` | `ObjectArena` (`m_objList` + `m_objVector`) and the `Object` shell |
//! | `sleepy` | `SleepyScheduler` (`m_sleepyUpdates`, `m_curUpdateModule`) |
//! | `update_body` | `UpdateBody`, the enum of concrete update modules |
//! | `player` | `PlayerArena` (`PlayerList`) shell, and `PlayerRelationMap` |
//! | `team`, `team_ops`, `team_xfer` | `TeamArena` (`TeamFactory`, every `TeamPrototype` and `Team`), their operations over the `TeamHost` trait, and their save and load |
//! | `events` | `ClientEventQueue` |
//! | `client_reads` | `ClientReads`: the client values logic reads, as state |
//! | `game_logic` | `World` and the translation of `GameLogic`'s object lifetime |

pub mod client_reads;
pub mod events;
pub mod game_logic;
pub mod ids;
pub mod object;
// --- p1w6-object ---
pub mod module_call;
pub mod module_probe;
pub mod object_create;
pub mod object_cuts;
pub mod object_damage;
pub mod object_events;
pub mod object_modules;
pub mod object_state;
pub mod object_team;
pub mod object_transform;
pub mod object_xfer;
#[cfg(test)]
mod object_tests;
// --- end p1w6-object ---
pub mod player;
// --- p1w10-sonnet ---
pub mod player_objects;
pub mod player_squads;
pub mod player_init;
pub mod player_list;
pub mod player_xfer;
pub mod player_ops;
pub mod build_assistant;
// --- end p1w10-sonnet ---
pub mod sleepy;
pub mod team;
// --- p1w9-sonnet ---
pub mod team_ops;
pub mod team_xfer;
// --- end p1w9-sonnet ---
pub mod update_body;
// --- p1w6-energy ---
pub mod academy_stats;
pub mod energy;
pub mod mission_stats;
pub mod score_keeper;
// --- end p1w6-energy ---
// --- p1w7-sonnet ---
pub mod cave_system;
pub mod resource_gathering_manager;
pub mod tunnel_tracker;
// --- end p1w7-sonnet ---
// --- p1w8-sonnet ---
pub mod object_weapons;
// --- end p1w8-sonnet ---

pub use game_logic::{Hook, World, WorldError};
pub use ids::{ObjectId, PlayerIndex, TeamId, INVALID_ID};

// --- p1w6-partition ---
pub mod partition;
// --- end p1w6-partition ---
// --- p1l9-example ---
pub mod statics;
// --- end p1l9-example ---
