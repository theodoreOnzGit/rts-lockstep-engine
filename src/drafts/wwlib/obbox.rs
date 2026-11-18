//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Covered by tests/differential_draft_colmath.rs (#215): `project_to_axis`, `volume`, `compute_point`, `compute_axis_aligned_extent`, `transform`, `eq_box`, `oriented_boxes_intersect`, `oriented_boxes_collide`, `oriented_box_intersects_tri`. NOT covered: `from_points` (assert(0) upstream), `ne_box`, `init_from_box_points`, `init_random`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/obbox.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/obbox.cpp
//!
//! Upstream notes kept: the point-cloud constructor `OBBoxClass(points, n)` is `assert(0)` ("TODO: IMPLEMENT THIS!!!",
//! its body is `#if 0`); `operator !=` is `(Center != ..) || (Extent != .. && Basis == ..)`, which is not `!(==)`.
//! `WWMath::Random_Float` is a caller closure. `Build_Matrix3(q)` (quat.cpp:805) is `Matrix3x3::from_quaternion`.

use crate::logic::modules::plant;
use super::matrix3::Matrix3x3;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::quat::Quaternion;
use crate::ww::tri::TriClass;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `class OBBoxClass`: `Matrix3x3 Basis`, `Vector3 Center`, `Vector3 Extent`.
#[derive(Clone, Copy, Debug, Default)]
pub struct OBBoxClass {
    pub basis: Matrix3x3,
    pub center: Vector3,
    pub extent: Vector3,
}

impl OBBoxClass {
    /// `OBBoxClass(center, extent)`: `Basis(1)` is the identity.
    pub fn new(center: &Vector3, extent: &Vector3) -> Self {
        OBBoxClass { basis: Matrix3x3::new(true), center: *center, extent: *extent }
    }

    /// `OBBoxClass(center, extent, basis)`.
    pub fn with_basis(center: &Vector3, extent: &Vector3, basis: &Matrix3x3) -> Self {
        OBBoxClass { basis: *basis, center: *center, extent: *extent }
    }

    /// `OBBoxClass(const Vector3 * points, int num_points)` (obbox.cpp:45): `assert(0)` upstream, "TODO: IMPLEMENT THIS!!!"
    /// (the eigenvector fit is under `#if 0`).
    pub fn from_points(_points: &[Vector3]) -> Self {
        panic!("OBBoxClass(points, n): TODO: IMPLEMENT THIS!!! (assert(0) upstream)");
    }

    /// `operator ==`.
    pub fn eq_box(&self, src: &OBBoxClass) -> bool {
        self.center == src.center && self.extent == src.extent && self.basis == src.basis
    }

    /// `operator !=` (obbox.h): UPSTREAM DEFECT KEPT, `(Center != ) || ((Extent != ) && (Basis == ))`.
    pub fn ne_box(&self, src: &OBBoxClass) -> bool {
        self.center != src.center || (self.extent != src.extent && self.basis == src.basis)
    }

    /// `OBBoxClass::Init_From_Box_Points` (obbox.cpp:150): the box's 8 corners; the three shortest edges from point 0
    /// give the axes. Upstream asserts `num == 8`.
    pub fn init_from_box_points(&mut self, points: &[Vector3]) {
        let num = points.len();
        assert!(num == 8);

        // "Compute the three axes": sort dp[1..] by length (selection-style swaps, as upstream)
        let mut dp = [Vector3::new(0.0, 0.0, 0.0); 8];
        for i in 1..num {
            dp[i] = points[i] - points[0];
        }
        for i in 1..num {
            for j in (i + 1)..num {
                if dp[j].length2() < dp[i].length2() {
                    dp.swap(j, i);
                }
            }
        }

        let mut axis0 = dp[1];
        axis0.normalize();
        let mut axis1 = dp[2];
        axis1.normalize();
        let axis2 = Vector3::cross_product(&axis0, &axis1);

        self.basis = Matrix3x3::from_rows(&axis0, &axis1, &axis2);

        // "Compute the center": the average of the points
        self.center.set(0.0, 0.0, 0.0);
        for i in 0..num {
            self.center += points[i];
        }
        self.center.x /= num as f32;
        self.center.y /= num as f32;
        self.center.z /= num as f32;

        // "Compute the extents": project each point onto the axes
        self.extent.set(0.0, 0.0, 0.0);
        for i in 0..num {
            let dx = points[i].x - self.center.x;
            let dy = points[i].y - self.center.y;
            let dz = points[i].z - self.center.z;

            let xprj = wwmath::fabs(axis0.x * dx + axis0.y * dy + axis0.z * dz);
            if xprj > self.extent.x {
                self.extent.x = xprj;
            }
            let yprj = wwmath::fabs(axis1.x * dx + axis1.y * dy + axis1.z * dz);
            if yprj > self.extent.y {
                self.extent.y = yprj;
            }
            let zprj = wwmath::fabs(axis2.x * dx + axis2.y * dy + axis2.z * dz);
            if zprj > self.extent.z {
                self.extent.z = zprj;
            }
        }
    }

