// SPDX-License-Identifier: GPL-3.0-only
//! The game client's translated parts that need no renderer (phase 2, C2, GitHub #6): the in-game UI's selection model and,
//! next, the message stream's input translators. The Bevy app (`client/`) gives them picking and input. Since #25,
//! also the drawables: `w3d_model_draw` (the model condition draw module) and
//! `drawable` (each object's drawable, fed by the logic's client events). Since #11, `terrain_height_map` (the
//! terrain's tiles, atlas, UVs and vertices).

pub mod in_game_ui;
pub mod selection_info;
pub mod selection_xlat;
pub mod selection_groups;
pub mod command_xlat;
pub mod command_world;
pub mod control_bar;
pub mod place_event;
pub mod world_host;
pub mod w3d_model_draw;
pub mod drawable;
pub mod terrain_height_map;
