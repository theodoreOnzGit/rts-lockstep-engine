//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathobbobb.cpp
//!
//! OBBox-OBBox intersection (boolean, 15 separating axes) and swept collision (time of impact, contact normal and
//! point after Dave Eberly's paper); the OBB-ABB and ABB-OBB forms wrap the AABox in an identity-basis `OBBoxClass`.
//! Upstream notes kept: `AXISLEN_EPSILON2` is `WWMATH_EPSILON * WWMATH_EPSILON`; the `INTERSECTION` axis id falls
//! through to `AXIS_A0` in `compute_contact_normal` ("Fatal assert disabled for demo"); `AXIS_A1B1` in
//! `compute_contact_point` sets `den = 1.0f / (1.0f - AB[1][1]*AB[1][1])` and then DIVIDES by it (the other eight cases
//! use `den = 1 - AB*AB`); the three "false collision" axes test `!result->StartBad` (the caller's result, not the
//! context's); `Side` is uninitialised upstream until some axis sets it (0 here).

use super::aabox::AABoxClass;
use super::colmath::CastResultStruct;
use super::obbox::OBBoxClass;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `AXISLEN_EPSILON2`: "squared length of a separating axis must be larger than this".
const AXISLEN_EPSILON2: f32 = wwmath::WWMATH_EPSILON * wwmath::WWMATH_EPSILON;

/// The axis ids (colmathobbobb.cpp:51): `INTERSECTION = 0, AXIS_A0, .. AXIS_A2B2`.
const INTERSECTION: i32 = 0;
const AXIS_A0: i32 = 1;
const AXIS_A1: i32 = 2;
const AXIS_A2: i32 = 3;
const AXIS_B0: i32 = 4;
const AXIS_B1: i32 = 5;
const AXIS_B2: i32 = 6;
const AXIS_A0B0: i32 = 7;
const AXIS_A0B1: i32 = 8;
const AXIS_A0B2: i32 = 9;
const AXIS_A1B0: i32 = 10;
const AXIS_A1B1: i32 = 11;
const AXIS_A1B2: i32 = 12;
const AXIS_A2B0: i32 = 13;
const AXIS_A2B1: i32 = 14;
const AXIS_A2B2: i32 = 15;

/// The box's basis vectors in world space (the columns of `Basis`).
fn box_axes(b: &OBBoxClass) -> [Vector3; 3] {
    [
        Vector3::new(b.basis[0][0], b.basis[1][0], b.basis[2][0]),
        Vector3::new(b.basis[0][1], b.basis[1][1], b.basis[2][1]),
        Vector3::new(b.basis[0][2], b.basis[1][2], b.basis[2][2]),
    ]
}

/// `struct ObbIntersectionStruct` (colmathobbobb.cpp:123).
struct ObbIntersectionStruct {
    c: Vector3,
    a: [Vector3; 3],
    b: [Vector3; 3],
    ab: [[f32; 3]; 3],
    box0: OBBoxClass,
    box1: OBBoxClass,
}

impl ObbIntersectionStruct {
    fn new(box0: &OBBoxClass, box1: &OBBoxClass) -> Self {
        ObbIntersectionStruct { c: Vector3::subtract_vectors(&box1.center, &box0.center), a: box_axes(box0), b: box_axes(box1), ab: [[0.0; 3]; 3], box0: *box0, box1: *box1 }
    }
}

/// `obb_intersect_box0_basis(context, axis_index)`: true when separated on box0's axis.
fn obb_intersect_box0_basis(context: &ObbIntersectionStruct, axis_index: usize) -> bool {
    // ra = box0 projection onto the axis, rb = box1 projection onto the axis
    let ra = context.box0.extent[axis_index];
    let rb = wwmath::fabs(context.box1.extent[0] * context.ab[axis_index][0]) + wwmath::fabs(context.box1.extent[1] * context.ab[axis_index][1]) + wwmath::fabs(context.box1.extent[2] * context.ab[axis_index][2]);
    let rsum = ra + rb;

    // u = projected distance between the box centers
    let u = Vector3::dot_product(&context.c, &context.a[axis_index]);

    // (gth) the epsilon here was not scaled to the length of the axis so it caused problems (so none is used)
    (u > rsum) || (u < -rsum)
}

