// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/vector4.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWMath)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `class Vector4`: only the subset that [`Matrix3D`](super::matrix3d::Matrix3D)
//! uses (its rows are `Vector4`s), namely construction, indexing and the
//! arithmetic operators.
//!
//! **Upstream defect, kept on purpose:** `operator -=` *adds* the `W`
//! components (`W += v.W`). `Matrix3D::Solve_Linear_System` row-reduces with
//! `-=`, so its fourth column inherits this. The differential test pins it.

use std::ops::{Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign};

/// `class Vector4`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vector4 {
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Vector4 { x, y, z, w }
    }

    /// `Set(float x, float y, float z, float w)`.
    #[inline]
    pub fn set(&mut self, x: f32, y: f32, z: f32, w: f32) {
        self.x = x;
        self.y = y;
        self.z = z;
        self.w = w;
    }

    /// `operator * (const Vector4 &a, const Vector4 &b)`: the dot product.
    #[inline]
    pub fn dot_product(a: &Vector4, b: &Vector4) -> f32 {
        a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w
    }
}

impl Index<usize> for Vector4 {
    type Output = f32;
    #[inline]
    fn index(&self, i: usize) -> &f32 {
        match i {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            3 => &self.w,
            _ => panic!("Vector4 index {i} out of range"),
        }
    }
}

impl IndexMut<usize> for Vector4 {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut f32 {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            3 => &mut self.w,
            _ => panic!("Vector4 index {i} out of range"),
        }
    }
}

impl Neg for Vector4 {
    type Output = Vector4;
    #[inline]
    fn neg(self) -> Vector4 {
        Vector4::new(-self.x, -self.y, -self.z, -self.w)
    }
}

impl AddAssign for Vector4 {
    #[inline]
    fn add_assign(&mut self, v: Vector4) {
        self.x += v.x;
        self.y += v.y;
        self.z += v.z;
        self.w += v.w;
    }
}

impl SubAssign for Vector4 {
    /// Upstream: `X -= v.X; Y -= v.Y; Z -= v.Z; W += v.W;` (sic).
    #[inline]
    fn sub_assign(&mut self, v: Vector4) {
        self.x -= v.x;
        self.y -= v.y;
        self.z -= v.z;
        self.w += v.w;
    }
}

impl MulAssign<f32> for Vector4 {
    #[inline]
    fn mul_assign(&mut self, k: f32) {
        self.x *= k;
        self.y *= k;
        self.z *= k;
        self.w *= k;
    }
}

impl DivAssign<f32> for Vector4 {
    /// `k = 1.0f/k`, then multiply.
    #[inline]
    fn div_assign(&mut self, k: f32) {
        let k = 1.0f32 / k;
        self.x *= k;
        self.y *= k;
        self.z *= k;
        self.w *= k;
    }
}

impl Mul<f32> for Vector4 {
    type Output = Vector4;
    #[inline]
    fn mul(self, k: f32) -> Vector4 {
        Vector4::new(self.x * k, self.y * k, self.z * k, self.w * k)
    }
}

impl Mul<Vector4> for f32 {
    type Output = Vector4;
    /// Upstream: `return a*k;`.
    #[inline]
    fn mul(self, a: Vector4) -> Vector4 {
        a * self
    }
}

impl Mul<Vector4> for Vector4 {
    type Output = f32;
    #[inline]
    fn mul(self, b: Vector4) -> f32 {
        Vector4::dot_product(&self, &b)
    }
}

impl Div<f32> for Vector4 {
    type Output = Vector4;
    #[inline]
    fn div(self, k: f32) -> Vector4 {
        let ook = 1.0f32 / k;
        Vector4::new(self.x * ook, self.y * ook, self.z * ook, self.w * ook)
    }
}

impl Add for Vector4 {
    type Output = Vector4;
    #[inline]
    fn add(self, b: Vector4) -> Vector4 {
        Vector4::new(self.x + b.x, self.y + b.y, self.z + b.z, self.w + b.w)
    }
}

impl Sub for Vector4 {
    type Output = Vector4;
    #[inline]
    fn sub(self, b: Vector4) -> Vector4 {
        Vector4::new(self.x - b.x, self.y - b.y, self.z - b.z, self.w - b.w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_assign_keeps_upstream_w_defect() {
        let mut a = Vector4::new(5.0, 5.0, 5.0, 5.0);
        a -= Vector4::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(a, Vector4::new(4.0, 3.0, 2.0, 9.0));
        assert_eq!(a[3], 9.0);
    }
}
