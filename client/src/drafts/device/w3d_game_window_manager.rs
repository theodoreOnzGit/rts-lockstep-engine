//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/W3DGameWindowManager.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DGameWindowManager.h
//!
//! `W3DGameWindowManager` only extends `GameWindowManager::init` and (in the header) routes window creation to the W3D
//! draw callbacks. The port's window manager is [`rts_lockstep_engine::client::window_manager`]; its draw callbacks are
//! the client's (`client/src/ui.rs`). Nothing of this file is left to translate beyond the wrapper below.

/// `class W3DGameWindowManager`: holds no data of its own.
#[derive(Clone, Copy, Debug, Default)]
pub struct W3DGameWindowManager;

impl W3DGameWindowManager {
    /// `W3DGameWindowManager::W3DGameWindowManager` (W3DGameWindowManager.cpp:44).
    pub fn new() -> Self {
        W3DGameWindowManager
    }

    /// `W3DGameWindowManager::init` (W3DGameWindowManager.cpp:58): "extend" — `GameWindowManager::init()` is the port's
    /// window manager's own; the caller runs it, then this.
    pub fn init(&mut self) {}
}
