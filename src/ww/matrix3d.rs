// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/matrix3d.h
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/matrix3d.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWMath, author Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `class Matrix3D`: a 3x4 affine transform (column vectors, right-handed;
//! the implied fourth row is `0 0 0 1`). Every product keeps upstream's
//! operation order. `ALLOW_TEMPORARIES` is never defined upstream, so the
//! `#else` branches (`mul`, `mulVector3`, ...) are the ones translated.
//!
//! **Not translated in this batch** (each needs code that is not here yet):
//! - `Get_Inverse`: calls `D3DXMatrixInverse` from the DirectX SDK, which is
//!   not in the upstream release, so it has no compilable reference. Use
//!   [`Matrix3D::get_orthogonal_inverse`] (which is what upstream's own TODO
//!   says most callers want).
//! - `Set(Matrix3x3, pos)`, `Set(Quaternion, pos)`, `Set_Rotation(...)`, the
//!   matching constructors, and `Lerp` (needs `Quaternion`/`Slerp`).
//!
//! Rust takes the matrix and vectors by value or by `&`/`&mut` with borrow
//! rules, so upstream's aliasing checks (`if (set_res == &A) tmp = A;`) are
//! satisfied by construction and give the same results.

use super::vector3::Vector3;
use super::vector4::Vector4;
use super::wwmath::{self, WWMATH_EPSILON};
use std::ops::{Index, IndexMut};
use petir::Real;

/// `class Matrix3D` (`Vector4 Row[3]`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Matrix3D {
    pub row: [Vector4; 3],
}

/// Upstream `submul(row, tmp1, tmp2, tmp3)`: `row.X*tmp1 + row.Y*tmp2 + row.Z*tmp3`.
#[inline(always)]
fn submul(row: &Vector4, tmp1: f32, tmp2: f32, tmp3: f32) -> f32 {
    row.x * tmp1 + row.y * tmp2 + row.z * tmp3
}

impl Matrix3D {
    /// `Matrix3D(float m11, ..., float m34)` (row-major).
    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub const fn new(
        m11: f32, m12: f32, m13: f32, m14: f32,
        m21: f32, m22: f32, m23: f32, m24: f32,
        m31: f32, m32: f32, m33: f32, m34: f32,
    ) -> Self {
        Matrix3D {
            row: [
                Vector4::new(m11, m12, m13, m14),
                Vector4::new(m21, m22, m23, m24),
                Vector4::new(m31, m32, m33, m34),
            ],
        }
    }

