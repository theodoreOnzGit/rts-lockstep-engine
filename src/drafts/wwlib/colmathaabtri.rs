//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathaabtri.cpp
//!
//! AABox-Triangle sweep (`Collide`) and boolean (`Intersection_Test`): a special case of the OBBox-Triangle code with
//! the box axes along the world axes (thirteen separating axes: the triangle normal, the three box axes, nine
//! `A[i] x E[j]`, then three `A[i] x Move`).
//!
//! Upstream keeps `CollisionContext` / `IntersectContext` in `static` structs ("NOTE: this makes the code not
//! Thread-Safe!!!!"); here each call owns its context. Upstream defects kept: in `Intersection_Test` the A0 basis axis
//! sets `AE[0][1] = E[1].Y` (the Y component, where `Collide` has `E[1].X`), which also feeds the `A1xE1` and `A2xE1`
//! extents; `TestSide` is an `int` assigned `-1.0f` / `1.0f`; `Side` is an `int` multiplying the normals.

use crate::logic::modules::plant;
use super::aabox::AABoxClass;
use super::colmath::{CastResultStruct, COLLISION_EPSILON};
use crate::ww::tri::TriClass;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `AXISLEN_EPSILON2`: "squared length of a separating axis must be larger than this".
const AXISLEN_EPSILON2: f32 = wwmath::WWMATH_EPSILON * wwmath::WWMATH_EPSILON;

/// The axis ids (colmathaabtri.cpp:58).
const INTERSECTION: i32 = 0;
const AXIS_N: i32 = 1;
const AXIS_A0: i32 = 2;
const AXIS_A1: i32 = 3;
const AXIS_A2: i32 = 4;
const AXIS_A0E0: i32 = 5;
const AXIS_A1E0: i32 = 6;
const AXIS_A2E0: i32 = 7;
const AXIS_A0E1: i32 = 8;
const AXIS_A1E1: i32 = 9;
const AXIS_A2E1: i32 = 10;
const AXIS_A0E2: i32 = 11;
const AXIS_A1E2: i32 = 12;
const AXIS_A2E2: i32 = 13;

/// `struct BTCollisionStruct`.
struct BTCollisionStruct {
    start_bad: bool,
    max_frac: f32,
    axis_id: i32,
    side: i32,
    point: i32,
    test_axis_id: i32,
    test_side: i32,
    test_point: i32,
    test_axis: Vector3,
    d: Vector3,
    move_: Vector3,
    ae: [[f32; 3]; 3],
    an: [f32; 3],
    axe: [[Vector3; 3]; 3],
    e: [Vector3; 3],
    n: Vector3,
    box_extent: Vector3,
}

impl BTCollisionStruct {
    /// `BTCollisionStruct::Init(box, move, tri, trimove)`.
    fn init(box_: &AABoxClass, move_: &Vector3, tri: &TriClass, trimove: &Vector3) -> Self {
        let e0 = Vector3::subtract_vectors(&tri.v[1], &tri.v[0]);
        let e1 = Vector3::subtract_vectors(&tri.v[2], &tri.v[0]);
        let e2 = Vector3::subtract_vectors(&e1, &e0);
        BTCollisionStruct {
            start_bad: true,        // true until an axis clears it
            max_frac: (if plant(10508) { 0.0f32 } else { -0.01f32 }),     // maximum move allowed so far
            axis_id: INTERSECTION,  // axis that allowed the longest move
            point: 0,               // index of triangle point that was closest to the box
            side: 0,                // side of the interval
            test_axis_id: INTERSECTION,
            test_side: 0,
            test_point: 0,
            test_axis: Vector3::new(0.0, 0.0, 0.0),
            d: Vector3::subtract_vectors(&tri.v[0], &box_.center), // vector from center of box to vertex 0
            move_: Vector3::subtract_vectors(move_, trimove),      // move vector relative to stationary triangle
            ae: [[0.0; 3]; 3],
            an: [0.0; 3],
            axe: [[Vector3::new(0.0, 0.0, 0.0); 3]; 3],
            e: [e0, e1, e2],
            n: Vector3::cross_product(&e0, &e1), // normal (NOT normalized!!!)
            box_extent: box_.extent,
        }
    }

