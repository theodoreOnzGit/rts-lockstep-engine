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
//! logic's client events, in the drawables' event order). Since #13, the audio: `audio_info` (the audio INI, settings and misc
//! sounds), `audio_event` (`AudioEventRTS`: the file chosen), `game_audio` (the audio manager, with the device behind a
//! trait) and `client_audio` (the logic's sound events played). Since #51, `unit_voice` (the units' voice responses to
//! orders and selection) and `drawable_ambient` (the drawables' ambient sounds). Since #54, `gui_command_xlat` (a pending
//! control bar command carried out by the next click) and `control_bar_callback` (the radar's clicks). Since #35 (the rest
//! of it), `radar` (`TheRadar`: its objects, events and coordinates, fed by the logic's client events), `w3d_radar` (what
//! the radar draws) and `water` (the water INI's radar colour). Since #4, `save_game` (the whole save file: the client's blocks
//! with the logic's). Since #61, `campaign_manager` (`TheCampaignManager`) and `challenge_generals` (`TheChallengeGenerals`),
//! which the World holds, as the logic reads them.

pub mod in_game_ui;
pub mod selection_info;
pub mod selection_xlat;
pub mod selection_groups;
pub mod command_xlat;
pub mod command_world;
pub mod control_bar;
pub mod place_event;
pub mod gui_command_xlat;
pub mod control_bar_callback;
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
pub mod window_manager;
pub mod gadget;
pub mod gadget_list_box;
pub mod gadget_slider;
pub mod gadget_text_entry;
pub mod gadget_combo_box;
pub mod gadget_tab_control;
pub mod gadget_look;
pub mod ime_manager;
pub mod shell;
pub mod main_menu;
pub mod message_box;
pub mod shell_gui;
pub mod window_draw;
pub mod window_transitions;
pub mod animate_window;
pub mod control_bar_scheme;
pub mod control_bar_command;
pub mod control_bar_popup;
pub mod audio_info;
pub mod audio_event;
pub mod game_audio;
pub mod client_audio;
pub mod unit_voice;
pub mod drawable_ambient;
pub mod drawable_save;
pub mod eva;
pub mod radar;
pub mod radar_world;
pub mod w3d_radar;
pub mod water;
pub mod save_game;
pub mod campaign_manager;
pub mod challenge_generals;
pub mod save_load_menu;
pub mod single_player_menu;
pub mod difficulty_select;
// (#89) the skirmish screens: MapUtil.cpp's GUI half, GUIUtil.cpp, SkirmishGameOptionsMenu.cpp, SkirmishMapSelectMenu.cpp
pub mod map_util;
pub mod gui_util;
pub mod skirmish_game_options;
pub mod user_preferences;
pub mod options_menu;
pub mod skirmish_map_select;
pub mod score_screen;
// (#98) the replay menu and the score screen's replay popup (ReplayMenu.cpp, PopupReplay.cpp), the challenge menu
// (ChallengeMenu.cpp)
pub mod replay_menu;
pub mod popup_replay;
pub mod challenge_menu;
