// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file: the module tree of layer 10, the AI and the pathfinder (`p1w13-sonnet`).

//! Layer 10: the AI and the pathfinder (`AIPathfind`, `AIStates`, `AIUpdate` and its subclasses, `AIPlayer`, `AIGroup`, ...), translated
//! batch by batch. See `docs/phase1-porting-graph.md`.

pub mod adjust;
pub mod ai_update;
pub mod ai_update_data;
pub mod ai_commands;
pub mod ai_move_states;
pub mod ai_waypoint_states;
pub mod ai_attack_states;
pub mod ai_attack_move;
pub mod ai_hunt_states;
pub mod ai_face_exit_states;
pub mod ai_fire_host;
pub mod ai_paths;
pub mod ai_states;
pub mod ai_update_types;
pub mod attack;
pub mod cell;
pub mod fire_state;
pub mod ground;
pub mod hierarchical;
pub mod layer;
pub mod movement;
pub mod path;
pub mod queue;
pub mod search;
pub mod turret;
pub mod pathfinder;
pub mod zone;