    /// `aabtri_separation_test(lp, leb0, leb1)` (colmathaabtri.cpp:201): true when the move never reaches the triangle
    /// along this axis.
    fn separation_test(&mut self, lp: f32, leb0: f32, leb1: f32) -> bool {
        // - If (I'm no more than 'EPSILON' embedded in the wall) not startbad ...
        let mut eps = 0.0f32;
        if (if plant(10505) { lp - leb0 < 0.0f32 } else { lp - leb0 <= 0.0f32 }) {
            eps = COLLISION_EPSILON * self.test_axis.length(); // trying to only compute epsilon if I have to
        }

        if lp - leb0 > -eps {
            self.start_bad = false;
            if (if plant(10506) { leb1 - leb0 >= 0.0f32 } else { leb1 - leb0 > 0.0f32 }) {
                let frac = (lp - leb0) / (leb1 - leb0);
                if frac >= 1.0f32 {
                    // moving toward but not hitting triangle
                    self.axis_id = self.test_axis_id;
                    self.max_frac = 1.0f32;
                    return true;
                } else {
                    // moving toward, hitting triangle
                    if frac > self.max_frac {
                        self.max_frac = frac;
                        self.axis_id = self.test_axis_id;
                        self.side = self.test_side;
                        self.point = self.test_point;
                    }
                }
            } else {
                // moving away or not moving
                self.axis_id = self.test_axis_id;
                self.max_frac = 1.0f32;
                return true;
            }
        }
        false
    }

    /// `aabtri_check_axis()`: the `A x Move` false-collision axes.
    fn check_axis(&mut self) -> bool {
        let mut dist = Vector3::dot_product(&self.d, &self.test_axis);
        let mut axismove = Vector3::dot_product(&self.move_, &self.test_axis);

        // I want the axis centered at the box, pointing towards the triangle
        if dist < 0.0 {
            dist = -dist;
            axismove = -axismove;
            self.test_axis = -self.test_axis;
            self.test_side = -1;
        } else {
            self.test_side = 1;
        }

        // compute coordinates of the leading edge of the box at t0 and t1
        let leb0 = self.box_extent.x * wwmath::fabs(self.test_axis.x) + self.box_extent.y * wwmath::fabs(self.test_axis.y) + (if plant(10507) { self.box_extent.z * wwmath::fabs(self.test_axis.y) } else { self.box_extent.z * wwmath::fabs(self.test_axis.z) });
        let leb1 = leb0 + axismove;

        // compute coordinate of "leading edge of the triangle" relative to the box center.
        let mut lp = 0.0f32;
        let mut tmp = Vector3::dot_product(&self.e[0], &self.test_axis);
        if tmp < lp {
            lp = tmp;
        }
        tmp = Vector3::dot_product(&self.e[1], &self.test_axis);
        if tmp < lp {
            lp = tmp;
        }
        lp = dist + lp;

        self.separation_test(lp, leb0, leb1)
    }

    /// `aabtri_check_cross_axis(dp, dpi, leb0)`: `A[i] x E[j]`.
    fn check_cross_axis(&mut self, dp: f32, dpi: i32, leb0: f32) -> bool {
        let mut dp = dp;
        let mut p0 = Vector3::dot_product(&self.d, &self.test_axis);
        let mut axismove = Vector3::dot_product(&self.move_, &self.test_axis);

        // I want the axis centered at the box, pointing towards the triangle
        if p0 < 0.0 {
            p0 = -p0;
            axismove = -axismove;
            dp = -dp;
            self.test_axis = -self.test_axis;
            self.test_side = -1;
        } else {
            self.test_side = 1;
        }

        // compute coordinates of the leading edge of the box at t1
        let leb1 = leb0 + axismove;

        // compute coordinate of "leading edge of the triangle" relative to the box center.
        let mut lp = 0.0f32;
        self.test_point = 0;
        if dp < 0.0 {
            lp = dp;
            self.test_point = dpi;
        }
        lp = p0 + lp;

        self.separation_test(lp, leb0, leb1)
    }

