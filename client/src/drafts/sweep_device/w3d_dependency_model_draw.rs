//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA'"'"'s GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DDependencyModelDraw.cpp
//!
//! Sweep 2 (#213): `W3DDependencyModelDraw::notifyDrawModuleDependencyCleared`.

/// `W3DDependencyModelDraw::m_dependencyCleared`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct W3DDependencyModelDraw {
    pub dependency_cleared: bool,
}

impl W3DDependencyModelDraw {
    /// `W3DDependencyModelDraw::notifyDrawModuleDependencyCleared` (W3DDependencyModelDraw.cpp:120).
    pub fn notify_draw_module_dependency_cleared(&mut self) {
        self.dependency_cleared = true;
    }
}
