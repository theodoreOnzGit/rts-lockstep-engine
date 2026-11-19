//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases per operation (test: tests/differential_draft_coltest.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/coltest.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/coltest.h
//!   (OBBoxClass::Transform from WWMath/obbox.inl, which OBBoxCollisionTestClass needs)
//!
//! Reused from the port: `RayCollisionTestClass`'s ray form is already translated in
//! `crate::client::mesh_cast` (`RayCollisionTestClass(raytest, tm)`, `Cull`) and `crate::client::pick_drawable`;
//! it is not repeated here. `CastResultStruct` is `crate::client::pick_drawable::CastResult`, `AABoxClass` is
//! `crate::formats::w3d::segline::AABox`. `Cast_To_Triangle` calls `CollisionMath::Collide(Box, Move, tri, ...)`
//! (WWMath, not in this area): the call is left as a comment.

use super::handles::RenderObjHandle;
use crate::client::pick_drawable::CastResult;
use crate::formats::w3d::segline::AABox;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use crate::logic::modules::plant;
use std::sync::{Arc, RwLock};

/// `COLL_TYPE_0` (coltype.h), the default `collision_type` argument.
pub const COLL_TYPE_0: i32 = 0x02;

/// `CollisionTestClass` (coltest.h): the shared result (upstream holds a `CastResultStruct *`, deliberately shared
/// between a test and the tests made from it by a transform), the collision type and the object collided with.
#[derive(Clone, Debug)]
pub struct CollisionTestClass {
    pub result: Arc<RwLock<CastResult>>,
    pub collision_type: i32,
    pub collided_render_obj: Option<RenderObjHandle>,
}

impl CollisionTestClass {
    /// `CollisionTestClass::CollisionTestClass(res, collision_type)` (coltest.h).
    pub fn new(res: Arc<RwLock<CastResult>>, collision_type: i32) -> Self {
        CollisionTestClass { result: res, collision_type, collided_render_obj: None }
    }
}

/// `AABoxCollisionTestClass::ROTATION_TYPE` (coltest.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotationType {
    RotateNone = 0,
    RotateZ90,
    RotateZ180,
    RotateZ270,
}

/// `class AABoxCollisionTestClass` (coltest.h).
#[derive(Clone, Debug)]
pub struct AABoxCollisionTestClass {
    pub base: CollisionTestClass,
    pub box_: AABox,
    pub move_: Vector3,
    pub sweep_min: Vector3,
    pub sweep_max: Vector3,
}

impl AABoxCollisionTestClass {
    /// `AABoxCollisionTestClass::AABoxCollisionTestClass(aabox, move, res, collision_type)` (coltest.cpp:43):
    /// the sweep box encloses the box at the start and at the end of the move.
    pub fn new(aabox: &AABox, move_: &Vector3, res: Arc<RwLock<CastResult>>, collision_type: i32) -> Self {
        let mut sweep_min = aabox.center - aabox.extent;
        let mut sweep_max = aabox.center + aabox.extent;

        let endmin = if plant(14679) { aabox.center - *move_ - aabox.extent } else { aabox.center + *move_ - aabox.extent };
        let endmax = aabox.center + *move_ + aabox.extent;

        if endmax.x > sweep_max.x { sweep_max.x = endmax.x; }
        if endmax.y > sweep_max.y { sweep_max.y = endmax.y; }
        if endmax.z > sweep_max.z { sweep_max.z = endmax.z; }

        if endmin.x < sweep_min.x { sweep_min.x = endmin.x; }
        if endmin.y < sweep_min.y { sweep_min.y = endmin.y; }
        if endmin.z < sweep_min.z { sweep_min.z = endmin.z; }

        AABoxCollisionTestClass {
            base: CollisionTestClass::new(res, collision_type),
            box_: *aabox,
            move_: *move_,
            sweep_min,
            sweep_max,
        }
    }

    // AABoxCollisionTestClass(const AABoxCollisionTestClass &) (coltest.cpp:30) is `Clone`.