    /// `Matrix3D(float m[12])` / `Set(float m[12])`.
    #[inline]
    pub const fn from_array(m: [f32; 12]) -> Self {
        Matrix3D::new(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11])
    }

    /// The 12 entries, row-major.
    #[inline]
    pub fn to_array(&self) -> [f32; 12] {
        let r = &self.row;
        [
            r[0].x, r[0].y, r[0].z, r[0].w, r[1].x, r[1].y, r[1].z, r[1].w, r[2].x, r[2].y, r[2].z, r[2].w,
        ]
    }

    /// `Matrix3D(bool init)` with `init == true`, i.e. `Make_Identity()`.
    #[inline]
    pub const fn identity() -> Self {
        Matrix3D::IDENTITY
    }

    /// `Matrix3D(x, y, z, pos)` / `Set(x, y, z, pos)`: axes as columns.
    #[inline]
    pub fn from_axes(x: &Vector3, y: &Vector3, z: &Vector3, pos: &Vector3) -> Self {
        let mut m = Matrix3D::default();
        m.set_axes(x, y, z, pos);
        m
    }

    /// `Set(x, y, z, pos)`.
    #[inline]
    pub fn set_axes(&mut self, x: &Vector3, y: &Vector3, z: &Vector3, pos: &Vector3) {
        self.row[0].set(x[0], y[0], z[0], pos[0]);
        self.row[1].set(x[1], y[1], z[1], pos[1]);
        self.row[2].set(x[2], y[2], z[2], pos[2]);
    }

    /// `Matrix3D(axis, angle)` / `Set(axis, angle)`.
    #[inline]
    pub fn from_axis_angle(axis: &Vector3, angle: f32) -> Self {
        let mut m = Matrix3D::default();
        m.set_axis_angle(axis, angle);
        m
    }

    /// `Set(const Vector3 & axis, float angle)`: `c = cosf`, `s = sinf`.
    #[inline]
    pub fn set_axis_angle(&mut self, axis: &Vector3, angle: f32) {
        let c = Real::cos(angle);
        let s = Real::sin(angle);
        self.set_axis_sin_cos(axis, s, c);
    }

    /// `Set(const Vector3 & axis, float s, float c)` (upstream asserts a unit
    /// axis in debug builds only).
    #[inline]
    pub fn set_axis_sin_cos(&mut self, axis: &Vector3, s: f32, c: f32) {
        self.row[0].set(
            axis[0] * axis[0] + c * (1.0f32 - axis[0] * axis[0]),
            axis[0] * axis[1] * (1.0f32 - c) - axis[2] * s,
            axis[2] * axis[0] * (1.0f32 - c) + axis[1] * s,
            0.0f32,
        );
        self.row[1].set(
            axis[0] * axis[1] * (1.0f32 - c) + axis[2] * s,
            axis[1] * axis[1] + c * (1.0f32 - axis[1] * axis[1]),
            axis[1] * axis[2] * (1.0f32 - c) - axis[0] * s,
            0.0f32,
        );
        self.row[2].set(
            axis[2] * axis[0] * (1.0f32 - c) - axis[1] * s,
            axis[1] * axis[2] * (1.0f32 - c) + axis[0] * s,
            axis[2] * axis[2] + c * (1.0f32 - axis[2] * axis[2]),
            0.0f32,
        );
    }

    /// `Matrix3D(const Vector3 & position)` / `Set(position)`: identity
    /// rotation at `position`.
    #[inline]
    pub fn from_translation(position: &Vector3) -> Self {
        let mut m = Matrix3D::default();
        m.set_position(position);
        m
    }

    /// `Set(const Vector3 & position)`.
    #[inline]
    pub fn set_position(&mut self, position: &Vector3) {
        self.row[0].set(1.0, 0.0, 0.0, position[0]);
        self.row[1].set(0.0, 1.0, 0.0, position[1]);
        self.row[2].set(0.0, 0.0, 1.0, position[2]);
    }

    /// `Get_Translation()`.
    #[inline]
    pub fn get_translation(&self) -> Vector3 {
        Vector3::new(self.row[0][3], self.row[1][3], self.row[2][3])
    }

    /// `Set_Translation(t)`.
    #[inline]
    pub fn set_translation(&mut self, t: &Vector3) {
        self.row[0][3] = t[0];
        self.row[1][3] = t[1];
        self.row[2][3] = t[2];
    }

    #[inline]
    pub fn get_x_translation(&self) -> f32 {
        self.row[0][3]
    }
    #[inline]
    pub fn get_y_translation(&self) -> f32 {
        self.row[1][3]
    }
    #[inline]
    pub fn get_z_translation(&self) -> f32 {
        self.row[2][3]
    }
    #[inline]
    pub fn set_x_translation(&mut self, x: f32) {
        self.row[0][3] = x;
    }
    #[inline]
    pub fn set_y_translation(&mut self, y: f32) {
        self.row[1][3] = y;
    }
    #[inline]
    pub fn set_z_translation(&mut self, z: f32) {
        self.row[2][3] = z;
    }

    /// `Adjust_Translation(t)`.
    #[inline]
    pub fn adjust_translation(&mut self, t: &Vector3) {
        self.row[0][3] += t[0];
        self.row[1][3] += t[1];
        self.row[2][3] += t[2];
    }
    #[inline]
    pub fn adjust_x_translation(&mut self, x: f32) {
        self.row[0][3] += x;
    }
    #[inline]
    pub fn adjust_y_translation(&mut self, y: f32) {
        self.row[1][3] += y;
    }
    #[inline]
    pub fn adjust_z_translation(&mut self, z: f32) {
        self.row[2][3] += z;
    }

    /// `Get_X_Rotation()`: `WWMath::Atan2(Row[2][1], Row[1][1])`.
    #[inline]
    pub fn get_x_rotation(&self) -> f32 {
        wwmath::atan2(self.row[2][1], self.row[1][1])
    }

    /// `Get_Y_Rotation()`: `WWMath::Atan2(Row[0][2], Row[2][2])`.
    #[inline]
    pub fn get_y_rotation(&self) -> f32 {
        wwmath::atan2(self.row[0][2], self.row[2][2])
    }

    /// `Get_Z_Rotation()`: `WWMath::Atan2(Row[1][0], Row[0][0])`.
    #[inline]
    pub fn get_z_rotation(&self) -> f32 {
        wwmath::atan2(self.row[1][0], self.row[0][0])
    }

    /// `Make_Identity()`.
    #[inline]
    pub fn make_identity(&mut self) {
        self.row[0].set(1.0, 0.0, 0.0, 0.0);
        self.row[1].set(0.0, 1.0, 0.0, 0.0);
        self.row[2].set(0.0, 0.0, 1.0, 0.0);
    }

    /// `Translate(float x, float y, float z)`: post-multiply by a translation.
    #[inline]
    pub fn translate(&mut self, x: f32, y: f32, z: f32) {
        for i in 0..3 {
            let r = self.row[i];
            self.row[i][3] += r[0] * x + r[1] * y + r[2] * z;
        }
    }

    /// `Translate(const Vector3 &t)`.
    #[inline]
    pub fn translate_v(&mut self, t: &Vector3) {
        self.translate(t[0], t[1], t[2]);
    }

    /// `Translate_X(float x)`.
    #[inline]
    pub fn translate_x(&mut self, x: f32) {
        for i in 0..3 {
            let r = self.row[i];
            self.row[i][3] += r[0] * x;
        }
    }

    /// `Translate_Y(float y)`.
    #[inline]
    pub fn translate_y(&mut self, y: f32) {
        for i in 0..3 {
            let r = self.row[i];
            self.row[i][3] += r[1] * y;
        }
    }

    /// `Translate_Z(float z)`.
    #[inline]
    pub fn translate_z(&mut self, z: f32) {
        for i in 0..3 {
            let r = self.row[i];
            self.row[i][3] += r[2] * z;
        }
    }

    /// Shared body of the post-multiplying rotations: for each row,
    /// `(a, b) := (c*a + s*b, -s*a + c*b)` on columns `(ca, cb)`.
    #[inline(always)]
    fn post_rotate_plus(&mut self, ca: usize, cb: usize, s: f32, c: f32) {
        for i in 0..3 {
            let tmp1 = self.row[i][ca];
            let tmp2 = self.row[i][cb];
            self.row[i][ca] = c * tmp1 + s * tmp2;
            self.row[i][cb] = -s * tmp1 + c * tmp2;
        }
    }

    /// `Rotate_X(float theta)`: post-multiply by a rotation about X.
    #[inline]
    pub fn rotate_x(&mut self, theta: f32) {
        let s = Real::sin(theta);
        let c = Real::cos(theta);
        self.post_rotate_plus(1, 2, s, c);
    }

    /// `Rotate_X(float s, float c)`.
    #[inline]
    pub fn rotate_x_sc(&mut self, s: f32, c: f32) {
        self.post_rotate_plus(1, 2, s, c);
    }

    /// `Rotate_Y(float theta)`. Upstream's Y rotation has the opposite sign
    /// pattern: `(a, b) := (c*a - s*b, s*a + c*b)` on columns 0 and 2.
    #[inline]
    pub fn rotate_y(&mut self, theta: f32) {
        let s = Real::sin(theta);
        let c = Real::cos(theta);
        self.rotate_y_sc(s, c);
    }

    /// `Rotate_Y(float s, float c)`.
    #[inline]
    pub fn rotate_y_sc(&mut self, s: f32, c: f32) {
        for i in 0..3 {
            let tmp1 = self.row[i][0];
            let tmp2 = self.row[i][2];
            self.row[i][0] = c * tmp1 - s * tmp2;
            self.row[i][2] = s * tmp1 + c * tmp2;
        }
    }

    /// `Rotate_Z(float theta)` (upstream computes `c` before `s` here; both
    /// are independent calls, so the order does not affect any bit).
    #[inline]
    pub fn rotate_z(&mut self, theta: f32) {
        let c = Real::cos(theta);
        let s = Real::sin(theta);
        self.post_rotate_plus(0, 1, s, c);
    }

    /// `Rotate_Z(float s, float c)`.
    #[inline]
    pub fn rotate_z_sc(&mut self, s: f32, c: f32) {
        self.post_rotate_plus(0, 1, s, c);
    }

    /// `Scale(float scale)`: scale all three axes (the 3x3 part).
    #[inline]
    pub fn scale(&mut self, scale: f32) {
        self.scale_xyz(scale, scale, scale);
    }

    /// `Scale(float x, float y, float z)`: column 0 by x, 1 by y, 2 by z.
    #[inline]
    pub fn scale_xyz(&mut self, x: f32, y: f32, z: f32) {
        for i in 0..3 {
            self.row[i][0] *= x;
        }
        for i in 0..3 {
            self.row[i][1] *= y;
        }
        for i in 0..3 {
            self.row[i][2] *= z;
        }
    }

    /// `Scale(Vector3 &scale)`.
    #[inline]
    pub fn scale_v(&mut self, scale: &Vector3) {
        self.scale_xyz(scale.x, scale.y, scale.z);
    }

    /// Shared body of the pre-multiplying rotations: on rows `(ra, rb)` and
    /// columns `0..ncols`, `(a, b) := (c*a - s*b, s*a + c*b)`.
    #[inline(always)]
    fn pre_rotate_minus(&mut self, ra: usize, rb: usize, s: f32, c: f32, ncols: usize) {
        for j in 0..ncols {
            let tmp1 = self.row[ra][j];
            let tmp2 = self.row[rb][j];
            self.row[ra][j] = c * tmp1 - s * tmp2;
            self.row[rb][j] = s * tmp1 + c * tmp2;
        }
    }

    /// The Y variant: `(a, b) := (c*a + s*b, -s*a + c*b)` on rows 0 and 2.
    #[inline(always)]
    fn pre_rotate_y_body(&mut self, s: f32, c: f32, ncols: usize) {
        for j in 0..ncols {
            let tmp1 = self.row[0][j];
            let tmp2 = self.row[2][j];
            self.row[0][j] = c * tmp1 + s * tmp2;
            self.row[2][j] = -s * tmp1 + c * tmp2;
        }
    }

    /// `Pre_Rotate_X(float theta)`: pre-multiply (all four columns).
    #[inline]
    pub fn pre_rotate_x(&mut self, theta: f32) {
        let c = Real::cos(theta);
        let s = Real::sin(theta);
        self.pre_rotate_minus(1, 2, s, c, 4);
    }
    /// `Pre_Rotate_Y(float theta)`.
    #[inline]
    pub fn pre_rotate_y(&mut self, theta: f32) {
        let c = Real::cos(theta);
        let s = Real::sin(theta);
        self.pre_rotate_y_body(s, c, 4);
    }
    /// `Pre_Rotate_Z(float theta)`.
    #[inline]
    pub fn pre_rotate_z(&mut self, theta: f32) {
        let c = Real::cos(theta);
        let s = Real::sin(theta);
        self.pre_rotate_minus(0, 1, s, c, 4);
    }
    /// `Pre_Rotate_X(float s, float c)`.
    #[inline]
    pub fn pre_rotate_x_sc(&mut self, s: f32, c: f32) {
        self.pre_rotate_minus(1, 2, s, c, 4);
    }
    /// `Pre_Rotate_Y(float s, float c)`.
    #[inline]
    pub fn pre_rotate_y_sc(&mut self, s: f32, c: f32) {
        self.pre_rotate_y_body(s, c, 4);
    }
    /// `Pre_Rotate_Z(float s, float c)`.
    #[inline]
    pub fn pre_rotate_z_sc(&mut self, s: f32, c: f32) {
        self.pre_rotate_minus(0, 1, s, c, 4);
    }
    /// `In_Place_Pre_Rotate_X(float theta)`: as `Pre_Rotate_X`, but the
    /// translation column is left alone.
    #[inline]
    pub fn in_place_pre_rotate_x(&mut self, theta: f32) {
        let c = Real::cos(theta);
        let s = Real::sin(theta);
        self.pre_rotate_minus(1, 2, s, c, 3);
    }
    /// `In_Place_Pre_Rotate_Y(float theta)`.
    #[inline]
    pub fn in_place_pre_rotate_y(&mut self, theta: f32) {
        let c = Real::cos(theta);
        let s = Real::sin(theta);
        self.pre_rotate_y_body(s, c, 3);
    }
    /// `In_Place_Pre_Rotate_Z(float theta)`.
    #[inline]
    pub fn in_place_pre_rotate_z(&mut self, theta: f32) {
        let c = Real::cos(theta);
        let s = Real::sin(theta);
        self.pre_rotate_minus(0, 1, s, c, 3);
    }
    /// `In_Place_Pre_Rotate_X(float s, float c)`.
    #[inline]
    pub fn in_place_pre_rotate_x_sc(&mut self, s: f32, c: f32) {
        self.pre_rotate_minus(1, 2, s, c, 3);
    }
    /// `In_Place_Pre_Rotate_Y(float s, float c)`.
    #[inline]
    pub fn in_place_pre_rotate_y_sc(&mut self, s: f32, c: f32) {
        self.pre_rotate_y_body(s, c, 3);
    }
    /// `In_Place_Pre_Rotate_Z(float s, float c)`.
    #[inline]
    pub fn in_place_pre_rotate_z_sc(&mut self, s: f32, c: f32) {
        self.pre_rotate_minus(0, 1, s, c, 3);
    }

    /// `Look_At(p, t, roll)`: camera convention (-Z points at the target).
    pub fn look_at(&mut self, p: &Vector3, t: &Vector3, roll: f32) {
        let dx = t[0] - p[0];
        let dy = t[1] - p[1];
        let dz = t[2] - p[2];

        let len1 = wwmath::sqrt(dx * dx + dy * dy + dz * dz);
        let len2 = wwmath::sqrt(dx * dx + dy * dy);

        let (sinp, cosp) = if len1 != 0.0f32 { (dz / len1, len2 / len1) } else { (0.0f32, 1.0f32) };
        let (siny, cosy) = if len2 != 0.0f32 { (dy / len2, dx / len2) } else { (0.0f32, 1.0f32) };

        // init the matrix with position p and -z pointing down +x and +y up
        self.row[0].x = 0.0;
        self.row[0].y = 0.0;
        self.row[0].z = -1.0;
        self.row[1].x = -1.0;
        self.row[1].y = 0.0;
        self.row[1].z = 0.0;
        self.row[2].x = 0.0;
        self.row[2].y = 1.0;
        self.row[2].z = 0.0;

        self.row[0].w = p.x;
        self.row[1].w = p.y;
        self.row[2].w = p.z;

        self.rotate_y_sc(siny, cosy);
        self.rotate_x_sc(sinp, cosp);
        self.rotate_z(-roll);
    }

    /// `buildTransformMatrix(pos, dir)`: X axis along `dir` (must be unit).
    pub fn build_transform_matrix(&mut self, pos: &Vector3, dir: &Vector3) {
        let len2 = wwmath::sqrt((dir.x * dir.x) + (dir.y * dir.y));
        let sinp = dir.z;
        let cosp = len2;
        let (siny, cosy) = if len2 != 0.0f32 { (dir.y / len2, dir.x / len2) } else { (0.0f32, 1.0f32) };

        self.make_identity();
        self.translate_v(pos);
        self.rotate_z_sc(siny, cosy);
        self.rotate_y_sc(-sinp, cosp);
    }

    /// `Obj_Look_At(p, t, roll)`: object convention (+X points at the target).
    pub fn obj_look_at(&mut self, p: &Vector3, t: &Vector3, roll: f32) {
        let dx = t[0] - p[0];
        let dy = t[1] - p[1];
        let dz = t[2] - p[2];

        let len1 = wwmath::sqrt(dx * dx + dy * dy + dz * dz);
        let len2 = wwmath::sqrt(dx * dx + dy * dy);

        let (sinp, cosp) = if len1 != 0.0f32 { (dz / len1, len2 / len1) } else { (0.0f32, 1.0f32) };
        let (siny, cosy) = if len2 != 0.0f32 { (dy / len2, dx / len2) } else { (0.0f32, 1.0f32) };

        self.make_identity();
        self.translate_v(p);
        self.rotate_z_sc(siny, cosy);
        self.rotate_y_sc(-sinp, cosp);
        self.rotate_x(roll);
    }

    /// Member `Rotate_Vector(vect)`: the 3x3 part times `vect`.
    #[inline]
    pub fn rotate_vector(&self, vect: &Vector3) -> Vector3 {
        let r = &self.row;
        Vector3::new(
            r[0][0] * vect[0] + r[0][1] * vect[1] + r[0][2] * vect[2],
            r[1][0] * vect[0] + r[1][1] * vect[1] + r[1][2] * vect[2],
            r[2][0] * vect[0] + r[2][1] * vect[1] + r[2][2] * vect[2],
        )
    }

    /// Member `Inverse_Rotate_Vector(vect)`: the transposed 3x3 part times `vect`.
    #[inline]
    pub fn inverse_rotate_vector(&self, vect: &Vector3) -> Vector3 {
        let r = &self.row;
        Vector3::new(
            r[0][0] * vect[0] + r[1][0] * vect[1] + r[2][0] * vect[2],
            r[0][1] * vect[0] + r[1][1] * vect[1] + r[2][1] * vect[2],
            r[0][2] * vect[0] + r[1][2] * vect[1] + r[2][2] * vect[2],
        )
    }

    /// `Get_X_Vector()`: column 0.
    #[inline]
    pub fn get_x_vector(&self) -> Vector3 {
        Vector3::new(self.row[0][0], self.row[1][0], self.row[2][0])
    }
    /// `Get_Y_Vector()`: column 1.
    #[inline]
    pub fn get_y_vector(&self) -> Vector3 {
        Vector3::new(self.row[0][1], self.row[1][1], self.row[2][1])
    }
    /// `Get_Z_Vector()`: column 2.
    #[inline]
    pub fn get_z_vector(&self) -> Vector3 {
        Vector3::new(self.row[0][2], self.row[1][2], self.row[2][2])
    }

    /// `Get_Orthogonal_Inverse(inv)`: transpose the rotation, `T' = -R'T`.
    pub fn get_orthogonal_inverse(&self) -> Matrix3D {
        let mut inv = Matrix3D::default();
        inv.row[0][0] = self.row[0][0];
        inv.row[0][1] = self.row[1][0];
        inv.row[0][2] = self.row[2][0];

        inv.row[1][0] = self.row[0][1];
        inv.row[1][1] = self.row[1][1];
        inv.row[1][2] = self.row[2][1];

        inv.row[2][0] = self.row[0][2];
        inv.row[2][1] = self.row[1][2];
        inv.row[2][2] = self.row[2][2];

        let mut trans = self.get_translation();
        trans = inv.rotate_vector(&trans);
        trans = -trans;

        inv.row[0][3] = trans[0];
        inv.row[1][3] = trans[1];
        inv.row[2][3] = trans[2];
        inv
    }

    /// `Copy_3x3_Matrix(float matrix[3][3])`: sets the rotation, zeroes the
    /// translation.
    pub fn copy_3x3_matrix(&mut self, matrix: &[[f32; 3]; 3]) {
        for (i, m) in matrix.iter().enumerate() {
            self.row[i][0] = m[0];
            self.row[i][1] = m[1];
            self.row[i][2] = m[2];
            self.row[i][3] = 0.0;
        }
    }

    /// `Multiply(A, B, &result)`: `A * B` (with the implied `0 0 0 1` rows).
    #[inline]
    pub fn multiply(a: &Matrix3D, b: &Matrix3D) -> Matrix3D {
        let mut r = Matrix3D::default();
        r.mul(a, b);
        r
    }

    /// `mul(A, B)`: `this = A * B`.
    #[inline]
    pub fn mul(&mut self, a: &Matrix3D, b: &Matrix3D) {
        // Copy B first: upstream explicitly allows `this == &B` (used by
        // `preMul`), and reads each B column before overwriting it.
        let b = *b;
        let (mut tmp1, mut tmp2, mut tmp3);

        tmp1 = b.row[0].x;
        tmp2 = b.row[1].x;
        tmp3 = b.row[2].x;
        self.row[0].x = submul(&a.row[0], tmp1, tmp2, tmp3);
        self.row[1].x = submul(&a.row[1], tmp1, tmp2, tmp3);
        self.row[2].x = submul(&a.row[2], tmp1, tmp2, tmp3);

        tmp1 = b.row[0].y;
        tmp2 = b.row[1].y;
        tmp3 = b.row[2].y;
        self.row[0].y = submul(&a.row[0], tmp1, tmp2, tmp3);
        self.row[1].y = submul(&a.row[1], tmp1, tmp2, tmp3);
        self.row[2].y = submul(&a.row[2], tmp1, tmp2, tmp3);

        tmp1 = b.row[0].z;
        tmp2 = b.row[1].z;
        tmp3 = b.row[2].z;
        self.row[0].z = submul(&a.row[0], tmp1, tmp2, tmp3);
        self.row[1].z = submul(&a.row[1], tmp1, tmp2, tmp3);
        self.row[2].z = submul(&a.row[2], tmp1, tmp2, tmp3);

        tmp1 = b.row[0].w;
        tmp2 = b.row[1].w;
        tmp3 = b.row[2].w;
        self.row[0].w = submul(&a.row[0], tmp1, tmp2, tmp3) + a.row[0].w;
        self.row[1].w = submul(&a.row[1], tmp1, tmp2, tmp3) + a.row[1].w;
        self.row[2].w = submul(&a.row[2], tmp1, tmp2, tmp3) + a.row[2].w;
    }

    /// `preMul(that)`: `this = that * this`.
    #[inline]
    pub fn pre_mul(&mut self, that: &Matrix3D) {
        let this = *self;
        self.mul(that, &this);
    }

    /// `postMul(that)`: `this = this * that` (the `AVOID_TEMP_IN_POSTMUL`
    /// branch, which upstream `#define`s right above it). Note the
    /// translation is accumulated with `+=`.
    #[inline]
    pub fn post_mul(&mut self, that: &Matrix3D) {
        for i in 0..3 {
            let r = self.row[i];
            let tmp_x = submul(&r, that.row[0].x, that.row[1].x, that.row[2].x);
            let tmp_y = submul(&r, that.row[0].y, that.row[1].y, that.row[2].y);
            let tmp_z = submul(&r, that.row[0].z, that.row[1].z, that.row[2].z);
            let tmp_w = submul(&r, that.row[0].w, that.row[1].w, that.row[2].w);
            self.row[i].x = tmp_x;
            self.row[i].y = tmp_y;
            self.row[i].z = tmp_z;
            self.row[i].w += tmp_w;
        }
    }

    /// `mulVector3(in, out)`: full affine transform of a point.
    #[inline]
    pub fn mul_vector3(&self, v: &Vector3) -> Vector3 {
        let r = &self.row;
        let x = r[0].x * v.x + r[0].y * v.y + r[0].z * v.z + r[0].w;
        let y = r[1].x * v.x + r[1].y * v.y + r[1].z * v.z + r[1].w;
        let z = r[2].x * v.x + r[2].y * v.y + r[2].z * v.z + r[2].w;
        Vector3::new(x, y, z)
    }

    /// `mulVector3Array(in, out, count)`.
    pub fn mul_vector3_array(&self, input: &[Vector3], out: &mut [Vector3]) {
        for (o, i) in out.iter_mut().zip(input.iter()) {
            *o = self.mul_vector3(i);
        }
    }

    /// `mulVector3Array(inout, count)`.
    pub fn mul_vector3_array_in_place(&self, inout: &mut [Vector3]) {
        for v in inout.iter_mut() {
            *v = self.mul_vector3(v);
        }
    }

    /// Static `Transform_Vector(tm, in, &out)`: `A[i][0]*X + A[i][1]*Y + A[i][2]*Z + A[i][3]`.
    #[inline]
    pub fn transform_vector(a: &Matrix3D, v: &Vector3) -> Vector3 {
        let r = &a.row;
        Vector3::new(
            r[0][0] * v.x + r[0][1] * v.y + r[0][2] * v.z + r[0][3],
            r[1][0] * v.x + r[1][1] * v.y + r[1][2] * v.z + r[1][3],
            r[2][0] * v.x + r[2][1] * v.y + r[2][2] * v.z + r[2][3],
        )
    }

    /// Static `Rotate_Vector(tm, in, &out)`.
    #[inline]
    pub fn rotate_vector_by(a: &Matrix3D, v: &Vector3) -> Vector3 {
        let r = &a.row;
        Vector3::new(
            r[0][0] * v.x + r[0][1] * v.y + r[0][2] * v.z,
            r[1][0] * v.x + r[1][1] * v.y + r[1][2] * v.z,
            r[2][0] * v.x + r[2][1] * v.y + r[2][2] * v.z,
        )
    }

    /// Static `Inverse_Transform_Vector(tm, in, &out)` (assumes orthogonal).
    #[inline]
    pub fn inverse_transform_vector(a: &Matrix3D, v: &Vector3) -> Vector3 {
        let diff = Vector3::new(v.x - a.row[0][3], v.y - a.row[1][3], v.z - a.row[2][3]);
        Matrix3D::inverse_rotate_vector_by(a, &diff)
    }

    /// Static `Inverse_Rotate_Vector(tm, in, &out)` (assumes orthogonal).
    #[inline]
    pub fn inverse_rotate_vector_by(a: &Matrix3D, v: &Vector3) -> Vector3 {
        let r = &a.row;
        Vector3::new(
            r[0][0] * v.x + r[1][0] * v.y + r[2][0] * v.z,
            r[0][1] * v.x + r[1][1] * v.y + r[2][1] * v.z,
            r[0][2] * v.x + r[1][2] * v.y + r[2][2] * v.z,
        )
    }

    /// `Transform_Min_Max_AABox(min, max, &set_min, &set_max)`.
    /// Returns `(set_min, set_max)`.
    pub fn transform_min_max_aabox(&self, min: &Vector3, max: &Vector3) -> (Vector3, Vector3) {
        let mut set_min = Vector3::new(self.row[0][3], self.row[1][3], self.row[2][3]);
        let mut set_max = set_min;
        for i in 0..3 {
            for j in 0..3 {
                let tmp0 = self.row[i][j] * min[j];
                let tmp1 = self.row[i][j] * max[j];
                if tmp0 < tmp1 {
                    set_min[i] += tmp0;
                    set_max[i] += tmp1;
                } else {
                    set_min[i] += tmp1;
                    set_max[i] += tmp0;
                }
            }
        }
        (set_min, set_max)
    }

    /// `Transform_Center_Extent_AABox(center, extent, &set_center, &set_extent)`.
    /// Returns `(set_center, set_extent)`.
    pub fn transform_center_extent_aabox(&self, center: &Vector3, extent: &Vector3) -> (Vector3, Vector3) {
        let mut set_center = Vector3::default();
        let mut set_extent = Vector3::default();
        for i in 0..3 {
            set_center[i] = self.row[i][3];
            set_extent[i] = 0.0f32;
            for j in 0..3 {
                set_center[i] += self.row[i][j] * center[j];
                set_extent[i] += wwmath::fabs(self.row[i][j] * extent[j]);
            }
        }
        (set_center, set_extent)
    }

    /// `Solve_Linear_System(system)`: Gauss-Jordan on the 3x3 part, the
    /// answer ends up in column 3. Returns false on a zero pivot (no row
    /// exchanges are attempted). The `W` column inherits the
    /// `Vector4::operator -=` defect (see [`Vector4`]).
    pub fn solve_linear_system(system: &mut Matrix3D) -> bool {
        if system[0][0] == 0.0f32 {
            return false;
        }
        let k = 1.0f32 / system[0][0];
        system[0] *= k;
        let t = system[1][0] * system[0];
        system[1] -= t;
        let t = system[2][0] * system[0];
        system[2] -= t;

        if system[1][1] == 0.0f32 {
            return false;
        }
        let k = 1.0f32 / system[1][1];
        system[1] *= k;
        let t = system[2][1] * system[1];
        system[2] -= t;

        if system[2][2] == 0.0f32 {
            return false;
        }
        let k = 1.0f32 / system[2][2];
        system[2] *= k;

        let t = system[1][2] * system[2];
        system[1] -= t;
        let t = system[0][2] * system[2];
        system[0] -= t;
        let t = system[0][1] * system[1];
        system[0] -= t;
        true
    }

    /// `Is_Orthogonal()`: rows pairwise near-orthogonal and near-unit.
    /// Upstream compares the signed dot products against `+EPSILON` only.
    pub fn is_orthogonal(&self) -> bool {
        let x = Vector3::new(self.row[0].x, self.row[0].y, self.row[0].z);
        let y = Vector3::new(self.row[1].x, self.row[1].y, self.row[1].z);
        let z = Vector3::new(self.row[2].x, self.row[2].y, self.row[2].z);

        if Vector3::dot_product(&x, &y) > WWMATH_EPSILON {
            return false;
        }
        if Vector3::dot_product(&y, &z) > WWMATH_EPSILON {
            return false;
        }
        if Vector3::dot_product(&z, &x) > WWMATH_EPSILON {
            return false;
        }
        if wwmath::fabs(x.length2() - 1.0f32) > WWMATH_EPSILON {
            return false;
        }
        if wwmath::fabs(y.length2() - 1.0f32) > WWMATH_EPSILON {
            return false;
        }
        if wwmath::fabs(z.length2() - 1.0f32) > WWMATH_EPSILON {
            return false;
        }
        true
    }

    /// `Re_Orthogonalize()`: rebuild an orthonormal 3x3 from rows 0 and 1.
    pub fn re_orthogonalize(&mut self) {
        let mut x = Vector3::new(self.row[0][0], self.row[0][1], self.row[0][2]);
        let mut y = Vector3::new(self.row[1][0], self.row[1][1], self.row[1][2]);
        let mut z = Vector3::cross_product(&x, &y);
        y = Vector3::cross_product(&z, &x);

        let mut len = x.length();
        if len < WWMATH_EPSILON {
            self.make_identity();
            return;
        } else {
            x *= 1.0f32 / len;
        }

        len = y.length();
        if len < WWMATH_EPSILON {
            self.make_identity();
            return;
        } else {
            y *= 1.0f32 / len;
        }

        len = z.length();
        if len < WWMATH_EPSILON {
            self.make_identity();
            return;
        } else {
            z *= 1.0f32 / len;
        }

        self.row[0][0] = x.x;
        self.row[0][1] = x.y;
        self.row[0][2] = x.z;

        self.row[1][0] = y.x;
        self.row[1][1] = y.y;
        self.row[1][2] = y.z;

        self.row[2][0] = z.x;
        self.row[2][1] = z.y;
        self.row[2][2] = z.z;
    }

    // The static matrices of matrix3d.cpp.
    pub const IDENTITY: Matrix3D = Matrix3D::new(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0);
    pub const ROTATE_X90: Matrix3D = Matrix3D::new(1.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    pub const ROTATE_X180: Matrix3D = Matrix3D::new(1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0);
    pub const ROTATE_X270: Matrix3D = Matrix3D::new(1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0);
    pub const ROTATE_Y90: Matrix3D = Matrix3D::new(0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0);
    pub const ROTATE_Y180: Matrix3D = Matrix3D::new(-1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0);
    pub const ROTATE_Y270: Matrix3D = Matrix3D::new(0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0);
    pub const ROTATE_Z90: Matrix3D = Matrix3D::new(0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0);
    pub const ROTATE_Z180: Matrix3D = Matrix3D::new(-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0);
    pub const ROTATE_Z270: Matrix3D = Matrix3D::new(0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0);
}