/// `obb_intersect_box1_basis(context, axis_index)`.
fn obb_intersect_box1_basis(context: &ObbIntersectionStruct, axis_index: usize) -> bool {
    let ra = wwmath::fabs(context.box0.extent[0] * context.ab[0][axis_index]) + wwmath::fabs(context.box0.extent[1] * context.ab[1][axis_index]) + wwmath::fabs(context.box0.extent[2] * context.ab[2][axis_index]);
    let rb = context.box1.extent[axis_index];
    let rsum = ra + rb;

    let u = Vector3::dot_product(&context.c, &context.b[axis_index]);

    (u > rsum) || (u < -rsum)
}

/// `obb_intersect_axis(context, axis, ra, rb)`.
fn obb_intersect_axis(context: &ObbIntersectionStruct, axis: &Vector3, ra: f32, rb: f32) -> bool {
    let rsum = ra + rb;
    let u = Vector3::dot_product(&context.c, axis);
    (u > rsum) || (u < -rsum)
}

/// `intersect_obb_obb(context)` (colmathobbobb.cpp:262): true when the boxes intersect.
fn intersect_obb_obb(context: &mut ObbIntersectionStruct) -> bool {
    // Axes A0, A1, A2 (the first three also compute the AB dot products); a zero extent skips the test
    for i in 0..3 {
        for j in 0..3 {
            context.ab[i][j] = Vector3::dot_product(&context.a[i], &context.b[j]);
        }
        if context.box0.extent[i] > 0.0f32 {
            if obb_intersect_box0_basis(context, i) {
                return false;
            }
        }
    }

    // Axes B0, B1, B2
    for i in 0..3 {
        if context.box1.extent[i] > 0.0f32 {
            if obb_intersect_box1_basis(context, i) {
                return false;
            }
        }
    }

    // The nine cross axes. Each has `ra` from box0's two other extents and `rb` from box1's.
    let e0 = context.box0.extent;
    let e1 = context.box1.extent;
    let ab = context.ab;
    let f = wwmath::fabs;

    // A0xB0
    let axis = Vector3::cross_product(&context.a[0], &context.b[0]);
    if axis.length2() > AXISLEN_EPSILON2 {
        let ra = f(e0[1] * ab[2][0]) + f(e0[2] * ab[1][0]);
        let rb = f(e1[1] * ab[0][2]) + f(e1[2] * ab[0][1]);
        if obb_intersect_axis(context, &axis, ra, rb) {
            return false;
        }
    }
    // A0xB1
    let axis = Vector3::cross_product(&context.a[0], &context.b[1]);
    if axis.length2() > AXISLEN_EPSILON2 {
        let ra = f(e0[1] * ab[2][1]) + f(e0[2] * ab[1][1]);
        let rb = f(e1[0] * ab[0][2]) + f(e1[2] * ab[0][0]);
        if obb_intersect_axis(context, &axis, ra, rb) {
            return false;
        }
    }
    // A0xB2
    let axis = Vector3::cross_product(&context.a[0], &context.b[2]);
    if axis.length2() > AXISLEN_EPSILON2 {
        let ra = f(e0[1] * ab[2][2]) + f(e0[2] * ab[1][2]);
        let rb = f(e1[0] * ab[0][1]) + f(e1[1] * ab[0][0]);
        if obb_intersect_axis(context, &axis, ra, rb) {
            return false;
        }
    }
    // A1xB0
    let axis = Vector3::cross_product(&context.a[1], &context.b[0]);
    if axis.length2() > AXISLEN_EPSILON2 {
        let ra = f(e0[0] * ab[2][0]) + f(e0[2] * ab[0][0]);
        let rb = f(e1[1] * ab[1][2]) + f(e1[2] * ab[1][1]);
        if obb_intersect_axis(context, &axis, ra, rb) {
            return false;
        }
    }
    // A1xB1
    let axis = Vector3::cross_product(&context.a[1], &context.b[1]);
    if axis.length2() > AXISLEN_EPSILON2 {
        let ra = f(e0[0] * ab[2][1]) + f(e0[2] * ab[0][1]);
        let rb = f(e1[0] * ab[1][2]) + f(e1[2] * ab[1][0]);
        if obb_intersect_axis(context, &axis, ra, rb) {
            return false;
        }
    }
    // A1xB2
    let axis = Vector3::cross_product(&context.a[1], &context.b[2]);
    if axis.length2() > AXISLEN_EPSILON2 {
        let ra = f(e0[0] * ab[2][2]) + f(e0[2] * ab[0][2]);
        let rb = f(e1[0] * ab[1][1]) + f(e1[1] * ab[1][0]);
        if obb_intersect_axis(context, &axis, ra, rb) {
            return false;
        }
    }
    // A2xB0
    let axis = Vector3::cross_product(&context.a[2], &context.b[0]);
    if axis.length2() > AXISLEN_EPSILON2 {
        let ra = f(e0[0] * ab[1][0]) + f(e0[1] * ab[0][0]);
        let rb = f(e1[1] * ab[2][2]) + f(e1[2] * ab[2][1]);
        if obb_intersect_axis(context, &axis, ra, rb) {
            return false;
        }
    }
    // A2xB1
    let axis = Vector3::cross_product(&context.a[2], &context.b[1]);
    if axis.length2() > AXISLEN_EPSILON2 {
        let ra = f(e0[0] * ab[1][1]) + f(e0[1] * ab[0][1]);
        let rb = f(e1[0] * ab[2][2]) + f(e1[2] * ab[2][0]);
        if obb_intersect_axis(context, &axis, ra, rb) {
            return false;
        }
    }
    // A2xB2
    let axis = Vector3::cross_product(&context.a[2], &context.b[2]);
    if axis.length2() > AXISLEN_EPSILON2 {
        let ra = f(e0[0] * ab[1][2]) + f(e0[1] * ab[0][2]);
        let rb = f(e1[0] * ab[2][1]) + f(e1[1] * ab[2][0]);
        if obb_intersect_axis(context, &axis, ra, rb) {
            return false;
        }
    }

    true
}