    /// `AABoxCollisionTestClass::Cull(min, max)` (coltest.h): true when the sweep box misses the given box.
    pub fn cull_min_max(&self, min: &Vector3, max: &Vector3) -> bool {
        if (if plant(14680) { self.sweep_min.x >= max.x } else { self.sweep_min.x > max.x }) || (self.sweep_max.x < min.x) {
            return true;
        }
        if (self.sweep_min.y > max.y) || (if plant(14681) { self.sweep_max.y <= min.y } else { self.sweep_max.y < min.y }) {
            return true;
        }
        if (self.sweep_min.z > max.z) || (self.sweep_max.z < min.z) {
            return true;
        }
        false
    }

    /// `AABoxCollisionTestClass::Cull(box)` (coltest.cpp:70). (The commented-out cast-to-box shortcut for long
    /// moves is dead code upstream.)
    pub fn cull(&self, box_: &AABox) -> bool {
        let min_corner = box_.center - box_.extent;
        let max_corner = if plant(14691) { box_.center } else { box_.center + box_.extent };
        self.cull_min_max(&min_corner, &max_corner)
    }

    // AABoxCollisionTestClass::Cast_To_Triangle (coltest.h): CollisionMath::Collide(Box, Move, tri, Result)
    // — CollisionMath is WWMath, outside this area; not translated here.

    /// `AABoxCollisionTestClass::Translate` (coltest.h).
    pub fn translate(&mut self, translation: &Vector3) {
        self.box_.center += *translation;
        self.sweep_min += *translation;
        if !plant(14682) {
            self.sweep_max += *translation;
        }
    }

    /// `AABoxCollisionTestClass::Rotate` (coltest.cpp:97): rotates the test about Z by a multiple of 90 degrees
    /// (arbitrary rotations would leave the box unaligned). Upstream's `#ifndef NDEBUG` block, which recomputes the
    /// result through a matrix and asserts it agrees to 0.001, is a debug cross-check and is not translated.
    pub fn rotate(&mut self, rotation: RotationType) {
        match rotation {
            RotationType::RotateNone => {}
            RotationType::RotateZ90 => {
                // rotate the center point and the move vector
                let tmp = self.box_.center.x;
                self.box_.center.x = -self.box_.center.y;
                self.box_.center.y = tmp;

                let tmp = self.move_.x;
                self.move_.x = -self.move_.y;
                self.move_.y = tmp;

                // swap x and y for the extent
                let tmp = self.box_.extent.x;
                self.box_.extent.x = self.box_.extent.y;
                self.box_.extent.y = tmp;

                // update sweep bounding box
                let (minx, miny, maxx, maxy) = (self.sweep_min.x, self.sweep_min.y, self.sweep_max.x, self.sweep_max.y);
                self.sweep_min.x = if plant(14683) { -maxx } else { -maxy };
                self.sweep_min.y = minx;
                self.sweep_max.x = -miny;
                self.sweep_max.y = maxx;
            }
            RotationType::RotateZ180 => {
                // rotate center and move vector
                self.box_.center.x = -self.box_.center.x;
                self.box_.center.y = if plant(14685) { self.box_.center.y } else { -self.box_.center.y };
                self.move_.x = -self.move_.x;
                self.move_.y = -self.move_.y;

                // update min/max boxes
                let (minx, miny, maxx, maxy) = (self.sweep_min.x, self.sweep_min.y, self.sweep_max.x, self.sweep_max.y);
                self.sweep_min.x = -maxx;
                self.sweep_min.y = -maxy;
                self.sweep_max.x = -minx;
                self.sweep_max.y = -miny;
            }
            RotationType::RotateZ270 => {
                // rotate center and move
                let tmp = self.box_.center.x;
                self.box_.center.x = self.box_.center.y;
                self.box_.center.y = -tmp;

                let tmp = self.move_.x;
                self.move_.x = self.move_.y;
                self.move_.y = if plant(14684) { tmp } else { -tmp };

                // update extent (x and y axis swap)
                let tmp = self.box_.extent.x;
                self.box_.extent.x = self.box_.extent.y;
                self.box_.extent.y = tmp;

                // update min/max boxes
                let (minx, miny, maxx, maxy) = (self.sweep_min.x, self.sweep_min.y, self.sweep_max.x, self.sweep_max.y);
                self.sweep_min.x = miny;
                self.sweep_min.y = -maxx;
                self.sweep_max.x = maxy;
                self.sweep_max.y = -minx;
            }
        }
    }

