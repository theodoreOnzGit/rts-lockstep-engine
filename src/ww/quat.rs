// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/quat.h (the Quaternion
//   data, its four-float constructor, and Build_Matrix3D)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/quat.cpp (Fast_Slerp)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWMath, author Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `class Quaternion`: only what the W3D hierarchy loader ~~needs so far, the
//! data and `Build_Matrix3D`. Slerp and the rest of `quat.h`/`quat.cpp` are
//! not translated yet.~~ **Corrected 2026-11-05 (#10, step 3):** and the
//! animation evaluation need: the data, `Build_Matrix3D` and `Fast_Slerp`.
//! `Slerp`, `Cached_Slerp` and the rest of `quat.h`/`quat.cpp` are not
//! translated yet.
//!
//! `Fast_Slerp` has two bodies in `quat.cpp`: x87 inline assembly under
//! `#if 0` (never compiled, not even in the shipped game) and the C body in
//! its `#else`, which is the one translated.

use super::matrix3d::Matrix3D;
use super::wwmath::{self, WWMATH_EPSILON};

/// `class Quaternion` (`X, Y, Z, W`; `q[0..4]` indexes them in that order).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quaternion {
    /// `Quaternion(float a, float b, float c, float d)` (no normalisation).
    pub const fn new(a: f32, b: f32, c: f32, d: f32) -> Self {
        Quaternion { x: a, y: b, z: c, w: d }
    }

    /// `Quaternion(true)`: the identity.
    pub const IDENTITY: Quaternion = Quaternion::new(0.0, 0.0, 0.0, 1.0);
}

/// `Fast_Slerp(res, p, q, alpha)` (quat.cpp, the compiled `#else` body):
/// spherical interpolation through `WWMath::Fast_Acos` and `Fast_Sin`
/// (tables), with a linear blend when `1 - |cos| < WWMATH_EPSILON^2`. All
/// `float`, in upstream's order.
pub fn fast_slerp(p: &Quaternion, q: &Quaternion, mut alpha: f32) -> Quaternion {
    let beta: f32;
    // cos theta = dot product of p and q
    let mut cos_t = p.x * q.x + p.y * q.y + p.z * q.z + p.w * q.w;
    // if q is on opposite hemisphere from A, use -B instead
    let qflip = if cos_t < 0.0f32 {
        cos_t = -cos_t;
        true
    } else {
        false
    };
    if 1.0f32 - cos_t < WWMATH_EPSILON * WWMATH_EPSILON {
        // if q is very close to p, just linearly interpolate
        beta = 1.0f32 - alpha;
    } else {
        let theta = wwmath::fast_acos(cos_t);
        let sin_t = wwmath::fast_sin(theta);
        let oo_sin_t = 1.0f32 / sin_t;
        beta = wwmath::fast_sin(theta - alpha * theta) * oo_sin_t;
        alpha = wwmath::fast_sin(alpha * theta) * oo_sin_t;
    }
    if qflip {
        alpha = -alpha;
    }
    Quaternion {
        x: beta * p.x + alpha * q.x,
        y: beta * p.y + alpha * q.y,
        z: beta * p.z + alpha * q.z,
        w: beta * p.w + alpha * q.w,
    }
}

/// `Build_Matrix3D(q, out)`: the rotation matrix of `q` (assumed unit), no
/// translation. Upstream mixes precisions: the products and sums of
/// components are `float`; `2.0 *` and `1.0 -` are `double` (except the
/// `2.0f` of `[1][1]`), rounded to `float` once at the end.
pub fn build_matrix3d(q: &Quaternion) -> Matrix3D {
    let (q0, q1, q2, q3) = (q.x, q.y, q.z, q.w);
    let mut out = Matrix3D::default();
    out[0][0] = (1.0 - 2.0 * f64::from(q1 * q1 + q2 * q2)) as f32;
    out[0][1] = (2.0 * f64::from(q0 * q1 - q2 * q3)) as f32;
    out[0][2] = (2.0 * f64::from(q2 * q0 + q1 * q3)) as f32;

    out[1][0] = (2.0 * f64::from(q0 * q1 + q2 * q3)) as f32;
    out[1][1] = (1.0 - f64::from(2.0f32 * (q2 * q2 + q0 * q0))) as f32;
    out[1][2] = (2.0 * f64::from(q1 * q2 - q0 * q3)) as f32;

    out[2][0] = (2.0 * f64::from(q2 * q0 - q1 * q3)) as f32;
    out[2][1] = (2.0 * f64::from(q1 * q2 + q0 * q3)) as f32;
    out[2][2] = (1.0 - 2.0 * f64::from(q1 * q1 + q0 * q0)) as f32;

    out[0][3] = 0.0;
    out[1][3] = 0.0;
    out[2][3] = 0.0;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Predictions: equal unit quaternions take the linear branch (`cos = 1`),
    /// so `alpha = 0.25` gives `0.75 p + 0.25 p`, exactly `p` for the
    /// identity; a 180-degree-apart pair (`cos = -1`) flips `q` and also
    /// blends linearly: `0.75 * 1 + (-0.25) * (-1) = 1`.
    #[test]
    fn fast_slerp_linear_cases() {
        let id = Quaternion::IDENTITY;
        assert_eq!(fast_slerp(&id, &id, 0.25), id);
        let neg = Quaternion::new(0.0, 0.0, 0.0, -1.0);
        assert_eq!(fast_slerp(&id, &neg, 0.25), id);
    }

    /// Prediction: halfway between the identity and a 90-degree turn about
    /// Z (`(0, 0, sin 45, cos 45)`) is close to a 45-degree turn,
    /// `(0, 0, 0.38268, 0.92388)`, within the tables' accuracy (about 1e-4).
    #[test]
    fn fast_slerp_halfway() {
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let r = fast_slerp(&Quaternion::IDENTITY, &Quaternion::new(0.0, 0.0, s, s), 0.5);
        assert!((r.z - 0.382_683_43).abs() < 2e-4, "{r:?}");
        assert!((r.w - 0.923_879_5).abs() < 2e-4, "{r:?}");
        assert_eq!((r.x, r.y), (0.0, 0.0));
    }

    #[test]
    fn identity_quaternion_gives_identity() {
        let m = build_matrix3d(&Quaternion::IDENTITY);
        let mut id = Matrix3D::default();
        id.make_identity();
        assert_eq!(m, id);
    }
}
