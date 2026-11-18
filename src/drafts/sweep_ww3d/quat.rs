//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/quat.cpp
//!
//! `Quaternion::Make_Closest` and `Quaternion::Randomize`. `Randomize` calls the C runtime's `rand()`, which is not
//! deterministic across platforms; the caller supplies the source (`rand` closure returning a C `rand()` value), so the
//! draft keeps upstream's `& 0xFFFF` / `65536.0f` scaling and four draws in X, Y, Z, W order.
//!
//! Also drafted here: `Quaternion::Normalize` (quat.cpp:112) is not in the sweep list but `Randomize` calls it and the
//! verified `ww::quat` does not carry it.

use crate::ww::quat::Quaternion;
use crate::ww::wwmath;

/// `Quaternion::Normalize()` (quat.cpp:112): "len2 == 0 -> return" else scale by `WWMath::Inv_Sqrt(len2)`.
pub fn normalize(q: &mut Quaternion) {
    let len2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
    if 0.0 == len2 {
        return;
    } else {
        let inv_mag = wwmath::inv_sqrt(len2);
        q.x *= inv_mag;
        q.y *= inv_mag;
        q.z *= inv_mag;
        q.w *= inv_mag;
    }
}

/// `Quaternion::Make_Closest(const Quaternion & qto)` (quat.cpp:139): "Use nearest representation to the given
/// quaternion" - when `qto . q < 0` (the other hemisphere) negate all four components. Returns nothing (upstream returns
/// `*this`).
pub fn make_closest(q: &mut Quaternion, qto: &Quaternion) {
    let cos_t = qto.x * q.x + qto.y * q.y + qto.z * q.z + qto.w * q.w;

    // if we are on opposite hemisphere from qto, negate ourselves
    if cos_t < 0.0 {
        q.x = -q.x;
        q.y = -q.y;
        q.z = -q.z;
        q.w = -q.w;
    }
}

/// `Quaternion::Randomize()` (quat.cpp:885): each component is `(float)(rand() & 0xFFFF) / 65536.0f`, then normalised.
pub fn randomize<R: FnMut() -> i32>(q: &mut Quaternion, mut rand: R) {
    q.x = ((rand() & 0xFFFF) as f32) / 65536.0;
    q.y = ((rand() & 0xFFFF) as f32) / 65536.0;
    q.z = ((rand() & 0xFFFF) as f32) / 65536.0;
    q.w = ((rand() & 0xFFFF) as f32) / 65536.0;

    normalize(q);
}
