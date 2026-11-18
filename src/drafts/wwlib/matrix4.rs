//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/matrix4.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/matrix4.cpp
//!
//! `class Matrix4x4`. Upstream defects kept: `operator /= (float d)` MULTIPLIES by `d` (`float ood = d;`), unlike
//! `operator /` which multiplies by `1/d`; `Inverse()` asserts `"Matrix4x4::Inverse does not work, re-implement!"`
//! (debug builds only, so it runs in release); `operator ==` compares the 16 floats' bit patterns, not their values.
//! (`crate::client::tactical_view::Matrix4x4` is the client's own, unrelated 4x4.)

use super::matrix3::Matrix3x3;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use crate::ww::vector4::Vector4;
use crate::ww::wwmath;
use std::ops::{Index, IndexMut};

/// `class Matrix4x4`: `Vector4 Row[4]`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Matrix4x4 {
    pub row: [Vector4; 4],
}

impl Matrix4x4 {
    /// `Matrix4x4(bool identity)`: identity when `true`; uninitialised upstream (zero here) when `false`.
    pub fn new(identity: bool) -> Self {
        let mut m = Matrix4x4::default();
        if identity {
            m.make_identity();
        }
        m
    }

    /// `Matrix4x4(const Matrix3D &)` -> `Init(m)`.
    pub fn from_matrix3d(m: &Matrix3D) -> Self {
        let mut r = Matrix4x4::default();
        r.init_matrix3d(m);
        r
    }

    /// `Matrix4x4(r0, r1, r2, r3)`.
    pub fn from_rows(r0: &Vector4, r1: &Vector4, r2: &Vector4, r3: &Vector4) -> Self {
        Matrix4x4 { row: [*r0, *r1, *r2, *r3] }
    }

    /// `Matrix4x4(m11 .. m44)`.
    #[allow(clippy::too_many_arguments)]
    pub fn from_elements(m11: f32, m12: f32, m13: f32, m14: f32, m21: f32, m22: f32, m23: f32, m24: f32, m31: f32, m32: f32, m33: f32, m34: f32, m41: f32, m42: f32, m43: f32, m44: f32) -> Self {
        let mut r = Matrix4x4::default();
        r.init_elements(m11, m12, m13, m14, m21, m22, m23, m24, m31, m32, m33, m34, m41, m42, m43, m44);
        r
    }

    /// `Make_Identity()`.
    pub fn make_identity(&mut self) {
        self.row[0].set(1.0, 0.0, 0.0, 0.0);
        self.row[1].set(0.0, 1.0, 0.0, 0.0);
        self.row[2].set(0.0, 0.0, 1.0, 0.0);
        self.row[3].set(0.0, 0.0, 0.0, 1.0);
    }

    /// `Init(const Matrix3D &)`: `Row[0..3] = m[0..3]`, `Row[3] = (0,0,0,1)`.
    pub fn init_matrix3d(&mut self, m: &Matrix3D) {
        self.row[0] = m[0];
        self.row[1] = m[1];
        self.row[2] = m[2];
        self.row[3] = Vector4::new(0.0, 0.0, 0.0, 1.0);
    }

    /// `Init(r0, r1, r2, r3)`.
    pub fn init_rows(&mut self, r0: &Vector4, r1: &Vector4, r2: &Vector4, r3: &Vector4) {
        self.row = [*r0, *r1, *r2, *r3];
    }

    /// `Init(m11 .. m44)`.
    #[allow(clippy::too_many_arguments)]
    pub fn init_elements(&mut self, m11: f32, m12: f32, m13: f32, m14: f32, m21: f32, m22: f32, m23: f32, m24: f32, m31: f32, m32: f32, m33: f32, m34: f32, m41: f32, m42: f32, m43: f32, m44: f32) {
        self.row[0].set(m11, m12, m13, m14);
        self.row[1].set(m21, m22, m23, m24);
        self.row[2].set(m31, m32, m33, m34);
        self.row[3].set(m41, m42, m43, m44);
    }

    /// `Init(const Matrix3x3 &)` is declared in matrix4.h but never defined upstream (a link error if called); the
    /// natural reading is the 3x3 in the upper left with the rest identity, which is NOT upstream: not translated.
    #[allow(dead_code)]
    fn init_matrix3x3_declared_only(_m: &Matrix3x3) {}

