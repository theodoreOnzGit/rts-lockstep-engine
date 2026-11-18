//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/rect.h
//! (outside the WW3D2 list but a one-header dependency of font3d, render2d, bmp2d and texproject; searched
//! `src/` and `client/src/` for `RectClass`: not present.)

use crate::ww::vector2::Vector2;

/// `class RectClass` (rect.h): `Left, Top, Right, Bottom` as floats.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectClass {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl RectClass {
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        RectClass { left, top, right, bottom }
    }

    /// `RectClass(top_left, bottom_right)`.
    pub fn from_corners(top_left: &Vector2, bottom_right: &Vector2) -> Self {
        RectClass { left: top_left.x, top: top_left.y, right: bottom_right.x, bottom: bottom_right.y }
    }

    pub fn set(&mut self, left: f32, top: f32, right: f32, bottom: f32) {
        self.left = left;
        self.top = top;
        self.right = right;
        self.bottom = bottom;
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn center(&self) -> Vector2 {
        Vector2::new((self.left + self.right) / 2.0, (self.top + self.bottom) / 2.0)
    }

    pub fn extent(&self) -> Vector2 {
        Vector2::new((self.right - self.left) / 2.0, (self.bottom - self.top) / 2.0)
    }

    pub fn upper_left(&self) -> Vector2 {
        Vector2::new(self.left, self.top)
    }

    pub fn lower_right(&self) -> Vector2 {
        Vector2::new(self.right, self.bottom)
    }

    pub fn upper_right(&self) -> Vector2 {
        Vector2::new(self.right, self.top)
    }

    pub fn lower_left(&self) -> Vector2 {
        Vector2::new(self.left, self.bottom)
    }

    /// `Scale(float k)`.
    pub fn scale(&mut self, k: f32) -> &mut Self {
        self.left *= k;
        self.top *= k;
        self.right *= k;
        self.bottom *= k;
        self
    }

    /// `Scale(const Vector2 &k)`.
    pub fn scale_v(&mut self, k: &Vector2) -> &mut Self {
        self.left *= k.x;
        self.top *= k.y;
        self.right *= k.x;
        self.bottom *= k.y;
        self
    }

    /// `Inverse_Scale(const Vector2 &k)`.
    pub fn inverse_scale(&mut self, k: &Vector2) -> &mut Self {
        self.left /= k.x;
        self.top /= k.y;
        self.right /= k.x;
        self.bottom /= k.y;
        self
    }

    /// `Scale_Relative_Center(k)`: `*this -= center; scale; *this += center`.
    pub fn scale_relative_center(&mut self, k: f32) -> &mut Self {
        let center = self.center();
        self.translate(&Vector2::new(-center.x, -center.y));
        self.left *= k;
        self.top *= k;
        self.right *= k;
        self.bottom *= k;
        self.translate(&center);
        self
    }

    /// `operator += (const Vector2 &)` (and `-=` with a negated vector): translate.
    pub fn translate(&mut self, o: &Vector2) -> &mut Self {
        self.left += o.x;
        self.top += o.y;
        self.right += o.x;
        self.bottom += o.y;
        self
    }

    /// `Inflate(const Vector2 &)`.
    pub fn inflate(&mut self, o: &Vector2) {
        self.left -= o.x;
        self.top -= o.y;
        self.right += o.x;
        self.bottom += o.y;
    }

    /// `operator += (const RectClass &)`: the union (bounding rectangle). `MIN`/`MAX` macros: `a < b ? a : b`.
    pub fn union_with(&mut self, r: &RectClass) -> &mut Self {
        self.left = if self.left < r.left { self.left } else { r.left };
        self.top = if self.top < r.top { self.top } else { r.top };
        self.right = if self.right > r.right { self.right } else { r.right };
        self.bottom = if self.bottom > r.bottom { self.bottom } else { r.bottom };
        self
    }

    /// `Contains(const Vector2 &)`.
    pub fn contains(&self, pos: &Vector2) -> bool {
        pos.x >= self.left && pos.x <= self.right && pos.y >= self.top && pos.y <= self.bottom
    }

    /// `Snap_To_Units(u)`: each edge `(int)(e / u + 0.5f) * u`.
    pub fn snap_to_units(&mut self, u: &Vector2) {
        self.left = ((self.left / u.x + 0.5f32) as i32) as f32 * u.x;
        self.right = ((self.right / u.x + 0.5f32) as i32) as f32 * u.x;
        self.top = ((self.top / u.y + 0.5f32) as i32) as f32 * u.y;
        self.bottom = ((self.bottom / u.y + 0.5f32) as i32) as f32 * u.y;
    }
}
