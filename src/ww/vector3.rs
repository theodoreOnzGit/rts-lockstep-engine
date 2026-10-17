// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/vector3.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWMath, author Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `class Vector3`, with upstream's operation order kept exactly.
//!
//! Kept as upstream: [`Vector3::quick_length`]'s third swap assigns
//! `min = mid` *after* `mid = min`, so it does not swap. `Rotate_X/Y/Z(angle)`
//! call `sinf`/`cosf` directly (here: petir's `f32` `Real`).
//!
//! Not translated: `Convert_To_ABGR`/`Convert_To_ARGB` (client-side colour
//! packing whose float-to-unsigned casts are undefined out of range), and the
//! `ALLOW_TEMPORARIES` variants (that macro is never defined upstream).

use super::wwmath;
use std::ops::{Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign};
use petir::Real;

/// `class Vector3`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Vector3 { x, y, z }
    }

    /// `Set(float x, float y, float z)`.
    #[inline]
    pub fn set(&mut self, x: f32, y: f32, z: f32) {
        self.x = x;
        self.y = y;
        self.z = z;
    }

    /// `Normalize()`: multiply by `Inv_Sqrt(Length2())` unless the length is 0.
    #[inline]
    pub fn normalize(&mut self) {
        let len2 = self.length2();
        if len2 != 0.0f32 {
            let oolen = wwmath::inv_sqrt(len2);
            self.x *= oolen;
            self.y *= oolen;
            self.z *= oolen;
        }
    }

    /// `Length()`: `WWMath::Sqrt(Length2())`.
    #[inline]
    pub fn length(&self) -> f32 {
        wwmath::sqrt(self.length2())
    }

    /// `Length2()`: `X*X + Y*Y + Z*Z`.
    #[inline]
    pub fn length2(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// `Quick_Length()` (Graphics Gems 1, "+/- 8%"), quirk included.
    #[inline]
    pub fn quick_length(&self) -> f32 {
        let mut max = wwmath::fabs(self.x);
        let mut mid = wwmath::fabs(self.y);
        let mut min = wwmath::fabs(self.z);
        let mut tmp;
        if max < mid {
            tmp = max;
            max = mid;
            mid = tmp;
        }
        if max < min {
            tmp = max;
            max = min;
            min = tmp;
        }
        // Upstream: `tmp = mid; mid = min; min = mid;` -- not a swap (tmp is
        // unused), kept as written; clippy's swap suggestion would change it.
        #[allow(clippy::almost_swapped)]
        if mid < min {
            tmp = mid;
            let _ = tmp;
            mid = min;
            min = mid;
        }
        max + (11.0f32 / 32.0f32) * mid + (1.0f32 / 4.0f32) * min
    }

    /// `Scale(const Vector3 & scale)`: component-wise multiply.
    #[inline]
    pub fn scale(&mut self, scale: &Vector3) {
        self.x *= scale.x;
        self.y *= scale.y;
        self.z *= scale.z;
    }

    /// `Rotate_X(float angle)`: `Rotate_X(sinf(angle), cosf(angle))`.
    #[inline]
    pub fn rotate_x(&mut self, angle: f32) {
        self.rotate_x_sc(Real::sin(angle), Real::cos(angle));
    }

    /// `Rotate_X(float s_angle, float c_angle)`.
    #[inline]
    pub fn rotate_x_sc(&mut self, s_angle: f32, c_angle: f32) {
        let tmp_y = self.y;
        let tmp_z = self.z;
        self.y = c_angle * tmp_y - s_angle * tmp_z;
        self.z = s_angle * tmp_y + c_angle * tmp_z;
    }

    /// `Rotate_Y(float angle)`.
    #[inline]
    pub fn rotate_y(&mut self, angle: f32) {
        self.rotate_y_sc(Real::sin(angle), Real::cos(angle));
    }

    /// `Rotate_Y(float s_angle, float c_angle)`.
    #[inline]
    pub fn rotate_y_sc(&mut self, s_angle: f32, c_angle: f32) {
        let tmp_x = self.x;
        let tmp_z = self.z;
        self.x = c_angle * tmp_x + s_angle * tmp_z;
        self.z = -s_angle * tmp_x + c_angle * tmp_z;
    }

    /// `Rotate_Z(float angle)`.
    #[inline]
    pub fn rotate_z(&mut self, angle: f32) {
        self.rotate_z_sc(Real::sin(angle), Real::cos(angle));
    }

    /// `Rotate_Z(float s_angle, float c_angle)`.
    #[inline]
    pub fn rotate_z_sc(&mut self, s_angle: f32, c_angle: f32) {
        let tmp_x = self.x;
        let tmp_y = self.y;
        self.x = c_angle * tmp_x - s_angle * tmp_y;
        self.y = s_angle * tmp_x + c_angle * tmp_y;
    }

    /// `Dot_Product(a, b)`.
    #[inline]
    pub fn dot_product(a: &Vector3, b: &Vector3) -> f32 {
        a.x * b.x + a.y * b.y + a.z * b.z
    }

    /// `Cross_Product(a, b, &result)`.
    #[inline]
    pub fn cross_product(a: &Vector3, b: &Vector3) -> Vector3 {
        Vector3::new(
            a.y * b.z - a.z * b.y,
            a.z * b.x - a.x * b.z,
            a.x * b.y - a.y * b.x,
        )
    }

    /// `Normalized_Cross_Product(a, b, &result)`.
    #[inline]
    pub fn normalized_cross_product(a: &Vector3, b: &Vector3) -> Vector3 {
        let mut r = Vector3::cross_product(a, b);
        r.normalize();
        r
    }

    /// `Cross_Product_X(a, b)`.
    #[inline]
    pub fn cross_product_x(a: &Vector3, b: &Vector3) -> f32 {
        a.y * b.z - a.z * b.y
    }

    /// `Cross_Product_Y(a, b)`.
    #[inline]
    pub fn cross_product_y(a: &Vector3, b: &Vector3) -> f32 {
        a.z * b.x - a.x * b.z
    }

    /// `Cross_Product_Z(a, b)`.
    #[inline]
    pub fn cross_product_z(a: &Vector3, b: &Vector3) -> f32 {
        a.x * b.y - a.y * b.x
    }

    /// `Add(a, b, &c)`.
    #[inline]
    pub fn add_vectors(a: &Vector3, b: &Vector3) -> Vector3 {
        Vector3::new(a.x + b.x, a.y + b.y, a.z + b.z)
    }

    /// `Subtract(a, b, &c)`.
    #[inline]
    pub fn subtract_vectors(a: &Vector3, b: &Vector3) -> Vector3 {
        Vector3::new(a.x - b.x, a.y - b.y, a.z - b.z)
    }

    /// `Find_X_At_Y(y, p1, p2)`.
    #[inline]
    pub fn find_x_at_y(y: f32, p1: &Vector3, p2: &Vector3) -> f32 {
        p1.x + ((y - p1.y) * ((p2.x - p1.x) / (p2.y - p1.y)))
    }

    /// `Find_X_At_Z(z, p1, p2)`.
    #[inline]
    pub fn find_x_at_z(z: f32, p1: &Vector3, p2: &Vector3) -> f32 {
        p1.x + ((z - p1.z) * ((p2.x - p1.x) / (p2.z - p1.z)))
    }

    /// `Find_Y_At_X(x, p1, p2)`.
    #[inline]
    pub fn find_y_at_x(x: f32, p1: &Vector3, p2: &Vector3) -> f32 {
        p1.y + ((x - p1.x) * ((p2.y - p1.y) / (p2.x - p1.x)))
    }

    /// `Find_Y_At_Z(z, p1, p2)`.
    #[inline]
    pub fn find_y_at_z(z: f32, p1: &Vector3, p2: &Vector3) -> f32 {
        p1.y + ((z - p1.z) * ((p2.y - p1.y) / (p2.z - p1.z)))
    }

    /// `Find_Z_At_X(x, p1, p2)`.
    #[inline]
    pub fn find_z_at_x(x: f32, p1: &Vector3, p2: &Vector3) -> f32 {
        p1.z + ((x - p1.x) * ((p2.z - p1.z) / (p2.x - p1.x)))
    }

    /// `Find_Z_At_Y(y, p1, p2)`.
    #[inline]
    pub fn find_z_at_y(y: f32, p1: &Vector3, p2: &Vector3) -> f32 {
        p1.z + ((y - p1.y) * ((p2.z - p1.z) / (p2.y - p1.y)))
    }

    /// `Update_Min(a)`.
    #[inline]
    pub fn update_min(&mut self, a: &Vector3) {
        if a.x < self.x {
            self.x = a.x;
        }
        if a.y < self.y {
            self.y = a.y;
        }
        if a.z < self.z {
            self.z = a.z;
        }
    }

    /// `Update_Max(a)`.
    #[inline]
    pub fn update_max(&mut self, a: &Vector3) {
        if a.x > self.x {
            self.x = a.x;
        }
        if a.y > self.y {
            self.y = a.y;
        }
        if a.z > self.z {
            self.z = a.z;
        }
    }

    /// `Cap_Absolute_To(a)`.
    #[inline]
    pub fn cap_absolute_to(&mut self, a: &Vector3) {
        if self.x > 0.0 {
            if a.x < self.x {
                self.x = a.x;
            }
        } else if -a.x > self.x {
            self.x = -a.x;
        }
        if self.y > 0.0 {
            if a.y < self.y {
                self.y = a.y;
            }
        } else if -a.y > self.y {
            self.y = -a.y;
        }
        if self.z > 0.0 {
            if a.z < self.z {
                self.z = a.z;
            }
        } else if -a.z > self.z {
            self.z = -a.z;
        }
    }

    /// `Is_Valid()`: no component is inf or NaN.
    #[inline]
    pub fn is_valid(&self) -> bool {
        wwmath::is_valid_float(self.x) && wwmath::is_valid_float(self.y) && wwmath::is_valid_float(self.z)
    }

    /// `Quick_Distance(p1, p2)`.
    #[inline]
    pub fn quick_distance(p1: &Vector3, p2: &Vector3) -> f32 {
        let temp = *p1 - *p2;
        temp.quick_length()
    }

    /// `Distance(p1, p2)`.
    #[inline]
    pub fn distance(p1: &Vector3, p2: &Vector3) -> f32 {
        let temp = *p1 - *p2;
        temp.length()
    }

    /// `Lerp(a, b, alpha, &result)`.
    #[inline]
    pub fn lerp(a: &Vector3, b: &Vector3, alpha: f32) -> Vector3 {
        Vector3::new(
            a.x + (b.x - a.x) * alpha,
            a.y + (b.y - a.y) * alpha,
            a.z + (b.z - a.z) * alpha,
        )
    }
}

