// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/vector2.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWMath)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `class Vector2`. Every expression keeps upstream's operation order, so the
//! rounding is the same. Two upstream oddities are kept as they are:
//! the free [`normalize`] *divides* by the inverse length (so it scales by the
//! length squared over the length, i.e. multiplies by the length), and
//! [`quick_distance_xy`] discards its `Fabs` calls, so it does not take
//! absolute values.

use super::wwmath;
use std::ops::{
    Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign,
};

/// `class Vector2` (`U`/`V` are upstream aliases of `X`/`Y`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl Vector2 {
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Vector2 { x, y }
    }

    /// `Set(float x, float y)`.
    #[inline]
    pub fn set(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    /// `Normalize()`: multiply by `Inv_Sqrt(Length2())` unless the length is 0.
    #[inline]
    pub fn normalize(&mut self) {
        let len2 = self.length2();
        if len2 != 0.0f32 {
            let oolen = wwmath::inv_sqrt(len2);
            self.x *= oolen;
            self.y *= oolen;
        }
    }

    /// `Length()`: `(float)WWMath::Sqrt(Length2())`.
    #[inline]
    pub fn length(&self) -> f32 {
        wwmath::sqrt(self.length2())
    }

    /// `Length2()`: `X*X + Y*Y`.
    #[inline]
    pub fn length2(&self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    /// `Dot_Product(a, b)`: `a*b`, i.e. `a.X*b.X + a.Y*b.Y`.
    #[inline]
    pub fn dot_product(a: &Vector2, b: &Vector2) -> f32 {
        a.x * b.x + a.y * b.y
    }

    /// `Perp_Dot_Product(a, b)`: `a.X * -b.Y + a.Y * b.X`.
    #[inline]
    pub fn perp_dot_product(a: &Vector2, b: &Vector2) -> f32 {
        a.x * -b.y + a.y * b.x
    }

    /// `Rotate(float theta)`: `Rotate(WWMath::Sin(theta), WWMath::Cos(theta))`.
    #[inline]
    pub fn rotate(&mut self, theta: f32) {
        self.rotate_sc(wwmath::sin(theta), wwmath::cos(theta));
    }

    /// `Rotate(float s, float c)`.
    #[inline]
    pub fn rotate_sc(&mut self, s: f32, c: f32) {
        let new_x = self.x * c + self.y * -s;
        let new_y = self.x * s + self.y * c;
        self.x = new_x;
        self.y = new_y;
    }

    /// `Rotate_Towards_Vector(target, max_theta, positive_turn)`.
    /// Returns `(reached_target, positive_turn)`.
    #[inline]
    pub fn rotate_towards_vector(&mut self, target: &Vector2, max_theta: f32) -> (bool, bool) {
        self.rotate_towards_vector_sc(target, wwmath::sin(max_theta), wwmath::cos(max_theta))
    }

    /// `Rotate_Towards_Vector(target, max_s, max_c, positive_turn)`.
    /// Returns `(reached_target, positive_turn)`.
    #[inline]
    pub fn rotate_towards_vector_sc(
        &mut self,
        target: &Vector2,
        max_s: f32,
        max_c: f32,
    ) -> (bool, bool) {
        let mut return_value = false;
        let positive_turn = Vector2::perp_dot_product(target, self) > 0.0f32;
        if Vector2::dot_product(self, target) >= max_c {
            *self = *target;
            return_value = true;
        } else if positive_turn {
            self.rotate_sc(max_s, max_c);
        } else {
            self.rotate_sc(-max_s, max_c);
        }
        (return_value, positive_turn)
    }

    /// `Is_Valid()`: neither component is inf or NaN.
    #[inline]
    pub fn is_valid(&self) -> bool {
        wwmath::is_valid_float(self.x) && wwmath::is_valid_float(self.y)
    }

    /// `Update_Min(a)`.
    #[inline]
    pub fn update_min(&mut self, a: &Vector2) {
        if a.x < self.x {
            self.x = a.x;
        }
        if a.y < self.y {
            self.y = a.y;
        }
    }

    /// `Update_Max(a)`.
    #[inline]
    pub fn update_max(&mut self, a: &Vector2) {
        if a.x > self.x {
            self.x = a.x;
        }
        if a.y > self.y {
            self.y = a.y;
        }
    }

    /// `Scale(float a, float b)`.
    #[inline]
    pub fn scale(&mut self, a: f32, b: f32) {
        self.x *= a;
        self.y *= b;
    }

    /// `Scale(const Vector2 & a)`.
    #[inline]
    pub fn scale_by(&mut self, a: &Vector2) {
        self.x *= a.x;
        self.y *= a.y;
    }

    /// `Distance(p1, p2)`: length of `p1 - p2`.
    #[inline]
    pub fn distance(a: &Vector2, b: &Vector2) -> f32 {
        let temp = *a - *b;
        temp.length()
    }

    /// `Quick_Distance(p1, p2)`: `::Quick_Distance(a.X, a.Y, b.X, b.Y)`.
    #[inline]
    pub fn quick_distance(a: &Vector2, b: &Vector2) -> f32 {
        quick_distance_xy(a.x, a.y, b.x, b.y)
    }

    /// `Lerp(a, b, t, set_result)`.
    #[inline]
    pub fn lerp(a: &Vector2, b: &Vector2, t: f32) -> Vector2 {
        Vector2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
    }
}

/// Free `Normalize(const Vector2 & vec)`. Upstream returns `vec / oolen`
/// (division by the *inverse* length), and `(0,0)` for a zero vector. Kept.
#[inline]
pub fn normalize(vec: &Vector2) -> Vector2 {
    let len2 = vec.length2();
    if len2 != 0.0f32 {
        let oolen = wwmath::inv_sqrt(len2);
        return *vec / oolen;
    }
    Vector2::new(0.0f32, 0.0f32)
}

/// `Equal_Within_Epsilon(a, b, epsilon)`.
#[inline]
pub fn equal_within_epsilon(a: &Vector2, b: &Vector2, epsilon: f32) -> bool {
    wwmath::fabs(a.x - b.x) < epsilon && wwmath::fabs(a.y - b.y) < epsilon
}

/// Free `Quick_Distance(x1, y1, x2, y2)` ("From Graphic Gems I").
///
/// Upstream calls `WWMath::Fabs(x_diff); WWMath::Fabs(y_diff);` and discards
/// the results, so the differences keep their signs. Kept as upstream.
#[inline]
pub fn quick_distance_xy(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let x_diff = x1 - x2;
    let y_diff = y1 - y2;
    if x_diff > y_diff {
        (y_diff / 2.0f32) + x_diff
    } else {
        (x_diff / 2.0f32) + y_diff
    }
}

/// Free `Distance(x1, y1, x2, y2)`.
#[inline]
pub fn distance_xy(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let x_diff = x1 - x2;
    let y_diff = y1 - y2;
    wwmath::sqrt((x_diff * x_diff) + (y_diff * y_diff))
}

impl Index<usize> for Vector2 {
    type Output = f32;
    #[inline]
    fn index(&self, i: usize) -> &f32 {
        match i {
            0 => &self.x,
            1 => &self.y,
            _ => panic!("Vector2 index {i} out of range"),
        }
    }
}

impl IndexMut<usize> for Vector2 {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut f32 {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            _ => panic!("Vector2 index {i} out of range"),
        }
    }
}