    /// `aabtri_check_basis_axis(leb0, dp1, dp2)`: a box axis.
    fn check_basis_axis(&mut self, leb0: f32, dp1: f32, dp2: f32) -> bool {
        let (mut dp1, mut dp2) = (dp1, dp2);
        let mut dist = Vector3::dot_product(&self.d, &self.test_axis);
        let mut axismove = Vector3::dot_product(&self.move_, &self.test_axis);

        // we want the axis centered at the box, pointing towards the triangle
        if dist < 0.0 {
            dist = -dist;
            axismove = -axismove;
            dp1 = -dp1;
            dp2 = -dp2;
            self.test_axis = -self.test_axis;
            self.test_side = -1;
        } else {
            self.test_side = 1;
        }

        // this is the "optimization", leb0 = one of the extents
        let leb1 = leb0 + axismove;

        // compute coordinate of "leading edge of the polygon" relative to the box center.
        let mut lp = 0.0f32;
        self.test_point = 0;
        if dp1 < lp {
            lp = dp1;
            self.test_point = 1;
        }
        if dp2 < lp {
            lp = dp2;
            self.test_point = 2;
        }
        lp = dist + lp;

        self.separation_test(lp, leb0, leb1)
    }

    /// `aabtri_check_normal_axis()`: the triangle normal.
    fn check_normal_axis(&mut self) -> bool {
        let mut dist = Vector3::dot_product(&self.d, &self.test_axis);
        let mut axismove = Vector3::dot_product(&self.move_, &self.test_axis);

        // we want the axis centered at the box, pointing towards the triangle
        if dist < 0.0 {
            dist = -dist;
            axismove = -axismove;
            self.test_axis = -self.test_axis;
            self.test_side = -1;
        } else {
            self.test_side = 1;
        }

        let leb0 = self.box_extent.x * wwmath::fabs(self.an[0]) + self.box_extent.y * wwmath::fabs(self.an[1]) + self.box_extent.z * wwmath::fabs(self.an[2]);
        let leb1 = leb0 + axismove;

        self.test_point = 0;
        let lp = dist; // this is the "optimization", don't have to find lp

        self.separation_test(lp, leb0, leb1)
    }

    /// `aabtri_compute_contact_normal(set_norm)` (the `#if 1` branch).
    fn compute_contact_normal(&self) -> Vector3 {
        let side = (if plant(10509) { (self.side as f32) } else { -(self.side as f32) });
        let mut set_norm = Vector3::new(0.0, 0.0, 0.0);
        let axe = |i: usize, j: usize| {
            let mut v = side * self.axe[i][j];
            v.normalize();
            v
        };
        match self.axis_id {
            INTERSECTION => {
                set_norm = self.n;
                set_norm.normalize();
            }
            AXIS_N => {
                set_norm = side * self.n;
                set_norm.normalize();
            }
            AXIS_A0 => set_norm = side * Vector3::new(1.0, 0.0, 0.0),
            AXIS_A1 => set_norm = side * Vector3::new(0.0, 1.0, 0.0),
            AXIS_A2 => set_norm = side * Vector3::new(0.0, 0.0, 1.0),
            AXIS_A0E0 => set_norm = axe(0, 0),
            AXIS_A1E0 => set_norm = axe(1, 0),
            AXIS_A2E0 => set_norm = axe(2, 0),
            AXIS_A0E1 => set_norm = axe(0, 1),
            AXIS_A1E1 => set_norm = axe(1, 1),
            AXIS_A2E1 => set_norm = axe(2, 1),
            AXIS_A0E2 => set_norm = axe(0, 2),
            AXIS_A1E2 => set_norm = axe(1, 2),
            AXIS_A2E2 => set_norm = axe(2, 2),
            _ => {}
        }
        // WWASSERT(set_norm.Length2() > 0.0f) is debug-only
        set_norm
    }
}