/// `CollisionMath::Intersection_Test(const OBBoxClass & box0, const OBBoxClass & box1)` (colmathobbobb.cpp:446).
pub fn intersection_test_obbox_obbox(box0: &OBBoxClass, box1: &OBBoxClass) -> bool {
    let mut context = ObbIntersectionStruct::new(box0, box1);
    intersect_obb_obb(&mut context)
}

/// `CollisionMath::Intersection_Test(const OBBoxClass & box0, const AABoxClass & box1)`: `OBBoxClass(center, extent)`.
pub fn intersection_test_obbox_aabox(box0: &OBBoxClass, box1: &AABoxClass) -> bool {
    let obbox1 = OBBoxClass::new(&box1.center, &box1.extent);
    let mut context = ObbIntersectionStruct::new(box0, &obbox1);
    intersect_obb_obb(&mut context)
}

/// `CollisionMath::Intersection_Test(const AABoxClass & box0, const OBBoxClass & box1)`.
pub fn intersection_test_aabox_obbox(box0: &AABoxClass, box1: &OBBoxClass) -> bool {
    let obbox0 = OBBoxClass::new(&box0.center, &box0.extent);
    let mut context = ObbIntersectionStruct::new(&obbox0, box1);
    intersect_obb_obb(&mut context)
}

/// `struct ObbCollisionStruct` (colmathobbobb.cpp:516).
struct ObbCollisionStruct {
    start_bad: bool,
    max_frac: f32,
    axis_id: i32,
    side: i32,
    test_axis_id: i32,
    test_axis: Vector3,
    c: Vector3,
    m: Vector3,
    a: [Vector3; 3],
    b: [Vector3; 3],
    ab: [[f32; 3]; 3],
    box0: OBBoxClass,
    move0: Vector3,
    box1: OBBoxClass,
    move1: Vector3,
}

impl ObbCollisionStruct {
    /// `ObbCollisionStruct(box0, move0, box1, move1)`: `StartBad(true)`, `AxisId(INTERSECTION)`, `MaxFrac(0)`.
    fn new(box0: &OBBoxClass, move0: &Vector3, box1: &OBBoxClass, move1: &Vector3) -> Self {
        ObbCollisionStruct {
            start_bad: true, // Startbad is true until one of the axes clears it
            max_frac: 0.0f32, // MaxFrac is the longest allowed move so far
            axis_id: INTERSECTION, // AxisId will be the axis that allowed the longest move
            side: 0,
            test_axis_id: INTERSECTION,
            test_axis: Vector3::new(0.0, 0.0, 0.0),
            c: Vector3::subtract_vectors(&box1.center, &box0.center), // vector from center of box0 to center of box1
            m: Vector3::subtract_vectors(move1, move0), // move vector relative to stationary box0
            a: box_axes(box0),
            b: box_axes(box1),
            ab: [[0.0; 3]; 3],
            box0: *box0,
            move0: *move0,
            box1: *box1,
            move1: *move1,
        }
    }
}