impl Neg for Vector2 {
    type Output = Vector2;
    #[inline]
    fn neg(self) -> Vector2 {
        Vector2::new(-self.x, -self.y)
    }
}

impl AddAssign for Vector2 {
    #[inline]
    fn add_assign(&mut self, v: Vector2) {
        self.x += v.x;
        self.y += v.y;
    }
}

impl SubAssign for Vector2 {
    #[inline]
    fn sub_assign(&mut self, v: Vector2) {
        self.x -= v.x;
        self.y -= v.y;
    }
}

impl MulAssign<f32> for Vector2 {
    /// `X = (float)(X*k); Y = (float)(Y*k);`
    #[inline]
    fn mul_assign(&mut self, k: f32) {
        self.x *= k;
        self.y *= k;
    }
}

impl DivAssign<f32> for Vector2 {
    /// `k = 1.0f/k; X *= k; Y *= k;`
    #[inline]
    fn div_assign(&mut self, k: f32) {
        let k = 1.0f32 / k;
        self.x *= k;
        self.y *= k;
    }
}

impl Mul<f32> for Vector2 {
    type Output = Vector2;
    #[inline]
    fn mul(self, k: f32) -> Vector2 {
        Vector2::new(self.x * k, self.y * k)
    }
}

impl Mul<Vector2> for f32 {
    type Output = Vector2;
    /// `k * a` is computed as `a[i] * k`, as upstream.
    #[inline]
    fn mul(self, a: Vector2) -> Vector2 {
        Vector2::new(a.x * self, a.y * self)
    }
}

impl Mul<Vector2> for Vector2 {
    type Output = f32;
    /// `operator * (Vector2, Vector2)`: the dot product.
    #[inline]
    fn mul(self, b: Vector2) -> f32 {
        Vector2::dot_product(&self, &b)
    }
}

impl Div<f32> for Vector2 {
    type Output = Vector2;
    /// `ook = 1.0f/k`, then multiply.
    #[inline]
    fn div(self, k: f32) -> Vector2 {
        let ook = 1.0f32 / k;
        Vector2::new(self.x * ook, self.y * ook)
    }
}

impl Add for Vector2 {
    type Output = Vector2;
    #[inline]
    fn add(self, b: Vector2) -> Vector2 {
        Vector2::new(self.x + b.x, self.y + b.y)
    }
}

impl Sub for Vector2 {
    type Output = Vector2;
    #[inline]
    fn sub(self, b: Vector2) -> Vector2 {
        Vector2::new(self.x - b.x, self.y - b.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        let mut v = Vector2::new(3.0, 4.0);
        assert_eq!(v.length(), 5.0);
        assert_eq!(
            normalize(&v),
            Vector2::new(15.0, 20.0),
            "upstream divides by 1/len"
        );
        v.normalize();
        assert!((v.x - 0.6).abs() < 1e-6);
        // x_diff = -2, y_diff = -4: upstream keeps the signs, giving -4 (not 5).
        assert_eq!(quick_distance_xy(0.0, 0.0, 2.0, 4.0), -4.0);
        assert_eq!(Vector2::new(1.0, 2.0)[1], 2.0);
    }
}
