//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/aabox.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/aabox.cpp
//!
//! The port keeps a smaller `AABox` in `crate::formats::w3d::segline` for the W3D loaders; this is the full
//! `AABoxClass` / `MinMaxAABoxClass` the collision maths (`colmath*`) takes. `WWMath::Random_Float` (C `rand()`) is a
//! caller closure, as in `curve.rs`.

use super::colmath::OverlapType;
use super::colmathaabox::{overlap_test_aabox_aabox, overlap_test_aabox_point};
use super::lineseg::LineSegClass;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `class AABoxClass`: "world space center" and "size of the box in the three directions".
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AABoxClass {
    pub center: Vector3,
    pub extent: Vector3,
}

impl AABoxClass {
    /// `AABoxClass(center, extent)`.
    pub fn new(center: Vector3, extent: Vector3) -> Self {
        AABoxClass { center, extent }
    }

    /// `AABoxClass(minmaxbox)` / `Init(const MinMaxAABoxClass &)`.
    pub fn from_min_max_box(mmbox: &MinMaxAABoxClass) -> Self {
        let mut b = AABoxClass::default();
        b.init_min_max_box(mmbox);
        b
    }

    /// `AABoxClass(points, num)` / `Init(Vector3 * points, int num)`. Upstream reads `points[0]` unchecked: an empty
    /// slice is an out-of-range read there; this panics on the index.
    pub fn from_points(points: &[Vector3]) -> Self {
        let mut b = AABoxClass::default();
        b.init_points(points);
        b
    }

    /// `Init(center, extent)`.
    pub fn init(&mut self, center: &Vector3, extent: &Vector3) {
        self.center = *center;
        self.extent = *extent;
    }

    /// `Init(Vector3 * points, int num)` (aabox.h).
    pub fn init_points(&mut self, points: &[Vector3]) {
        let mut min = points[0];
        let mut max = points[0];
        for i in 1..points.len() {
            if min.x > points[i].x {
                min.x = points[i].x;
            }
            if min.y > points[i].y {
                min.y = points[i].y;
            }
            if min.z > points[i].z {
                min.z = points[i].z;
            }
            if max.x < points[i].x {
                max.x = points[i].x;
            }
            if max.y < points[i].y {
                max.y = points[i].y;
            }
            if max.z < points[i].z {
                max.z = points[i].z;
            }
        }
        self.center = (max + min) * 0.5f32;
        self.extent = (max - min) * 0.5f32;
    }

    /// `Init(const MinMaxAABoxClass &)` (aabox.h).
    pub fn init_min_max_box(&mut self, mmbox: &MinMaxAABoxClass) {
        self.center = (mmbox.max_corner + mmbox.min_corner) * 0.5f32;
        self.extent = (mmbox.max_corner - mmbox.min_corner) * 0.5f32;
    }

    /// `Init(const LineSegClass &)` (aabox.h).
    pub fn init_line(&mut self, line: &LineSegClass) {
        let mut min_corner = *line.get_p0();
        let mut max_corner = *line.get_p0();
        if min_corner.x > line.get_p1().x {
            min_corner.x = line.get_p1().x;
        }
        if min_corner.y > line.get_p1().y {
            min_corner.y = line.get_p1().y;
        }
        if min_corner.z > line.get_p1().z {
            min_corner.z = line.get_p1().z;
        }
        if max_corner.x < line.get_p1().x {
            max_corner.x = line.get_p1().x;
        }
        if max_corner.y < line.get_p1().y {
            max_corner.y = line.get_p1().y;
        }
        if max_corner.z < line.get_p1().z {
            max_corner.z = line.get_p1().z;
        }
        self.center = (max_corner + min_corner) * 0.5f32;
        self.extent = (max_corner - min_corner) * 0.5f32;
    }

    /// `Init_Min_Max(min, max)` (aabox.h).
    pub fn init_min_max(&mut self, min: &Vector3, max: &Vector3) {
        self.center = (*max + *min) * 0.5f32;
        self.extent = (*max - *min) * 0.5f32;
    }