/// `obb_separation_test(context, ra, rb, u0, u1)`: true when the boxes never touch along this axis for the whole move.
fn obb_separation_test(context: &mut ObbCollisionStruct, ra: f32, rb: f32, u0: f32, u1: f32) -> bool {
    let rsum = ra + rb;

    if u0 + wwmath::WWMATH_EPSILON > rsum {
        context.start_bad = false;
        if u1 > rsum {
            context.max_frac = 1.0f32;
            return true;
        } else if wwmath::fabs(u1 - u0) > 0.0f32 {
            let tmp = (rsum - u0) / (u1 - u0);
            if tmp > context.max_frac {
                context.max_frac = tmp;
                context.axis_id = context.test_axis_id;
                context.side = 1;
            }
        }
    } else if u0 - wwmath::WWMATH_EPSILON < -rsum {
        context.start_bad = false;
        if u1 < -rsum {
            context.max_frac = 1.0f32;
            return true;
        } else if wwmath::fabs(u1 - u0) > 0.0f32 {
            let tmp = (-rsum - u0) / (u1 - u0);
            if tmp > context.max_frac {
                context.max_frac = tmp;
                context.axis_id = context.test_axis_id;
                context.side = -1;
            }
        }
    }
    false
}

/// `obb_check_box0_basis(context, axis_index)`.
fn obb_check_box0_basis(context: &mut ObbCollisionStruct, axis_index: usize) -> bool {
    let ra = context.box0.extent[axis_index];
    let rb = wwmath::fabs(context.box1.extent[0] * context.ab[axis_index][0]) + wwmath::fabs(context.box1.extent[1] * context.ab[axis_index][1]) + wwmath::fabs(context.box1.extent[2] * context.ab[axis_index][2]);

    // u0 = projected distance between the box centers at t0, u1 = at t1
    let u0 = Vector3::dot_product(&context.c, &context.a[axis_index]);
    let u1 = u0 + Vector3::dot_product(&context.m, &context.a[axis_index]);

    obb_separation_test(context, ra, rb, u0, u1)
}

/// `obb_check_box1_basis(context, axis_index)`.
fn obb_check_box1_basis(context: &mut ObbCollisionStruct, axis_index: usize) -> bool {
    let ra = wwmath::fabs(context.box0.extent[0] * context.ab[0][axis_index]) + wwmath::fabs(context.box0.extent[1] * context.ab[1][axis_index]) + wwmath::fabs(context.box0.extent[2] * context.ab[2][axis_index]);
    let rb = context.box1.extent[axis_index];

    let u0 = Vector3::dot_product(&context.c, &context.b[axis_index]);
    let u1 = u0 + Vector3::dot_product(&context.m, &context.b[axis_index]);

    obb_separation_test(context, ra, rb, u0, u1)
}

/// `obb_check_axis(context, ra, rb)`: the context's `TestAxis`.
fn obb_check_axis(context: &mut ObbCollisionStruct, ra: f32, rb: f32) -> bool {
    let u0 = Vector3::dot_product(&context.c, &context.test_axis);
    let u1 = u0 + Vector3::dot_product(&context.m, &context.test_axis);

    obb_separation_test(context, ra, rb, u0, u1)
}

/// `obb_compute_projections(context, &ra, &rb)`: both boxes projected onto `TestAxis`.
fn obb_compute_projections(context: &ObbCollisionStruct) -> (f32, f32) {
    let ra = context.box0.extent.x * wwmath::fabs(Vector3::dot_product(&context.a[0], &context.test_axis))
        + context.box0.extent.y * wwmath::fabs(Vector3::dot_product(&context.a[1], &context.test_axis))
        + context.box0.extent.z * wwmath::fabs(Vector3::dot_product(&context.a[2], &context.test_axis));
    let rb = context.box1.extent.x * wwmath::fabs(Vector3::dot_product(&context.b[0], &context.test_axis))
        + context.box1.extent.y * wwmath::fabs(Vector3::dot_product(&context.b[1], &context.test_axis))
        + context.box1.extent.z * wwmath::fabs(Vector3::dot_product(&context.b[2], &context.test_axis));
    (ra, rb)
}

