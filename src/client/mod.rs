// SPDX-License-Identifier: GPL-3.0-only
//! The game client's translated parts that need no renderer (phase 2, C2, GitHub #6): the in-game UI's selection model and,
//! next, the message stream's input translators. The Bevy app (`client/`) gives them picking and input.

pub mod in_game_ui;
pub mod selection_info;
pub mod selection_xlat;
pub mod selection_groups;
pub mod command_xlat;
pub mod command_world;
