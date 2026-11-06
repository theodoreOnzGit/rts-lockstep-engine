// SPDX-License-Identifier: GPL-3.0-only
//! The game client's translated parts that need no renderer (phase 2, C2, GitHub #6): the in-game UI's selection model and,
//! next, the message stream's input translators. The Bevy app (`client/`) gives them picking and input. Since #25,
//! also the drawables: `w3d_model_draw` (the model condition draw module) and
//! `drawable` (each object's drawable, fed by the logic's client events). Since #11, `terrain_height_map` (the
//! terrain's tiles, atlas, UVs and vertices). Since #12, the game's own UI from its files: `image` (the mapped images),
//! `window_script` (the WND layout reader), `control_bar_scheme` (the side's look of the control bar) and
//! `control_bar_command` (the selected object's command buttons and build queue). Since #35, `control_bar_popup` (the
//! selected object's portrait and the command buttons' popup description). Since #43 and #44, the effects: `particle_sys`
//! (the particle system templates, systems and particles), `fx_list` (the FX lists) and `client_fx` (both fed by the
//! logic's client events, in the drawables' event order).

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
pub mod w3d_truck_draw;
pub mod drawable;
pub mod particle_sys;
pub mod fx_list;
pub mod client_fx;
pub mod drawable_loco;
pub mod terrain_height_map;
pub mod image;
pub mod window_script;
pub mod control_bar_scheme;
pub mod control_bar_command;
pub mod control_bar_popup;
