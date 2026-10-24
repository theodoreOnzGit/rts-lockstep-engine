// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/quat.h (the Quaternion
//   data, its four-float constructor, and Build_Matrix3D)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWMath, author Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `class Quaternion`: only what the W3D hierarchy loader needs so far, the
//! data and `Build_Matrix3D`. Slerp and the rest of `quat.h`/`quat.cpp` are
//! not translated yet.

use super::matrix3d::Matrix3D;

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
        Quaternion {
            x: a,
            y: b,
            z: c,
            w: d,
        }
    }

    /// `Quaternion(true)`: the identity.
    pub const IDENTITY: Quaternion = Quaternion::new(0.0, 0.0, 0.0, 1.0);
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

    #[test]
    fn identity_quaternion_gives_identity() {
        let m = build_matrix3d(&Quaternion::IDENTITY);
        let mut id = Matrix3D::default();
        id.make_identity();
        assert_eq!(m, id);
    }
}