/// `Equal_Within_Epsilon(a, b, epsilon)`.
#[inline]
pub fn equal_within_epsilon(a: &Vector3, b: &Vector3, epsilon: f32) -> bool {
    wwmath::fabs(a.x - b.x) < epsilon && wwmath::fabs(a.y - b.y) < epsilon && wwmath::fabs(a.z - b.z) < epsilon
}

impl Index<usize> for Vector3 {
    type Output = f32;
    #[inline]
    fn index(&self, i: usize) -> &f32 {
        match i {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            _ => panic!("Vector3 index {i} out of range"),
        }
    }
}

impl IndexMut<usize> for Vector3 {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut f32 {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            _ => panic!("Vector3 index {i} out of range"),
        }
    }
}

impl Neg for Vector3 {
    type Output = Vector3;
    #[inline]
    fn neg(self) -> Vector3 {
        Vector3::new(-self.x, -self.y, -self.z)
    }
}

impl AddAssign for Vector3 {
    #[inline]
    fn add_assign(&mut self, v: Vector3) {
        self.x += v.x;
        self.y += v.y;
        self.z += v.z;
    }
}

impl SubAssign for Vector3 {
    #[inline]
    fn sub_assign(&mut self, v: Vector3) {
        self.x -= v.x;
        self.y -= v.y;
        self.z -= v.z;
    }
}

