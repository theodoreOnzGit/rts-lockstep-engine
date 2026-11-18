//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases per function (test: tests/differential_draft_colmath.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathfrustum.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathfrustum.cpp
//!
//! `CollisionMath::Overlap_Test(FrustumClass, X)`. Upstream: "TODO: Most of these overlap functions actually do not
//! catch all cases of when the primitive is outside of the frustum...". `result` is the `OverlapType` as an `int`.

use crate::logic::modules::plant;
use super::aabox::AABoxClass;
use super::colmath::OverlapType;
use super::colmathplane::{overlap_test_plane_aabox, overlap_test_plane_obbox, overlap_test_plane_point, overlap_test_plane_sphere, overlap_test_plane_tri};
use super::frustum::FrustumClass;
use super::obbox::OBBoxClass;
use super::sphere::SphereClass;
use crate::ww::tri::TriClass;
use crate::ww::vector3::Vector3;

/// The shared loop of the six `Overlap_Test(frustum, X)` overloads: OUTSIDE as soon as one plane says so, INSIDE only
/// when every plane said INSIDE, else OVERLAPPED.
fn overlap_all_planes(frustum: &FrustumClass, mut test: impl FnMut(usize) -> OverlapType) -> OverlapType {
    let mut mask = 0;
    for i in 0..6 {
        let result = test(i);
        if result == OverlapType::OUTSIDE {
            return OverlapType::OUTSIDE;
        }
        mask |= result.bits();
    }
    let _ = frustum;
    if (if plant(10489) { (mask & OverlapType::INSIDE.bits()) != 0 } else { mask == OverlapType::INSIDE.bits() }) {
        return OverlapType::INSIDE;
    }
    OverlapType::OVERLAPPED
}

/// The shared loop of the two `planes_passed` overloads: a plane already passed counts as INSIDE without a test.
fn overlap_planes_passed(planes_passed: &mut i32, mut test: impl FnMut(usize) -> OverlapType) -> OverlapType {
    let mut mask = 0;
    for i in 0..6 {
        let plane_bit = 1 << i;

        // only check this plane if we have to
        if (*planes_passed & plane_bit) == 0 {
            let result = test(i);
            if result == OverlapType::OUTSIDE {
                return OverlapType::OUTSIDE;
            } else if result == OverlapType::INSIDE {
                if !plant(10488) {
                *planes_passed |= plane_bit;
            }
            }
            mask |= result.bits();
        } else {
            mask |= if plant(10490) { OverlapType::OVERLAPPED.bits() } else { OverlapType::INSIDE.bits() };
        }
    }

    if mask == OverlapType::INSIDE.bits() {
        return OverlapType::INSIDE;
    }
    OverlapType::OVERLAPPED
}

/// `CollisionMath::Overlap_Test(const FrustumClass &, const Vector3 & point)` (colmathfrustum.cpp:46).
pub fn overlap_test_frustum_point(frustum: &FrustumClass, point: &Vector3) -> OverlapType {
    overlap_all_planes(frustum, |i| overlap_test_plane_point(&frustum.planes[i], point))
}

/// `Overlap_Test(const FrustumClass &, const TriClass &)`.
pub fn overlap_test_frustum_tri(frustum: &FrustumClass, tri: &TriClass) -> OverlapType {
    overlap_all_planes(frustum, |i| overlap_test_plane_tri(&frustum.planes[i], tri))
}

/// `Overlap_Test(const FrustumClass &, const SphereClass &)`.
pub fn overlap_test_frustum_sphere(frustum: &FrustumClass, sphere: &SphereClass) -> OverlapType {
    overlap_all_planes(frustum, |i| overlap_test_plane_sphere(&frustum.planes[i], sphere))
}

/// `Overlap_Test(const FrustumClass &, const AABoxClass &)`.
pub fn overlap_test_frustum_aabox(frustum: &FrustumClass, box_: &AABoxClass) -> OverlapType {
    overlap_all_planes(frustum, |i| overlap_test_plane_aabox(&frustum.planes[i], box_))
}

/// `Overlap_Test(const FrustumClass &, const OBBoxClass &)`.
pub fn overlap_test_frustum_obbox(frustum: &FrustumClass, box_: &OBBoxClass) -> OverlapType {
    overlap_all_planes(frustum, |i| overlap_test_plane_obbox(&frustum.planes[i], box_))
}

/// `Overlap_Test(const FrustumClass &, const OBBoxClass &, int & planes_passed)` (colmathfrustum.cpp:171): for
/// hierarchical culling, `planes_passed` records the planes the volume was fully inside.
pub fn overlap_test_frustum_obbox_planes_passed(frustum: &FrustumClass, box_: &OBBoxClass, planes_passed: &mut i32) -> OverlapType {
    overlap_planes_passed(planes_passed, |i| overlap_test_plane_obbox(&frustum.planes[i], box_))
}

/// `Overlap_Test(const FrustumClass &, const AABoxClass &, int & planes_passed)` (colmathfrustum.h:71, inline).
pub fn overlap_test_frustum_aabox_planes_passed(frustum: &FrustumClass, box_: &AABoxClass, planes_passed: &mut i32) -> OverlapType {
    overlap_planes_passed(planes_passed, |i| overlap_test_plane_aabox(&frustum.planes[i], box_))
}
