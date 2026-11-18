//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/w3d_util.cpp
//!
//! `W3dUtilityClass::Convert_Quaternion(const W3dQuaternionStruct &, Quaternion *)` (w3d_util.cpp:67) and its inverse
//! overload (:74, not on the sweep list, drafted because they are a pair). `W3dQuaternionStruct` is
//! `IOQuaternionStruct { float Q[4]; }`, read in the file in `X, Y, Z, W` order.

use crate::ww::quat::Quaternion;

/// `W3dQuaternionStruct` (w3d_file.h:537, an `IOQuaternionStruct`): `Q[0..4]` is `X, Y, Z, W`.
pub type W3dQuaternionStruct = [f32; 4];

/// `W3dUtilityClass::Convert_Quaternion(q, set)` (w3d_util.cpp:67): file quaternion to `Quaternion`.
pub fn convert_quaternion_from_w3d(q: &W3dQuaternionStruct) -> Quaternion {
    Quaternion { x: q[0], y: q[1], z: q[2], w: q[3] }
}

/// `W3dUtilityClass::Convert_Quaternion(q, set)` (w3d_util.cpp:74): `Quaternion` to file quaternion.
pub fn convert_quaternion_to_w3d(q: &Quaternion) -> W3dQuaternionStruct {
    [q.x, q.y, q.z, q.w]
}
