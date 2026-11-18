//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/frustum.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/frustum.cpp
//!
//! `PlaneClass::Set(p1,p2,p3)` is the port's `PlaneClass::from_points` (same cross-product construction).

use crate::ww::matrix3d::Matrix3D;
use crate::ww::plane::PlaneClass;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;

/// `class FrustumClass`.
#[derive(Clone, Copy, Default)]
pub struct FrustumClass {
    pub camera_transform: Matrix3D,
    pub planes: [PlaneClass; 6],
    pub corners: [Vector3; 8],
    pub bound_min: Vector3,
    pub bound_max: Vector3,
}

impl FrustumClass {
    /// `FrustumClass::Get_Bound_Min`.
    pub fn get_bound_min(&self) -> &Vector3 {
        &self.bound_min
    }

    /// `FrustumClass::Get_Bound_Max`.
    pub fn get_bound_max(&self) -> &Vector3 {
        &self.bound_max
    }

    /// `FrustumClass::Init` (frustum.cpp:46).
    pub fn init(&mut self, camera: &Matrix3D, vpmin: &Vector2, vpmax: &Vector2, znear: f32, zfar: f32) {
        let (mut znear, mut zfar) = (znear, zfar);
        // Store the camera transform
        self.camera_transform = *camera;

        // Forward is negative Z in our viewspace coordinate system.
        // Just flip the sign if the user passed in positive values.
        if znear > 0.0 && zfar > 0.0 {
            znear = -znear;
            zfar = -zfar;
        }

        // calculate a proper z-vector assuming our right-handed coordinate system
        let zv = Vector3::cross_product(&self.camera_transform.get_x_vector(), &self.camera_transform.get_y_vector());

        // compare correct z-vector with the one in the camera matrix.  If they point in
        // opposite directions, we have a reflected camera matrix.
        let c = &mut self.corners;
        if Vector3::dot_product(&self.camera_transform.get_z_vector(), &zv) < 0.0 {
            // flip the frustum corners horizontally for a reflected matrix
            c[1].set(vpmin.x, vpmax.y, 1.0);
            c[5] = c[1];
            c[1] *= znear;
            c[5] *= zfar;

            c[0].set(vpmax.x, vpmax.y, 1.0);
            c[4] = c[0];
            c[0] *= znear;
            c[4] *= zfar;

            c[3].set(vpmin.x, vpmin.y, 1.0);
            c[7] = c[3];
            c[3] *= znear;
            c[7] *= zfar;

            c[2].set(vpmax.x, vpmin.y, 1.0);
            c[6] = c[2];
            c[2] *= znear;
            c[6] *= zfar;
        } else {
            // normal camera
            c[0].set(vpmin.x, vpmax.y, 1.0);
            c[4] = c[0];
            c[0] *= znear;
            c[4] *= zfar;

            c[1].set(vpmax.x, vpmax.y, 1.0);
            c[5] = c[1];
            c[1] *= znear;
            c[5] *= zfar;

            c[2].set(vpmin.x, vpmin.y, 1.0);
            c[6] = c[2];
            c[2] *= znear;
            c[6] *= zfar;

            c[3].set(vpmax.x, vpmin.y, 1.0);
            c[7] = c[3];
            c[3] *= znear;
            c[7] *= zfar;
        }

        // Transform the eight corners of the view frustum from camera space to world space.
        for i in 0..8 {
            c[i] = Matrix3D::transform_vector(&self.camera_transform, &c[i]);
        }

        // Create the six frustum bounding planes from the eight corner Corners.
        // The bounding planes are oriented so that their normals point outward
        self.planes[0] = PlaneClass::from_points(&c[0], &c[3], &c[1]); // near
        self.planes[1] = PlaneClass::from_points(&c[0], &c[5], &c[4]); // bottom
        self.planes[2] = PlaneClass::from_points(&c[0], &c[6], &c[2]); // right
        self.planes[3] = PlaneClass::from_points(&c[2], &c[7], &c[3]); // top
        self.planes[4] = PlaneClass::from_points(&c[1], &c[7], &c[5]); // left
        self.planes[5] = PlaneClass::from_points(&c[4], &c[7], &c[6]); // far

        // find the bounding box of the entire frustum (may be used for sloppy quick rejection)
        self.bound_min = c[0];
        self.bound_max = c[0];
        for i in 1..8 {
            if c[i].x < self.bound_min.x {
                self.bound_min.x = c[i].x;
            }
            if c[i].x > self.bound_max.x {
                self.bound_max.x = c[i].x;
            }
            if c[i].y < self.bound_min.y {
                self.bound_min.y = c[i].y;
            }
            if c[i].y > self.bound_max.y {
                self.bound_max.y = c[i].y;
            }
            if c[i].z < self.bound_min.z {
                self.bound_min.z = c[i].z;
            }
            if c[i].z > self.bound_max.z {
                self.bound_max.z = c[i].z;
            }
        }
    }
}