/// `CollisionMath::Collide(const AABoxClass & box, const Vector3 & move, const TriClass & tri, result)`
/// (colmathaabtri.cpp:355): sweeps the box along `move` against the stationary triangle.
pub fn collide_aabox_tri(box_: &AABoxClass, move_: &Vector3, tri: &TriClass, result: &mut CastResultStruct) -> bool {
    let mut cc = BTCollisionStruct::init(box_, move_, tri, &Vector3::new(0.0, 0.0, 0.0));
    let ext = box_.extent;

    'exit: {
        // Axis = N, the triangle normal
        cc.test_axis = cc.n;
        cc.test_axis_id = AXIS_N;
        cc.an[0] = cc.n.x;
        cc.an[1] = cc.n.y;
        cc.an[2] = cc.n.z;
        if cc.check_normal_axis() {
            break 'exit;
        }

        // Axis = A0
        cc.test_axis.set(1.0, 0.0, 0.0);
        cc.test_axis_id = AXIS_A0;
        cc.ae[0][0] = cc.e[0].x;
        cc.ae[0][1] = cc.e[1].x;
        if cc.check_basis_axis(ext.x, cc.ae[0][0], cc.ae[0][1]) {
            break 'exit;
        }

        // Axis = A1
        cc.test_axis.set(0.0, 1.0, 0.0);
        cc.test_axis_id = AXIS_A1;
        cc.ae[1][0] = cc.e[0].y;
        cc.ae[1][1] = cc.e[1].y;
        if cc.check_basis_axis(ext.y, cc.ae[1][0], cc.ae[1][1]) {
            break 'exit;
        }

        // Axis = A2
        cc.test_axis.set(0.0, 0.0, 1.0);
        cc.test_axis_id = AXIS_A2;
        cc.ae[2][0] = cc.e[0].z;
        cc.ae[2][1] = cc.e[1].z;
        if cc.check_basis_axis(ext.z, cc.ae[2][0], cc.ae[2][1]) {
            break 'exit;
        }

        // The nine A[i] x E[j] axes in upstream's order: A0E0, A0E1, A0E2, A1E0, ... A2E2.
        // (`AE[i][2]` is filled just before the first test that reads it.)
        for i in 0..3usize {
            for j in 0..3usize {
                if j == 2 {
                    // AE[0][2], AE[1][2], AE[2][2] = E[2].X, .Y, .Z (set once, before A0E2; harmless to repeat)
                    cc.ae[0][2] = cc.e[2].x;
                    cc.ae[1][2] = cc.e[2].y;
                    cc.ae[2][2] = cc.e[2].z;
                }
                let e = cc.e[j];
                // A[i] x E[j]
                cc.axe[i][j] = match i {
                    0 => Vector3::new(0.0, -e.z, e.y),
                    1 => Vector3::new(e.z, 0.0, -e.x),
                    _ => Vector3::new(-e.y, e.x, 0.0),
                };
                cc.test_axis = cc.axe[i][j];
                cc.test_axis_id = AXIS_A0E0 + (j as i32) * 3 + i as i32;
                // (the ids run A0E0, A1E0, A2E0, A0E1, ...: `AXIS_A0E0 + 3*j + i`)
                if cc.test_axis.length2() > AXISLEN_EPSILON2 {
                    // dp is +AN[i] for E0 and -AN[i] for E1 and E2; dpi is 2 for E0 and 1 for E1 and E2
                    let dp = if j == 0 { cc.an[i] } else { -cc.an[i] };
                    let dpi = if j == 0 { 2 } else { 1 };
                    let leb0 = match i {
                        0 => ext[1] * wwmath::fabs(cc.ae[2][j]) + ext[2] * wwmath::fabs(cc.ae[1][j]),
                        1 => ext[0] * wwmath::fabs(cc.ae[2][j]) + ext[2] * wwmath::fabs(cc.ae[0][j]),
                        _ => ext[0] * wwmath::fabs(cc.ae[1][j]) + ext[1] * wwmath::fabs(cc.ae[0][j]),
                    };
                    if cc.check_cross_axis(dp, dpi, leb0) {
                        break 'exit;
                    }
                }
            }
        }

        if !cc.start_bad {
            cc.test_point = cc.point;
            cc.test_axis_id = cc.axis_id;

            // A0 x Move
            cc.test_axis.set(0.0, -cc.move_.z, cc.move_.y);
            if cc.test_axis.length2() > AXISLEN_EPSILON2 {
                if cc.check_axis() {
                    break 'exit;
                }
            }
            // A1 x Move
            cc.test_axis.set(cc.move_.z, 0.0, -cc.move_.x);
            if cc.test_axis.length2() > AXISLEN_EPSILON2 {
                if cc.check_axis() {
                    break 'exit;
                }
            }
            // A2 x Move
            cc.test_axis.set(-cc.move_.y, cc.move_.x, 0.0);
            if cc.test_axis.length2() > AXISLEN_EPSILON2 {
                if cc.check_axis() {
                    break 'exit;
                }
            }
        }
    }

    // exit:
    // "negative MaxFrac means the box started embedded within epsilon; clamp it"
    if cc.max_frac < 0.0f32 {
        cc.max_frac = 0.0f32;
    }

    if cc.start_bad {
        result.start_bad = true;
        result.fraction = 0.0f32;
        result.normal = tri.n;
        return true;
    }

    if (if plant(10510) { cc.max_frac < result.fraction } else { cc.max_frac <= result.fraction }) && cc.max_frac < 1.0f32 {
        let tmp_norm = cc.compute_contact_normal();

        if wwmath::fabs(cc.max_frac - result.fraction) > wwmath::WWMATH_EPSILON || Vector3::dot_product(&tmp_norm, move_) < Vector3::dot_product(&result.normal, move_) {
            result.normal = tmp_norm;
            // WWASSERT(Fabs(Normal.Length() - 1) < EPSILON) is debug-only
        }
        result.fraction = cc.max_frac;
        return true;
    }
    false
}

