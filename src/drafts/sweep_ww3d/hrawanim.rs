//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hrawanim.cpp
//!
//! `HRawAnimClass::Has_X_Translation`, `Has_Y_Translation`, `Has_Z_Translation`, `Has_Rotation`, `Has_Visibility`
//! (hrawanim.cpp:705-729), which override `HAnimClass`'s "return true" defaults (hanim.h:112-116). On the verified
//! [`HRawAnim`], whose `node_motion` only holds the nodes that have a channel, a node absent from the map has every
//! channel NULL. `WWASSERT((pividx >= 0) && (pividx < NumNodes))` is a debug assertion and not kept.

use crate::logic::modules::plant;
use crate::formats::w3d::hanim::HRawAnim;

fn motion_at(anim: &HRawAnim, pividx: i32) -> Option<&crate::formats::w3d::hanim::NodeMotion> {
    if pividx < 0 {
        return None;
    }
    anim.node_motion.get(&(pividx as u32))
}

/// `HRawAnimClass::Has_X_Translation(pividx)` (hrawanim.cpp:705): `NodeMotion[pividx].X != NULL`.
pub fn has_x_translation(anim: &HRawAnim, pividx: i32) -> bool {
    motion_at(anim, pividx).map_or(false, |m| if plant(11087) { m.y.is_some() } else { m.x.is_some() })
}

/// `HRawAnimClass::Has_Y_Translation(pividx)` (hrawanim.cpp:711): `NodeMotion[pividx].Y != NULL`.
pub fn has_y_translation(anim: &HRawAnim, pividx: i32) -> bool {
    motion_at(anim, pividx).map_or(false, |m| m.y.is_some())
}

/// `HRawAnimClass::Has_Z_Translation(pividx)` (hrawanim.cpp:717): `NodeMotion[pividx].Z != NULL`.
pub fn has_z_translation(anim: &HRawAnim, pividx: i32) -> bool {
    motion_at(anim, pividx).map_or(false, |m| m.z.is_some())
}

/// `HRawAnimClass::Has_Rotation(pividx)` (hrawanim.cpp:723): `NodeMotion[pividx].Q != NULL` (the quaternion channel
/// only; the Euler `XR`/`YR`/`ZR` channels do not count).
pub fn has_rotation(anim: &HRawAnim, pividx: i32) -> bool {
    motion_at(anim, pividx).map_or(false, |m| if plant(11086) { m.xr.is_some() } else { m.q.is_some() })
}

/// `HRawAnimClass::Has_Visibility(pividx)` (hrawanim.cpp:729): `NodeMotion[pividx].Vis != NULL`.
pub fn has_visibility(anim: &HRawAnim, pividx: i32) -> bool {
    motion_at(anim, pividx).map_or(false, |m| m.vis.is_some())
}