/// `compute_contact_normal(context, result)` (colmathobbobb.cpp:755): the axis (or a normalised cross product), scaled
/// by `-Side`. `INTERSECTION` falls through to `AXIS_A0`.
fn compute_contact_normal(context: &ObbCollisionStruct, result: &mut CastResultStruct) {
    let cross = |i: usize, j: usize| {
        let mut n = Vector3::cross_product(&context.a[i], &context.b[j]);
        n.normalize();
        n
    };
    result.normal = match context.axis_id {
        INTERSECTION | AXIS_A0 => context.a[0],
        AXIS_A1 => context.a[1],
        AXIS_A2 => context.a[2],
        AXIS_B0 => context.b[0],
        AXIS_B1 => context.b[1],
        AXIS_B2 => context.b[2],
        AXIS_A0B0 => cross(0, 0),
        AXIS_A0B1 => cross(0, 1),
        AXIS_A0B2 => cross(0, 2),
        AXIS_A1B0 => cross(1, 0),
        AXIS_A1B1 => cross(1, 1),
        AXIS_A1B2 => cross(1, 2),
        AXIS_A2B0 => cross(2, 0),
        AXIS_A2B1 => cross(2, 1),
        AXIS_A2B2 => cross(2, 2),
        _ => result.normal,
    };
    // `result->Normal *= -context.Side` (Side is an int)
    result.normal *= (-context.side) as f32;
}

/// `eval_side(ab, side)`: the sign of `ab` applied to `side`.
fn eval_side(ab: f32, side: f32) -> f32 {
    if ab > 0.0f32 {
        side
    } else if ab < 0.0f32 {
        -side
    } else {
        0.0f32
    }
}

