// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/quat.h (the Quaternion
//   data, its four-float constructor, operator[], Rotate_Vector,
//   SlerpInfoStruct and Build_Matrix3D)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/quat.cpp (Fast_Slerp,
//   Build_Quaternion(const Matrix3D &), Slerp_Setup, Cached_Slerp)
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
//! ~~`Slerp`, `Cached_Slerp` and the rest of `quat.h`/`quat.cpp` are not
//! translated yet.~~ **Corrected 2026-11-07 (#49):** and what the particle
//! emitter needs: `Build_Quaternion(const Matrix3D &)`, `Slerp_Setup`,
//! `Cached_Slerp`, `Rotate_Vector` and `operator[]`. `Slerp` and the rest
//! are not translated yet.
//!
//! `Fast_Slerp` has two bodies in `quat.cpp`: x87 inline assembly under
//! `#if 0` (never compiled, not even in the shipped game) and the C body in
//! its `#else`, which is the one translated.

use super::matrix3d::Matrix3D;
use super::vector3::Vector3;
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

/// `Quaternion::operator[]`: `X, Y, Z, W` by index.
impl std::ops::Index<usize> for Quaternion {
    type Output = f32;
    fn index(&self, i: usize) -> &f32 {
        match i {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            3 => &self.w,
            _ => panic!("quaternion index {i}"),
        }
    }
}

impl std::ops::IndexMut<usize> for Quaternion {
    fn index_mut(&mut self, i: usize) -> &mut f32 {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            3 => &mut self.w,
            _ => panic!("quaternion index {i}"),
        }
    }
}

impl Quaternion {
    /// `Rotate_Vector(v)` (quat.h): `q v q*`, in upstream's order of
    /// operations.
    pub fn rotate_vector(&self, v: &Vector3) -> Vector3 {
        let (qx, qy, qz, qw) = (self.x, self.y, self.z, self.w);
        let x = qw * v.x + (qy * v.z - v.y * qz);
        let y = qw * v.y - (qx * v.z - v.x * qz);
        let z = qw * v.z + (qx * v.y - v.x * qy);
        let w = -(qx * v.x + qy * v.y + qz * v.z);
        Vector3::new(
            w * (-qx) + qw * x + (y * (-qz) - (-qy) * z),
            w * (-qy) + qw * y - (x * (-qz) - (-qx) * z),
            w * (-qz) + qw * z + (x * (-qy) - (-qx) * y),
        )
    }
}

/// `_nxt` (quat.cpp).
const NXT: [usize; 3] = [1, 2, 0];

/// `Build_Quaternion(const Matrix3D &)` (quat.cpp): the trace branch and
/// the largest-diagonal branch. `sqrt(x + 1.0)` and `0.5 / s` are `double`
/// (rounded to the `float` `s`), as are `s * 0.5`.
pub fn build_quaternion(mat: &Matrix3D) -> Quaternion {
    let mut q = Quaternion::default();
    // sum the diagonal of the rotation matrix
    let tr = mat[0][0] + mat[1][1] + mat[2][2];
    if tr > 0.0f32 {
        let mut s = petir::Real::sqrt(f64::from(tr) + 1.0) as f32;
        q[3] = (f64::from(s) * 0.5) as f32;
        s = (0.5 / f64::from(s)) as f32;
        q[0] = (mat[2][1] - mat[1][2]) * s;
        q[1] = (mat[0][2] - mat[2][0]) * s;
        q[2] = (mat[1][0] - mat[0][1]) * s;
    } else {
        let mut i = 0usize;
        if mat[1][1] > mat[0][0] {
            i = 1;
        }
        if mat[2][2] > mat[i][i] {
            i = 2;
        }
        let j = NXT[i];
        let k = NXT[j];
        let mut s = petir::Real::sqrt(f64::from(mat[i][i] - (mat[j][j] + mat[k][k])) + 1.0) as f32;
        q[i] = (f64::from(s) * 0.5) as f32;
        if f64::from(s) != 0.0 {
            s = (0.5 / f64::from(s)) as f32;
        }
        q[3] = (mat[k][j] - mat[j][k]) * s;
        q[j] = (mat[j][i] + mat[i][j]) * s;
        q[k] = (mat[k][i] + mat[i][k]) * s;
    }
    q
}

/// `struct SlerpInfoStruct` (quat.h).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SlerpInfo {
    pub sin_t: f32,
    pub theta: f32,
    pub flip: bool,
    pub linear: bool,
}

/// `SLERP_EPSILON` (quat.cpp: `0.001`, a `double`).
const SLERP_EPSILON: f64 = 0.001;

/// `Slerp_Setup(p, q, &info)` (quat.cpp): `WWMath::Acos` and `WWMath::Sin`
/// (the portable branches, petir).
pub fn slerp_setup(p: &Quaternion, q: &Quaternion) -> SlerpInfo {
    let mut info = SlerpInfo::default();
    // cos theta = dot product of p and q
    let mut cos_t = p.x * q.x + p.y * q.y + p.z * q.z + p.w * q.w;
    // if q is on opposite hemisphere from A, use -B instead
    if cos_t < 0.0f32 {
        cos_t = -cos_t;
        info.flip = true;
    } else {
        info.flip = false;
    }
    if f64::from(1.0f32 - cos_t) < SLERP_EPSILON {
        info.linear = true;
        info.theta = 0.0;
        info.sin_t = 0.0;
    } else {
        info.linear = false;
        info.theta = wwmath::acos(cos_t);
        info.sin_t = wwmath::sin(info.theta);
    }
    info
}

/// `Cached_Slerp(p, q, alpha, &info)` (quat.cpp, the returning overload).
///
/// Upstream quirk kept: the normal branch divides by `Theta`, not by
/// `SinT` (`oo_sin_t = 1.0f / slerpinfo->Theta`), so the result is not a
/// unit quaternion (shorter by `sin(theta) / theta`).
pub fn cached_slerp(p: &Quaternion, q: &Quaternion, mut alpha: f32, info: &SlerpInfo) -> Quaternion {
    let beta;
    if info.linear {
        // if q is very close to p, just linearly interpolate between the two.
        beta = 1.0f32 - alpha;
    } else {
        // normal slerp!
        let oo_sin_t = 1.0f32 / info.theta;
        beta = wwmath::sin(info.theta - alpha * info.theta) * oo_sin_t;
        alpha = wwmath::sin(alpha * info.theta) * oo_sin_t;
    }
    if info.flip {
        alpha = -alpha;
    }
    Quaternion {
        x: beta * p.x + alpha * q.x,
        y: beta * p.y + alpha * q.y,
        z: beta * p.z + alpha * q.z,
        w: beta * p.w + alpha * q.w,
    }
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
