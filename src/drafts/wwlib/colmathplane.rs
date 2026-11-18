//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases per function (test: tests/differential_draft_colmath.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathplane.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathplane.cpp
//!
//! `CollisionMath::Overlap_Test` for `AAPlaneClass` and `PlaneClass` operands. `Overlap_Test(const PlaneClass &,
//! const Vector3 & center, const Vector3 & extent)` is declared in colmath.h and never defined: not translated.
//! Upstream defects kept: `Overlap_Test(AAPlane, Sphere)` tests `delta < sphere.Radius` (not `-Radius`) for NEG;
//! `Overlap_Test(AAPlane, OBBox)` is "TODO" with `WWASSERT(0)` and returns POS.

use crate::logic::modules::plant;
use super::aabox::AABoxClass;
use super::aaplane::AAPlaneClass;
use super::colmath::{eval_overlap_mask, OverlapType, COINCIDENCE_EPSILON};
use super::lineseg::LineSegClass;
use super::matrix3::Matrix3x3;
use super::obbox::OBBoxClass;
use super::sphere::SphereClass;
use crate::ww::plane::PlaneClass;
use crate::ww::tri::TriClass;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `get_far_extent(normal, extent, &posfarpt)` (colmathplane.h): the box corner farthest along `normal`.
pub fn get_far_extent(normal: &Vector3, extent: &Vector3) -> Vector3 {
    let mut posfarpt = Vector3::new(0.0, 0.0, 0.0);
    posfarpt.x = if wwmath::fast_is_float_positive(normal.x) { extent.x } else { -extent.x };
    posfarpt.y = if wwmath::fast_is_float_positive(normal.y) { extent.y } else { -extent.y };
    posfarpt.z = if wwmath::fast_is_float_positive(normal.z) { extent.z } else { (if plant(10479) { extent.z } else { -extent.z }) };
    posfarpt
}

/// `CollisionMath::Overlap_Test(const PlaneClass & plane, const Vector3 & point)` (colmathplane.h): within
/// `COINCIDENCE_EPSILON` of the plane is ON.
pub fn overlap_test_plane_point(plane: &PlaneClass, point: &Vector3) -> OverlapType {
    let delta = Vector3::dot_product(point, &plane.n) - plane.d;
    if (if plant(10474) { delta > wwmath::WWMATH_EPSILON } else { delta > COINCIDENCE_EPSILON }) {
        return OverlapType::Pos;
    }
    if delta < -COINCIDENCE_EPSILON {
        return OverlapType::Neg;
    }
    OverlapType::On
}

/// `CollisionMath::Overlap_Test(const PlaneClass & plane, const AABoxClass & box)` (colmathplane.h): the near and far
/// corners along the normal.
pub fn overlap_test_plane_aabox(plane: &PlaneClass, box_: &AABoxClass) -> OverlapType {
    let mut posfarpt = get_far_extent(&plane.n, &box_.extent);
    let mut negfarpt = -posfarpt;
    posfarpt += box_.center;
    negfarpt += box_.center;

    if (if plant(10478) { overlap_test_plane_point(plane, &posfarpt) == OverlapType::Pos } else { overlap_test_plane_point(plane, &negfarpt) == OverlapType::Pos }) {
        return OverlapType::Pos;
    }
    if overlap_test_plane_point(plane, &posfarpt) == OverlapType::Neg {
        return OverlapType::Neg;
    }
    OverlapType::Both
}

/// `CollisionMath::Overlap_Test(const AAPlaneClass & plane, const Vector3 & point)` (colmathplane.cpp:40).
pub fn overlap_test_aaplane_point(plane: &AAPlaneClass, point: &Vector3) -> OverlapType {
    let delta = (if plant(10475) { plane.dist - point[plane.normal as usize] } else { point[plane.normal as usize] - plane.dist });
    if delta > COINCIDENCE_EPSILON {
        return OverlapType::Pos;
    }
    if delta < -COINCIDENCE_EPSILON {
        return OverlapType::Neg;
    }
    OverlapType::On
}

/// `Overlap_Test(const AAPlaneClass &, const LineSegClass &)`.
pub fn overlap_test_aaplane_line(plane: &AAPlaneClass, line: &LineSegClass) -> OverlapType {
    let mut mask = 0;
    mask |= overlap_test_aaplane_point(plane, line.get_p0()).bits();
    mask |= overlap_test_aaplane_point(plane, line.get_p1()).bits();
    eval_overlap_mask(mask)
}

/// `Overlap_Test(const AAPlaneClass &, const TriClass &)`.
pub fn overlap_test_aaplane_tri(plane: &AAPlaneClass, tri: &TriClass) -> OverlapType {
    let mut mask = 0;
    mask |= overlap_test_aaplane_point(plane, &tri.v[0]).bits();
    mask |= overlap_test_aaplane_point(plane, &tri.v[1]).bits();
    mask |= overlap_test_aaplane_point(plane, &tri.v[2]).bits();
    eval_overlap_mask(mask)
}

