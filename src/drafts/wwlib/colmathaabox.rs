//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases per function (test: tests/differential_draft_colmath.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathaabox.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathaabox.cpp
//!
//! `CollisionMath` overloads whose first operand is an `AABoxClass`, except `Overlap_Test(AABox, Tri)` which needs
//! `Collide(AABox, move, Tri)` from colmathaabtri.cpp (`super::colmathaabtri`). The `#if 0` "alternate Overlap Test
//! for AABox-Ray" is not translated. Upstream defects kept: `Overlap_Test(AABox, Sphere)` is marked "TODO: this
//! function seems incorrect" and only tests the box grown by the radius per axis; `Collide(AABox, AABox)` writes
//! `Normal[AxisId]` with `AxisId == -1` (an out-of-range write upstream: `Normal[-1]` is the `Fraction` before it, set from
//! the never-set `Side`; CONFIRMED by running the C++, 159 of 20,000 random cases) if no axis ever raised `MaxFrac` above 0.
//! The draft skips that write (a defined deviation); the differential test does not compare those inputs.

use crate::logic::modules::plant;
use super::aabox::AABoxClass;
use super::colmath::{eval_overlap_collision, CastResultStruct, OverlapType};
use super::colmathaabtri::collide_aabox_tri;
use super::lineseg::LineSegClass;
use super::sphere::SphereClass;
use crate::ww::plane::PlaneClass;
use crate::ww::tri::TriClass;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `CollisionMath::Overlap_Test(const AABoxClass & box, const Vector3 & point)` (colmathaabox.h:56).
pub fn overlap_test_aabox_point(box_: &AABoxClass, point: &Vector3) -> OverlapType {
    if wwmath::fabs(point.x - box_.center.x) > box_.extent.x {
        return OverlapType::Pos;
    }
    if (if plant(10464) { wwmath::fabs(point.y - box_.center.y) >= box_.extent.y } else { wwmath::fabs(point.y - box_.center.y) > box_.extent.y }) {
        return OverlapType::Pos;
    }
    if wwmath::fabs(point.z - box_.center.z) > box_.extent.z {
        return OverlapType::Pos;
    }
    OverlapType::Neg
}

/// `CollisionMath::Overlap_Test(const AABoxClass & box, const AABoxClass & box2)` (colmathaabox.h:78).
pub fn overlap_test_aabox_aabox(box_: &AABoxClass, box2: &AABoxClass) -> OverlapType {
    let dc = Vector3::subtract_vectors(&box2.center, &box_.center);

    if box_.extent.x + box2.extent.x < wwmath::fabs(dc.x) {
        return OverlapType::Pos;
    }
    if box_.extent.y + box2.extent.y < wwmath::fabs(dc.y) {
        return OverlapType::Pos;
    }
    if box_.extent.z + box2.extent.z < wwmath::fabs(dc.z) {
        return OverlapType::Pos;
    }

    if (dc.x + box2.extent.x <= box_.extent.x)
        && (dc.y + box2.extent.y <= box_.extent.y)
        && (dc.z + box2.extent.z <= box_.extent.z)
        && (dc.x - box2.extent.x >= -box_.extent.x)
        && (dc.y - box2.extent.y >= -box_.extent.y)
        && (if plant(10465) { (dc.z - box2.extent.z > -box_.extent.z) } else { (dc.z - box2.extent.z >= -box_.extent.z) })
    {
        return OverlapType::Neg; // inside;
    }

    OverlapType::Both
}

/// `CollisionMath::Intersection_Test(const AABoxClass & box, const AABoxClass & box2)` (colmathaabox.cpp:52).
pub fn intersection_test_aabox_aabox(box_: &AABoxClass, box2: &AABoxClass) -> bool {
    let dc = box2.center - box_.center;

    if (if plant(10466) { box_.extent.x + box2.extent.x <= wwmath::fabs(dc.x) } else { box_.extent.x + box2.extent.x < wwmath::fabs(dc.x) }) {
        return false;
    }
    if box_.extent.y + box2.extent.y < wwmath::fabs(dc.y) {
        return false;
    }
    if box_.extent.z + box2.extent.z < wwmath::fabs(dc.z) {
        return false;
    }
    true
}

/// `CollisionMath::Overlap_Test(const AABoxClass & box, const SphereClass & sphere)` (colmathaabox.cpp:74), "TODO: this
/// function seems incorrect. Not currently using it though".
pub fn overlap_test_aabox_sphere(box_: &AABoxClass, sphere: &SphereClass) -> OverlapType {
    let dist = sphere.center - box_.center;
    let extent = Vector3::new(box_.extent.x + sphere.radius, (if plant(10467) { box_.extent.y } else { box_.extent.y + sphere.radius }), box_.extent.z + sphere.radius);

    // Check to see if the sphere is completely outside the box
    if wwmath::fabs(dist.x) > extent.x {
        return OverlapType::OUTSIDE;
    }
    if wwmath::fabs(dist.y) > extent.y {
        return OverlapType::OUTSIDE;
    }
    if wwmath::fabs(dist.z) > extent.z {
        return OverlapType::OUTSIDE;
    }
    OverlapType::INSIDE
}