/// `compute_contact_point(context, result)` (colmathobbobb.cpp:874). "PROBLEMS: in case of edge-face or face-face or
/// perfectly aligned edge-edge this routine is only computing a single point."
fn compute_contact_point(context: &ObbCollisionStruct, result: &mut CastResultStruct) {
    let mut x = [0.0f32; 3]; // box0 parameters
    let mut y = [0.0f32; 3]; // box1 parameters
    let mut dcnew = Vector3::new(0.0, 0.0, 0.0);
    let side = context.side as f32;
    let ab = &context.ab;

    if context.axis_id >= AXIS_A0B0 {
        let cnew0 = Vector3::add_vectors(&context.box0.center, &(context.max_frac * context.move0));
        let cnew1 = Vector3::add_vectors(&context.box1.center, &(context.max_frac * context.move1));
        dcnew = Vector3::subtract_vectors(&cnew1, &cnew0);
    }

    match context.axis_id {
        AXIS_A0 | AXIS_A1 | AXIS_A2 => {
            let i = (context.axis_id - AXIS_A0) as usize;
            for j in 0..3 {
                y[j] = -eval_side(ab[i][j], side);
            }
            result.contact_point = context.box1.compute_point(&y);
            result.contact_point += result.fraction * context.move1;
            return;
        }
        AXIS_B0 | AXIS_B1 | AXIS_B2 => {
            let j = (context.axis_id - AXIS_B0) as usize;
            for i in 0..3 {
                x[i] = eval_side(ab[i][j], side);
            }
            result.contact_point = context.box0.compute_point(&x);
            result.contact_point += result.fraction * context.move0;
            return;
        }
        AXIS_A0B0 => {
            x[1] = -eval_side(ab[2][0], side) * context.box0.extent[1];
            x[2] = eval_side(ab[1][0], side) * context.box0.extent[2];
            y[1] = -eval_side(ab[0][2], side) * context.box1.extent[1];
            y[2] = eval_side(ab[0][1], side) * context.box1.extent[2];
            let den = 1.0f32 - ab[0][0] * ab[0][0];
            if wwmath::fabs(den) > 0.0f32 {
                x[0] = Vector3::dot_product(&context.a[0], &dcnew);
                x[0] += ab[0][0] * (Vector3::dot_product(&-context.b[0], &dcnew) + ab[1][0] * x[1] + ab[2][0] * x[2]);
                x[0] += ab[0][1] * y[1] + ab[0][2] * y[2];
                x[0] /= den;
            } else {
                x[0] = 0.0f32;
            }
        }
        AXIS_A0B1 => {
            x[1] = -eval_side(ab[2][1], side) * context.box0.extent[1];
            x[2] = eval_side(ab[1][1], side) * context.box0.extent[2];
            y[0] = eval_side(ab[0][2], side) * context.box1.extent[0];
            y[2] = -eval_side(ab[0][0], side) * context.box1.extent[2];
            let den = 1.0f32 - ab[0][1] * ab[0][1];
            if wwmath::fabs(den) > 0.0f32 {
                x[0] = Vector3::dot_product(&context.a[0], &dcnew);
                x[0] += ab[0][1] * (Vector3::dot_product(&-context.b[1], &dcnew) + ab[1][1] * x[1] + ab[2][1] * x[2]);
                x[0] += ab[0][0] * y[0] + ab[0][2] * y[2];
                x[0] /= den;
            } else {
                x[0] = 0.0f32;
            }
        }
        AXIS_A0B2 => {
            x[1] = -eval_side(ab[2][2], side) * context.box0.extent[1];
            x[2] = eval_side(ab[1][2], side) * context.box0.extent[2];
            y[0] = -eval_side(ab[0][1], side) * context.box1.extent[0];
            y[1] = eval_side(ab[0][0], side) * context.box1.extent[1];
            let den = 1.0f32 - ab[0][2] * ab[0][2];
            if wwmath::fabs(den) > 0.0f32 {
                x[0] = Vector3::dot_product(&context.a[0], &dcnew);
                x[0] += ab[0][2] * (Vector3::dot_product(&-context.b[2], &dcnew) + ab[1][2] * x[1] + ab[2][2] * x[2]);
                x[0] += ab[0][0] * y[0] + ab[0][1] * y[1];
                x[0] /= den;
            } else {
                x[0] = 0.0f32;
            }
        }
        AXIS_A1B0 => {
            x[0] = eval_side(ab[2][0], side) * context.box0.extent[0];
            x[2] = -eval_side(ab[0][0], side) * context.box0.extent[2];
            y[1] = -eval_side(ab[1][2], side) * context.box1.extent[1];
            y[2] = eval_side(ab[1][1], side) * context.box1.extent[2];
            let den = 1.0f32 - ab[1][0] * ab[1][0];
            if wwmath::fabs(den) > 0.0f32 {
                x[1] = Vector3::dot_product(&context.a[1], &dcnew);
                x[1] += ab[1][0] * (Vector3::dot_product(&-context.b[0], &dcnew) + ab[0][0] * x[0] + ab[2][0] * x[2]);
                x[1] += ab[1][1] * y[1] + ab[1][2] * y[2];
                x[1] /= den;
            } else {
                x[1] = 0.0f32;
            }
        }
        AXIS_A1B1 => {
            x[0] = eval_side(ab[2][1], side) * context.box0.extent[0];
            x[2] = -eval_side(ab[0][1], side) * context.box0.extent[2];
            y[0] = eval_side(ab[1][2], side) * context.box1.extent[0];
            y[2] = -eval_side(ab[1][0], side) * context.box1.extent[2];
            // UPSTREAM DEFECT KEPT: the reciprocal, then divided by below
            let den = 1.0f32 / (1.0f32 - ab[1][1] * ab[1][1]);
            if wwmath::fabs(den) > 0.0f32 {
                x[1] = Vector3::dot_product(&context.a[1], &dcnew);
                x[1] += ab[1][1] * (Vector3::dot_product(&-context.b[1], &dcnew) + ab[0][1] * x[0] + ab[2][1] * x[2]);
                x[1] += ab[1][0] * y[0] + ab[1][2] * y[2];
                x[1] /= den;
            } else {
                x[1] = 0.0f32;
            }
        }
        AXIS_A1B2 => {
            x[0] = eval_side(ab[2][2], side) * context.box0.extent[0];
            x[2] = -eval_side(ab[0][2], side) * context.box0.extent[2];
            y[0] = -eval_side(ab[1][1], side) * context.box1.extent[0];
            y[1] = eval_side(ab[1][0], side) * context.box1.extent[1];
            let den = 1.0f32 - ab[1][2] * ab[1][2];
            if wwmath::fabs(den) > 0.0f32 {
                x[1] = Vector3::dot_product(&context.a[1], &dcnew);
                x[1] += ab[1][2] * (Vector3::dot_product(&-context.b[2], &dcnew) + ab[0][2] * x[0] + ab[2][2] * x[2]);
                x[1] += ab[1][0] * y[0] + ab[1][1] * y[1];
                x[1] /= den;
            } else {
                x[1] = 0.0f32;
            }
        }
        AXIS_A2B0 => {
            x[0] = -eval_side(ab[1][0], side) * context.box0.extent[0];
            x[1] = eval_side(ab[0][0], side) * context.box0.extent[1];
            y[1] = -eval_side(ab[2][2], side) * context.box1.extent[1];
            y[2] = eval_side(ab[2][1], side) * context.box1.extent[2];
            let den = 1.0f32 - ab[2][0] * ab[2][0];
            if wwmath::fabs(den) > 0.0f32 {
                x[2] = Vector3::dot_product(&context.a[2], &dcnew);
                x[2] += ab[2][0] * (Vector3::dot_product(&-context.b[0], &dcnew) + ab[0][0] * x[0] + ab[1][0] * x[1]);
                x[2] += ab[2][1] * y[1] + ab[2][2] * y[2];
                x[2] /= den;
            } else {
                x[2] = 0.0f32;
            }
        }
        AXIS_A2B1 => {
            x[0] = -eval_side(ab[1][1], side) * context.box0.extent[0];
            x[1] = eval_side(ab[0][1], side) * context.box0.extent[1];
            y[0] = eval_side(ab[2][2], side) * context.box1.extent[0];
            y[2] = -eval_side(ab[2][0], side) * context.box1.extent[2];
            let den = 1.0f32 - ab[2][1] * ab[2][1];
            if wwmath::fabs(den) > 0.0f32 {
                x[2] = Vector3::dot_product(&context.a[2], &dcnew);
                x[2] += ab[2][1] * (Vector3::dot_product(&-context.b[1], &dcnew) + ab[0][1] * x[0] + ab[1][1] * x[1]);
                x[2] += ab[2][0] * y[0] + ab[2][2] * y[2];
                x[2] /= den;
            } else {
                x[2] = 0.0f32;
            }
        }
        AXIS_A2B2 => {
            x[0] = -eval_side(ab[1][2], side) * context.box0.extent[0];
            x[1] = eval_side(ab[0][2], side) * context.box0.extent[1];
            y[0] = -eval_side(ab[2][1], side) * context.box1.extent[0];
            y[1] = eval_side(ab[2][0], side) * context.box1.extent[1];
            let den = 1.0f32 - ab[2][2] * ab[2][2];
            if wwmath::fabs(den) > 0.0f32 {
                x[2] = Vector3::dot_product(&context.a[2], &dcnew);
                x[2] += ab[2][2] * (Vector3::dot_product(&-context.b[2], &dcnew) + ab[0][2] * x[0] + ab[1][2] * x[1]);
                x[2] += ab[2][0] * y[0] + ab[2][1] * y[1];
                x[2] /= den;
            } else {
                x[2] = 0.0f32;
            }
        }
        _ => {}
    }

    // all but the first two cases fall through to here
    result.contact_point.x = context.box0.center.x + x[0] * context.a[0].x + x[1] * context.a[1].x + x[2] * context.a[2].x;
    result.contact_point.y = context.box0.center.y + x[0] * context.a[0].y + x[1] * context.a[1].y + x[2] * context.a[2].y;
    result.contact_point.z = context.box0.center.z + x[0] * context.a[0].z + x[1] * context.a[1].z + x[2] * context.a[2].z;

    result.contact_point = Vector3::add_vectors(&result.contact_point, &(result.fraction * context.move0));
}

