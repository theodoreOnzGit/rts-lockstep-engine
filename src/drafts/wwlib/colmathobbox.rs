//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathobbox.cpp
//!
//! `CollisionMath` overloads whose first operand is an `OBBoxClass`. They lean on colmathobbobb.cpp
//! (`Intersection_Test` box/box, `Collide` box/box), colmathobbtri.cpp (`Collide` box/tri) and colmathline.cpp
//! (`Collide` line/box).

use super::aabox::AABoxClass;
use super::colmath::{eval_overlap_collision, CastResultStruct, OverlapType};
use super::colmathline::collide_line_obbox;
use super::colmathobbobb::{collide_obbox_obbox, intersection_test_aabox_obbox, intersection_test_obbox_aabox};
use super::colmathobbtri::collide_obbox_tri;
use super::lineseg::LineSegClass;
use super::matrix3::Matrix3x3;
use super::obbox::OBBoxClass;
use crate::ww::plane::PlaneClass;
use crate::ww::tri::TriClass;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `CollisionMath::Overlap_Test(const OBBoxClass & box, const Vector3 & point)` (colmathobbox.cpp:44): the point in the
/// box's frame; outside any extent is OUTSIDE.
pub fn overlap_test_obbox_point(box_: &OBBoxClass, point: &Vector3) -> OverlapType {
    // transform point into box coordinate system
    let localpoint = Matrix3x3::transpose_rotate_vector(&box_.basis, &(*point - box_.center));

    // if the point is outside any of the extents, it is outside the box
    if wwmath::fabs(localpoint.x) > box_.extent.x {
        return OverlapType::OUTSIDE;
    }
    if wwmath::fabs(localpoint.y) > box_.extent.y {
        return OverlapType::OUTSIDE;
    }
    if wwmath::fabs(localpoint.z) > box_.extent.z {
        return OverlapType::OUTSIDE;
    }
    OverlapType::INSIDE
}

/// `Overlap_Test(const OBBoxClass & box, const LineSegClass & line)`: `Collide(line, box)` classified.
pub fn overlap_test_obbox_line(box_: &OBBoxClass, line: &LineSegClass) -> OverlapType {
    let mut res = CastResultStruct::default();
    collide_line_obbox(line, box_, &mut res);
    eval_overlap_collision(&res)
}

/// `Overlap_Test(const OBBoxClass & box, const TriClass & tri)`: a zero-length move of both, classified.
pub fn overlap_test_obbox_tri(box_: &OBBoxClass, tri: &TriClass) -> OverlapType {
    let mut res = CastResultStruct::default();
    collide_obbox_tri(box_, &Vector3::new(0.0, 0.0, 0.0), tri, &Vector3::new(0.0, 0.0, 0.0), &mut res);
    eval_overlap_collision(&res)
}

/// `Overlap_Test(const AABoxClass & aabox, const OBBoxClass & obbox)`: BOTH ("inside or overlapping") or OUTSIDE.
pub fn overlap_test_aabox_obbox(aabox: &AABoxClass, obbox: &OBBoxClass) -> OverlapType {
    if intersection_test_aabox_obbox(aabox, obbox) {
        OverlapType::Both
    } else {
        OverlapType::OUTSIDE
    }
}

/// `Overlap_Test(const OBBoxClass & obbox, const AABoxClass & aabox)`: BOTH or OUTSIDE.
pub fn overlap_test_obbox_aabox(obbox: &OBBoxClass, aabox: &AABoxClass) -> OverlapType {
    if intersection_test_obbox_aabox(obbox, aabox) {
        OverlapType::Both
    } else {
        OverlapType::OUTSIDE
    }
}

/// `Overlap_Test(const OBBoxClass & box, const OBBoxClass & box2)`: a zero-length move of both, classified.
pub fn overlap_test_obbox_obbox(box_: &OBBoxClass, box2: &OBBoxClass) -> OverlapType {
    let mut res = CastResultStruct::default();
    collide_obbox_obbox(box_, &Vector3::new(0.0, 0.0, 0.0), box2, &Vector3::new(0.0, 0.0, 0.0), &mut res);
    eval_overlap_collision(&res)
}

/// `CollisionMath::Collide(const OBBoxClass & box, const Vector3 & move_vector, const PlaneClass & plane, result)`
/// (colmathobbox.cpp:134): as the AABox version, with the box's own `Project_To_Axis`.
pub fn collide_obbox_plane(box_: &OBBoxClass, move_vector: &Vector3, plane: &PlaneClass, result: &mut CastResultStruct) -> bool {
    let frac: f32;

    let extent = box_.project_to_axis(&plane.n);
    let dist = Vector3::dot_product(&plane.n, &box_.center) + plane.d;
    let mv = Vector3::dot_product(&plane.n, move_vector);

    if dist > extent {
        if dist + mv > extent {
            // entire move ok!
            frac = 1.0f32;
        } else {
            // partial move allowed
            frac = (extent - dist) / mv;
        }
    } else if dist < -extent {
        if dist + mv < -extent {
            // entire move ok!
            frac = 1.0f32;
        } else {
            // partial move allowed
            frac = (-extent - dist) / mv;
        }
    } else {
        result.start_bad = true;
        result.normal = plane.n;
        return true;
    }

    if frac < result.fraction {
        result.fraction = frac;
        result.normal = plane.n;

        if result.compute_contact_point {
            let mut move_dir = *move_vector;
            move_dir.normalize();
            let move_extent = Vector3::dot_product(&move_dir, &box_.extent);
            result.contact_point = box_.center + result.fraction * *move_vector + move_extent * move_dir;
        }
        return true;
    }
    false
}
