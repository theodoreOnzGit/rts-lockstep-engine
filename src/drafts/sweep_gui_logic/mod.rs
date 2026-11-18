//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Sweep 2 (#213) group `gui_logic`: the `Class::method` definitions of GameClient/GUI, GameLogic (Object, AI, System,
//! Map) and GameNetwork whose names appear nowhere in the verified port or the first-pass drafts.

pub mod ai_group;
pub mod ai_states;
pub mod battle_plan_update;
pub mod chinook_ai_update;
pub mod misc_object_accessors;
pub mod partition_manager;
pub mod physics_update;
pub mod network;
pub mod game_message_parser;
pub mod file_transfer;
pub mod chat;
