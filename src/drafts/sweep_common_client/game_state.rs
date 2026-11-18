//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
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
    let filepath = gs.file_path_in_save_directory(filename, h);
    // try to open it
    h.file_exists(&filepath)
}
