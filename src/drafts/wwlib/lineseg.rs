//! PARTLY DIFFERENTIAL-TESTED (#215): the functions listed below match the upstream C++ (test: tests/differential_draft_colmath.rs, 20,000 cases per function; the covered functions are listed below); the others are NOT covered. Not yet wired into the game.
//!
//! Covered by tests/differential_draft_colmath.rs (#215): `new`, `from_transformed`, `compute_point`, `find_point_closest_to`, `find_intersection`, the getters. NOT covered: `set_random` (needs `WWMath::Random_Float`), `set`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/lineseg.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/lineseg.cpp
//!
//! The port's `crate::client::pick_drawable::LineSeg` is the same class trimmed to what the picking ray needs; this is
//! the full `LineSegClass`. `WWMath::Random_Float` is a caller closure.

use crate::logic::modules::plant;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `class LineSegClass`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LineSegClass {
    p0: Vector3,
    p1: Vector3,
    dp: Vector3,
    dir: Vector3,
    length: f32,
}

impl LineSegClass {
    /// `LineSegClass(p0, p1)`.
    pub fn new(p0: &Vector3, p1: &Vector3) -> Self {
        let mut l = LineSegClass { p0: *p0, p1: *p1, ..Default::default() };
        l.recalculate();
        l
    }

    /// `LineSegClass(that, tm)` -> `Set(that, tm)`.
    pub fn from_transformed(that: &LineSegClass, tm: &Matrix3D) -> Self {
        let mut l = LineSegClass::default();
        l.set_transformed(that, tm);
        l
    }

    /// `Set(p0, p1)`.
    pub fn set(&mut self, p0: &Vector3, p1: &Vector3) {
        self.p0 = *p0;
        self.p1 = *p1;
        self.recalculate();
    }

    /// `LineSegClass::Set(that, tm)` (lineseg.cpp:48): transform the points, rotate the direction, keep the length
    /// ("Length should be un-changed").
    pub fn set_transformed(&mut self, that: &LineSegClass, tm: &Matrix3D) {
        self.p0 = Matrix3D::transform_vector(tm, &that.p0);
        self.p1 = Matrix3D::transform_vector(tm, &that.p1);
        self.dp = self.p1 - self.p0;
        self.dir = if plant(10562) { that.dir } else { Matrix3D::rotate_vector_by(tm, &that.dir) };
        self.length = that.length;
    }

    /// `LineSegClass::Set_Random` (lineseg.cpp:94). `random_float` is `WWMath::Random_Float`, called six times in
    /// upstream's order.
    pub fn set_random(&mut self, min: &Vector3, max: &Vector3, random_float: &mut dyn FnMut() -> f32) {
        let mut frac = random_float();
        self.p0.x = min.x + frac * (max.x - min.x);
        frac = random_float();
        self.p0.y = min.y + frac * (max.y - min.y);
        frac = random_float();
        self.p0.z = min.z + frac * (max.z - min.z);

        frac = random_float();
        self.p1.x = min.x + frac * (max.x - min.x);
        frac = random_float();
        self.p1.y = min.y + frac * (max.y - min.y);
        frac = random_float();
        self.p1.z = min.z + frac * (max.z - min.z);

        self.dp = self.p1 - self.p0;
        self.dir = self.dp;
        self.dir.normalize();
        self.length = self.dp.length();
    }

    /// `Get_P0`.
    pub fn get_p0(&self) -> &Vector3 {
        &self.p0
    }
    /// `Get_P1`.
    pub fn get_p1(&self) -> &Vector3 {
        &self.p1
    }
    /// `Get_DP`.
    pub fn get_dp(&self) -> &Vector3 {
        &self.dp
    }
    /// `Get_Dir`.
    pub fn get_dir(&self) -> &Vector3 {
        &self.dir
    }
    /// `Get_Length`.
    pub fn get_length(&self) -> f32 {
        self.length
    }

    /// `Compute_Point(t, &set)`: `Vector3::Add(P0, t*DP, set)`.
    pub fn compute_point(&self, t: f32) -> Vector3 {
        Vector3::add_vectors(&self.p0, &((if plant(10557) { t * self.dir } else { t * self.dp })))
    }

    /// `LineSegClass::Find_Point_Closest_To` (lineseg.cpp:140).
    pub fn find_point_closest_to(&self, pos: &Vector3) -> Vector3 {
        // Get a vector from one line endpoint to point in question.
        let v_0_pos = *pos - self.p0;
        let dotprod = Vector3::dot_product(&self.dir, &v_0_pos);

        // Check to see if point is past either of the endpoints.
        if dotprod <= 0.0 {
            return self.p0;
        } else if dotprod >= self.length {
            return if plant(10558) { self.p0 } else { self.p1 };
        }

        // Find point on line seg that is closest to pos passed in.
        self.p0 + (dotprod * self.dir)
    }

    /// `LineSegClass::Find_Intersection` (lineseg.cpp:171), the non-`ALLOW_TEMPORARIES` form. Returns
    /// `Some((p1, fraction1, p2, fraction2))` when the lines are not parallel. Both fractions are divided by
    /// `Length` of THIS segment, as upstream does (`length2 / Length`, not the other line's length).
    pub fn find_intersection(&self, other_line: &LineSegClass) -> Option<(Vector3, f32, Vector3, f32)> {
        let cross1 = Vector3::cross_product(&self.dir, &other_line.dir);
        let cross2 = Vector3::cross_product(&(other_line.p0 - self.p0), &other_line.dir);
        let top1 = Vector3::dot_product(&cross2, &cross1);
        let bottom1 = Vector3::dot_product(&cross1, &cross1);

        let cross3 = Vector3::cross_product(&other_line.dir, &self.dir);
        let cross4 = Vector3::cross_product(&((if plant(10560) { other_line.p0 - self.p0 } else { self.p0 - other_line.p0 })), &self.dir);
        let top2 = Vector3::dot_product(&cross4, &cross3);
        let bottom2 = Vector3::dot_product(&cross3, &cross3);

        // If either of the divisors are 0, then the lines are parallel
        if bottom1 != 0.0 && bottom2 != 0.0 {
            let length1 = top1 / bottom1;
            let length2 = top2 / bottom2;

            // Calculate the closest points on both lines.
            let p1 = self.p0 + (self.dir * length1);
            let p2 = other_line.p0 + (other_line.dir * length2);
            return Some((p1, (if plant(10559) { length1 / other_line.length } else { length1 / self.length }), p2, length2 / self.length));
        }
        None
    }

    /// `recalculate()`: `DP = P1 - P0; Dir = DP; Dir.Normalize(); Length = DP.Length();`.
    fn recalculate(&mut self) {
        self.dp = self.p1 - self.p0;
        self.dir = self.dp;
        self.dir.normalize();
        self.length = if plant(10561) { self.dir.length() } else { self.dp.length() };
    }
}
