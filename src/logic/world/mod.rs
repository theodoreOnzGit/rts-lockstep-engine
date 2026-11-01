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
pub mod victory_conditions;
pub mod script_conditions;
pub mod script_actions;
pub mod script_engine;
pub mod build_assistant;
pub mod create_host_world;
pub mod weapon_host_world; // layer 12, batch W1b: WeaponHost for World
pub mod update_host_world;
pub mod helper_host_world;
pub mod action_host_world;
pub mod partition_host_world; // layer 12, batch I1: PartitionHost for World
pub mod terrain_host_world; // batch I2: TerrainWorld for World
pub mod object_map_props; // layer 8, batch O1: updateObjValuesFromMapProperties
pub mod player_host_world; // batch I3: the team and player hosts for World
pub mod team_live_world; // batch I3b: the world's own team and player answers (live_teams)
pub mod game_logic_host_world; // batch I4: GameLogicHost for World (the frame and the CRC)
pub mod victory_host_world; // batch I5: VictoryHost for World, TheVictoryConditions in the world
pub mod build_assistant_host_world; // batch I6: BuildAssistantHost for World, TheBuildAssistant in the world
pub mod game_logic_save_host_world; // batch I7a: GameLogicSaveHost for World (the GameLogic save block)
pub mod dispatch_host_world; // batch I7b: DispatchHost for World (the command list's dispatcher)
pub mod start_host_world; // batch I7c: StartHost for World (the starting buildings and units)
pub mod new_game_host_world; // batch I7c: NewGameHost for World (GameLogic::startNewGame)
pub mod script_engine_host_world; // batch I8a: ScriptEngineHost for World, TheScriptEngine in the world
pub mod script_conditions_host_world; // batch I8b: ScriptConditionsHost for World, TheScriptConditions in the world
pub mod script_actions_host_world; // batch I8c: ScriptActionsHost for World, TheScriptActions in the world
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
// --- p1w11-sonnet ---
pub mod thing_factory_world;
pub mod object_queries;
// --- end p1w11-sonnet ---
// --- p1w13-sonnet ---
pub mod body_cuts;
pub mod contain_cuts;
// --- end p1w13-sonnet ---
// --- layer 12 ---
pub mod game_logic_dispatch;
pub mod game_logic_new_game;
pub mod game_logic_start;
pub mod game_logic_update;
pub mod game_logic_xfer;
pub mod pathfinder_host_world;
pub mod ai_host_world;
pub mod ai_state_host_world;
pub mod ai_module_world;
pub mod gate_scenario; // batch I9d: the scripted game of Phase 1 gate (a)
pub mod ai_group_world; // batch I9c-2: TheAI's groups in the World
pub mod ai_query_world; // batch I9c-3: TheAI's queries in the World
pub mod ai_player_world;
pub mod script_orders_world; // batch I8c: the script actions' orders to units, groups and players // batch I9e: the computer players in the World
pub mod ai_combat_world; // batch I9c-5: the attack machine's weapon, body and partition calls in the World
pub mod collision_world; // batch I9c-4: the AI's collisions in the World
pub mod turret_host_world; // batch I9b-6: the turrets' AIs, the World as TurretHost
