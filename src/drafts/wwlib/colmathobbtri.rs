//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathobbtri.cpp
//!
//! OBBox-Triangle sweep (`Collide`, with contact normal and point) and boolean `Intersection_Test`: the thirteen
//! separating axes of colmathaabtri.cpp with the box axes taken from the box's `Basis`. Upstream notes kept:
//! `MaxFrac` starts at -1 (colmathaabtri.cpp uses -0.01); the three "false collision" axes are `Move x A[i]` (the
//! AABox file has `A[i] x Move`); at `exit:` an `AxisId` still `INTERSECTION` forces `StartBad` ("(gth) disabling an
//! assert in obb->tri collision, investigate later"); `TestSide` is an `int` assigned `-1.0f` / `1.0f`; the contact
//! normal for `AXIS_N` is `-Side * *Tri.N` (the triangle's own normal) whereas the context's `N` is unnormalised.

use super::colmath::{CastResultStruct, COLLISION_EPSILON};
use super::obbox::OBBoxClass;
use crate::ww::tri::TriClass;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `AXISLEN_EPSILON2`.
const AXISLEN_EPSILON2: f32 = wwmath::WWMATH_EPSILON * wwmath::WWMATH_EPSILON;

/// The axis ids (colmathobbtri.cpp:57), as in colmathaabtri.cpp.
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
    point: i32,
    side: i32,
    test_side: i32,
    test_axis_id: i32,
    test_point: i32,
    test_axis: Vector3,
    d: Vector3,
    move_: Vector3,
    ae: [[f32; 3]; 3],
    an: [f32; 3],
    axe: [[Vector3; 3]; 3],
    a: [Vector3; 3],
    e: [Vector3; 3],
    n: Vector3,
    final_d: Vector3,
    box_: OBBoxClass,
    tri: TriClass,
    box_move: Vector3,
    tri_move: Vector3,
}

impl BTCollisionStruct {
    /// `BTCollisionStruct(box, move, tri, trimove)` and `Reset()`.
    fn new(box_: &OBBoxClass, move_: &Vector3, tri: &TriClass, trimove: &Vector3) -> Self {
        let e0 = Vector3::subtract_vectors(&tri.v[1], &tri.v[0]);
        let e1 = Vector3::subtract_vectors(&tri.v[2], &tri.v[0]);
        let e2 = Vector3::subtract_vectors(&e1, &e0);
        BTCollisionStruct {
            start_bad: true,       // true until an axis clears it
            max_frac: -1.0f32,     // maximum move allowed so far (accept slightly negative but clamp to zero at end)
            axis_id: INTERSECTION, // axis that allowed the longest move
            point: 0,              // index of triangle point that was closest to the box
            side: 0,               // side of the interval
            test_side: 0,
            test_axis_id: INTERSECTION,
            test_point: 0,
            test_axis: Vector3::new(0.0, 0.0, 0.0),
            d: Vector3::subtract_vectors(&tri.v[0], &box_.center), // vector from center of box to vertex 0
            move_: Vector3::subtract_vectors(move_, trimove),      // move vector relative to stationary triangle
            ae: [[0.0; 3]; 3],
            an: [0.0; 3],
            axe: [[Vector3::new(0.0, 0.0, 0.0); 3]; 3],
            a: [
                Vector3::new(box_.basis[0][0], box_.basis[1][0], box_.basis[2][0]),
                Vector3::new(box_.basis[0][1], box_.basis[1][1], box_.basis[2][1]),
                Vector3::new(box_.basis[0][2], box_.basis[1][2], box_.basis[2][2]),
            ],
            e: [e0, e1, e2],
            n: Vector3::cross_product(&e0, &e1),
            final_d: Vector3::new(0.0, 0.0, 0.0),
            box_: *box_,
            tri: *tri,
            box_move: *move_,
            tri_move: *trimove,
        }
    }