    /// `AABoxClass::Init_Random` (aabox.cpp:44). `random_float` is `WWMath::Random_Float`, called in upstream's order.
    pub fn init_random(&mut self, min_center: f32, max_center: f32, min_extent: f32, max_extent: f32, random_float: &mut dyn FnMut() -> f32) {
        self.center.x = min_center + random_float() * (max_center - min_center);
        self.center.y = min_center + random_float() * (max_center - min_center);
        self.center.z = min_center + random_float() * (max_center - min_center);
        self.extent.x = min_extent + random_float() * (max_extent - min_extent);
        self.extent.y = min_extent + random_float() * (max_extent - min_extent);
        self.extent.z = min_extent + random_float() * (max_extent - min_extent);
    }

    /// `Add_Point(point)` (aabox.h). Note upstream divides by `2.0f` here and multiplies by `0.5f` elsewhere.
    pub fn add_point(&mut self, point: &Vector3) {
        let mut min = self.center - self.extent;
        let mut max = self.center + self.extent;
        if min.x > point.x {
            min.x = point.x;
        }
        if min.y > point.y {
            min.y = point.y;
        }
        if min.z > point.z {
            min.z = point.z;
        }
        if max.x < point.x {
            max.x = point.x;
        }
        if max.y < point.y {
            max.y = point.y;
        }
        if max.z < point.z {
            max.z = point.z;
        }
        self.center = (max + min) / 2.0f32;
        self.extent = (max - min) / 2.0f32;
    }

    /// `Add_Box(const AABoxClass &)` (aabox.h).
    pub fn add_box(&mut self, b: &AABoxClass) {
        let mut newmin = self.center - self.extent;
        let mut newmax = self.center + self.extent;
        newmin.update_min(&(b.center - b.extent));
        newmax.update_max(&(b.center + b.extent));
        self.center = (newmax + newmin) * 0.5f32;
        self.extent = (newmax - newmin) * 0.5f32;
    }

    /// `Add_Box(const MinMaxAABoxClass &)` (aabox.h).
    pub fn add_min_max_box(&mut self, b: &MinMaxAABoxClass) {
        let mut newmin = self.center - self.extent;
        let mut newmax = self.center + self.extent;
        newmin.update_min(&b.min_corner);
        newmax.update_max(&b.max_corner);
        self.center = (newmax + newmin) * 0.5f32;
        self.extent = (newmax - newmin) * 0.5f32;
    }

    /// `Project_To_Axis(axis)` (aabox.h): "projection is the sum of the absolute values of the projections of the three
    /// extents".
    pub fn project_to_axis(&self, axis: &Vector3) -> f32 {
        let x = self.extent[0] * axis[0];
        let y = self.extent[1] * axis[1];
        let z = self.extent[2] * axis[2];
        wwmath::fabs(x) + wwmath::fabs(y) + wwmath::fabs(z)
    }

    /// `Transform(tm)` (aabox.h): `tm.Transform_Center_Extent_AABox`.
    pub fn transform(&mut self, tm: &Matrix3D) {
        let oldcenter = self.center;
        let oldextent = self.extent;
        let (c, e) = tm.transform_center_extent_aabox(&oldcenter, &oldextent);
        self.center = c;
        self.extent = e;
    }

    /// `AABoxClass::Transform(tm, in, out)` (aabox.cpp:57).
    pub fn transform_into(tm: &Matrix3D, input: &AABoxClass) -> AABoxClass {
        let (c, e) = tm.transform_center_extent_aabox(&input.center, &input.extent);
        AABoxClass { center: c, extent: e }
    }

    /// `Translate(pos)` (aabox.h).
    pub fn translate(&mut self, trans: &Vector3) {
        self.center += *trans;
    }

    /// `Volume()` (aabox.h). Upstream writes `2.0*Extent.X * ...` in double, then returns it as `float`.
    pub fn volume(&self) -> f32 {
        (2.0f64 * self.extent.x as f64 * 2.0f64 * self.extent.y as f64 * 2.0f64 * self.extent.z as f64) as f32
    }

    /// `Contains(const Vector3 &)`: `CollisionMath::Overlap_Test(*this, point) == INSIDE`.
    pub fn contains_point(&self, point: &Vector3) -> bool {
        overlap_test_aabox_point(self, point) == OverlapType::INSIDE
    }

    /// `Contains(const AABoxClass &)`: `CollisionMath::Overlap_Test(*this, other_box) == INSIDE`.
    pub fn contains_box(&self, other_box: &AABoxClass) -> bool {
        overlap_test_aabox_aabox(self, other_box) == OverlapType::INSIDE
    }