/// `collide_obb_obb(context, result)` (colmathobbobb.cpp:1000): the 15 axes, then three "false collision" axes from
/// `A[i] x Move0`; `goto exit` is the early `break` out of the labelled block.
fn collide_obb_obb(context: &mut ObbCollisionStruct, result: &mut CastResultStruct) -> bool {
    'exit: {
        // Axes A0, A1, A2 (computing the AB dot products as they go)
        for i in 0..3usize {
            context.test_axis_id = AXIS_A0 + i as i32;
            for j in 0..3 {
                context.ab[i][j] = Vector3::dot_product(&context.a[i], &context.b[j]);
            }
            if obb_check_box0_basis(context, i) {
                break 'exit;
            }
        }

        // Axes B0, B1, B2
        for i in 0..3usize {
            context.test_axis_id = AXIS_B0 + i as i32;
            if obb_check_box1_basis(context, i) {
                break 'exit;
            }
        }

        // The nine cross axes, in upstream's order: (A index, B index, ra terms, rb terms) with
        // ra = |e0[p] * ab[q][r]| + |e0[s] * ab[t][u]|, likewise rb.
        let e0 = context.box0.extent;
        let e1 = context.box1.extent;
        let ab = context.ab;
        let f = wwmath::fabs;
        // (i, j, axis id)
        let table: [(usize, usize, i32); 9] = [
            (0, 0, AXIS_A0B0),
            (0, 1, AXIS_A0B1),
            (0, 2, AXIS_A0B2),
            (1, 0, AXIS_A1B0),
            (1, 1, AXIS_A1B1),
            (1, 2, AXIS_A1B2),
            (2, 0, AXIS_A2B0),
            (2, 1, AXIS_A2B1),
            (2, 2, AXIS_A2B2),
        ];
        for (i, j, id) in table.iter() {
            let (i, j) = (*i, *j);
            context.test_axis = Vector3::cross_product(&context.a[i], &context.b[j]);
            context.test_axis_id = *id;
            if context.test_axis.length2() > AXISLEN_EPSILON2 {
                let (ra, rb) = match (i, j) {
                    (0, 0) => (f(e0[1] * ab[2][0]) + f(e0[2] * ab[1][0]), f(e1[1] * ab[0][2]) + f(e1[2] * ab[0][1])),
                    (0, 1) => (f(e0[1] * ab[2][1]) + f(e0[2] * ab[1][1]), f(e1[0] * ab[0][2]) + f(e1[2] * ab[0][0])),
                    (0, 2) => (f(e0[1] * ab[2][2]) + f(e0[2] * ab[1][2]), f(e1[0] * ab[0][1]) + f(e1[1] * ab[0][0])),
                    (1, 0) => (f(e0[0] * ab[2][0]) + f(e0[2] * ab[0][0]), f(e1[1] * ab[1][2]) + f(e1[2] * ab[1][1])),
                    (1, 1) => (f(e0[0] * ab[2][1]) + f(e0[2] * ab[0][1]), f(e1[0] * ab[1][2]) + f(e1[2] * ab[1][0])),
                    (1, 2) => (f(e0[0] * ab[2][2]) + f(e0[2] * ab[0][2]), f(e1[0] * ab[1][1]) + f(e1[1] * ab[1][0])),
                    (2, 0) => (f(e0[0] * ab[1][0]) + f(e0[1] * ab[0][0]), f(e1[1] * ab[2][2]) + f(e1[2] * ab[2][1])),
                    (2, 1) => (f(e0[0] * ab[1][1]) + f(e0[1] * ab[0][1]), f(e1[0] * ab[2][2]) + f(e1[2] * ab[2][0])),
                    _ => (f(e0[0] * ab[1][2]) + f(e0[1] * ab[0][2]), f(e1[0] * ab[2][1]) + f(e1[1] * ab[2][0])),
                };
                if obb_check_axis(context, ra, rb) {
                    break 'exit;
                }
            }
        }

        if !result.start_bad {
            context.test_axis_id = context.axis_id;

            // Final three checks are for eliminating false collisions: A[i] x Move0
            for i in 0..3usize {
                context.test_axis = Vector3::cross_product(&context.a[i], &context.move0);
                if context.test_axis.length2() > AXISLEN_EPSILON2 {
                    let (ra, rb) = obb_compute_projections(context);
                    if obb_check_axis(context, ra, rb) {
                        break 'exit;
                    }
                }
            }
        }
    }

    // exit:
    if context.start_bad {
        result.start_bad = true;
        result.fraction = 0.0f32;
        return true;
    }

    if context.max_frac < result.fraction {
        result.fraction = context.max_frac;
        compute_contact_normal(context, result);

        if result.compute_contact_point {
            compute_contact_point(context, result);
        }
        return true;
    }
    false
}