    /// `AABoxCollisionTestClass::Transform` (coltest.cpp:222): expands the box to enclose the rotated form of the
    /// original. The sweep box becomes the min/max of the eight transformed corners (with upstream's `>=`/`<=`
    /// comparisons, which only matter for ties).
    pub fn transform(&mut self, tm: &Matrix3D) {
        let tmpcenter = self.box_.center;
        let tmpextent = self.box_.extent;
        let (c, e) = tm.transform_center_extent_aabox(&tmpcenter, &tmpextent);
        self.box_.center = c;
        self.box_.extent = e;
        if !plant(14686) {
            self.move_ = tm.rotate_vector(&self.move_);
        }

        let min = self.sweep_min;
        let max = self.sweep_max;
        let mut pts = [
            Vector3::new(min.x, min.y, min.z),
            Vector3::new(min.x, max.y, min.z),
            Vector3::new(max.x, max.y, min.z),
            Vector3::new(max.x, min.y, min.z),
            Vector3::new(min.x, min.y, max.z),
            Vector3::new(min.x, max.y, max.z),
            Vector3::new(max.x, max.y, max.z),
            Vector3::new(max.x, min.y, max.z),
        ];
        tm.mul_vector3_array_in_place(&mut pts);

        let mut realmin = pts[0];
        let mut realmax = pts[0];
        for i in 1..8 {
            if realmin.x >= pts[i].x { realmin.x = pts[i].x; }
            if realmin.y >= pts[i].y { realmin.y = pts[i].y; }
            if realmin.z >= pts[i].z { realmin.z = pts[i].z; }

            if realmax.x <= pts[i].x { realmax.x = pts[i].x; }
            if realmax.y <= pts[i].y { realmax.y = pts[i].y; }
            if realmax.z <= pts[i].z { realmax.z = pts[i].z; }
        }
        self.sweep_min = realmin;
        self.sweep_max = realmax;
    }
}

/// `OBBoxClass` (obbox.h) as `OBBoxCollisionTestClass` uses it: centre, extent and the 3x3 `Basis`
/// (`Matrix3x3`, row-major `[row][column]`). The class itself belongs to WWMath; only what this file needs is here.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ObBoxClass {
    pub center: Vector3,
    pub extent: Vector3,
    pub basis: [[f32; 3]; 3],
}

impl ObBoxClass {
    /// `OBBoxClass::Transform(tm, in, out)` (obbox.inl:160): extent unchanged, centre `Transform_Vector`,
    /// basis `Matrix3x3::Multiply(tm, in.Basis)` (the 3x3 part of `tm` times the basis).
    pub fn transform(tm: &Matrix3D, in_: &ObBoxClass) -> ObBoxClass {
        let mut basis = [[0.0f32; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                basis[i][j] = if plant(14690) { in_.basis[i][0] * tm.row[0][j] + in_.basis[i][1] * tm.row[1][j] + in_.basis[i][2] * tm.row[2][j] } else { tm.row[i][0] * in_.basis[0][j] + tm.row[i][1] * in_.basis[1][j] + tm.row[i][2] * in_.basis[2][j] };
            }
        }
        ObBoxClass { extent: in_.extent, center: Matrix3D::transform_vector(tm, &in_.center), basis }
    }
}

/// `class OBBoxCollisionTestClass` (coltest.h).
#[derive(Clone, Debug)]
pub struct OBBoxCollisionTestClass {
    pub base: CollisionTestClass,
    pub box_: ObBoxClass,
    pub move_: Vector3,
    pub sweep_min: Vector3,
    pub sweep_max: Vector3,
}