    /// `Init_Ortho(left, right, bottom, top, znear, zfar)` (asserts `znear >= 0`, `zfar > znear`).
    pub fn init_ortho(&mut self, left: f32, right: f32, bottom: f32, top: f32, znear: f32, zfar: f32) {
        assert!(znear >= 0.0f32);
        assert!(zfar > znear);

        self.make_identity();
        self.row[0][0] = 2.0f32 / (right - left);
        self.row[0][3] = -(right + left) / (right - left);
        self.row[1][1] = 2.0f32 / (top - bottom);
        self.row[1][3] = -(top + bottom) / (top - bottom);
        self.row[2][2] = -2.0f32 / (zfar - znear);
        self.row[2][3] = -(zfar + znear) / (zfar - znear);
    }

    /// `Init_Perspective(hfov, vfov, znear, zfar)`: `static_cast<float>(1.0 / tan(hfov*0.5))` in double (the C library's
    /// `tan`, here `f64::tan`).
    pub fn init_perspective_fov(&mut self, hfov: f32, vfov: f32, znear: f32, zfar: f32) {
        assert!(znear > 0.0f32);
        assert!(zfar > znear);

        self.make_identity();
        self.row[0][0] = (1.0f64 / (hfov as f64 * 0.5f64).tan()) as f32;
        self.row[1][1] = (1.0f64 / (vfov as f64 * 0.5f64).tan()) as f32;
        self.row[2][2] = -(zfar + znear) / (zfar - znear);
        self.row[2][3] = (-(2.0f64 * zfar as f64 * znear as f64) / (zfar - znear) as f64) as f32;
        self.row[3][2] = -1.0f32;
        self.row[3][3] = 0.0f32;
    }

    /// `Init_Perspective(left, right, bottom, top, znear, zfar)`.
    pub fn init_perspective(&mut self, left: f32, right: f32, bottom: f32, top: f32, znear: f32, zfar: f32) {
        assert!(znear > 0.0f32);
        assert!(zfar > 0.0f32);

        self.make_identity();
        self.row[0][0] = (2.0f64 * znear as f64 / (right - left) as f64) as f32;
        self.row[0][2] = (right + left) / (right - left);
        self.row[1][1] = (2.0f64 * znear as f64 / (top - bottom) as f64) as f32;
        self.row[1][2] = (top + bottom) / (top - bottom);
        self.row[2][2] = -(zfar + znear) / (zfar - znear);
        self.row[2][3] = (-(2.0f64 * zfar as f64 * znear as f64) / (zfar - znear) as f64) as f32;
        self.row[3][2] = -1.0f32;
        self.row[3][3] = 0.0f32;
    }

    /// `Transpose()`.
    pub fn transpose(&self) -> Matrix4x4 {
        let r = &self.row;
        Matrix4x4::from_rows(
            &Vector4::new(r[0][0], r[1][0], r[2][0], r[3][0]),
            &Vector4::new(r[0][1], r[1][1], r[2][1], r[3][1]),
            &Vector4::new(r[0][2], r[1][2], r[2][2], r[3][2]),
            &Vector4::new(r[0][3], r[1][3], r[2][3], r[3][3]),
        )
    }

