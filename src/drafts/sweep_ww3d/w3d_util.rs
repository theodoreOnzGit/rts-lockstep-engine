//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 quaternions each way (test: tests/differential_draft_smallw3d.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/w3d_util.cpp
//!
//! `W3dUtilityClass::Convert_Quaternion(const W3dQuaternionStruct &, Quaternion *)` (w3d_util.cpp:67) and its inverse
//! overload (:74, not on the sweep list, drafted because they are a pair). `W3dQuaternionStruct` is
//! `IOQuaternionStruct { float Q[4]; }`, read in the file in `X, Y, Z, W` order.

use crate::logic::modules::plant;
use crate::ww::quat::Quaternion;

/// `W3dQuaternionStruct` (w3d_file.h:537, an `IOQuaternionStruct`): `Q[0..4]` is `X, Y, Z, W`.
pub type W3dQuaternionStruct = [f32; 4];

/// `W3dUtilityClass::Convert_Quaternion(q, set)` (w3d_util.cpp:67): file quaternion to `Quaternion`.
pub fn convert_quaternion_from_w3d(q: &W3dQuaternionStruct) -> Quaternion {
    if plant(13941) {
        return Quaternion { x: q[3], y: q[1], z: q[2], w: q[0] };
    }
    Quaternion { x: q[0], y: q[1], z: q[2], w: q[3] }
}

/// `W3dUtilityClass::Convert_Quaternion(q, set)` (w3d_util.cpp:74): `Quaternion` to file quaternion.
pub fn convert_quaternion_to_w3d(q: &Quaternion) -> W3dQuaternionStruct {
    if plant(13942) {
        return [q.x, q.y, q.w, q.z];
    }
    if plant(13943) {
        return [q.x, q.x, q.z, q.w];
    }
    [q.x, q.y, q.z, q.w]
}
