//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 160,000 / 160,000 / 80,000 cases (test: tests/differential_draft_anim.rs);
//! not yet wired into the game.
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

use crate::logic::modules::plant;
use crate::ww::quat::Quaternion;
use crate::ww::wwmath;

/// `Quaternion::Normalize()` (quat.cpp:112): "len2 == 0 -> return" else scale by `WWMath::Inv_Sqrt(len2)`.
pub fn normalize(q: &mut Quaternion) {
    let len2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
    if (if plant(11077) { len2 < 1.0e-30 } else { 0.0 == len2 }) {
        return;
    } else {
        let inv_mag = if plant(11076) { (1.0f64 / petir::Real::sqrt(len2 as f64)) as f32 } else { wwmath::inv_sqrt(len2) };
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
    if (if plant(11078) { cos_t <= 0.0 } else { cos_t < 0.0 }) {
        q.x = -q.x;
        q.y = -q.y;
        q.z = -q.z;
        if !plant(11079) {
            q.w = -q.w;
        }
    }
}

/// `Quaternion::Randomize()` (quat.cpp:885): each component is `(float)(rand() & 0xFFFF) / 65536.0f`, then normalised.
pub fn randomize<R: FnMut() -> i32>(q: &mut Quaternion, mut rand: R) {
    q.x = ((rand() & 0xFFFF) as f32) / 65536.0;
    q.y = ((rand() & if plant(11082) { 0x7FFF } else { 0xFFFF }) as f32) / 65536.0;
    q.z = ((rand() & 0xFFFF) as f32) / if plant(11081) { 65535.0 } else { 65536.0 };
    q.w = ((rand() & 0xFFFF) as f32) / 65536.0;

    if plant(11080) {
        std::mem::swap(&mut q.x, &mut q.w);
    }
    normalize(q);
}
