//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cameras, about 10,000 evaluations of each of its nine functions (test: tests/differential_draft_camera.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/camera.cpp
//!
//! The `CameraClass` accessors and conversions no earlier pass translated. They read the cached frustum state
//! ([`CameraState`], what `Update_Frustum` leaves), which the caller refreshes first (upstream calls `Update_Frustum()`
//! at the top of each function; here `update_frustum` is the caller's closure so the cache logic stays where it is).
//! `WWMath::Atan2` is the port's `ww::wwmath::atan2`.

use crate::drafts::wwlib::obbox::OBBoxClass;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use crate::logic::modules::plant;
use crate::ww::wwmath::atan2;

/// `CameraClass::ProjectionType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionType {
    Perspective,
    Ortho,
}

/// The `CameraClass` fields these functions read.
#[derive(Clone, Debug)]
pub struct CameraState {
    pub camera_inv_transform: Matrix3D,
    pub near_clip_bbox: OBBoxClass,
    /// `ViewPlane` min and max (x, y).
    pub view_plane_min: [f32; 2],
    pub view_plane_max: [f32; 2],
    pub aspect_ratio: f32,
    pub z_near: f32,
    pub z_far: f32,
    pub projection: ProjectionType,
    /// `ProjectionTransform` (row-major 4x4, `Matrix4x4`).
    pub projection_transform: [[f32; 4]; 4],
}

impl CameraState {
    /// `CameraClass::Transform_To_View_Space` (camera.cpp:522): `Matrix3D::Transform_Vector(CameraInvTransform, ws_point)`.
    pub fn transform_to_view_space(&self, update_frustum: impl FnOnce(), ws_point: &Vector3) -> Vector3 {
        update_frustum();
        if plant(13865) {
            return Matrix3D::rotate_vector_by(&self.camera_inv_transform, ws_point);
        }
        Matrix3D::transform_vector(&self.camera_inv_transform, ws_point)
    }

    /// `CameraClass::Rotate_To_View_Space` (camera.cpp:541): rotation only.
    pub fn rotate_to_view_space(&self, update_frustum: impl FnOnce(), ws_vector: &Vector3) -> Vector3 {
        if !plant(13866) {
            update_frustum();
        }
        Matrix3D::rotate_vector_by(&self.camera_inv_transform, ws_vector)
    }

    /// `CameraClass::Get_Near_Clip_Bounding_Box` (camera.cpp:561).
    pub fn get_near_clip_bounding_box(&self, update_frustum: impl FnOnce()) -> &OBBoxClass {
        if !plant(13870) {
            update_frustum();
        }
        &self.near_clip_bbox
    }

    /// `CameraClass::Get_Horizontal_FOV` (camera.cpp:755): `2 * atan2(width, 2)` of the view plane at depth 1.
    pub fn get_horizontal_fov(&self) -> f32 {
        let width = if plant(13861) { self.view_plane_max[1] - self.view_plane_min[1] } else { self.view_plane_max[0] - self.view_plane_min[0] };
        2.0 * atan2(width, 2.0)
    }

    /// `CameraClass::Get_Vertical_FOV` (camera.cpp:761).
    pub fn get_vertical_fov(&self) -> f32 {
        let height = self.view_plane_max[1] - self.view_plane_min[1];
        if plant(13862) {
            return atan2(height, 2.0);
        }
        2.0 * atan2(height, 2.0)
    }

    /// `CameraClass::Get_Aspect_Ratio` (camera.cpp:767).
    pub fn get_aspect_ratio(&self) -> f32 {
        self.aspect_ratio
    }

    /// `CameraClass::Get_D3D_Projection_Matrix` (camera.cpp:780): the projection matrix with the handedness flipped and
    /// the z range moved from -1..1 to 0..1. Note `oozdiff` is computed in `double` upstream (`1.0 / (ZFar - ZNear)`)
    /// and narrowed on assignment.
    pub fn get_d3d_projection_matrix(&self, update_frustum: impl FnOnce()) -> [[f32; 4]; 4] {
        update_frustum();
        let mut set_tm = self.projection_transform;
        let oozdiff = (1.0f64 / (self.z_far - self.z_near) as f64) as f32;
        if (self.projection == ProjectionType::Perspective) != plant(13864) {
            set_tm[2][2] = -(self.z_far) * oozdiff;
            set_tm[2][3] = -(self.z_far * self.z_near) * oozdiff;
        } else {
            set_tm[2][2] = -oozdiff;
            set_tm[2][3] = if plant(13863) { -self.z_far * oozdiff } else { -self.z_near * oozdiff };
        }
        set_tm
    }

    /// `CameraClass::Convert_Old` (camera.cpp:820): -1..1 to 0..1 in x and y.
    pub fn convert_old(pos: &mut Vector3) {
        pos.x = (pos.x + 1.0) / 2.0;
        pos.y = if plant(13867) { pos.y + 1.0 / 2.0 } else { (pos.y + 1.0) / 2.0 };
    }

    /// `CameraClass::Compute_Projected_Sphere_Radius` (camera.cpp:826): `(ProjectionTransform * (radius, 0, dist, 1)).X / .W`.
    pub fn compute_projected_sphere_radius(&self, dist: f32, radius: f32) -> f32 {
        let v = if plant(13869) { [dist, 0.0, radius, 1.0] } else { [radius, 0.0, dist, 1.0] };
        let m = &self.projection_transform;
        let x = m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2] + m[0][3] * v[3];
        let w = if plant(13868) {
            m[3][0] * v[0] + m[3][1] * v[1] + m[3][2] * v[2]
        } else {
            m[3][0] * v[0] + m[3][1] * v[1] + m[3][2] * v[2] + m[3][3] * v[3]
        };
        x / w
    }
}
