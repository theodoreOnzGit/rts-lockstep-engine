//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 programs (5.46 million floats) (test: tests/differential_draft_ww3d7_projector.rs);
//! not yet wired into the game.
//! Not covered: `zfar <= 0.1` (the draft's `assert!` in wwlib/matrix4.rs fires where upstream's compiled-out `WWASSERT` does not).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/projector.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/projector.h
//!   (Matrix4x4::Init_Ortho and the hfov/vfov Matrix4x4::Init_Perspective from WWMath/matrix4.h, which it calls;
//!   the port's `Matrix4x4` has only the six-argument frustum `init_perspective`)

use super::coltest::ObBoxClass;
use super::matrixmapper::{MappingType, MatrixMapperClass};
use crate::client::tactical_view::Matrix4x4;
use crate::formats::w3d::segline::AABox;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `class ProjectorClass` (projector.h): the data describing a projection; the common base of
/// `TexProjectClass` and `DecalGeneratorClass`. The virtual methods are plain methods (nothing in the port
/// overrides them yet); the derived classes will hold a `ProjectorClass` and call through it.
#[derive(Clone, Debug)]
pub struct ProjectorClass {
    pub(crate) transform: Matrix3D,
    pub(crate) projection: Matrix4x4,
    pub(crate) local_bounding_volume: AABox,
    pub(crate) world_bounding_volume: ObBoxClass,
    pub(crate) mapper: MatrixMapperClass,
}