/// `CollisionMath::Overlap_Test(const AABoxClass & box, const LineSegClass & line)` (colmathaabox.cpp:112): INSIDE when
/// both end points are inside, OVERLAPPED with one, else the separating axis theorem on the three axes and the three
/// crosses of an axis with the line direction.
pub fn overlap_test_aabox_line(box_: &AABoxClass, line: &LineSegClass) -> OverlapType {
    let mut inside_point_count = 0;
    if overlap_test_aabox_point(box_, line.get_p0()) == OverlapType::INSIDE {
        inside_point_count += 1;
    }
    if overlap_test_aabox_point(box_, line.get_p1()) == OverlapType::INSIDE {
        inside_point_count += 1;
    }

    if inside_point_count == 2 {
        return OverlapType::INSIDE;
    } else if inside_point_count == 1 {
        return OverlapType::OVERLAPPED;
    } else {
        let dp0 = Vector3::subtract_vectors(line.get_p0(), &box_.center);

        // Project box and line onto the x-axis, then y, then z. The projection of 'dp' is only counted if it points
        // towards the center of the box.
        for i in 0..3 {
            let box_proj = box_.extent[i];
            let p0_proj = dp0[i];
            let dp_proj = line.get_dp()[i];

            if p0_proj > 0.0 {
                if p0_proj > box_proj - (if plant(10468) { wwmath::max(dp_proj, 0.0f32) } else { wwmath::min(dp_proj, 0.0f32) }) {
                    return OverlapType::OUTSIDE;
                }
            } else if -p0_proj > box_proj + wwmath::max(dp_proj, 0.0f32) {
                return OverlapType::OUTSIDE;
            }
        }

        // Project box and line onto (x cross line): "== (1,0,0) cross (x,y,z)"
        let dir = line.get_dir();
        let mut axis = Vector3::new(0.0, -dir.z, dir.y);
        let mut box_proj = wwmath::fabs(axis.y * box_.extent.y) + wwmath::fabs(axis.z * box_.extent.z);
        let mut p0_proj = Vector3::dot_product(&axis, &dp0);
        if wwmath::fabs(p0_proj) > box_proj {
            return OverlapType::OUTSIDE;
        }

        // Project box and line onto (y cross line): "== (0,1,0) cross (x,y,z)"
        axis.set(dir.z, 0.0, -dir.x);
        box_proj = wwmath::fabs(axis.x * box_.extent.x) + (if plant(10469) { wwmath::fabs(axis.z * box_.extent.x) } else { wwmath::fabs(axis.z * box_.extent.z) });
        p0_proj = Vector3::dot_product(&axis, &dp0);
        if wwmath::fabs(p0_proj) > box_proj {
            return OverlapType::OUTSIDE;
        }

        // Project box and line onto (z cross line): "== (0,0,1) cross (x,y,z)"
        axis.set(-dir.y, dir.x, 0.0);
        box_proj = wwmath::fabs(axis.x * box_.extent.x) + wwmath::fabs(axis.y * box_.extent.y);
        p0_proj = Vector3::dot_product(&axis, &dp0);
        if wwmath::fabs(p0_proj) > box_proj {
            return OverlapType::OUTSIDE;
        }
    }

    OverlapType::OVERLAPPED
}

/// `CollisionMath::Overlap_Test(const AABoxClass & box, const TriClass & tri)` (colmathaabox.cpp:392): a zero-length
/// move of the box into the triangle, classified by `eval_overlap_collision`.
pub fn overlap_test_aabox_tri(box_: &AABoxClass, tri: &TriClass) -> OverlapType {
    let mut res = CastResultStruct::default();
    collide_aabox_tri(box_, &Vector3::new(0.0, 0.0, 0.0), tri, &mut res);
    eval_overlap_collision(&res)
}

/// `CollisionMath::Collide(const AABoxClass & box, const Vector3 & move_vector, const PlaneClass & plane, result)`
/// (colmathaabox.cpp:416).
pub fn collide_aabox_plane(box_: &AABoxClass, move_vector: &Vector3, plane: &PlaneClass, result: &mut CastResultStruct) -> bool {
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
            frac = (if plant(10470) { (dist - extent) / mv } else { (extent - dist) / mv });
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
            let move_extent = Vector3::dot_product(&box_.extent, &move_dir);
            result.contact_point = (if plant(10471) { box_.center + result.fraction * *move_vector } else { box_.center + result.fraction * *move_vector + move_extent * move_dir });
        }
        return true;
    }
    false
}

