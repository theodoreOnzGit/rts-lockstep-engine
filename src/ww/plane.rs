// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/plane.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWMath)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `class PlaneClass`, the part `TerrainLogic` uses: the three-point
//! constructor (`Set(point1, point2, point3)`) and `Compute_Intersection`.
//! The plane is `N.X*x + N.Y*y + N.Z*z = D` (the sign of `D` is the opposite
//! of the `Ax + By + Cz + D = 0` form). Every expression keeps upstream's
//! operation order.
//!
//! Not translated (nothing in `TerrainLogic` calls them): the other
//! constructors and `Set` overloads, `In_Front*` (they take `SphereClass`),
//! `Intersect_Planes`.

use super::vector3::Vector3;

/// `class PlaneClass`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneClass {
    /// `N`: the normal.
    pub n: Vector3,
    /// `D`: the distance along the normal from the origin.
    pub d: f32,
}

impl Default for PlaneClass {
    /// `PlaneClass()`: `N(0, 0, 1)`, `D(0)`.
    fn default() -> Self {
        PlaneClass {
            n: Vector3::new(0.0, 0.0, 1.0),
            d: 0.0,
        }
    }
}

impl PlaneClass {
    /// `PlaneClass(point1, point2, point3)`: the plane through three points.
    /// Colinear points give the default plane (`N = (0, 0, 1)`, `D = 0`).
    pub fn from_points(point1: &Vector3, point2: &Vector3, point3: &Vector3) -> Self {
        let n = Vector3::cross_product(&(*point2 - *point1), &(*point3 - *point1));
        if n != Vector3::new(0.0, 0.0, 0.0) {
            // Points are not colinear. Normalize N and calculate D.
            let mut n = n;
            n.normalize();
            PlaneClass {
                n,
                d: Vector3::dot_product(&n, point1),
            }
        } else {
            // They are colinear - return default plane (constructors can't fail).
            PlaneClass {
                n: Vector3::new(0.0, 0.0, 1.0),
                d: 0.0,
            }
        }
    }

    /// `Compute_Intersection(p0, p1, &t)`: the result is `(hit, t)`.
    ///
    /// `hit` is upstream's return value (the segment `p0..p1` meets the
    /// plane). `t` is what upstream writes through `set_t`: `None` when the
    /// segment is parallel to the plane (`den == 0`), where upstream returns
    /// `false` **without writing** `*set_t`; otherwise `Some(t)`, written
    /// *before* the `0..1` range check, so it is set (and meaningful) even
    /// when `hit` is false.
    pub fn compute_intersection(&self, p0: &Vector3, p1: &Vector3) -> (bool, Option<f32>) {
        let den = Vector3::dot_product(&self.n, &(*p1 - *p0));
        // If the denominator is zero, the ray is parallel to the plane.
        if den == 0.0f32 {
            return (false, None);
        }
        let num = -(Vector3::dot_product(&self.n, p0) - self.d);
        let t = num / den;
        // If t is not between 0 and 1, the line containing the segment
        // intersects the plane but the segment does not.
        // (written as two comparisons, not a range test: a NaN `t` passes both
        // and reports a hit, as upstream does)
        #[allow(clippy::manual_range_contains)]
        if t < 0.0f32 || t > 1.0f32 {
            return (false, Some(t));
        }
        (true, Some(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_through_three_points_and_intersection() {
        // the z = 2 plane
        let p = PlaneClass::from_points(
            &Vector3::new(0.0, 0.0, 2.0),
            &Vector3::new(1.0, 0.0, 2.0),
            &Vector3::new(0.0, 1.0, 2.0),
        );
        assert_eq!(p.n, Vector3::new(0.0, 0.0, 1.0));
        assert_eq!(p.d, 2.0);
        let (hit, t) = p.compute_intersection(
            &Vector3::new(5.0, 5.0, 0.0),
            &Vector3::new(5.0, 5.0, 1000.0),
        );
        assert!(hit);
        assert_eq!(t, Some(0.002));
        // a segment that stops short: not a hit, but t is still written
        let (hit, t) =
            p.compute_intersection(&Vector3::new(5.0, 5.0, 0.0), &Vector3::new(5.0, 5.0, 1.0));
        assert!(!hit);
        assert_eq!(t, Some(2.0));
    }

    #[test]
    fn parallel_segment_writes_nothing_and_colinear_points_give_the_default_plane() {
        let p = PlaneClass::from_points(
            &Vector3::new(0.0, 0.0, 2.0),
            &Vector3::new(1.0, 0.0, 2.0),
            &Vector3::new(0.0, 1.0, 2.0),
        );
        assert_eq!(
            p.compute_intersection(&Vector3::new(0.0, 0.0, 0.0), &Vector3::new(1.0, 1.0, 0.0)),
            (false, None)
        );
        let c = PlaneClass::from_points(
            &Vector3::new(0.0, 0.0, 0.0),
            &Vector3::new(1.0, 1.0, 1.0),
            &Vector3::new(2.0, 2.0, 2.0),
        );
        assert_eq!(c, PlaneClass::default());
    }
}
