//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathline.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathline.cpp
//!
//! `CollisionMath::Collide(LineSegClass, X)`. The port already has `Collide(line, tri)` and the aligned box slab test
//! for the picking ray in `crate::client::mesh_cast` (`collide_line_tri`, on the client's trimmed `LineSeg`); this is the
//! full set on `LineSegClass`. Upstream notes kept: `Collide(line, AAPlane)` does not set a contact point;
//! `Collide(line, tri)` is written through `PlaneClass(*tri.N, *tri.V[0])` (`D = N . V0`).
//! (`Overlap_Test(min, max, line)` of colmathline.h is `colmathaabox::overlap_test_min_max_line`.)

use super::aabox::AABoxClass;
use super::aaplane::AAPlaneClass;
use super::colmath::CastResultStruct;
use super::lineseg::LineSegClass;
use super::obbox::OBBoxClass;
use super::sphere::SphereClass;
use crate::ww::plane::PlaneClass;
use crate::ww::tri::TriClass;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `struct BoxTestStruct` (colmathline.cpp:42).
#[derive(Clone, Copy, Debug, Default)]
struct BoxTestStruct {
    min: Vector3,
    max: Vector3,
    p0: Vector3,
    dp: Vector3,
    fraction: f32,
    inside: bool,
    axis: usize,
    side: i32,
}

/// `enum BoxSideType`.
const BOX_SIDE_NEGATIVE: i32 = 0;
const BOX_SIDE_POSITIVE: i32 = 1;
const BOX_SIDE_MIDDLE: i32 = 2;

/// `_box_normal[axis][side]` (colmathline.cpp:62): left/right, down/up, back/front unit normals.
fn box_normal(axis: usize, side: i32) -> Vector3 {
    let mut n = Vector3::new(0.0, 0.0, 0.0);
    n[axis] = if side == BOX_SIDE_NEGATIVE { -1.0 } else { 1.0 };
    n
}

/// `CollisionMath::Collide(const LineSegClass & line, const AAPlaneClass & plane, result)` (colmathline.cpp:97).
pub fn collide_line_aaplane(line: &LineSegClass, plane: &AAPlaneClass, result: &mut CastResultStruct) -> bool {
    let n = plane.normal as usize;
    let den = line.get_dp()[n];

    // "If the denominator is zero, the ray is parallel to the plane"
    if den == 0.0f32 {
        return false;
    }

    let num = -(line.get_p0()[n] - plane.dist);
    let t = num / den;

    // "If t is not between 0 and 1, the line containing the segment intersects the plane but the segment does not"
    if (t < 0.0f32) || (t > 1.0f32) {
        return false;
    }

    // "If the new intersection is closer, record it"
    if t < result.fraction {
        result.fraction = t;
        result.normal.set(0.0, 0.0, 0.0);
        result.normal[n] = 1.0f32;
        return true;
    }
    false
}

/// `CollisionMath::Collide(const LineSegClass & line, const PlaneClass & plane, result)` (colmathline.cpp:139).
pub fn collide_line_plane(line: &LineSegClass, plane: &PlaneClass, result: &mut CastResultStruct) -> bool {
    let den = Vector3::dot_product(&plane.n, line.get_dp());

    if den == 0.0f32 {
        return false;
    }

    let num = -(Vector3::dot_product(&plane.n, line.get_p0()) - plane.d);
    let t = num / den;

    if (t < 0.0f32) || (t > 1.0f32) {
        return false;
    }

    if t < result.fraction {
        result.fraction = t;
        result.normal = plane.n;

        if result.compute_contact_point {
            result.contact_point = *line.get_p0() + result.fraction * *line.get_dp();
        }
        return true;
    }
    false
}