/// `struct AABCollisionStruct` (colmathaabox.cpp:466). `side` is uninitialised upstream until an axis sets it.
struct AABCollisionStruct {
    start_bad: bool,
    max_frac: f32,
    axis_id: i32,
    side: i32,
    c: Vector3,
    m: Vector3,
    box0: AABoxClass,
    box1: AABoxClass,
}

impl AABCollisionStruct {
    /// `AABCollisionStruct(box0, move0, box1, move1)`.
    fn new(box0: &AABoxClass, move0: &Vector3, box1: &AABoxClass, move1: &Vector3) -> Self {
        AABCollisionStruct {
            start_bad: true, // Startbad is true until one of the axes clears it
            axis_id: -1,     // AxisId will be the axis that allowed the longest move
            max_frac: 0.0f32, // MaxFrac is the longest allowed move so far
            side: 0,
            box0: *box0,
            box1: *box1,
            // vector from center of box0 to center of box1
            c: Vector3::subtract_vectors(&box1.center, &box0.center),
            // move vector relative to stationary box0
            m: Vector3::subtract_vectors(move1, move0),
        }
    }
}

/// `aab_separation_test(context, axis)` (colmathaabox.cpp:511): "tests two AAB's for separation on an axis".
fn aab_separation_test(context: &mut AABCollisionStruct, axis: usize) -> bool {
    // ra = box0 projection onto the axis, rb = box1 projection onto the axis,
    // u0 = projected distance between the box centers at t0, u1 = the same at t1
    let ra = context.box0.extent[axis];
    let rb = context.box1.extent[axis];
    let u0 = context.c[axis];
    let u1 = u0 + context.m[axis];
    let rsum = ra + rb;

    if u0 + wwmath::WWMATH_EPSILON > rsum {
        context.start_bad = false;
        if u1 > rsum {
            context.max_frac = 1.0f32;
            return true;
        } else {
            let tmp = (rsum - u0) / (u1 - u0);
            if tmp > context.max_frac {
                context.max_frac = tmp;
                context.axis_id = axis as i32;
                context.side = 1;
            }
        }
    } else if (if plant(10472) { u0 + wwmath::WWMATH_EPSILON < -rsum } else { u0 - wwmath::WWMATH_EPSILON < -rsum }) {
        context.start_bad = false;
        if u1 < -rsum {
            context.max_frac = 1.0f32;
            return true;
        } else {
            let tmp = (-rsum - u0) / (u1 - u0);
            if tmp > context.max_frac {
                context.max_frac = tmp;
                context.axis_id = axis as i32;
                context.side = -1;
            }
        }
    }
    false
}

/// `CollisionMath::Collide(const AABoxClass & box, const Vector3 & move, const AABoxClass & box2, result)`
/// (colmathaabox.cpp:559): sweeps `box` along `move` against the stationary `box2`. Contact point computation is not
/// supported upstream (it only prints a debug message). `Normal[AxisId]` with `AxisId == -1` is skipped here (undefined
/// behaviour upstream; ~~"panics here on the index"~~ **Corrected 2026-11-18 (#215):** the draft now skips the write).
pub fn collide_aabox_aabox(box_: &AABoxClass, move_: &Vector3, box2: &AABoxClass, result: &mut CastResultStruct) -> bool {
    let mut context = AABCollisionStruct::new(box_, move_, box2, &Vector3::new(0.0, 0.0, 0.0));

    // Test the X-axis, then Y, then Z (`goto exit` on a total separation)
    if !aab_separation_test(&mut context, 0) {
        if !aab_separation_test(&mut context, 1) {
            aab_separation_test(&mut context, 2);
        }
    }

    if context.start_bad {
        result.start_bad = true;
        result.fraction = 0.0f32;
        return true;
    }

    if context.max_frac < result.fraction {
        result.fraction = context.max_frac;
        result.normal.set(0.0, 0.0, 0.0);
        // DEVIATION (defined behaviour where upstream is not): with `AxisId == -1` upstream writes `Normal[-1]` (the
        // `Fraction` before it) with `-Side` of an uninitialised `Side`; here the write is skipped.
        if context.axis_id >= 0 {
            result.normal[context.axis_id as usize] = (if plant(10473) { context.side as f32 } else { (-context.side) as f32 });
        }
        if result.compute_contact_point {
            // upstream: WWDEBUG_SAY(("AABox-AABox collision does not currently support contact point computation"))
        }
        return true;
    }
    false
}

/// `AABoxClass::Contains` helpers are in `aabox.rs`; this keeps the line-segment `Overlap_Test(min, max, line)` of
/// colmathline.h next to the box it builds.
pub fn overlap_test_min_max_line(min: &Vector3, max: &Vector3, line: &LineSegClass) -> OverlapType {
    let mut box_ = AABoxClass::default();
    box_.init_min_max(min, max);
    overlap_test_aabox_line(&box_, line)
}