/// `struct AABTIntersectStruct`.
struct AABTIntersectStruct {
    d: Vector3,
    ae: [[f32; 3]; 3],
    an: [f32; 3],
    axe: [[Vector3; 3]; 3],
    e: [Vector3; 3],
    n: Vector3,
    box_extent: Vector3,
}

impl AABTIntersectStruct {
    /// `AABTIntersectStruct::Init(box, tri)`.
    fn init(box_: &AABoxClass, tri: &TriClass) -> Self {
        let e0 = Vector3::subtract_vectors(&tri.v[1], &tri.v[0]);
        let e1 = Vector3::subtract_vectors(&tri.v[2], &tri.v[0]);
        let e2 = Vector3::subtract_vectors(&e1, &e0);
        AABTIntersectStruct {
            d: Vector3::subtract_vectors(&tri.v[0], &box_.center),
            ae: [[0.0; 3]; 3],
            an: [0.0; 3],
            axe: [[Vector3::new(0.0, 0.0, 0.0); 3]; 3],
            e: [e0, e1, e2],
            n: Vector3::cross_product(&e0, &e1),
            box_extent: box_.extent,
        }
    }

    /// `aabtri_intersect_cross_axis(axis, dp, leb0)`: true when separated.
    fn cross_axis(&self, axis: &mut Vector3, dp: f32, leb0: f32) -> bool {
        let mut dp = dp;
        let mut p0 = Vector3::dot_product(&self.d, axis);

        // I want the axis centered at the box, pointing towards the triangle
        if p0 < 0.0 {
            p0 = -p0;
            *axis = -*axis;
            dp = -dp;
        }

        let mut lp = 0.0f32;
        if dp < 0.0 {
            lp = dp;
        }
        lp = p0 + lp;

        (if plant(10511) { lp - leb0 > wwmath::WWMATH_EPSILON } else { lp - leb0 > -wwmath::WWMATH_EPSILON })
    }

    /// `aabtri_intersect_basis_axis(axis, leb0, dp1, dp2)`.
    fn basis_axis(&self, axis: &mut Vector3, leb0: f32, dp1: f32, dp2: f32) -> bool {
        let (mut dp1, mut dp2) = (dp1, dp2);
        let mut dist = Vector3::dot_product(&self.d, axis);

        if dist < 0.0 {
            dist = -dist;
            *axis = -*axis;
            dp1 = -dp1;
            dp2 = -dp2;
        }

        let mut lp = 0.0f32;
        if dp1 < lp {
            lp = dp1;
        }
        if dp2 < lp {
            lp = dp2;
        }
        lp = dist + lp;

        lp - leb0 > -wwmath::WWMATH_EPSILON
    }