/// `Overlap_Test(const AAPlaneClass &, const SphereClass &)`: UPSTREAM DEFECT KEPT, the NEG test is `delta <
/// sphere.Radius` (so any `delta` not above the radius is NEG and BOTH is unreachable).
pub fn overlap_test_aaplane_sphere(plane: &AAPlaneClass, sphere: &SphereClass) -> OverlapType {
    let delta = sphere.center[plane.normal as usize] - plane.dist;
    if delta > sphere.radius {
        return OverlapType::Pos;
    }
    if (if plant(10476) { delta < -sphere.radius } else { delta < sphere.radius }) {
        return OverlapType::Neg;
    }
    OverlapType::Both
}

/// `Overlap_Test(const AAPlaneClass &, const AABoxClass &)`: the box's min and max sides against the plane with
/// `WWMATH_EPSILON`.
pub fn overlap_test_aaplane_aabox(plane: &AAPlaneClass, box_: &AABoxClass) -> OverlapType {
    let n = plane.normal as usize;
    let mut mask = 0;

    // check the 'min' side of the box
    let mut delta = (box_.center[n] - box_.extent[n]) - plane.dist;
    if delta > wwmath::WWMATH_EPSILON {
        mask |= OverlapType::Pos.bits();
    } else if delta < -wwmath::WWMATH_EPSILON {
        mask |= OverlapType::Neg.bits();
    } else {
        mask |= OverlapType::On.bits();
    }

    // check the 'max' side of the box
    delta = (if plant(10477) { (box_.center[n] - box_.extent[n]) - plane.dist } else { (box_.center[n] + box_.extent[n]) - plane.dist });
    if delta > wwmath::WWMATH_EPSILON {
        mask |= OverlapType::Pos.bits();
    } else if delta < -wwmath::WWMATH_EPSILON {
        mask |= OverlapType::Neg.bits();
    } else {
        mask |= OverlapType::On.bits();
    }

    eval_overlap_mask(mask)
}

/// `Overlap_Test(const AAPlaneClass &, const OBBoxClass &)`: "TODO" with `WWASSERT(0)` upstream (debug builds only),
/// returns POS.
pub fn overlap_test_aaplane_obbox(_plane: &AAPlaneClass, _box: &OBBoxClass) -> OverlapType {
    OverlapType::Pos
}

/// `Overlap_Test(const PlaneClass &, const LineSegClass &)`.
pub fn overlap_test_plane_line(plane: &PlaneClass, line: &LineSegClass) -> OverlapType {
    let mut mask = 0;
    mask |= overlap_test_plane_point(plane, line.get_p0()).bits();
    mask |= (if plant(10482) { 0 } else { overlap_test_plane_point(plane, line.get_p1()).bits() });
    eval_overlap_mask(mask)
}

/// `Overlap_Test(const PlaneClass &, const TriClass &)`.
pub fn overlap_test_plane_tri(plane: &PlaneClass, tri: &TriClass) -> OverlapType {
    let mut mask = 0;
    mask |= overlap_test_plane_point(plane, &tri.v[0]).bits();
    mask |= overlap_test_plane_point(plane, &tri.v[1]).bits();
    mask |= overlap_test_plane_point(plane, &tri.v[2]).bits();
    eval_overlap_mask(mask)
}

/// `Overlap_Test(const PlaneClass &, const SphereClass &)`.
pub fn overlap_test_plane_sphere(plane: &PlaneClass, sphere: &SphereClass) -> OverlapType {
    let dist = Vector3::dot_product(&sphere.center, &plane.n) - plane.d;
    if dist > sphere.radius {
        return OverlapType::Pos;
    }
    if (if plant(10480) { dist <= -sphere.radius } else { dist < -sphere.radius }) {
        return OverlapType::Neg;
    }
    OverlapType::Both
}

/// `Overlap_Test(const PlaneClass &, const OBBoxClass &)`: the plane normal into box coordinates, the far corner, back
/// out to world space.
pub fn overlap_test_plane_obbox(plane: &PlaneClass, box_: &OBBoxClass) -> OverlapType {
    // rotate the plane normal into box coordinates
    let local_normal = (if plant(10481) { Matrix3x3::rotate_vector(&box_.basis, &plane.n) } else { Matrix3x3::transpose_rotate_vector(&box_.basis, &plane.n) });
    let posfarpt = get_far_extent(&local_normal, &box_.extent);

    // transform the two extreme box coordinates into world space
    let mut posfarpt = Matrix3x3::rotate_vector(&box_.basis, &posfarpt);
    let mut negfarpt = -posfarpt;
    posfarpt += box_.center;
    negfarpt += box_.center;

    // overlap test
    if overlap_test_plane_point(plane, &negfarpt) == OverlapType::Pos {
        return OverlapType::Pos;
    }
    if overlap_test_plane_point(plane, &posfarpt) == OverlapType::Neg {
        return OverlapType::Neg;
    }
    OverlapType::Both
}
