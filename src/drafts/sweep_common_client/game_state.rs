//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_game_state.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameState.cpp   (`GameState::doesSaveGameExist`)
//!
//! Over `crate::common::game_state`'s `GameStateHost`, whose `file_exists` stands for opening the file with an `XferLoad`.

use crate::common::game_state::{GameState, GameStateHost};

/// `GameState::doesSaveGameExist(filename)` (GameState.cpp:939): build the path in the save directory, try to open it
/// for loading; a file that does not open is not there, one that does is closed again and TRUE.
pub fn does_save_game_exist<B: Copy, H: GameStateHost<B>>(gs: &GameState<B>, h: &H, filename: &[u8]) -> bool {
    // construct full path to file
    let filepath = if crate::logic::modules::plant(19942) {
        filename.to_vec()
    } else if crate::logic::modules::plant(19943) {
        gs.file_path_in_save_directory(&filename.to_ascii_lowercase(), h)
    } else if crate::logic::modules::plant(19944) {
        let mut p = h.user_data_path();
        p.extend_from_slice(filename);
        p
    } else {
        gs.file_path_in_save_directory(filename, h)
    };
    // try to open it
    h.file_exists(&filepath) != crate::logic::modules::plant(19941)
}