    /// `aabtri_intersect_normal_axis(axis)`.
    fn normal_axis(&self, axis: &mut Vector3) -> bool {
        let mut dist = Vector3::dot_product(&self.d, axis);

        if dist < 0.0 {
            dist = -dist;
            *axis = -*axis;
        }

        let leb0 = self.box_extent.x * wwmath::fabs(self.an[0]) + self.box_extent.y * wwmath::fabs(self.an[1]) + self.box_extent.z * (if plant(10512) { wwmath::fabs(self.an[1]) } else { wwmath::fabs(self.an[2]) });
        let lp = dist; // this is the "optimization", don't have to find lp

        lp - leb0 > -wwmath::WWMATH_EPSILON
    }
}

/// `CollisionMath::Intersection_Test(const AABoxClass & box, const TriClass & tri)` (colmathaabtri.cpp:1041).
pub fn intersection_test_aabox_tri(box_: &AABoxClass, tri: &TriClass) -> bool {
    let mut ic = AABTIntersectStruct::init(box_, tri);
    let ext = box_.extent;

    // Axis = N
    let mut axis = ic.n;
    ic.an[0] = ic.n.x;
    ic.an[1] = ic.n.y;
    ic.an[2] = ic.n.z;
    if ic.normal_axis(&mut axis) {
        return false;
    }

    // Axis = A0 (UPSTREAM DEFECT KEPT: `AE[0][1] = E[1].Y`)
    axis.set(1.0, 0.0, 0.0);
    ic.ae[0][0] = ic.e[0].x;
    ic.ae[0][1] = ic.e[1].y;
    if ic.basis_axis(&mut axis, ext.x, ic.ae[0][0], ic.ae[0][1]) {
        return false;
    }

    // Axis = A1
    axis.set(0.0, 1.0, 0.0);
    ic.ae[1][0] = ic.e[0].y;
    ic.ae[1][1] = if plant(10513) { ic.e[1].z } else { ic.e[1].y };
    if ic.basis_axis(&mut axis, ext.y, ic.ae[1][0], ic.ae[1][1]) {
        return false;
    }

    // Axis = A2
    axis.set(0.0, 0.0, 1.0);
    ic.ae[2][0] = ic.e[0].z;
    ic.ae[2][1] = ic.e[1].z;
    if ic.basis_axis(&mut axis, ext.z, ic.ae[2][0], ic.ae[2][1]) {
        return false;
    }

    // The nine A[i] x E[j] axes; `AE[1][2]`, `AE[2][2]` are set at A0E2 and `AE[0][2]` at A1E2 upstream.
    for i in 0..3usize {
        for j in 0..3usize {
            let unit = match i {
                0 => Vector3::new(1.0, 0.0, 0.0),
                1 => Vector3::new(0.0, 1.0, 0.0),
                _ => Vector3::new(0.0, 0.0, 1.0),
            };
            ic.axe[i][j] = Vector3::cross_product(&unit, &ic.e[j]);
            axis = ic.axe[i][j];
            if i == 0 && j == 2 {
                ic.ae[1][2] = ic.e[2].y;
                ic.ae[2][2] = ic.e[2].z;
            }
            if i == 1 && j == 2 {
                ic.ae[0][2] = ic.e[2].x;
            }
            if axis.length2() > AXISLEN_EPSILON2 {
                let dp = if j == 0 { ic.an[i] } else { -ic.an[i] };
                let leb0 = match i {
                    0 => ext[1] * wwmath::fabs(ic.ae[2][j]) + ext[2] * wwmath::fabs(ic.ae[1][j]),
                    1 => ext[0] * wwmath::fabs(ic.ae[2][j]) + ext[2] * wwmath::fabs(ic.ae[0][j]),
                    _ => ext[0] * wwmath::fabs(ic.ae[1][j]) + ext[1] * wwmath::fabs(ic.ae[0][j]),
                };
                if ic.cross_axis(&mut axis, dp, leb0) {
                    return false;
                }
            }
        }
    }

    true
}