    /// `obbtri_collision_separation_test(context, lp, leb0, leb1)`.
    fn separation_test(&mut self, lp: f32, leb0: f32, leb1: f32) -> bool {
        let mut eps = 0.0f32;
        if lp - leb0 <= 0.0f32 {
            eps = COLLISION_EPSILON * self.test_axis.length(); // trying to only compute epsilon if I have to
        }

        if lp - leb0 > -eps {
            self.start_bad = false;
            if leb1 - leb0 > 0.0f32 {
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

    /// `obbtri_check_collision_axis(context)`: `Move x A[i]`.
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
        let leb0 = self.box_.extent.x * wwmath::fabs(Vector3::dot_product(&self.test_axis, &self.a[0]))
            + self.box_.extent.y * wwmath::fabs(Vector3::dot_product(&self.test_axis, &self.a[1]))
            + self.box_.extent.z * wwmath::fabs(Vector3::dot_product(&self.test_axis, &self.a[2]));
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

    /// `obbtri_check_collision_cross_axis(context, dp, dpi, leb0)`.
    fn check_cross_axis(&mut self, dp: f32, dpi: i32, leb0: f32) -> bool {
        let mut dp = dp;
        let mut p0 = Vector3::dot_product(&self.d, &self.test_axis);
        let mut axismove = Vector3::dot_product(&self.move_, &self.test_axis);

        if p0 < 0.0 {
            p0 = -p0;
            axismove = -axismove;
            dp = -dp;
            self.test_axis = -self.test_axis;
            self.test_side = -1;
        } else {
            self.test_side = 1;
        }

        let leb1 = leb0 + axismove;

        let mut lp = 0.0f32;
        self.test_point = 0;
        if dp < 0.0 {
            lp = dp;
            self.test_point = dpi;
        }
        lp = p0 + lp;

        self.separation_test(lp, leb0, leb1)
    }

    /// `obbtri_check_collision_basis_axis(context, leb0, dp1, dp2)`.
    fn check_basis_axis(&mut self, leb0: f32, dp1: f32, dp2: f32) -> bool {
        let (mut dp1, mut dp2) = (dp1, dp2);
        let mut dist = Vector3::dot_product(&self.d, &self.test_axis);
        let mut axismove = Vector3::dot_product(&self.move_, &self.test_axis);

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

    /// `obbtri_check_collision_normal_axis(context)`.
    fn check_normal_axis(&mut self) -> bool {
        let mut dist = Vector3::dot_product(&self.d, &self.test_axis);
        let mut axismove = Vector3::dot_product(&self.move_, &self.test_axis);

        if dist < 0.0 {
            dist = -dist;
            axismove = -axismove;
            self.test_axis = -self.test_axis;
            self.test_side = -1;
        } else {
            self.test_side = 1;
        }

        let leb0 = self.box_.extent.x * wwmath::fabs(self.an[0]) + self.box_.extent.y * wwmath::fabs(self.an[1]) + self.box_.extent.z * wwmath::fabs(self.an[2]);
        let leb1 = leb0 + axismove;

        self.test_point = 0;
        let lp = dist; // this is the "optimization", don't have to find lp

        self.separation_test(lp, leb0, leb1)
    }

    /// `obbtri_compute_contact_normal(context, &set_normal)`. `INTERSECTION` leaves the normal untouched.
    fn compute_contact_normal(&self, normal: &mut Vector3) {
        let side = -(self.side as f32);
        let axe = |i: usize, j: usize| {
            let mut v = side * self.axe[i][j];
            v.normalize();
            v
        };
        match self.axis_id {
            INTERSECTION => {}
            AXIS_N => *normal = side * self.tri.n,
            AXIS_A0 => *normal = side * self.a[0],
            AXIS_A1 => *normal = side * self.a[1],
            AXIS_A2 => *normal = side * self.a[2],
            AXIS_A0E0 => *normal = axe(0, 0),
            AXIS_A1E0 => *normal = axe(1, 0),
            AXIS_A2E0 => *normal = axe(2, 0),
            AXIS_A0E1 => *normal = axe(0, 1),
            AXIS_A1E1 => *normal = axe(1, 1),
            AXIS_A2E1 => *normal = axe(2, 1),
            AXIS_A0E2 => *normal = axe(0, 2),
            AXIS_A1E2 => *normal = axe(1, 2),
            AXIS_A2E2 => *normal = axe(2, 2),
            _ => {}
        }
        // WWASSERT(set_normal->Length2() > 0.0f) is debug-only
    }

    /// `eval_A0_point` / `eval_A1_point` / `eval_A2_point` (colmathobbtri.cpp:355..): the box parameters `x` of the
    /// contact point on an edge-edge contact of box axis `k` with triangle edge `edge`.
    fn eval_point(&self, x: &mut [f32; 3], k: usize, edge: usize) {
        match k {
            0 => {
                x[1] = -eval_side(self.ae[2][edge], self.side) * self.box_.extent[1];
                x[2] = eval_side(self.ae[1][edge], self.side) * self.box_.extent[2];
            }
            1 => {
                x[0] = eval_side(self.ae[2][edge], self.side) * self.box_.extent[0];
                x[2] = -eval_side(self.ae[0][edge], self.side) * self.box_.extent[2];
            }
            _ => {
                x[0] = -eval_side(self.ae[1][edge], self.side) * self.box_.extent[0];
                x[1] = eval_side(self.ae[0][edge], self.side) * self.box_.extent[1];
            }
        }

        let yval = if self.point == 0 { 0.0f32 } else { 1.0f32 };
        let den = Vector3::dot_product(&self.n, &self.axe[k][edge]);
        if wwmath::fabs(den) > 0.0f32 {
            let dxe = Vector3::cross_product(&self.final_d, &self.e[edge]);
            x[k] = Vector3::dot_product(&self.n, &dxe);
            if edge == 0 {
                x[k] -= self.n.length2() * yval;
            } else {
                x[k] += self.n.length2() * yval;
            }
            // the other two box axes, in index order
            for a in 0..3usize {
                if a != k {
                    x[k] -= Vector3::dot_product(&self.n, &self.axe[a][edge]) * x[a];
                }
            }
            x[k] /= den;
        } else {
            x[k] = 0.0f32;
        }
    }

    /// `obbtri_compute_contact_point(context, result)`.
    fn compute_contact_point(&mut self, result: &mut CastResultStruct) {
        let mut x = [0.0f32; 3];
        if self.axis_id >= AXIS_A0E0 {
            let newc = self.box_.center + result.fraction * self.box_move;
            let newv0 = self.tri.v[0] + result.fraction * self.tri_move;
            self.final_d = newv0 - newc;
        }

        match self.axis_id {
            // WWASSERT(0) upstream (debug only)
            INTERSECTION => return,
            // part of the box is touching the face of the triangle
            AXIS_N => {
                for i in 0..3 {
                    x[i] = eval_side(self.an[i], self.side) * self.box_.extent[i];
                }
            }
            // part of the triangle is touching a face of the box
            AXIS_A0 | AXIS_A1 | AXIS_A2 => {
                result.contact_point = if self.point == 0 {
                    self.tri.v[0]
                } else if self.point == 1 {
                    self.tri.v[1]
                } else {
                    self.tri.v[2]
                };
                result.contact_point = Vector3::add_vectors(&result.contact_point, &(result.fraction * self.tri_move));
                return;
            }
            // one of the box's edges is contacting edge e of the triangle
            AXIS_A0E0 => self.eval_point(&mut x, 0, 0),
            AXIS_A0E1 => self.eval_point(&mut x, 0, 1),
            AXIS_A0E2 => self.eval_point(&mut x, 0, 2),
            AXIS_A1E0 => self.eval_point(&mut x, 1, 0),
            AXIS_A1E1 => self.eval_point(&mut x, 1, 1),
            AXIS_A1E2 => self.eval_point(&mut x, 1, 2),
            AXIS_A2E0 => self.eval_point(&mut x, 2, 0),
            AXIS_A2E1 => self.eval_point(&mut x, 2, 1),
            AXIS_A2E2 => self.eval_point(&mut x, 2, 2),
            _ => {}
        }

        // (the clamp of x[] to the box extents is commented out upstream)
        result.contact_point.x = self.box_.center.x + x[0] * self.a[0].x + x[1] * self.a[1].x + x[2] * self.a[2].x;
        result.contact_point.y = self.box_.center.y + x[0] * self.a[0].y + x[1] * self.a[1].y + x[2] * self.a[2].y;
        result.contact_point.z = self.box_.center.z + x[0] * self.a[0].z + x[1] * self.a[1].z + x[2] * self.a[2].z;

        result.contact_point = Vector3::add_vectors(&result.contact_point, &(result.fraction * self.box_move));
    }
}

/// `eval_side(val, side)`.
fn eval_side(val: f32, side: i32) -> f32 {
    if val > 0.0f32 {
        side as f32
    } else if val < 0.0f32 {
        -(side as f32)
    } else {
        0.0f32
    }
}

/// `CollisionMath::Collide(const OBBoxClass & box, move, const TriClass & tri, trimove, result)` (:477).
pub fn collide_obbox_tri(box_: &OBBoxClass, move_: &Vector3, tri: &TriClass, trimove: &Vector3, result: &mut CastResultStruct) -> bool {
    let mut cc = BTCollisionStruct::new(box_, move_, tri, trimove);
    let ext = box_.extent;

    'exit: {
        // Axis = N
        cc.test_axis = cc.n;
        cc.test_axis_id = AXIS_N;
        cc.an[0] = Vector3::dot_product(&cc.a[0], &cc.n);
        cc.an[1] = Vector3::dot_product(&cc.a[1], &cc.n);
        cc.an[2] = Vector3::dot_product(&cc.a[2], &cc.n);
        if cc.check_normal_axis() {
            break 'exit;
        }

        // Axes A0, A1, A2
        for i in 0..3usize {
            cc.test_axis = cc.a[i];
            cc.test_axis_id = AXIS_A0 + i as i32;
            cc.ae[i][0] = Vector3::dot_product(&cc.a[i], &cc.e[0]);
            cc.ae[i][1] = Vector3::dot_product(&cc.a[i], &cc.e[1]);
            if cc.check_basis_axis(ext[i], cc.ae[i][0], cc.ae[i][1]) {
                break 'exit;
            }
        }

        // The nine A[i] x E[j] axes: A0E0, A0E1, A0E2, A1E0, ... A2E2
        for i in 0..3usize {
            for j in 0..3usize {
                cc.axe[i][j] = Vector3::cross_product(&cc.a[i], &cc.e[j]);
                cc.test_axis = cc.axe[i][j];
                cc.test_axis_id = AXIS_A0E0 + (j as i32) * 3 + i as i32;
                if j == 2 && i == 0 {
                    cc.ae[1][2] = Vector3::dot_product(&cc.a[1], &cc.e[2]);
                    cc.ae[2][2] = Vector3::dot_product(&cc.a[2], &cc.e[2]);
                }
                if j == 2 && i == 1 {
                    cc.ae[0][2] = Vector3::dot_product(&cc.a[0], &cc.e[2]);
                }
                if cc.test_axis.length2() > AXISLEN_EPSILON2 {
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

            // Move x A[i]
            for i in 0..3usize {
                cc.test_axis = Vector3::cross_product(&cc.move_, &cc.a[i]);
                if cc.test_axis.length2() > AXISLEN_EPSILON2 {
                    if cc.check_axis() {
                        break 'exit;
                    }
                }
            }
        }
    }

    // exit: "(gth) disabling an assert in obb->tri collision, investigate later"
    if cc.axis_id == INTERSECTION {
        cc.start_bad = true;
    }

    if cc.start_bad {
        result.start_bad = true;
        result.fraction = 0.0f32;
        result.normal = tri.n;
        return true;
    }

    if cc.max_frac < 0.0f32 {
        cc.max_frac = 0.0f32;
    }

    if cc.max_frac < 1.0f32 && cc.max_frac <= result.fraction {
        let mut normal = Vector3::new(0.0, 0.0, 0.0);
        cc.compute_contact_normal(&mut normal);

        if wwmath::fabs(cc.max_frac - result.fraction) > wwmath::WWMATH_EPSILON || Vector3::dot_product(&normal, move_) < Vector3::dot_product(&result.normal, move_) {
            result.normal = normal;
        }
        result.fraction = cc.max_frac;

        if result.compute_contact_point {
            result.fraction = cc.max_frac;
            cc.compute_contact_point(result);
        }
        return true;
    }
    false
}

/// `struct BTIntersectStruct`.
struct BTIntersectStruct {
    d: Vector3,
    ae: [[f32; 3]; 3],
    an: [f32; 3],
    axe: [[Vector3; 3]; 3],
    a: [Vector3; 3],
    e: [Vector3; 3],
    n: Vector3,
    test_axis: Vector3,
    box_extent: Vector3,
}

impl BTIntersectStruct {
    fn new(box_: &OBBoxClass, tri: &TriClass) -> Self {
        let e0 = Vector3::subtract_vectors(&tri.v[1], &tri.v[0]);
        let e1 = Vector3::subtract_vectors(&tri.v[2], &tri.v[0]);
        let e2 = Vector3::subtract_vectors(&e1, &e0);
        BTIntersectStruct {
            d: Vector3::subtract_vectors(&tri.v[0], &box_.center),
            ae: [[0.0; 3]; 3],
            an: [0.0; 3],
            axe: [[Vector3::new(0.0, 0.0, 0.0); 3]; 3],
            a: [
                Vector3::new(box_.basis[0][0], box_.basis[1][0], box_.basis[2][0]),
                Vector3::new(box_.basis[0][1], box_.basis[1][1], box_.basis[2][1]),
                Vector3::new(box_.basis[0][2], box_.basis[1][2], box_.basis[2][2]),
            ],
            e: [e0, e1, e2],
            n: Vector3::cross_product(&e0, &e1),
            test_axis: Vector3::new(0.0, 0.0, 0.0),
            box_extent: box_.extent,
        }
    }

    /// `obbtri_intersection_separation_test(context, lp, leb0)`: true when separated.
    fn separation_test(&self, lp: f32, leb0: f32) -> bool {
        let mut eps = 0.0f32;
        if lp - leb0 <= 0.0f32 {
            eps = COLLISION_EPSILON * self.test_axis.length(); // trying to only compute epsilon if I have to
        }
        lp - leb0 > -eps
    }

    fn cross_axis(&mut self, dp: f32, leb0: f32) -> bool {
        let mut dp = dp;
        let mut p0 = Vector3::dot_product(&self.d, &self.test_axis);

        if p0 < 0.0 {
            self.test_axis = -self.test_axis;
            p0 = -p0;
            dp = -dp;
        }

        let mut lp = 0.0f32;
        if dp < 0.0 {
            lp = dp;
        }
        lp = p0 + lp;
        self.separation_test(lp, leb0)
    }

    fn basis_axis(&mut self, leb0: f32, dp1: f32, dp2: f32) -> bool {
        let (mut dp1, mut dp2) = (dp1, dp2);
        let mut dist = Vector3::dot_product(&self.d, &self.test_axis);

        if dist < 0.0 {
            self.test_axis = -self.test_axis;
            dist = -dist;
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
        self.separation_test(lp, leb0)
    }

    fn normal_axis(&mut self) -> bool {
        let mut dist = Vector3::dot_product(&self.d, &self.test_axis);

        if dist < 0.0 {
            self.test_axis = -self.test_axis;
            dist = -dist;
        }

        let leb0 = self.box_extent.x * wwmath::fabs(self.an[0]) + self.box_extent.y * wwmath::fabs(self.an[1]) + self.box_extent.z * wwmath::fabs(self.an[2]);
        let lp = dist; // this is the "optimization", don't have to find lp
        self.separation_test(lp, leb0)
    }
}

/// `CollisionMath::Intersection_Test(const OBBoxClass & box, const TriClass & tri)` (colmathobbtri.cpp:1366).
pub fn intersection_test_obbox_tri(box_: &OBBoxClass, tri: &TriClass) -> bool {
    let mut cc = BTIntersectStruct::new(box_, tri);
    let ext = box_.extent;

    // Axis = N
    cc.test_axis = cc.n;
    cc.an[0] = Vector3::dot_product(&cc.a[0], &cc.n);
    cc.an[1] = Vector3::dot_product(&cc.a[1], &cc.n);
    cc.an[2] = Vector3::dot_product(&cc.a[2], &cc.n);
    if cc.normal_axis() {
        return false;
    }

    // Axes A0, A1, A2
    for i in 0..3usize {
        cc.test_axis = cc.a[i];
        cc.ae[i][0] = Vector3::dot_product(&cc.a[i], &cc.e[0]);
        cc.ae[i][1] = Vector3::dot_product(&cc.a[i], &cc.e[1]);
        if cc.basis_axis(ext[i], cc.ae[i][0], cc.ae[i][1]) {
            return false;
        }
    }

    // The nine A[i] x E[j] axes
    for i in 0..3usize {
        for j in 0..3usize {
            cc.axe[i][j] = Vector3::cross_product(&cc.a[i], &cc.e[j]);
            cc.test_axis = cc.axe[i][j];
            if j == 2 && i == 0 {
                cc.ae[1][2] = Vector3::dot_product(&cc.a[1], &cc.e[2]);
                cc.ae[2][2] = Vector3::dot_product(&cc.a[2], &cc.e[2]);
            }
            if j == 2 && i == 1 {
                cc.ae[0][2] = Vector3::dot_product(&cc.a[0], &cc.e[2]);
            }
            if cc.test_axis.length2() > AXISLEN_EPSILON2 {
                let dp = if j == 0 { cc.an[i] } else { -cc.an[i] };
                let leb0 = match i {
                    0 => ext[1] * wwmath::fabs(cc.ae[2][j]) + ext[2] * wwmath::fabs(cc.ae[1][j]),
                    1 => ext[0] * wwmath::fabs(cc.ae[2][j]) + ext[2] * wwmath::fabs(cc.ae[0][j]),
                    _ => ext[0] * wwmath::fabs(cc.ae[1][j]) + ext[1] * wwmath::fabs(cc.ae[0][j]),
                };
                if cc.cross_axis(dp, leb0) {
                    return false;
                }
            }
        }
    }

    true
}