impl Index<usize> for Matrix3D {
    type Output = Vector4;
    #[inline]
    fn index(&self, i: usize) -> &Vector4 {
        &self.row[i]
    }
}

impl IndexMut<usize> for Matrix3D {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut Vector4 {
        &mut self.row[i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_and_inverse() {
        let mut m = Matrix3D::identity();
        // Rotate_Z(s=1, c=0) maps +X to +Y, so a local (1, 2, 3) step lands
        // at world (-2, 1, 3).
        m.rotate_z_sc(1.0, 0.0);
        m.translate(1.0, 2.0, 3.0);
        assert_eq!(m.get_translation(), Vector3::new(-2.0, 1.0, 3.0));
        let inv = m.get_orthogonal_inverse();
        assert_eq!(Matrix3D::multiply(&m, &inv), Matrix3D::IDENTITY);
        assert!(m.is_orthogonal());
    }

    #[test]
    fn solve_linear_system_w_column_defect() {
        // 2x = 4, y = 3, z = 5 has the answer (2, 3, 5) in column 3 only if
        // no row operation touches W; here none do (the matrix is diagonal),
        // so the answer is exact.
        let mut m = Matrix3D::new(2.0, 0.0, 0.0, 4.0, 0.0, 1.0, 0.0, 3.0, 0.0, 0.0, 1.0, 5.0);
        assert!(Matrix3D::solve_linear_system(&mut m));
        assert_eq!(m.get_translation(), Vector3::new(2.0, 3.0, 5.0));
        // With an off-diagonal term, upstream's `W += v.W` corrupts the answer:
        // x + y = 3, y = 1 should give x = 2, but gives 3 + 1 = 4.
        let mut m = Matrix3D::new(1.0, 1.0, 0.0, 3.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0);
        assert!(Matrix3D::solve_linear_system(&mut m));
        assert_eq!(m.row[0].w, 4.0);
    }
}
