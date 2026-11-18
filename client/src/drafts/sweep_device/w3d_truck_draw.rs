//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA'"'"'s GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTruckDraw.cpp
//!
//! Sweep 2 (#213): `W3DTruckDraw::tossEmitters`. A particle system is an id; `attachToObject(NULL)` and `destroy()` are the
//! particle system manager'"'"'s, so they are returned as the ids to detach and destroy, and the members cleared.

/// A `ParticleSystem *` member (None = NULL): the id the particle system manager knows it by.
pub type ParticleSystemId = Option<u32>;

/// The emitter members of `W3DTruckDraw` (also `W3DTankTruckDraw`, which has the same three).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TruckEmitters {
    pub dust_effect: ParticleSystemId,
    pub dirt_effect: ParticleSystemId,
    pub powerslide_effect: ParticleSystemId,
}

impl TruckEmitters {
    /// `W3DTruckDraw::tossEmitters` (W3DTruckDraw.cpp:133) and `W3DTankTruckDraw::tossEmitters` (W3DTankTruckDraw.cpp:187, the
    /// same body): each existing effect is detached from its object (`attachToObject(NULL)`), destroyed, and the member set to
    /// NULL. Returns the ids to detach-and-destroy, in upstream order (dust, dirt, powerslide).
    pub fn toss_emitters(&mut self) -> Vec<u32> {
        let mut out = Vec::new();
        if let Some(id) = self.dust_effect.take() {
            out.push(id);
        }
        if let Some(id) = self.dirt_effect.take() {
            out.push(id);
        }
        if let Some(id) = self.powerslide_effect.take() {
            out.push(id);
        }
        out
    }
}