/// `CollisionMath::Collide(const LineSegClass & line, const TriClass & tri, result)` (colmathline.cpp:192). Both
/// faces collide (no backface test). `TRACK_COLLISION_RAY_TRI` counters are debug-only.
pub fn collide_line_tri(line: &LineSegClass, tri: &TriClass, res: &mut CastResultStruct) -> bool {
    // "Compute intersection of the line with the plane of the polygon": PlaneClass(*tri.N, *tri.V[0])
    let plane = PlaneClass { n: tri.n, d: Vector3::dot_product(&tri.n, &tri.v[0]) };

    let den = Vector3::dot_product(&plane.n, line.get_dp());

    // "If the denominator is zero, the ray is parallel to the plane"
    if den == 0.0f32 {
        return false;
    }

    let num = -(Vector3::dot_product(&plane.n, line.get_p0()) - plane.d);
    let t = num / den;

    if (t < 0.0f32) || (t > 1.0f32) {
        return false;
    }

    let ipoint = *line.get_p0() + t * *line.get_dp();

    // "Check if this point is inside the triangle"
    if !tri.contains_point(&ipoint) {
        return false;
    }

    if t < res.fraction {
        res.fraction = t;
        res.normal = plane.n;

        if res.compute_contact_point {
            res.contact_point = *line.get_p0() + res.fraction * *line.get_dp();
        }
        return true;
    }
    false
}

/// `CollisionMath::Collide(const LineSegClass & line, const SphereClass & sphere, res)` (colmathline.cpp:258): "this game
/// from graphics gems 1, page 388, intersection of a ray with a sphere". A line starting inside the sphere uses the far
/// root `(clen + d)`.
pub fn collide_line_sphere(line: &LineSegClass, sphere: &SphereClass, res: &mut CastResultStruct) -> bool {
    let dc = sphere.center - *line.get_p0();
    let clen = Vector3::dot_product(&dc, line.get_dir());
    let disc = (sphere.radius * sphere.radius) - (dc.length2() - clen * clen);

    if disc < 0.0f32 {
        return false;
    } else {
        let d = wwmath::sqrt(disc);
        let mut frac = (clen - d) / line.get_length();
        if frac < 0.0f32 {
            frac = (clen + d) / line.get_length();
        }

        if frac < 0.0f32 {
            return false;
        }

        if frac < res.fraction {
            res.fraction = frac;

            let p = *line.get_p0() + (clen - d) * *line.get_dir();
            let mut norm = p - sphere.center;
            norm /= sphere.radius;
            res.normal = norm;

            if res.compute_contact_point {
                res.contact_point = *line.get_p0() + res.fraction * *line.get_dp();
            }
            return true;
        }
    }
    false
}

/// `CollisionMath::Collide(const LineSegClass & line, const AABoxClass & box, res)` (colmathline.cpp:302).
pub fn collide_line_aabox(line: &LineSegClass, box_: &AABoxClass, res: &mut CastResultStruct) -> bool {
    // set up the test struct
    let mut test = BoxTestStruct::default();
    test.min = box_.center - box_.extent;
    test.max = box_.center + box_.extent;
    test.p0 = *line.get_p0();
    test.dp = *line.get_dp();

    // check ray against the box, exit if the ray totally missed the box,
    if !test_aligned_box(&mut test) {
        return false;
    }

    // if ray starts inside the box, note that fact and bail.
    if test.inside {
        res.start_bad = true;
        return true;
    }

    // Now, if this intersection is before any current intersection that we've found, install our intersection
    if test.fraction < res.fraction {
        res.fraction = test.fraction;
        assert!(test.side != BOX_SIDE_MIDDLE);
        res.normal = box_normal(test.axis, test.side);

        if res.compute_contact_point {
            res.contact_point = *line.get_p0() + res.fraction * *line.get_dp();
        }
        return true;
    }
    false
}

