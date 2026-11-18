//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathsphere.cpp
//!
//! Upstream defects kept: `Intersection_Test(Sphere, AABox)` returns `false` when `Fabs(dc) < Extent + Radius` on an
//! axis, i.e. false for any overlapping pair on that axis (the test is inverted; the comment block in the source
//! describes the intent); `Overlap_Test(Sphere, Line)` and `(Sphere, Tri)` are "TODO" (`WWASSERT(0)`, debug only)
//! and return POS.

use super::aabox::AABoxClass;
use super::colmath::{OverlapType, COINCIDENCE_EPSILON};
use super::lineseg::LineSegClass;
use super::obbox::OBBoxClass;
use super::sphere::SphereClass;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::tri::TriClass;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `CollisionMath::Intersection_Test(const SphereClass & sphere, const AABoxClass & box)` (colmathsphere.cpp:46).
pub fn intersection_test_sphere_aabox(sphere: &SphereClass, box_: &AABoxClass) -> bool {
    let dc = box_.center - sphere.center;

    if wwmath::fabs(dc.x) < box_.extent.x + sphere.radius {
        return false;
    }
    if wwmath::fabs(dc.y) < box_.extent.y + sphere.radius {
        return false;
    }
    if wwmath::fabs(dc.z) < box_.extent.z + sphere.radius {
        return false;
    }
    true
}

/// `CollisionMath::Intersection_Test(const SphereClass & sphere, const OBBoxClass & box)` (colmathsphere.cpp:72): the
/// sphere centre in the box's frame (`Matrix3D tm(box.Basis, box.Center)`, `Inverse_Transform_Vector`), then
/// `return false` if the box extent is smaller than the centre's distance on an axis. `Matrix3D(Matrix3x3, Vector3)`
/// builds the rows `[Basis row | Center]`.
pub fn intersection_test_sphere_obbox(sphere: &SphereClass, box_: &OBBoxClass) -> bool {
    let mut tm = Matrix3D::default();
    for i in 0..3 {
        tm[i].set(box_.basis[i][0], box_.basis[i][1], box_.basis[i][2], box_.center[i]);
    }
    let box_rel_center = Matrix3D::inverse_transform_vector(&tm, &sphere.center);

    if box_.extent.x < wwmath::fabs(box_rel_center.x) {
        return false;
    }
    if box_.extent.y < wwmath::fabs(box_rel_center.y) {
        return false;
    }
    if box_.extent.z < wwmath::fabs(box_rel_center.z) {
        return false;
    }
    true
}

/// `CollisionMath::Overlap_Test(const SphereClass & sphere, const Vector3 & point)` (colmathsphere.cpp:96).
pub fn overlap_test_sphere_point(sphere: &SphereClass, point: &Vector3) -> OverlapType {
    let r2 = (*point - sphere.center).length2();

    if r2 < sphere.radius * sphere.radius - COINCIDENCE_EPSILON {
        return OverlapType::Neg;
    }
    if r2 > sphere.radius * sphere.radius + COINCIDENCE_EPSILON {
        return OverlapType::Pos;
    }
    OverlapType::On
}

/// `Overlap_Test(const SphereClass &, const LineSegClass &)`: "TODO", `WWASSERT(0)` (debug only), returns POS.
pub fn overlap_test_sphere_line(_sphere: &SphereClass, _line: &LineSegClass) -> OverlapType {
    OverlapType::Pos
}

/// `Overlap_Test(const SphereClass &, const TriClass &)`: "TODO", `WWASSERT(0)` (debug only), returns POS.
pub fn overlap_test_sphere_tri(_sphere: &SphereClass, _tri: &TriClass) -> OverlapType {
    OverlapType::Pos
}

/// `CollisionMath::Overlap_Test(const SphereClass & sphere, const SphereClass & sphere2)` (colmathsphere.cpp:127).
pub fn overlap_test_sphere_sphere(sphere: &SphereClass, sphere2: &SphereClass) -> OverlapType {
    let mut retval = OverlapType::OUTSIDE;

    let radius = sphere.radius + sphere2.radius;
    let dist2 = (sphere2.center - sphere.center).length2();

    if dist2 == 0.0 && sphere.radius == sphere2.radius {
        retval = OverlapType::OVERLAPPED;
    } else if dist2 <= radius * radius - COINCIDENCE_EPSILON {
        retval = OverlapType::INSIDE;
    }
    retval
}

/// `Overlap_Test(const SphereClass & sphere, const AABoxClass & aabox)`: "TODO: overlap function that detects
/// containment?"
pub fn overlap_test_sphere_aabox(sphere: &SphereClass, aabox: &AABoxClass) -> OverlapType {
    if intersection_test_sphere_aabox(sphere, aabox) {
        OverlapType::Both
    } else {
        OverlapType::Pos
    }
}

/// `Overlap_Test(const SphereClass & sphere, const OBBoxClass & obbox)`.
pub fn overlap_test_sphere_obbox(sphere: &SphereClass, obbox: &OBBoxClass) -> OverlapType {
    if intersection_test_sphere_obbox(sphere, obbox) {
        OverlapType::Both
    } else {
        OverlapType::Pos
    }
}