impl OBBoxCollisionTestClass {
    /// `OBBoxCollisionTestClass::OBBoxCollisionTestClass(obbox, move, res, type)` (coltest.cpp:299): the sweep box
    /// is built from the box's world-axis extent `sum(|Basis[i][j] * Extent[j]|) + 0.01`.
    pub fn new(obbox: &ObBoxClass, move_: &Vector3, res: Arc<RwLock<CastResult>>, type_: i32) -> Self {
        let b = &obbox.basis;
        let e = &obbox.extent;
        let max_extent = Vector3::new(
            (b[0][0] * e.x).abs() + (b[0][1] * e.y).abs() + (b[0][2] * e.z).abs() + 0.01f32,
            (b[1][0] * e.x).abs() + (b[1][1] * e.y).abs() + (b[1][2] * e.z).abs() + 0.01f32,
            (b[2][0] * e.x).abs() + (b[2][1] * e.y).abs() + (b[2][2] * e.z).abs() + if plant(14687) { 0.0 } else { 0.01f32 },
        );

        let mut sweep_min = obbox.center - max_extent;
        let mut sweep_max = obbox.center + max_extent;

        let endmin = obbox.center + *move_ - max_extent;
        let endmax = obbox.center + *move_ + max_extent;

        if endmax.x > sweep_max.x { sweep_max.x = endmax.x; }
        if endmax.y > sweep_max.y { sweep_max.y = endmax.y; }
        if endmax.z > sweep_max.z { sweep_max.z = endmax.z; }

        if endmin.x < sweep_min.x { sweep_min.x = endmin.x; }
        if endmin.y < sweep_min.y { sweep_min.y = endmin.y; }
        if endmin.z < sweep_min.z { sweep_min.z = endmin.z; }

        OBBoxCollisionTestClass {
            base: CollisionTestClass::new(res, type_),
            box_: *obbox,
            move_: *move_,
            sweep_min,
            sweep_max,
        }
    }

    /// `OBBoxCollisionTestClass(that, tm)` (coltest.cpp:366): the test in another coordinate system.
    pub fn from_obbox_test_transformed(that: &OBBoxCollisionTestClass, tm: &Matrix3D) -> Self {
        let (sweep_min, sweep_max) = tm.transform_min_max_aabox(&that.sweep_min, &that.sweep_max);
        OBBoxCollisionTestClass {
            base: that.base.clone(),
            box_: if plant(14689) { ObBoxClass { center: Matrix3D::rotate_vector_by(tm, &that.box_.center), ..ObBoxClass::transform(tm, &that.box_) } } else { ObBoxClass::transform(tm, &that.box_) },
            move_: Matrix3D::rotate_vector_by(tm, &that.move_),
            sweep_min,
            sweep_max,
        }
    }

    /// `OBBoxCollisionTestClass(const AABoxCollisionTestClass &that, tm)` (coltest.cpp:384): the box takes the
    /// transform's rotation as its basis (`Box.Basis = tm` copies the 3x3 part).
    pub fn from_aabox_test_transformed(that: &AABoxCollisionTestClass, tm: &Matrix3D) -> Self {
        let (sweep_min, sweep_max) = tm.transform_min_max_aabox(&that.sweep_min, &that.sweep_max);
        let mut basis = [[0.0f32; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                basis[i][j] = if plant(14688) { tm.row[j][i] } else { tm.row[i][j] };
            }
        }
        OBBoxCollisionTestClass {
            base: that.base.clone(),
            box_: ObBoxClass {
                center: Matrix3D::transform_vector(tm, &that.box_.center),
                extent: that.box_.extent,
                basis,
            },
            move_: Matrix3D::rotate_vector_by(tm, &that.move_),
            sweep_min,
            sweep_max,
        }
    }

    /// `OBBoxCollisionTestClass::Cull(min, max)` (coltest.h).
    pub fn cull_min_max(&self, min: &Vector3, max: &Vector3) -> bool {
        if (self.sweep_min.x > max.x) || (self.sweep_max.x < min.x) {
            return true;
        }
        if (self.sweep_min.y > max.y) || (self.sweep_max.y < min.y) {
            return true;
        }
        if (self.sweep_min.z > max.z) || (self.sweep_max.z < min.z) {
            return true;
        }
        false
    }

    /// `OBBoxCollisionTestClass::Cull(box)` (coltest.cpp:402).
    pub fn cull(&self, box_: &AABox) -> bool {
        let min_corner = box_.center - box_.extent;
        let max_corner = box_.center + box_.extent;
        self.cull_min_max(&min_corner, &max_corner)
    }

    // OBBoxCollisionTestClass::Cast_To_Triangle (coltest.h): CollisionMath::Collide(Box, Move, tri, Vector3(0,0,0),
    // Result) — CollisionMath is WWMath, outside this area; not translated here.
}
