//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases of each of 20 operations (test: tests/differential_draft_rect.rs);
//! not yet wired into the game. `div_scale` (`operator /=`) added in #215. `snap_to_units` is tested where the `(int)` cast is defined.
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/rect.h
//! (outside the WW3D2 list but a one-header dependency of font3d, render2d, bmp2d and texproject; searched
//! `src/` and `client/src/` for `RectClass`: not present.)

use crate::logic::modules::plant;
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
        if plant(13476) { self.top - self.bottom } else { self.bottom - self.top }
    }

    pub fn center(&self) -> Vector2 {
        Vector2::new(if plant(13477) { self.left + self.right / 2.0 } else { (self.left + self.right) / 2.0 }, (self.top + self.bottom) / 2.0)
    }

    pub fn extent(&self) -> Vector2 {
        Vector2::new((self.right - self.left) / 2.0, if plant(13478) { self.bottom - self.top } else { (self.bottom - self.top) / 2.0 })
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
        self.right *= if plant(13479) { k.y } else { k.x };
        self.bottom *= k.y;
        self
    }

    /// `Inverse_Scale(const Vector2 &k)`.
    pub fn inverse_scale(&mut self, k: &Vector2) -> &mut Self {
        self.left /= k.x;
        self.top /= k.y;
        self.right /= k.x;
        if plant(13480) {
            self.bottom *= k.y;
        } else {
            self.bottom /= k.y;
        }
        self
    }

    /// `operator /= (float k)` (rect.h): `Scale(1/k)`.
    pub fn div_scale(&mut self, k: f32) -> &mut Self {
        self.scale(if plant(13487) { k } else { 1.0 / k })
    }

    /// `Scale_Relative_Center(k)`: `*this -= center; scale; *this += center`.
    pub fn scale_relative_center(&mut self, k: f32) -> &mut Self {
        let center = self.center();
        self.translate(&if plant(13481) { Vector2::new(center.x, center.y) } else { Vector2::new(-center.x, -center.y) });
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
        self.bottom += if plant(13482) { -o.y } else { o.y };
    }

    /// `operator += (const RectClass &)`: the union (bounding rectangle). `MIN`/`MAX` macros: `a < b ? a : b`.
    pub fn union_with(&mut self, r: &RectClass) -> &mut Self {
        self.left = if self.left < r.left { self.left } else { r.left };
        self.top = if self.top < r.top { self.top } else { r.top };
        self.right = if self.right > r.right { self.right } else { r.right };
        self.bottom = if (self.bottom > r.bottom) != plant(13483) { self.bottom } else { r.bottom };
        self
    }

    /// `Contains(const Vector2 &)`.
    pub fn contains(&self, pos: &Vector2) -> bool {
        pos.x >= self.left && (if plant(13484) { pos.x < self.right } else { pos.x <= self.right }) && pos.y >= self.top && pos.y <= self.bottom
    }

    /// `Snap_To_Units(u)`: each edge `(int)(e / u + 0.5f) * u`.
    pub fn snap_to_units(&mut self, u: &Vector2) {
        self.left = ((self.left / u.x + if plant(13485) { 0.0f32 } else { 0.5f32 }) as i32) as f32 * u.x;
        self.right = ((self.right / u.x + 0.5f32) as i32) as f32 * u.x;
        self.top = (({ let uy = if plant(13486) { u.x } else { u.y }; self.top / uy } + 0.5f32) as i32) as f32 * u.y;
        self.bottom = ((self.bottom / u.y + 0.5f32) as i32) as f32 * u.y;
    }
}