impl MulAssign<f32> for Vector3 {
    #[inline]
    fn mul_assign(&mut self, k: f32) {
        self.x *= k;
        self.y *= k;
        self.z *= k;
    }
}

impl DivAssign<f32> for Vector3 {
    /// `ook = 1.0f/k`, then multiply.
    #[inline]
    fn div_assign(&mut self, k: f32) {
        let ook = 1.0f32 / k;
        self.x *= ook;
        self.y *= ook;
        self.z *= ook;
    }
}

impl Mul<f32> for Vector3 {
    type Output = Vector3;
    #[inline]
    fn mul(self, k: f32) -> Vector3 {
        Vector3::new(self.x * k, self.y * k, self.z * k)
    }
}

impl Mul<Vector3> for f32 {
    type Output = Vector3;
    /// `k * a` is computed as `a.X * k`, as upstream.
    #[inline]
    fn mul(self, a: Vector3) -> Vector3 {
        Vector3::new(a.x * self, a.y * self, a.z * self)
    }
}

impl Div<f32> for Vector3 {
    type Output = Vector3;
    #[inline]
    fn div(self, k: f32) -> Vector3 {
        let ook = 1.0f32 / k;
        Vector3::new(self.x * ook, self.y * ook, self.z * ook)
    }
}

impl Add for Vector3 {
    type Output = Vector3;
    #[inline]
    fn add(self, b: Vector3) -> Vector3 {
        Vector3::new(self.x + b.x, self.y + b.y, self.z + b.z)
    }
}

impl Sub for Vector3 {
    type Output = Vector3;
    #[inline]
    fn sub(self, b: Vector3) -> Vector3 {
        Vector3::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        let v = Vector3::new(1.0, 2.0, 2.0);
        assert_eq!(v.length(), 3.0);
        let c = Vector3::cross_product(&Vector3::new(1.0, 0.0, 0.0), &Vector3::new(0.0, 1.0, 0.0));
        assert_eq!(c, Vector3::new(0.0, 0.0, 1.0));
        // max=3, mid=1, min=2: the broken third swap gives mid=min=2.
        assert_eq!(Vector3::new(3.0, 1.0, 2.0).quick_length(), 3.0 + (11.0 / 32.0) * 2.0 + 0.25 * 2.0);
        assert_eq!(Vector3::new(4.0, 5.0, 6.0)[2], 6.0);
    }
}