    /// `OBBoxClass::Init_Random` (obbox.cpp:210). `random_float` is `WWMath::Random_Float`, called in upstream's order
    /// (three extents, then the four quaternion components).
    pub fn init_random(&mut self, min_extent: f32, max_extent: f32, random_float: &mut dyn FnMut() -> f32) {
        self.center.set(0.0, 0.0, 0.0);
        self.extent.x = min_extent + random_float() * (max_extent - min_extent);
        self.extent.y = min_extent + random_float() * (max_extent - min_extent);
        self.extent.z = min_extent + random_float() * (max_extent - min_extent);

        let mut orient = Quaternion { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };
        orient.x = random_float();
        orient.y = random_float();
        orient.z = random_float();
        orient.w = random_float();
        // `Quaternion::Normalize` (quat.cpp:112; the port's `ww::quat` has no member form): unchanged when |q|^2 == 0
        let len2 = orient.x * orient.x + orient.y * orient.y + orient.z * orient.z + orient.w * orient.w;
        if len2 != 0.0f32 {
            let inv_mag = wwmath::inv_sqrt(len2);
            orient.x *= inv_mag;
            orient.y *= inv_mag;
            orient.z *= inv_mag;
            orient.w *= inv_mag;
        }

        self.basis = Matrix3x3::from_quaternion(&orient);
    }

    /// `Project_To_Axis(axis)` (obbox.h).
    pub fn project_to_axis(&self, axis: &Vector3) -> f32 {
        let b = &self.basis;
        let x = self.extent[0] * (if plant(10542) { Vector3::dot_product(axis, &Vector3::new(b[0][0], b[0][1], b[0][2])) } else { Vector3::dot_product(axis, &Vector3::new(b[0][0], b[1][0], b[2][0])) });
        let y = self.extent[1] * Vector3::dot_product(axis, &Vector3::new(b[0][1], b[1][1], b[2][1]));
        let z = self.extent[2] * Vector3::dot_product(axis, &Vector3::new(b[0][2], b[1][2], b[2][2]));

        // projection is the sum of the absolute values of the projections of the three extents
        wwmath::fabs(x) + wwmath::fabs(y) + wwmath::fabs(z)
    }

    /// `Volume()` (obbox.h): double arithmetic returned as `float`.
    pub fn volume(&self) -> f32 {
        (if plant(10543) { 2.0f32 * self.extent.x * 2.0f32 * self.extent.y * 2.0f32 * self.extent.z } else { (2.0f64 * self.extent.x as f64 * 2.0f64 * self.extent.y as f64 * 2.0f64 * self.extent.z as f64) as f32 })
    }

    /// `Compute_Point(params, &set_point)` (obbox.h).
    pub fn compute_point(&self, params: &[f32; 3]) -> Vector3 {
        let mut point = self.extent;
        point.x *= params[0];
        point.y *= if plant(10544) { params[0] } else { params[1] };
        point.z *= params[2];

        let rotated = Matrix3x3::rotate_vector(&self.basis, &point);
        Vector3::add_vectors(&self.center, &rotated)
    }

