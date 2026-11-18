//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTankDraw.cpp
//!
//! Sweep 2 (#213): `W3DTankDraw::tossEmitters`.

use super::w3d_truck_draw::ParticleSystemId;

/// The tread debris emitters of `W3DTankDraw` (and `W3DTankTruckDraw`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TankTreadEmitters {
    pub tread_debris_left: ParticleSystemId,
    pub tread_debris_right: ParticleSystemId,
}

impl TankTreadEmitters {
    /// `W3DTankDraw::tossEmitters` (W3DTankDraw.cpp:112): left then right tread debris detached (`attachToObject(NULL)`),
    /// destroyed and cleared. Returns the ids to detach-and-destroy.
    pub fn toss_emitters(&mut self) -> Vec<u32> {
        let mut out = Vec::new();
        if let Some(id) = self.tread_debris_left.take() {
            out.push(id);
        }
        if let Some(id) = self.tread_debris_right.take() {
            out.push(id);
        }
        out
    }
}