    /// `Inverse()`: Gauss-Jordan with partial pivoting; upstream asserts "does not work, re-implement!" (debug only).
    pub fn inverse(&self) -> Matrix4x4 {
        let mut a = *self;
        let mut b = Matrix4x4::new(true);

        for j in 0..4 {
            let mut i1 = j;
            for i in (j + 1)..4 {
                if wwmath::fabs(a.row[i][j]) > wwmath::fabs(a.row[i1][j]) {
                    i1 = i;
                }
            }

            a.row.swap(i1, j);
            b.row.swap(i1, j);

            // (if a[j][j]==0. upstream does nothing)
            let bd = a.row[j][j];
            b.row[j] /= bd;
            let ad = a.row[j][j];
            a.row[j] /= ad;

            for i in 0..4 {
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

    /// `operator += (const Matrix4x4 &)`.
    pub fn add_assign(&mut self, m: &Matrix4x4) {
        for i in 0..4 {
            self.row[i] += m.row[i];
        }
    }

    /// `operator -= (const Matrix4x4 &)`.
    pub fn sub_assign(&mut self, m: &Matrix4x4) {
        for i in 0..4 {
            self.row[i] -= m.row[i];
        }
    }

    /// `operator *= (float)`.
    pub fn mul_assign_scalar(&mut self, d: f32) {
        for i in 0..4 {
            self.row[i] *= d;
        }
    }

    /// `operator /= (float d)`: UPSTREAM DEFECT KEPT, it multiplies by `d` (`float ood = d; Row[i] *= ood;`).
    pub fn div_assign_scalar(&mut self, d: f32) {
        let ood = d;
        for i in 0..4 {
            self.row[i] *= ood;
        }
    }

    /// `operator - (a)`.
    pub fn negated(&self) -> Matrix4x4 {
        Matrix4x4::from_rows(&-self.row[0], &-self.row[1], &-self.row[2], &-self.row[3])
    }

    /// `operator * (a, d)` / `operator * (d, a)`.
    pub fn scaled(&self, d: f32) -> Matrix4x4 {
        Matrix4x4::from_rows(&(self.row[0] * d), &(self.row[1] * d), &(self.row[2] * d), &(self.row[3] * d))
    }

    /// `operator / (a, d)`: multiplies by `ood = 1.0f / d`.
    pub fn divided(&self, d: f32) -> Matrix4x4 {
        let ood = 1.0f32 / d;
        Matrix4x4::from_rows(&(self.row[0] * ood), &(self.row[1] * ood), &(self.row[2] * ood), &(self.row[3] * ood))
    }

    /// `operator + (a, b)` / `Add(a, b)`.
    pub fn add(a: &Matrix4x4, b: &Matrix4x4) -> Matrix4x4 {
        Matrix4x4::from_rows(&(a.row[0] + b.row[0]), &(a.row[1] + b.row[1]), &(a.row[2] + b.row[2]), &(a.row[3] + b.row[3]))
    }

    /// `operator - (a, b)` / `Subtract(a, b)`.
    pub fn subtract(a: &Matrix4x4, b: &Matrix4x4) -> Matrix4x4 {
        Matrix4x4::from_rows(&(a.row[0] - b.row[0]), &(a.row[1] - b.row[1]), &(a.row[2] - b.row[2]), &(a.row[3] - b.row[3]))
    }

    /// `operator * (a, b)` / `Multiply(a, b)` (matrix4.h): `ROWCOL(i,j) = a[i][0]*b[0][j] + .. + a[i][3]*b[3][j]`.
    pub fn mul_matrix(a: &Matrix4x4, b: &Matrix4x4) -> Matrix4x4 {
        let rc = |i: usize, j: usize| a.row[i][0] * b.row[0][j] + a.row[i][1] * b.row[1][j] + a.row[i][2] * b.row[2][j] + a.row[i][3] * b.row[3][j];
        Matrix4x4::from_rows(
            &Vector4::new(rc(0, 0), rc(0, 1), rc(0, 2), rc(0, 3)),
            &Vector4::new(rc(1, 0), rc(1, 1), rc(1, 2), rc(1, 3)),
            &Vector4::new(rc(2, 0), rc(2, 1), rc(2, 2), rc(2, 3)),
            &Vector4::new(rc(3, 0), rc(3, 1), rc(3, 2), rc(3, 3)),
        )
    }

    /// `operator * (const Matrix4x4 & a, const Matrix3D & b)` (matrix4.h): "hand coded to handle the last row of b as
    /// 0,0,0,1".
    pub fn mul_matrix_matrix3d(a: &Matrix4x4, b: &Matrix3D) -> Matrix4x4 {
        let rc = |i: usize, j: usize| a.row[i][0] * b[0][j] + a.row[i][1] * b[1][j] + a.row[i][2] * b[2][j];
        let rc_last = |i: usize, j: usize| a.row[i][0] * b[0][j] + a.row[i][1] * b[1][j] + a.row[i][2] * b[2][j] + a.row[i][3];
        Matrix4x4::from_rows(
            &Vector4::new(rc(0, 0), rc(0, 1), rc(0, 2), rc_last(0, 3)),
            &Vector4::new(rc(1, 0), rc(1, 1), rc(1, 2), rc_last(1, 3)),
            &Vector4::new(rc(2, 0), rc(2, 1), rc(2, 2), rc_last(2, 3)),
            &Vector4::new(rc(3, 0), rc(3, 1), rc(3, 2), rc_last(3, 3)),
        )
    }

    /// `operator * (const Matrix3D & a, const Matrix4x4 & b)` (matrix4.h): "hand coded to handle the last row of a as
    /// 0,0,0,1"; the fourth row is `b`'s.
    pub fn mul_matrix3d_matrix(a: &Matrix3D, b: &Matrix4x4) -> Matrix4x4 {
        let rc = |i: usize, j: usize| a[i][0] * b.row[0][j] + a[i][1] * b.row[1][j] + a[i][2] * b.row[2][j] + a[i][3] * b.row[3][j];
        Matrix4x4::from_rows(
            &Vector4::new(rc(0, 0), rc(0, 1), rc(0, 2), rc(0, 3)),
            &Vector4::new(rc(1, 0), rc(1, 1), rc(1, 2), rc(1, 3)),
            &Vector4::new(rc(2, 0), rc(2, 1), rc(2, 2), rc(2, 3)),
            &Vector4::new(b.row[3][0], b.row[3][1], b.row[3][2], b.row[3][3]),
        )
    }

    /// `Matrix4x4::Multiply(A, B, &res)` (matrix4.cpp:38): same sums as `mul_matrix`; upstream asserts `res` is neither.
    pub fn multiply(a: &Matrix4x4, b: &Matrix4x4) -> Matrix4x4 {
        Matrix4x4::mul_matrix(a, b)
    }

    /// `Matrix4x4::Multiply(const Matrix3D & a, const Matrix4x4 & b, &res)` (matrix4.cpp:67): rows 0..2 as
    /// `ROWCOL`, "last row of a is 0,0,0,1; this leaves the last row of b unchanged".
    pub fn multiply_matrix3d_matrix(a: &Matrix3D, b: &Matrix4x4) -> Matrix4x4 {
        Matrix4x4::mul_matrix3d_matrix(a, b)
    }

    /// `Matrix4x4::Multiply(const Matrix4x4 & a, const Matrix3D & b, &res)` (matrix4.cpp:95): `ROWCOL4` carries `a[i][3]`
    /// into the fourth column for all four rows.
    pub fn multiply_matrix_matrix3d(a: &Matrix4x4, b: &Matrix3D) -> Matrix4x4 {
        Matrix4x4::mul_matrix_matrix3d(a, b)
    }

    /// `operator * (const Matrix4x4 & a, const Vector3 & v)`: `v` extended with `w = 1.0f`.
    pub fn mul_vector3(&self, v: &Vector3) -> Vector4 {
        let a = &self.row;
        Vector4::new(
            a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2] + a[0][3] * 1.0f32,
            a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2] + a[1][3] * 1.0f32,
            a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2] + a[2][3] * 1.0f32,
            a[3][0] * v[0] + a[3][1] * v[1] + a[3][2] * v[2] + a[3][3] * 1.0f32,
        )
    }

