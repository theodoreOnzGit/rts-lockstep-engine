//! UNTESTED DRAFT (#202, #203): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/W3DGameWindowManager.cpp
//!     (`W3DGameWindowManager::W3DGameWindowManager`, `~W3DGameWindowManager`, `init`)
//!
//! Already in the port: the manager itself is [`crate::client::window_manager::GameWindowManager`]; its draw
//! callbacks (the W3D device's part) are [`crate::client::window_draw`]. The device subclass adds nothing but the
//! constructor, the destructor and an `init` that extends the base class's.

use crate::client::window_manager::GameWindowManager;

/// `W3DGameWindowManager::W3DGameWindowManager()`: empty upstream; the base manager is created as the port does.
pub fn new_w3d_game_window_manager(display: (i32, i32)) -> GameWindowManager {
    GameWindowManager::new(display)
}

/// `W3DGameWindowManager::init()`: "extend" `GameWindowManager::init()`. The port's manager needs no separate init
/// (its construction is [`GameWindowManager::new`]); a reset is the closest base-class call and is not made here.
pub fn w3d_game_window_manager_init(_wm: &mut GameWindowManager) {
    // upstream: GameWindowManager::init() — covered by GameWindowManager::new in the port
}