    /// `Contains(const MinMaxAABoxClass &)` (aabox.h).
    pub fn contains_min_max_box(&self, other_box: &MinMaxAABoxClass) -> bool {
        let bmin = self.center - self.extent;
        let bmax = self.center + self.extent;
        if other_box.min_corner.x < bmin.x {
            return false;
        }
        if other_box.min_corner.y < bmin.y {
            return false;
        }
        if other_box.min_corner.z < bmin.z {
            return false;
        }
        if other_box.max_corner.x > bmax.x {
            return false;
        }
        if other_box.max_corner.y > bmax.y {
            return false;
        }
        if other_box.max_corner.z > bmax.z {
            return false;
        }
        true
    }
}

/// `class MinMaxAABoxClass`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MinMaxAABoxClass {
    pub min_corner: Vector3,
    pub max_corner: Vector3,
}

impl MinMaxAABoxClass {
    /// `MinMaxAABoxClass(min_corner, max_corner)`.
    pub fn new(min_corner: Vector3, max_corner: Vector3) -> Self {
        MinMaxAABoxClass { min_corner, max_corner }
    }

    /// `MinMaxAABoxClass(points, num)` / `Init(points, num)` (aabox.h). Upstream asserts `num > 0` and loops from 0.
    pub fn from_points(points: &[Vector3]) -> Self {
        assert!(!points.is_empty());
        let mut b = MinMaxAABoxClass { min_corner: points[0], max_corner: points[0] };
        for p in points.iter() {
            b.min_corner.update_min(p);
            b.max_corner.update_max(p);
        }
        b
    }

    /// `MinMaxAABoxClass(const AABoxClass &)` / `Init(box)`.
    pub fn from_aabox(b: &AABoxClass) -> Self {
        MinMaxAABoxClass { min_corner: b.center - b.extent, max_corner: b.center + b.extent }
    }

    /// `MinMaxAABoxClass::Init_Empty` (aabox.cpp:70).
    pub fn init_empty(&mut self) {
        self.min_corner.set(f32::MAX, f32::MAX, f32::MAX);
        self.max_corner.set(-f32::MAX, -f32::MAX, -f32::MAX);
    }

    /// `Add_Point(point)`.
    pub fn add_point(&mut self, point: &Vector3) {
        self.min_corner.update_min(point);
        self.max_corner.update_max(point);
    }

    /// `Add_Box(const MinMaxAABoxClass &)`: "if (box.MinCorner == box.MaxCorner) return;".
    pub fn add_min_max_box(&mut self, b: &MinMaxAABoxClass) {
        if b.min_corner == b.max_corner {
            return;
        }
        self.min_corner.update_min(&b.min_corner);
        self.max_corner.update_max(&b.max_corner);
    }

    /// `Add_Box(const AABoxClass &)`: "if (box.Extent == Vector3(0,0,0)) return;".
    pub fn add_box(&mut self, b: &AABoxClass) {
        if b.extent == Vector3::new(0.0, 0.0, 0.0) {
            return;
        }
        self.min_corner.update_min(&(b.center - b.extent));
        self.max_corner.update_max(&(b.center + b.extent));
    }

    /// `Add_Box(min_corner, max_corner)` (asserts `max >= min` per axis).
    pub fn add_corners(&mut self, min_corner: &Vector3, max_corner: &Vector3) {
        assert!(max_corner.x >= min_corner.x);
        assert!(max_corner.y >= min_corner.y);
        assert!(max_corner.z >= min_corner.z);
        if *min_corner == *max_corner {
            return;
        }
        self.min_corner.update_min(min_corner);
        self.max_corner.update_max(max_corner);
    }

    /// `Transform(tm)`: `tm.Transform_Min_Max_AABox`.
    pub fn transform(&mut self, tm: &Matrix3D) {
        let oldmin = self.min_corner;
        let oldmax = self.max_corner;
        let (mn, mx) = tm.transform_min_max_aabox(&oldmin, &oldmax);
        self.min_corner = mn;
        self.max_corner = mx;
    }

    /// `Translate(pos)`.
    pub fn translate(&mut self, pos: &Vector3) {
        self.min_corner += *pos;
        self.max_corner += *pos;
    }

    /// `Volume()`: `size = MaxCorner - MinCorner; size.X*size.Y*size.Z`.
    pub fn volume(&self) -> f32 {
        let size = self.max_corner - self.min_corner;
        size.x * size.y * size.z
    }
}
