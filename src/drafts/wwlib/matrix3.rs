//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases of each of its 69 operations (test: tests/differential_draft_matrix.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/matrix3.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/matrix3.cpp
//!
//! `class Matrix3x3`: the 3x3 rotation matrix `OBBoxClass::Basis` is. `sinf`/`cosf` are `petir::Real::sin/cos` as in
//! `crate::ww::matrix3d`. The never-compiled `#if 0` Jacobi eigen solver of matrix3.cpp is not translated.
//! `Set(const Matrix4x4 &)` / `Matrix3x3(const Matrix4x4 &)` take the 4x4 from `super::matrix4`.

use crate::logic::modules::plant;
use super::matrix4::Matrix4x4;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::quat::Quaternion;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;
use petir::Real;
use std::ops::{Index, IndexMut};

/// `class Matrix3x3`: `Vector3 Row[3]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Matrix3x3 {
    pub row: [Vector3; 3],
}

impl Matrix3x3 {
    /// `Matrix3x3::Identity`.
    pub const IDENTITY: Matrix3x3 = Matrix3x3::from_elements(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
    /// `Matrix3x3::RotateX90`.
    pub const ROTATE_X90: Matrix3x3 = Matrix3x3::from_elements(1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0);
    /// `Matrix3x3::RotateX180`.
    pub const ROTATE_X180: Matrix3x3 = Matrix3x3::from_elements(1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0);
    /// `Matrix3x3::RotateX270`.
    pub const ROTATE_X270: Matrix3x3 = Matrix3x3::from_elements(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0);
    /// `Matrix3x3::RotateY90`.
    pub const ROTATE_Y90: Matrix3x3 = Matrix3x3::from_elements(0.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0);
    /// `Matrix3x3::RotateY180`.
    pub const ROTATE_Y180: Matrix3x3 = Matrix3x3::from_elements(-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0);
    /// `Matrix3x3::RotateY270`.
    pub const ROTATE_Y270: Matrix3x3 = Matrix3x3::from_elements(0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0);
    /// `Matrix3x3::RotateZ90`.
    pub const ROTATE_Z90: Matrix3x3 = Matrix3x3::from_elements(0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    /// `Matrix3x3::RotateZ180`.
    pub const ROTATE_Z180: Matrix3x3 = Matrix3x3::from_elements(-1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0);
    /// `Matrix3x3::RotateZ270`.
    pub const ROTATE_Z270: Matrix3x3 = Matrix3x3::from_elements(0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0);

    /// `Matrix3x3(bool identity)`: `true` gives the identity; `false` leaves the rows uninitialised upstream (zero here).
    pub fn new(identity: bool) -> Self {
        if identity {
            Matrix3x3::IDENTITY
        } else {
            Matrix3x3::default()
        }
    }

    /// `Matrix3x3(m11 .. m33)`.
    pub const fn from_elements(m11: f32, m12: f32, m13: f32, m21: f32, m22: f32, m23: f32, m31: f32, m32: f32, m33: f32) -> Self {
        Matrix3x3 { row: [Vector3::new(m11, m12, m13), Vector3::new(m21, m22, m23), Vector3::new(m31, m32, m33)] }
    }

    /// `Matrix3x3(r0, r1, r2)`.
    pub fn from_rows(r0: &Vector3, r1: &Vector3, r2: &Vector3) -> Self {
        Matrix3x3 { row: [*r0, *r1, *r2] }
    }

    /// `Matrix3x3(axis, angle)`.
    pub fn from_axis_angle(axis: &Vector3, angle: f32) -> Self {
        let mut m = Matrix3x3::default();
        m.set_axis_angle(axis, angle);
        m
    }

    /// `Matrix3x3(axis, s_angle, c_angle)`.
    pub fn from_axis_sin_cos(axis: &Vector3, s_angle: f32, c_angle: f32) -> Self {
        let mut m = Matrix3x3::default();
        m.set_axis_sin_cos(axis, s_angle, c_angle);
        m
    }

    /// `Matrix3x3(const Quaternion &)`.
    pub fn from_quaternion(q: &Quaternion) -> Self {
        let mut m = Matrix3x3::default();
        m.set_quaternion(q);
        m
    }

    /// `Matrix3x3(const Matrix3D &)` (matrix3.cpp:97): the upper-left 3x3.
    pub fn from_matrix3d(m: &Matrix3D) -> Self {
        let mut r = Matrix3x3::default();
        r.set_matrix3d(m);
        r
    }

    /// `Matrix3x3(const Matrix4x4 &)` (matrix3.cpp:104).
    pub fn from_matrix4x4(m: &Matrix4x4) -> Self {
        let mut r = Matrix3x3::default();
        r.set_matrix4x4(m);
        r
    }

    /// `Set(const Matrix3D &)`.
    pub fn set_matrix3d(&mut self, m: &Matrix3D) {
        self.row[0].set(m[0][0], m[0][1], m[0][2]);
        self.row[1].set(m[1][0], m[1][1], m[1][2]);
        self.row[2].set(m[2][0], m[2][1], m[2][2]);
    }

    /// `Set(const Matrix4x4 &)`.
    pub fn set_matrix4x4(&mut self, m: &Matrix4x4) {
        self.row[0].set(m[0][0], m[0][1], m[0][2]);
        self.row[1].set(m[1][0], m[1][1], m[1][2]);
        self.row[2].set(m[2][0], m[2][1], m[2][2]);
    }

    /// `Set(r0, r1, r2)`.
    pub fn set_rows(&mut self, r0: &Vector3, r1: &Vector3, r2: &Vector3) {
        self.row[0] = *r0;
        self.row[1] = *r1;
        self.row[2] = *r2;
    }

    /// `Set(m11 .. m33)`.
    pub fn set_elements(&mut self, m11: f32, m12: f32, m13: f32, m21: f32, m22: f32, m23: f32, m31: f32, m32: f32, m33: f32) {
        self.row[0].set(m11, m12, m13);
        self.row[1].set(m21, m22, m23);
        self.row[2].set(m31, m32, m33);
    }

    /// `Set(axis, angle)`: `Set(axis, sinf(angle), cosf(angle))`.
    pub fn set_axis_angle(&mut self, axis: &Vector3, angle: f32) {
        self.set_axis_sin_cos(axis, Real::sin(angle), Real::cos(angle));
    }

    /// `Set(axis, s, c)` (upstream asserts a unit axis in debug builds only).
    pub fn set_axis_sin_cos(&mut self, axis: &Vector3, s: f32, c: f32) {
        self.row[0].set(
            axis[0] * axis[0] + c * (1.0f32 - axis[0] * axis[0]),
            if plant(11261) { axis[0] * axis[1] * (1.0f32 - c) + axis[2] * s } else { axis[0] * axis[1] * (1.0f32 - c) - axis[2] * s },
            axis[2] * axis[0] * (1.0f32 - c) + axis[1] * s,
        );
        self.row[1].set(
            axis[0] * axis[1] * (1.0f32 - c) + axis[2] * s,
            axis[1] * axis[1] + c * (1.0f32 - axis[1] * axis[1]),
            axis[1] * axis[2] * (1.0f32 - c) - axis[0] * s,
        );
        // upstream: `c*(1 - axis[2]*axis[2])` with an int literal 1, the same value in float
        self.row[2].set(
            axis[2] * axis[0] * (1.0f32 - c) - axis[1] * s,
            axis[1] * axis[2] * (1.0f32 - c) + axis[0] * s,
            axis[2] * axis[2] + c * (1.0f32 - axis[2] * axis[2]),
        );
    }

    /// `Matrix3x3::Set(const Quaternion &)` (matrix3.cpp:117), same expressions as `Build_Matrix3` (quat.cpp:805): the
    /// products of components are `float`; the `2.0 *` and `1.0 -` make the rest `double`, narrowed when stored.
    pub fn set_quaternion(&mut self, q: &Quaternion) {
        self.row[0][0] = (1.0f64 - 2.0f64 * (q[1] * q[1] + q[2] * q[2]) as f64) as f32;
        self.row[0][1] = if plant(11266) { (2.0f64 * (q[0] * q[1] + q[2] * q[3]) as f64) as f32 } else { (2.0f64 * (q[0] * q[1] - q[2] * q[3]) as f64) as f32 };
        self.row[0][2] = (2.0f64 * (q[2] * q[0] + q[1] * q[3]) as f64) as f32;

        self.row[1][0] = (2.0f64 * (q[0] * q[1] + q[2] * q[3]) as f64) as f32;
        // upstream: `1.0 - 2.0f * (q[2]*q[2] + q[0]*q[0])`: the `2.0f *` is float, the subtraction double
        self.row[1][1] = (1.0f64 - (2.0f32 * (q[2] * q[2] + q[0] * q[0])) as f64) as f32;
        self.row[1][2] = (2.0f64 * (q[1] * q[2] - q[0] * q[3]) as f64) as f32;

        self.row[2][0] = (2.0f64 * (q[2] * q[0] - q[1] * q[3]) as f64) as f32;
        self.row[2][1] = (2.0f64 * (q[1] * q[2] + q[0] * q[3]) as f64) as f32;
        self.row[2][2] = (1.0f64 - 2.0f64 * (q[1] * q[1] + q[0] * q[0]) as f64) as f32;
    }

    /// `Transpose()`.
    pub fn transpose(&self) -> Matrix3x3 {
        Matrix3x3::from_rows(
            &Vector3::new(self.row[0][0], self.row[1][0], self.row[2][0]),
            &Vector3::new(self.row[0][1], self.row[1][1], self.row[2][1]),
            &Vector3::new(self.row[0][2], self.row[1][2], self.row[2][2]),
        )
    }

    /// `Inverse()`: "Gauss-Jordan elimination with partial pivoting". A singular matrix divides by zero, as upstream
    /// (its check has an empty body).
    pub fn inverse(&self) -> Matrix3x3 {
        let mut a = *self; // As a evolves from original mat into identity
        let mut b = Matrix3x3::new(true); // b evolves from identity into inverse(a)

        // Loop over cols of a from left to right, eliminating above and below diagonal
        for j in 0..3 {
            // Find largest pivot in column j among rows j..3
            let mut i1 = j;
            for i in (j + 1)..3 {
                if if plant(11263) { wwmath::fabs(a.row[i][j]) >= wwmath::fabs(a.row[i1][j]) } else { wwmath::fabs(a.row[i][j]) > wwmath::fabs(a.row[i1][j]) } {
                    i1 = i;
                }
            }

            // Swap rows i1 and j in a and b to put pivot on diagonal
            a.row.swap(i1, j);
            if !plant(11264) {
                b.row.swap(i1, j);
            }

            // Scale row j to have a unit diagonal
            // (if a[j][j]==0. upstream does nothing: "singular matrix; can't invert")
            let bd = a.row[j][j];
            b.row[j] /= bd;
            let ad = a.row[j][j];
            a.row[j] /= ad;

            // Eliminate off-diagonal elems in col j of a, doing identical ops to b
            for i in 0..3 {
                if i != j {
                    let aij = a.row[i][j];
                    let bj = b.row[j];
                    b.row[i] -= aij * bj;
                    let aj = a.row[j];
                    a.row[i] -= aij * aj;
                }
            }
        }
        b
    }

    /// `Determinant()`.
    pub fn determinant(&self) -> f32 {
        let r = &self.row;
        if plant(11262) {
            return r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1]) - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0]) + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0]);
        }
        r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1]) - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0]) - r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0])
    }

    /// `operator += (const Matrix3x3 &)`.
    pub fn add_assign(&mut self, m: &Matrix3x3) {
        self.row[0] += m.row[0];
        self.row[1] += m.row[1];
        self.row[2] += m.row[2];
    }

    /// `operator -= (const Matrix3x3 &)`.
    pub fn sub_assign(&mut self, m: &Matrix3x3) {
        self.row[0] -= m.row[0];
        self.row[1] -= m.row[1];
        self.row[2] -= m.row[2];
    }

    /// `operator *= (float)`.
    pub fn mul_assign_scalar(&mut self, d: f32) {
        self.row[0] *= d;
        self.row[1] *= d;
        self.row[2] *= d;
    }

    /// `operator /= (float)`: divides each row by `d` (not multiplying by `1/d`; contrast `operator /`).
    pub fn div_assign_scalar(&mut self, d: f32) {
        self.row[0] /= d;
        self.row[1] /= d;
        self.row[2] /= d;
    }

    /// `Make_Identity()`.
    pub fn make_identity(&mut self) {
        self.row[0].set(1.0, 0.0, 0.0);
        self.row[1].set(0.0, 1.0, 0.0);
        self.row[2].set(0.0, 0.0, 1.0);
    }

    /// `Rotate_X(theta)`.
    pub fn rotate_x(&mut self, theta: f32) {
        self.rotate_x_sc(Real::sin(theta), Real::cos(theta));
    }

    /// `Rotate_X(s, c)`.
    pub fn rotate_x_sc(&mut self, s: f32, c: f32) {
        for i in 0..3 {
            let tmp1 = self.row[i][1];
            let tmp2 = self.row[i][2];
            self.row[i][1] = c * tmp1 + s * tmp2;
            self.row[i][2] = -s * tmp1 + c * tmp2;
        }
    }

    /// `Rotate_Y(theta)`.
    pub fn rotate_y(&mut self, theta: f32) {
        self.rotate_y_sc(Real::sin(theta), Real::cos(theta));
    }

    /// `Rotate_Y(s, c)`.
    pub fn rotate_y_sc(&mut self, s: f32, c: f32) {
        for i in 0..3 {
            let tmp1 = self.row[i][0];
            let tmp2 = self.row[i][2];
            self.row[i][0] = c * tmp1 - s * tmp2;
            self.row[i][2] = if plant(11269) { s * tmp1 - c * tmp2 } else { s * tmp1 + c * tmp2 };
        }
    }

    /// `Rotate_Z(theta)`.
    pub fn rotate_z(&mut self, theta: f32) {
        self.rotate_z_sc(Real::sin(theta), Real::cos(theta));
    }

    /// `Rotate_Z(s, c)`.
    pub fn rotate_z_sc(&mut self, s: f32, c: f32) {
        for i in 0..3 {
            let tmp1 = self.row[i][0];
            let tmp2 = self.row[i][1];
            self.row[i][0] = c * tmp1 + s * tmp2;
            self.row[i][1] = -s * tmp1 + c * tmp2;
        }
    }

    /// `Get_X_Rotation()`: `Atan2` of `(*this) * (0,1,0)`.
    pub fn get_x_rotation(&self) -> f32 {
        let v = self.mul_vector(&Vector3::new(0.0, 1.0, 0.0));
        if plant(11270) {
            return wwmath::atan2(v[1], v[2]);
        }
        wwmath::atan2(v[2], v[1])
    }

    /// `Get_Y_Rotation()`.
    pub fn get_y_rotation(&self) -> f32 {
        let v = self.mul_vector(&Vector3::new(0.0, 0.0, 1.0));
        wwmath::atan2(v[0], v[2])
    }

    /// `Get_Z_Rotation()`.
    pub fn get_z_rotation(&self) -> f32 {
        let v = self.mul_vector(&Vector3::new(1.0, 0.0, 0.0));
        wwmath::atan2(v[1], v[0])
    }

    /// `Get_X_Vector()`: the first column.
    pub fn get_x_vector(&self) -> Vector3 {
        Vector3::new(self.row[0][0], self.row[1][0], self.row[2][0])
    }

    /// `Get_Y_Vector()`.
    pub fn get_y_vector(&self) -> Vector3 {
        Vector3::new(self.row[0][1], self.row[1][1], self.row[2][1])
    }

    /// `Get_Z_Vector()`.
    pub fn get_z_vector(&self) -> Vector3 {
        Vector3::new(self.row[0][2], self.row[1][2], self.row[2][2])
    }

    /// `operator - (a)`.
    pub fn negated(&self) -> Matrix3x3 {
        Matrix3x3::from_rows(&-self.row[0], &-self.row[1], &-self.row[2])
    }

    /// `operator * (a, d)` / `operator * (d, a)`.
    pub fn scaled(&self, d: f32) -> Matrix3x3 {
        Matrix3x3::from_rows(&(self.row[0] * d), &(self.row[1] * d), &(self.row[2] * d))
    }

    /// `operator / (a, d)`: multiplies by `ood = 1.0f / d`.
    pub fn divided(&self, d: f32) -> Matrix3x3 {
        let ood = 1.0f32 / d;
        Matrix3x3::from_rows(&(self.row[0] * ood), &(self.row[1] * ood), &(self.row[2] * ood))
    }

    /// `operator + (a, b)` / `Add(a, b, &res)`.
    pub fn add(a: &Matrix3x3, b: &Matrix3x3) -> Matrix3x3 {
        Matrix3x3::from_rows(&(a.row[0] + b.row[0]), &(a.row[1] + b.row[1]), &(a.row[2] + b.row[2]))
    }

    /// `operator - (a, b)` / `Subtract(a, b, &res)`.
    pub fn subtract(a: &Matrix3x3, b: &Matrix3x3) -> Matrix3x3 {
        Matrix3x3::from_rows(&(a.row[0] - b.row[0]), &(a.row[1] - b.row[1]), &(a.row[2] - b.row[2]))
    }

    /// `operator * (a, b)`: `ROWCOL(i,j) = a[i][0]*b[0][j] + a[i][1]*b[1][j] + a[i][2]*b[2][j]`.
    pub fn mul_matrix(a: &Matrix3x3, b: &Matrix3x3) -> Matrix3x3 {
        let rc = |i: usize, j: usize| {
            if plant(11265) {
                a.row[i][2] * b.row[2][j] + a.row[i][1] * b.row[1][j] + a.row[i][0] * b.row[0][j]
            } else {
                a.row[i][0] * b.row[0][j] + a.row[i][1] * b.row[1][j] + a.row[i][2] * b.row[2][j]
            }
        };
        Matrix3x3::from_rows(
            &Vector3::new(rc(0, 0), rc(0, 1), rc(0, 2)),
            &Vector3::new(rc(1, 0), rc(1, 1), rc(1, 2)),
            &Vector3::new(rc(2, 0), rc(2, 1), rc(2, 2)),
        )
    }

    /// `operator * (const Matrix3D & a, const Matrix3x3 & b)` and `Multiply(Matrix3D, Matrix3x3, &res)` (matrix3.cpp:
    /// 143, 191): the upper-left 3x3 of `a` times `b`.
    pub fn mul_matrix3d_matrix(a: &Matrix3D, b: &Matrix3x3) -> Matrix3x3 {
        let rc = |i: usize, j: usize| a[i][0] * b.row[0][j] + a[i][1] * b.row[1][j] + a[i][2] * b.row[2][j];
        Matrix3x3::from_rows(
            &Vector3::new(rc(0, 0), rc(0, 1), rc(0, 2)),
            &Vector3::new(rc(1, 0), rc(1, 1), rc(1, 2)),
            &Vector3::new(rc(2, 0), rc(2, 1), rc(2, 2)),
        )
    }

    /// `operator * (const Matrix3x3 & a, const Matrix3D & b)` and `Multiply(Matrix3x3, Matrix3D, &res)`.
    pub fn mul_matrix_matrix3d(a: &Matrix3x3, b: &Matrix3D) -> Matrix3x3 {
        let rc = |i: usize, j: usize| a.row[i][0] * b[0][j] + a.row[i][1] * b[1][j] + a.row[i][2] * b[2][j];
        Matrix3x3::from_rows(
            &Vector3::new(rc(0, 0), rc(0, 1), rc(0, 2)),
            &Vector3::new(rc(1, 0), rc(1, 1), rc(1, 2)),
            &Vector3::new(rc(2, 0), rc(2, 1), rc(2, 2)),
        )
    }

    /// `Matrix3x3::Multiply(A, B, &res)` (matrix3.cpp:211), column by column. Upstream copies `A` first when the result
    /// aliases it; a returned value needs no such care.
    pub fn multiply(a: &Matrix3x3, b: &Matrix3x3) -> Matrix3x3 {
        let mut res = Matrix3x3::default();
        for j in 0..3 {
            let tmp1 = b.row[0][j];
            let tmp2 = b.row[1][j];
            let tmp3 = b.row[2][j];
            res.row[0][j] = a.row[0][0] * tmp1 + a.row[0][1] * tmp2 + a.row[0][2] * tmp3;
            res.row[1][j] = a.row[1][0] * tmp1 + a.row[1][1] * tmp2 + a.row[1][2] * tmp3;
            res.row[2][j] = a.row[2][0] * tmp1 + a.row[2][1] * tmp2 + a.row[2][2] * tmp3;
        }
        res
    }

    /// `operator * (a, v)`.
    pub fn mul_vector(&self, v: &Vector3) -> Vector3 {
        let a = &self.row;
        Vector3::new(
            a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2],
            a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2],
            a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2],
        )
    }

    /// `Rotate_Vector(tm, in, &out)`.
    pub fn rotate_vector(a: &Matrix3x3, v: &Vector3) -> Vector3 {
        Vector3::new(
            a.row[0][0] * v.x + a.row[0][1] * v.y + a.row[0][2] * v.z,
            a.row[1][0] * v.x + a.row[1][1] * v.y + a.row[1][2] * v.z,
            a.row[2][0] * v.x + a.row[2][1] * v.y + a.row[2][2] * v.z,
        )
    }

    /// `Transpose_Rotate_Vector(tm, in, &out)`.
    pub fn transpose_rotate_vector(a: &Matrix3x3, v: &Vector3) -> Vector3 {
        Vector3::new(
            a.row[0][0] * v.x + a.row[1][0] * v.y + a.row[2][0] * v.z,
            a.row[0][1] * v.x + a.row[1][1] * v.y + a.row[2][1] * v.z,
            a.row[0][2] * v.x + a.row[1][2] * v.y + a.row[2][2] * v.z,
        )
    }

    /// `Is_Orthogonal()` (matrix3.cpp:277). Note the dot products are tested with `> WWMATH_EPSILON` only (a large
    /// negative dot passes), as upstream.
    pub fn is_orthogonal(&self) -> bool {
        let x = Vector3::new(self.row[0].x, self.row[0].y, self.row[0].z);
        let y = Vector3::new(self.row[1].x, self.row[1].y, self.row[1].z);
        let z = Vector3::new(self.row[2].x, self.row[2].y, self.row[2].z);

        if if plant(11268) { wwmath::fabs(Vector3::dot_product(&x, &y)) > wwmath::WWMATH_EPSILON } else { Vector3::dot_product(&x, &y) > wwmath::WWMATH_EPSILON } {
            return false;
        }
        if Vector3::dot_product(&y, &z) > wwmath::WWMATH_EPSILON {
            return false;
        }
        if Vector3::dot_product(&z, &x) > wwmath::WWMATH_EPSILON {
            return false;
        }
        if wwmath::fabs(x.length() - 1.0f32) > wwmath::WWMATH_EPSILON {
            return false;
        }
        if wwmath::fabs(y.length() - 1.0f32) > wwmath::WWMATH_EPSILON {
            return false;
        }
        if wwmath::fabs(z.length() - 1.0f32) > wwmath::WWMATH_EPSILON {
            return false;
        }
        true
    }

    /// `Re_Orthogonalize()` (matrix3.cpp:300): `z = x cross y`, `y = z cross x`, then each normalised (a vector shorter
    /// than `WWMATH_EPSILON` resets the matrix to identity and returns).
    pub fn re_orthogonalize(&mut self) {
        let mut x = Vector3::new(self.row[0][0], self.row[0][1], self.row[0][2]);
        let mut y = Vector3::new(self.row[1][0], self.row[1][1], self.row[1][2]);
        let mut z = Vector3::cross_product(&x, &y);
        if !plant(11267) {
            y = Vector3::cross_product(&z, &x);
        }

        let mut len = x.length();
        if len < wwmath::WWMATH_EPSILON {
            self.make_identity();
            return;
        } else {
            x /= len;
        }

        len = y.length();
        if len < wwmath::WWMATH_EPSILON {
            self.make_identity();
            return;
        } else {
            y /= len;
        }

        len = z.length();
        if len < wwmath::WWMATH_EPSILON {
            self.make_identity();
            return;
        } else {
            z /= len;
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

    /// `Rotate_AABox_Extent(extent, &new_extent)`: "push each extent out to the projections of the original extents".
    pub fn rotate_aabox_extent(&self, extent: &Vector3) -> Vector3 {
        let mut set_extent = Vector3::new(0.0, 0.0, 0.0);
        for i in 0..3 {
            set_extent[i] = 0.0f32;
            for j in 0..3 {
                set_extent[i] += wwmath::fabs(self.row[i][j] * extent[j]);
            }
        }
        set_extent
    }
}

impl Index<usize> for Matrix3x3 {
    type Output = Vector3;
    fn index(&self, i: usize) -> &Vector3 {
        &self.row[i]
    }
}

impl IndexMut<usize> for Matrix3x3 {
    fn index_mut(&mut self, i: usize) -> &mut Vector3 {
        &mut self.row[i]
    }
}

/// `Create_X_Rotation_Matrix3(s, c)`.
pub fn create_x_rotation_matrix3(s: f32, c: f32) -> Matrix3x3 {
    Matrix3x3::from_elements(1.0, 0.0, 0.0, 0.0, c, -s, 0.0, s, c)
}

/// `Create_X_Rotation_Matrix3(rad)`.
pub fn create_x_rotation_matrix3_rad(rad: f32) -> Matrix3x3 {
    create_x_rotation_matrix3(Real::sin(rad), Real::cos(rad))
}

/// `Create_Y_Rotation_Matrix3(s, c)`.
pub fn create_y_rotation_matrix3(s: f32, c: f32) -> Matrix3x3 {
    Matrix3x3::from_elements(c, 0.0, s, 0.0, 1.0, 0.0, -s, 0.0, c)
}

/// `Create_Y_Rotation_Matrix3(rad)`.
pub fn create_y_rotation_matrix3_rad(rad: f32) -> Matrix3x3 {
    create_y_rotation_matrix3(Real::sin(rad), Real::cos(rad))
}

/// `Create_Z_Rotation_Matrix3(s, c)`.
pub fn create_z_rotation_matrix3(s: f32, c: f32) -> Matrix3x3 {
    Matrix3x3::from_elements(c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0)
}

/// `Create_Z_Rotation_Matrix3(rad)`.
pub fn create_z_rotation_matrix3_rad(rad: f32) -> Matrix3x3 {
    create_z_rotation_matrix3(Real::sin(rad), Real::cos(rad))
}
