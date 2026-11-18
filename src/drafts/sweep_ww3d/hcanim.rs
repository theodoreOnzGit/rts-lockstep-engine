//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hcanim.cpp
//!
//! `HCompressedAnimClass::Has_X_Translation`, `Has_Y_Translation`, `Has_Z_Translation`, `Has_Rotation`,
//! `Has_Visibility` (hcanim.cpp:679-703). Upstream reads `NodeMotion[pividx].vd.X` (the union of the time-coded and
//! adaptive-delta channel pointers); the verified [`HCompressedAnim`] holds that as `Option<CompressedChannel>`, absent
//! nodes having none. `WWASSERT((pividx >= 0) && (pividx < NumNodes))` is a debug assertion and not kept.

use crate::formats::w3d::hanim::{HCompressedAnim, NodeCompressedMotion};

fn motion_at(anim: &HCompressedAnim, pividx: i32) -> Option<&NodeCompressedMotion> {
    if pividx < 0 {
        return None;
    }
    anim.node_motion.get(&(pividx as u32))
}

/// `HCompressedAnimClass::Has_X_Translation(pividx)` (hcanim.cpp:679): `NodeMotion[pividx].vd.X != NULL`.
pub fn has_x_translation(anim: &HCompressedAnim, pividx: i32) -> bool {
    motion_at(anim, pividx).map_or(false, |m| m.x.is_some())
}

/// `HCompressedAnimClass::Has_Y_Translation(pividx)` (hcanim.cpp:685).
pub fn has_y_translation(anim: &HCompressedAnim, pividx: i32) -> bool {
    motion_at(anim, pividx).map_or(false, |m| m.y.is_some())
}

/// `HCompressedAnimClass::Has_Z_Translation(pividx)` (hcanim.cpp:691).
pub fn has_z_translation(anim: &HCompressedAnim, pividx: i32) -> bool {
    motion_at(anim, pividx).map_or(false, |m| m.z.is_some())
}

/// `HCompressedAnimClass::Has_Rotation(pividx)` (hcanim.cpp:697): `NodeMotion[pividx].vd.Q != NULL`.
pub fn has_rotation(anim: &HCompressedAnim, pividx: i32) -> bool {
    motion_at(anim, pividx).map_or(false, |m| m.q.is_some())
}

/// `HCompressedAnimClass::Has_Visibility(pividx)` (hcanim.cpp:703): `NodeMotion[pividx].Vis != NULL`.
pub fn has_visibility(anim: &HCompressedAnim, pividx: i32) -> bool {
    motion_at(anim, pividx).map_or(false, |m| m.vis.is_some())
}
