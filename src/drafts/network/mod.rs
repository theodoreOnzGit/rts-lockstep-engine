//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! `GameEngine/Source/GameNetwork`: the lockstep network layer.

pub mod ip_enumeration;
pub mod lan_api;
pub mod lan_api_handlers;
pub mod lan_game_info;
pub mod nat;
pub mod net_command_msg;
pub mod connection;
pub mod connection_manager;
pub mod disconnect_manager;
pub mod frame_data;
pub mod game_spy_game_info;
pub mod frame_data_manager;
pub mod frame_metrics;
pub mod net_command_list;
pub mod net_command_ref;
pub mod net_command_wrapper_list;
pub mod net_message_stream;
pub mod net_packet;
pub mod network_util;
pub mod transport;
pub mod udp;
pub mod user;
pub mod staging_room_game_info;
pub mod buddy_thread;
pub mod persistent_storage_thread;
pub mod peer_thread;
pub mod ping_thread;
pub mod game_results_thread;
pub mod gs_config;
pub mod ladder_defs;
pub mod download_manager;
pub mod main_menu_utils;
pub mod firewall_helper;