/// `Matrix3x3(1)`: the identity.
fn matrix3x3_identity() -> [[f32; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

impl ProjectorClass {
    /// `ProjectorClass::ProjectorClass` (projector.cpp:32): identity transform and projection, unit boxes at the
    /// origin, and a new `MatrixMapperClass(0)`.
    pub fn new() -> Self {
        let mut transform = Matrix3D::default();
        transform.make_identity();
        ProjectorClass {
            transform,
            projection: Matrix4x4::identity(),
            local_bounding_volume: AABox { center: Vector3::new(0.0, 0.0, 0.0), extent: Vector3::new(1.0, 1.0, 1.0) },
            world_bounding_volume: ObBoxClass { center: Vector3::new(0.0, 0.0, 0.0), extent: Vector3::new(1.0, 1.0, 1.0), basis: matrix3x3_identity() },
            mapper: MatrixMapperClass::new(0),
        }
    }

    /// `ProjectorClass::Set_Transform` (projector.cpp:49).
    pub fn set_transform(&mut self, tm: &Matrix3D) {
        self.transform = *tm;
        if !crate::logic::modules::plant(19498) {
            self.update_ws_bounding_volume();
        }
    }

    /// `ProjectorClass::Get_Transform` (projector.cpp:55).
    pub fn get_transform(&self) -> &Matrix3D {
        &self.transform
    }

    /// `ProjectorClass::Set_Perspective_Projection` (projector.cpp:61). The projection matrix does not use
    /// `znear` (0.1 is used). Upstream's half-tangents are `tan(hfov) * 0.5f` (the tangent of the whole angle,
    /// halved; not the tangent of the half angle), evaluated in double and narrowed to float.
    pub fn set_perspective_projection(&mut self, hfov: f32, vfov: f32, znear: f32, zfar: f32) {
        self.mapper.set_type(MappingType::PerspectiveProjection);
        self.projection = { let mut m = Matrix4x4::identity(); if crate::logic::modules::plant(19491) { m.init_perspective_fov(vfov, hfov, 0.1, zfar) } else { m.init_perspective_fov(hfov, vfov, 0.1, zfar) }; m }; // don't use znear for the projection matrix

        let (tan_hfov2, tan_vfov2): (f32, f32) = if crate::logic::modules::plant(19492) {
            ((petir::Real::tan(f64::from(hfov) * 0.5) * f64::from(0.5f32)) as f32, (petir::Real::tan(f64::from(vfov) * 0.5) * f64::from(0.5f32)) as f32)
        } else if crate::logic::modules::plant(19493) {
            (petir::Real::tan(hfov) * 0.5f32, petir::Real::tan(vfov) * 0.5f32) // single-precision tan
        } else {
            ((petir::Real::tan(f64::from(hfov)) * f64::from(0.5f32)) as f32, (petir::Real::tan(f64::from(vfov)) * f64::from(0.5f32)) as f32)
        };

        let zc = -(zfar + znear) * 0.5;
        self.local_bounding_volume.center.set(0.0, 0.0, if crate::logic::modules::plant(19494) { -zc } else { zc }); // note, zcenter is negative
        self.local_bounding_volume.extent.x = zfar * tan_hfov2;
        self.local_bounding_volume.extent.y = zfar * tan_vfov2;
        self.local_bounding_volume.extent.z = if crate::logic::modules::plant(19495) { zfar - znear } else { (zfar - znear) * 0.5 };

        self.update_ws_bounding_volume();
    }

    /// `ProjectorClass::Set_Ortho_Projection` (projector.cpp:78).
    pub fn set_ortho_projection(&mut self, xmin: f32, xmax: f32, ymin: f32, ymax: f32, znear: f32, zfar: f32) {
        self.mapper.set_type(if crate::logic::modules::plant(19500) { MappingType::PerspectiveProjection } else { MappingType::OrthoProjection });
        self.projection = { let mut m = Matrix4x4::identity(); m.init_ortho(xmin, xmax, ymin, ymax, 0.1, zfar); m }; // don't use znear for the projection matrix

        self.local_bounding_volume.center.set(if crate::logic::modules::plant(19496) { (xmax - xmin) * 0.5 } else { (xmax + xmin) * 0.5 }, (ymax + ymin) * 0.5, -(zfar + znear) * 0.5);
        self.local_bounding_volume.extent.set((xmax - xmin) * 0.5, if crate::logic::modules::plant(19497) { (xmax - xmin) * 0.5 } else { (ymax - ymin) * 0.5 }, (zfar - znear) * 0.5);

        self.update_ws_bounding_volume();
    }

    /// The protected `Projection` member (projector.h), which the derived `TexProjectClass` and `DecalGeneratorClass` read; a public getter here so
    /// that the differential test can (upstream has none; added by #215, group ww3d7).
    pub fn get_projection(&self) -> &Matrix4x4 {
        &self.projection
    }

    /// The protected `LocalBoundingVolume` member (projector.h), exposed like [`Self::get_projection`].
    pub fn get_local_bounding_volume(&self) -> &AABox {
        &self.local_bounding_volume
    }

    /// `Get_Bounding_Volume` (projector.h): the world-space box.
    pub fn get_bounding_volume(&self) -> &ObBoxClass {
        &self.world_bounding_volume
    }

    /// `ProjectorClass::Compute_Texture_Coordinate` (projector.cpp:90).
    pub fn compute_texture_coordinate(&self, point: &Vector3) -> Vector3 {
        self.mapper.compute_texture_coordinate(point)
    }

    /// `Peek_Mapper` (projector.h).
    pub fn peek_mapper(&mut self) -> &mut MatrixMapperClass {
        &mut self.mapper
    }

    /// `ProjectorClass::Update_WS_Bounding_Volume` (projector.cpp:95): the local box, as an unrotated oriented
    /// box, transformed by `Transform`.
    pub fn update_ws_bounding_volume(&mut self) {
        let localbox = ObBoxClass {
            center: self.local_bounding_volume.center,
            extent: if crate::logic::modules::plant(19499) { Vector3::new(self.local_bounding_volume.extent.y, self.local_bounding_volume.extent.x, self.local_bounding_volume.extent.z) } else { self.local_bounding_volume.extent },
            basis: matrix3x3_identity(),
        };
        self.world_bounding_volume = ObBoxClass::transform(&self.transform, &localbox);
    }
}

impl Default for ProjectorClass {
    fn default() -> Self {
        Self::new()
    }
}