/// `CollisionMath::Collide(const LineSegClass & line, const OBBoxClass & box, result)` (colmathline.cpp:352): the line
/// is taken into the box's frame (`Basis.Transpose() * (P0 - Center) + Center`, `Basis.Transpose() * DP`), and a hit's
/// normal is the box axis, negated on the negative side.
pub fn collide_line_obbox(line: &LineSegClass, box_: &OBBoxClass, result: &mut CastResultStruct) -> bool {
    let mut test = BoxTestStruct::default();
    test.min = box_.center - box_.extent;
    test.max = box_.center + box_.extent;
    test.p0 = box_.basis.transpose().mul_vector(&(*line.get_p0() - box_.center)) + box_.center;
    test.dp = box_.basis.transpose().mul_vector(line.get_dp());

    // check ray against the box, exit if the ray totally missed the box,
    if !test_aligned_box(&mut test) {
        return false;
    }

    // if ray starts inside the box, don't collide
    if test.inside {
        result.start_bad = true;
        return true;
    }

    if test.fraction < result.fraction {
        result.fraction = test.fraction;
        assert!(test.side != BOX_SIDE_MIDDLE);

        match test.axis {
            0 => result.normal.set(box_.basis[0][0], box_.basis[1][0], box_.basis[2][0]),
            1 => result.normal.set(box_.basis[0][1], box_.basis[1][1], box_.basis[2][1]),
            2 => result.normal.set(box_.basis[0][2], box_.basis[1][2], box_.basis[2][2]),
            _ => {}
        }

        if test.side == BOX_SIDE_NEGATIVE {
            result.normal = -result.normal;
        }

        if result.compute_contact_point {
            result.contact_point = *line.get_p0() + result.fraction * *line.get_dp();
        }
        return true;
    }
    false
}

/// `Test_Aligned_Box(BoxTestStruct * test)` (colmathline.cpp:418): the Graphics Gems slab test of a ray against an
/// axis-aligned box. The fraction is not bounded above by 1 (the callers compare it with the result's).
fn test_aligned_box(test: &mut BoxTestStruct) -> bool {
    // candidate intersection plane distance for each axis
    let mut candidateplane = [0.0f32; 3];
    // t value along the ray for each axis
    let mut maxt = [0.0f32; 3];
    // intersection point
    let mut coord = Vector3::new(0.0, 0.0, 0.0);
    // which "side" of the box for each axis
    let mut quadrant = [0i32; 3];
    let mut inside = true;

    // Find the candidate planes
    for i in 0..3 {
        if test.p0[i] < test.min[i] {
            quadrant[i] = BOX_SIDE_NEGATIVE;
            candidateplane[i] = test.min[i];
            inside = false;
        } else if test.p0[i] > test.max[i] {
            quadrant[i] = BOX_SIDE_POSITIVE;
            candidateplane[i] = test.max[i];
            inside = false;
        } else {
            quadrant[i] = BOX_SIDE_MIDDLE;
        }
    }

    // Ray origin inside the box
    if inside {
        test.fraction = 0.0f32;
        test.inside = true;
        return true;
    }

    // Calculate the t distances to the candidate planes
    for i in 0..3 {
        if quadrant[i] != BOX_SIDE_MIDDLE && test.dp[i] != 0.0f32 {
            maxt[i] = (candidateplane[i] - test.p0[i]) / test.dp[i];
        } else {
            maxt[i] = -1.0f32;
        }
    }

    // Get the largest of the maxt's for the final choice of intersection
    let mut intersection_plane = 0usize;
    for i in 1..3 {
        if maxt[i] > maxt[intersection_plane] {
            intersection_plane = i;
        }
    }

    // Check final candidate actually inside box
    if maxt[intersection_plane] < 0.0f32 {
        return false;
    }

    for i in 0..3 {
        if intersection_plane != i {
            coord[i] = test.p0[i] + maxt[intersection_plane] * test.dp[i];
            if (coord[i] < test.min[i]) || (coord[i] > test.max[i]) {
                return false; // ray is outside the box
            }
        } else {
            coord[i] = candidateplane[i];
        }
    }

    // The ray hits the box
    test.fraction = maxt[intersection_plane];
    test.inside = false;
    test.axis = intersection_plane;
    test.side = quadrant[intersection_plane];
    true
}