    /// `operator * (const Matrix4x4 & a, const Vector4 & v)`.
    pub fn mul_vector4(&self, v: &Vector4) -> Vector4 {
        let a = &self.row;
        Vector4::new(
            a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2] + a[0][3] * v[3],
            a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2] + a[1][3] * v[3],
            a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2] + a[2][3] * v[3],
            a[3][0] * v[0] + a[3][1] * v[1] + a[3][2] * v[2] + a[3][3] * v[3],
        )
    }

    /// `Transform_Vector(tm, Vector3 in, Vector3 * out)`: the translation column is added, no divide.
    pub fn transform_vector3(a: &Matrix4x4, v: &Vector3) -> Vector3 {
        Vector3::new(
            a.row[0][0] * v.x + a.row[0][1] * v.y + a.row[0][2] * v.z + a.row[0][3],
            a.row[1][0] * v.x + a.row[1][1] * v.y + a.row[1][2] * v.z + a.row[1][3],
            a.row[2][0] * v.x + a.row[2][1] * v.y + a.row[2][2] * v.z + a.row[2][3],
        )
    }

    /// `Transform_Vector(tm, Vector3 in, Vector4 * out)`: `W = 1.0f` (not the matrix's fourth row).
    pub fn transform_vector3_to_vector4(a: &Matrix4x4, v: &Vector3) -> Vector4 {
        Vector4::new(
            a.row[0][0] * v.x + a.row[0][1] * v.y + a.row[0][2] * v.z + a.row[0][3],
            a.row[1][0] * v.x + a.row[1][1] * v.y + a.row[1][2] * v.z + a.row[1][3],
            a.row[2][0] * v.x + a.row[2][1] * v.y + a.row[2][2] * v.z + a.row[2][3],
            1.0f32,
        )
    }

    /// `Transform_Vector(tm, Vector4 in, Vector4 * out)`.
    pub fn transform_vector4(a: &Matrix4x4, v: &Vector4) -> Vector4 {
        a.mul_vector4(v)
    }
}

impl PartialEq for Matrix4x4 {
    /// `operator == (a, b)` (matrix4.cpp:123): XOR of the sixteen 32-bit patterns; equal when all are identical (so
    /// `0.0 != -0.0` and a NaN equals itself when the bits match).
    fn eq(&self, other: &Matrix4x4) -> bool {
        let mut res: u32 = 0;
        for i in 0..4 {
            for j in 0..4 {
                res |= self.row[i][j].to_bits() ^ other.row[i][j].to_bits();
            }
        }
        res == 0
    }
}

impl Index<usize> for Matrix4x4 {
    type Output = Vector4;
    fn index(&self, i: usize) -> &Vector4 {
        &self.row[i]
    }
}

impl IndexMut<usize> for Matrix4x4 {
    fn index_mut(&mut self, i: usize) -> &mut Vector4 {
        &mut self.row[i]
    }
}