/// `CollisionMath::Collide(const OBBoxClass & box0, move0, const OBBoxClass & box1, move1, result)` (:1344).
pub fn collide_obbox_obbox(box0: &OBBoxClass, move0: &Vector3, box1: &OBBoxClass, move1: &Vector3, result: &mut CastResultStruct) -> bool {
    let mut context = ObbCollisionStruct::new(box0, move0, box1, move1);
    collide_obb_obb(&mut context, result)
}

/// `CollisionMath::Collide(const OBBoxClass & box0, move0, const AABoxClass & box1, move1, result)` (:1370).
pub fn collide_obbox_aabox(box0: &OBBoxClass, move0: &Vector3, box1: &AABoxClass, move1: &Vector3, result: &mut CastResultStruct) -> bool {
    let obbox1 = OBBoxClass::new(&box1.center, &box1.extent);
    let mut context = ObbCollisionStruct::new(box0, move0, &obbox1, move1);
    collide_obb_obb(&mut context, result)
}

/// `CollisionMath::Collide(const AABoxClass & box0, move0, const OBBoxClass & box1, move1, result)` (:1397).
pub fn collide_aabox_obbox(box0: &AABoxClass, move0: &Vector3, box1: &OBBoxClass, move1: &Vector3, result: &mut CastResultStruct) -> bool {
    let obbox0 = OBBoxClass::new(&box0.center, &box0.extent);
    let mut context = ObbCollisionStruct::new(&obbox0, move0, box1, move1);
    collide_obb_obb(&mut context, result)
}