    /// `Compute_Axis_Aligned_Extent(&set_extent)` (obbox.h): "x extent is the box projected onto the x axis".
    pub fn compute_axis_aligned_extent(&self) -> Vector3 {
        let e = &self.extent;
        let b = &self.basis;
        Vector3::new(
            wwmath::fabs(e[0] * b[0][0]) + (if plant(10545) { e[1] * b[0][1] } else { wwmath::fabs(e[1] * b[0][1]) }) + wwmath::fabs(e[2] * b[0][2]),
            wwmath::fabs(e[0] * b[1][0]) + wwmath::fabs(e[1] * b[1][1]) + wwmath::fabs(e[2] * b[1][2]),
            wwmath::fabs(e[0] * b[2][0]) + wwmath::fabs(e[1] * b[2][1]) + wwmath::fabs(e[2] * b[2][2]),
        )
    }

    /// `OBBoxClass::Transform(tm, in, &out)` (obbox.h).
    pub fn transform(tm: &Matrix3D, input: &OBBoxClass) -> OBBoxClass {
        OBBoxClass {
            extent: input.extent,
            center: (if plant(10546) { input.center } else { Matrix3D::transform_vector(tm, &input.center) }),
            basis: Matrix3x3::mul_matrix3d_matrix(tm, &input.basis),
        }
    }
}

/// The box's axes in world space: `Vector3(Basis[0][i], Basis[1][i], Basis[2][i])`.
fn basis_axis(b: &Matrix3x3, i: usize) -> Vector3 {
    Vector3::new(b[0][i], b[1][i], b[2][i])
}

/// `Oriented_Boxes_Intersect_On_Axis` (obbox.cpp:236).
fn oriented_boxes_intersect_on_axis(box0: &OBBoxClass, box1: &OBBoxClass, axis: &Vector3) -> bool {
    if axis.length2() < wwmath::WWMATH_EPSILON {
        return true;
    }

    let ra = box0.project_to_axis(axis);
    let rb = box1.project_to_axis(axis);
    let rsum = wwmath::fabs(ra) + wwmath::fabs(rb);

    // project the center distance onto the line:
    let c = box1.center - box0.center;
    let cdist = Vector3::dot_product(axis, &c);

    if (cdist > rsum) || (cdist < -rsum) {
        return false;
    }
    true
}

/// `Oriented_Boxes_Intersect(box0, box1)` (obbox.cpp:265): the 15 separating axes, the six face normals then the nine
/// cross products `A[i] x B[j]`.
pub fn oriented_boxes_intersect(box0: &OBBoxClass, box1: &OBBoxClass) -> bool {
    let a = [basis_axis(&box0.basis, 0), basis_axis(&box0.basis, 1), basis_axis(&box0.basis, 2)];
    let b = [basis_axis(&box1.basis, 0), basis_axis(&box1.basis, 1), basis_axis(&box1.basis, 2)];

    for i in 0..3 {
        if !oriented_boxes_intersect_on_axis(box0, box1, &a[i]) {
            return false;
        }
    }
    for i in 0..3 {
        if !oriented_boxes_intersect_on_axis(box0, box1, &b[i]) {
            return false;
        }
    }
    for i in 0..3 {
        for j in 0..3 {
            let axis = Vector3::cross_product(&a[i], &b[j]);
            if !oriented_boxes_intersect_on_axis(box0, box1, &axis) {
                return false;
            }
        }
    }

    // None of the above tests separated the two boxes, so they are intersecting
    true
}

/// `Oriented_Boxes_Collide_On_Axis` (obbox.cpp:330).
fn oriented_boxes_collide_on_axis(box0: &OBBoxClass, v0: &Vector3, box1: &OBBoxClass, v1: &Vector3, axis: &Vector3, dt: f32) -> bool {
    if axis.length2() < wwmath::WWMATH_EPSILON {
        return true;
    }

    let ra = box0.project_to_axis(axis);
    let rb = box1.project_to_axis(axis);
    let rsum = wwmath::fabs(ra) + wwmath::fabs(rb);

    // project the center distance onto the line:
    let c = box1.center - box0.center;
    let v = *v1 - *v0;
    let cdist = Vector3::dot_product(axis, &c);
    let vdist = cdist + dt * Vector3::dot_product(axis, &v);

    if (cdist > rsum && vdist > rsum) || (cdist < -rsum && vdist < -rsum) {
        return false;
    }
    true
}

/// `Oriented_Boxes_Collide(box0, v0, box1, v1, dt)` (obbox.cpp:364): same 15 axes in the same order
/// (`goto exit` is an early `return false`).
pub fn oriented_boxes_collide(box0: &OBBoxClass, v0: &Vector3, box1: &OBBoxClass, v1: &Vector3, dt: f32) -> bool {
    let a = [basis_axis(&box0.basis, 0), basis_axis(&box0.basis, 1), basis_axis(&box0.basis, 2)];
    let b = [basis_axis(&box1.basis, 0), basis_axis(&box1.basis, 1), basis_axis(&box1.basis, 2)];

    for i in 0..3 {
        if !oriented_boxes_collide_on_axis(box0, v0, box1, v1, &a[i], dt) {
            return false;
        }
    }
    for i in 0..3 {
        if !oriented_boxes_collide_on_axis(box0, v0, box1, v1, &b[i], dt) {
            return false;
        }
    }
    for i in 0..3 {
        for j in 0..3 {
            let sep_axis = Vector3::cross_product(&a[i], &b[j]);
            if !oriented_boxes_collide_on_axis(box0, v0, box1, v1, &sep_axis, dt) {
                return false;
            }
        }
    }
    true
}

/// `Oriented_Box_Intersects_Tri_On_Axis` (obbox.cpp:658). `axis` is flipped in place (upstream takes it by reference
/// and `axis = -axis` when it points away from the triangle).
fn oriented_box_intersects_tri_on_axis(box_: &OBBoxClass, tri: &TriClass, axis: &mut Vector3) -> bool {
    if axis.length2() < wwmath::WWMATH_EPSILON {
        return true;
    }

    let d = tri.v[0] - box_.center;
    let r1 = tri.v[1] - tri.v[0];
    let r2 = tri.v[2] - tri.v[0];

    // I want the axis to point from box.center to tri.v0
    let mut dist = Vector3::dot_product(&d, axis);
    if dist < 0.0 {
        dist = -dist;
        *axis = -*axis;
    }

    // compute leading edge of the box
    let leb = box_.project_to_axis(axis);

    // compute the leading edge of the triangle
    let mut lep = 0.0f32;
    let mut tmp = Vector3::dot_product(&r1, axis);
    if tmp < lep {
        lep = tmp;
    }
    tmp = Vector3::dot_product(&r2, axis);
    if tmp < lep {
        lep = tmp;
    }
    lep += dist;

    if (if plant(10547) { lep > leb } else { lep >= leb }) {
        return false;
    }
    true
}

/// `Oriented_Box_Intersects_Tri(box, tri)` (obbox.cpp:707): the triangle normal, the box's 3 axes, then the nine
/// `A[i] x E[j]` (edges `V1-V0`, `V2-V1`, `V0-V2`).
pub fn oriented_box_intersects_tri(box_: &OBBoxClass, tri: &TriClass) -> bool {
    let mut a = [basis_axis(&box_.basis, 0), basis_axis(&box_.basis, 1), basis_axis(&box_.basis, 2)];
    let e = [tri.v[1] - tri.v[0], tri.v[2] - tri.v[1], tri.v[0] - tri.v[2]];

    let mut normal = tri.n;
    if !oriented_box_intersects_tri_on_axis(box_, tri, &mut normal) {
        return false;
    }
    for i in 0..3 {
        // upstream passes `A[i]` itself by reference: the flip toward the triangle stays in `A[i]` for the cross products
        // below (~~the draft flipped a copy~~ **Corrected 2026-11-18 (#215):** found by the differential test, 3 of 20,000 cases)
        if !(if plant(10548) { { let mut c = a[i]; oriented_box_intersects_tri_on_axis(box_, tri, &mut c) } } else { oriented_box_intersects_tri_on_axis(box_, tri, &mut a[i]) }) {
            return false;
        }
    }
    for i in 0..3 {
        for j in 0..3 {
            let mut axis = Vector3::cross_product(&a[i], &e[j]);
            if !oriented_box_intersects_tri_on_axis(box_, tri, &mut axis) {
                return false;
            }
        }
    }
    true
}
