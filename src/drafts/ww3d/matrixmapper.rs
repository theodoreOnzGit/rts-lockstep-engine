//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/matrixmapper.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/matrixmapper.h
//!
//! Reused from the port: `Matrix4x4` is `crate::client::tactical_view::Matrix4x4` (only what it has; the
//! `Matrix4x4(Matrix3D)` constructor and `Multiply` are written here from matrix4.h/matrix4.cpp as small helpers).
//! `Apply` sets DX8 transforms and texture stage states; it is device-only, so what it would install is returned by
//! [`MatrixMapperClass::apply_state`] and the device calls are comments.

use crate::client::tactical_view::Matrix4x4;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use crate::ww::vector4::Vector4;

/// `MatrixMapperClass::INVERT_DEPTH_GRADIENT` (matrixmapper.h).
pub const INVERT_DEPTH_GRADIENT: u32 = 0x0000_0001;

/// `MatrixMapperClass::MappingType` (matrixmapper.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MappingType {
    OrthoProjection = 0,
    PerspectiveProjection,
    DepthGradient,
    NormalGradient,
}

/// `Matrix4x4::Matrix4x4(const Matrix3D &)` (matrix4.h:246): rows 0-2 copied, row 3 is (0,0,0,1).
pub fn matrix4x4_from_matrix3d(m: &Matrix3D) -> Matrix4x4 {
    Matrix4x4 { row: [m.row[0], m.row[1], m.row[2], Vector4::new(0.0, 0.0, 0.0, 1.0)] }
}

/// `Matrix4x4::Multiply(a, b, &res)` (matrix4.cpp:61): `res[i][j] = sum_k a[i][k] * b[k][j]`.
pub fn matrix4x4_multiply(a: &Matrix4x4, b: &Matrix4x4) -> Matrix4x4 {
    let mut res = Matrix4x4::identity();
    for i in 0..4 {
        for j in 0..4 {
            res.row[i][j] = a.row[i][0] * b.row[0][j] + a.row[i][1] * b.row[1][j] + a.row[i][2] * b.row[2][j] + a.row[i][3] * b.row[3][j];
        }
    }
    res
}

/// What `MatrixMapperClass::Apply` would give the device for one stage: the texture transform, the texcoord
/// source and the transform flags (as the D3D constants' names).
#[derive(Clone, Debug, PartialEq)]
pub struct MapperDeviceState {
    pub texture_transform: Matrix4x4,
    /// `D3DTSS_TCI_CAMERASPACEPOSITION` or `D3DTSS_TCI_CAMERASPACENORMAL`.
    pub tex_coord_index: &'static str,
    /// `D3DTTFF_COUNT2`, or `D3DTTFF_PROJECTED|D3DTTFF_COUNT3`.
    pub texture_transform_flags: &'static str,
}

/// `class MatrixMapperClass : public TextureMapperClass` (matrixmapper.h): the u-v coordinates of a projected
/// texture. Something external must keep its `ViewToTexture` up to date.
#[derive(Clone, Debug)]
pub struct MatrixMapperClass {
    /// `TextureMapperClass::Stage`.
    pub stage: i32,
    flags: u32,
    type_: MappingType,
    view_to_texture: Matrix4x4,
    pub(crate) view_to_pixel: Matrix4x4,
    view_space_projection_normal: Vector3,
    gradient_u_coord: f32,
}

impl MatrixMapperClass {
    /// `MatrixMapperClass::MatrixMapperClass(stage)` (matrixmapper.cpp:36): ortho, identity matrices (`true` =
    /// initialise to identity), gradient U 0.5.
    pub fn new(stage: i32) -> Self {
        MatrixMapperClass {
            stage,
            flags: 0,
            type_: MappingType::OrthoProjection,
            view_to_texture: Matrix4x4::identity(),
            view_to_pixel: Matrix4x4::identity(),
            view_space_projection_normal: Vector3::new(0.0, 0.0, 0.0),
            gradient_u_coord: 0.5,
        }
    }

    /// `Set_Flag` (matrixmapper.h).
    pub fn set_flag(&mut self, flag: u32, onoff: bool) {
        if onoff {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    /// `Get_Flag` (matrixmapper.h): all the bits of `flag` set.
    pub fn get_flag(&self, flag: u32) -> bool {
        (self.flags & flag) == flag
    }

    /// `Set_Type` (matrixmapper.h).
    pub fn set_type(&mut self, type_: MappingType) {
        self.type_ = type_;
    }

    /// `Get_Type` (matrixmapper.h).
    pub fn get_type(&self) -> MappingType {
        self.type_
    }

    /// `MatrixMapperClass::Set_Texture_Transform(const Matrix3D &, texsize)` (matrixmapper.cpp:50).
    pub fn set_texture_transform_3d(&mut self, view_to_texture: &Matrix3D, texsize: f32) {
        self.view_to_texture = matrix4x4_from_matrix3d(view_to_texture);
        self.update_view_to_pixel_transform(texsize);
    }

    /// `MatrixMapperClass::Set_Texture_Transform(const Matrix4x4 &, texsize)` (matrixmapper.cpp:56).
    pub fn set_texture_transform(&mut self, view_to_texture: &Matrix4x4, texsize: f32) {
        self.view_to_texture = *view_to_texture;
        self.update_view_to_pixel_transform(texsize);
    }

    /// `Get_Texture_Transform` (matrixmapper.h).
    pub fn get_texture_transform(&self) -> &Matrix4x4 {
        &self.view_to_texture
    }

    /// `Set_Gradient_U_Coord` (matrixmapper.h).
    pub fn set_gradient_u_coord(&mut self, coord: f32) {
        self.gradient_u_coord = coord;
    }

    /// `Get_Gradient_U_Coord` (matrixmapper.h).
    pub fn get_gradient_u_coord(&self) -> f32 {
        self.gradient_u_coord
    }

    /// `MatrixMapperClass::Update_View_To_Pixel_Transform` (matrixmapper.cpp:63): builds the ViewToPixel matrix
    /// that also does the offsetting and flipping needed to get texture coordinates (K = 0.5 * (texsize-2)/texsize;
    /// rows 0 and 1 are the "KOOK" matrix times ViewToTexture, row 2 maps z from [-1,1] to [0,1], flipped under
    /// `INVERT_DEPTH_GRADIENT`, row 3 is ViewToTexture's row 3), and stores the view space negative z vector, normalised.
    /// `0.5 * (tex_size - 2.0f) / tex_size` is evaluated in double (`0.5` is a double literal) and narrowed.
    pub fn update_view_to_pixel_transform(&mut self, tex_size: f32) {
        let k: f32 = (0.5f64 * f64::from(tex_size - 2.0f32) / f64::from(tex_size)) as f32;
        let v = self.view_to_texture;
        let p = &mut self.view_to_pixel;

        p.row[0][0] = k * v.row[0][0] + k * v.row[3][0];
        p.row[0][1] = k * v.row[0][1] + k * v.row[3][1];
        p.row[0][2] = k * v.row[0][2] + k * v.row[3][2];
        p.row[0][3] = k * v.row[0][3] + k * v.row[3][3];

        p.row[1][0] = -k * v.row[1][0] + k * v.row[3][0];
        p.row[1][1] = -k * v.row[1][1] + k * v.row[3][1];
        p.row[1][2] = -k * v.row[1][2] + k * v.row[3][2];
        p.row[1][3] = -k * v.row[1][3] + k * v.row[3][3];

        if (self.flags & INVERT_DEPTH_GRADIENT) == INVERT_DEPTH_GRADIENT {
            p.row[2][0] = -0.5f32 * v.row[2][0] + 0.5f32 * v.row[3][0];
            p.row[2][1] = -0.5f32 * v.row[2][1] + 0.5f32 * v.row[3][1];
            p.row[2][2] = -0.5f32 * v.row[2][2] + 0.5f32 * v.row[3][2];
            p.row[2][3] = -0.5f32 * v.row[2][3] + 0.5f32 * v.row[3][3];
        } else {
            p.row[2][0] = 0.5f32 * v.row[2][0] + 0.5f32 * v.row[3][0];
            p.row[2][1] = 0.5f32 * v.row[2][1] + 0.5f32 * v.row[3][1];
            p.row[2][2] = 0.5f32 * v.row[2][2] + 0.5f32 * v.row[3][2];
            p.row[2][3] = 0.5f32 * v.row[2][3] + 0.5f32 * v.row[3][3];
        }

        p.row[3] = v.row[3];

        // Store the view space negative z vector for lighting effects
        self.view_space_projection_normal.x = -v.row[2][0];
        self.view_space_projection_normal.y = -v.row[2][1];
        self.view_space_projection_normal.z = -v.row[2][2];
        self.view_space_projection_normal.normalize();
    }

    /// `MatrixMapperClass::Compute_Texture_Coordinate` (matrixmapper.cpp:121): (s, t, q) of a point; q comes from
    /// row 3.
    pub fn compute_texture_coordinate(&self, point: &Vector3) -> Vector3 {
        let m = &self.view_to_pixel;
        Vector3::new(
            m.row[0][0] * point.x + m.row[0][1] * point.y + m.row[0][2] * point.z + m.row[0][3],
            m.row[1][0] * point.x + m.row[1][1] * point.y + m.row[1][2] * point.z + m.row[1][3],
            m.row[3][0] * point.x + m.row[3][1] * point.y + m.row[3][2] * point.z + m.row[3][3],
        )
    }

    /// `MatrixMapperClass::Apply` (matrixmapper.cpp:129): what is given to the device for this mapping type.
    /// upstream: DX8Wrapper::Set_Transform, Set_DX8_Texture_Stage_State — device-only, not translated; the matrix
    /// and state values it would set are returned.
    pub fn apply_state(&self, _uv_array_index: i32) -> MapperDeviceState {
        let vp = &self.view_to_pixel;
        match self.type_ {
            MappingType::OrthoProjection => MapperDeviceState {
                texture_transform: *vp,
                tex_coord_index: "D3DTSS_TCI_CAMERASPACEPOSITION",
                texture_transform_flags: "D3DTTFF_COUNT2",
            },
            MappingType::PerspectiveProjection => {
                let mut m = Matrix4x4::identity();
                m.row[0] = vp.row[0];
                m.row[1] = vp.row[1];
                m.row[2] = vp.row[3];
                MapperDeviceState {
                    texture_transform: m,
                    tex_coord_index: "D3DTSS_TCI_CAMERASPACEPOSITION",
                    texture_transform_flags: "D3DTTFF_PROJECTED|D3DTTFF_COUNT3",
                }
            }
            MappingType::DepthGradient => {
                // Second stage coordinates apply a depth gradient; depth varies 0..1 (Update_View_To_Pixel_Transform)
                let mut m = Matrix4x4::identity();
                m.row[0].set(0.0, 0.0, 0.0, self.gradient_u_coord);
                m.row[1] = vp.row[2];
                MapperDeviceState {
                    texture_transform: m,
                    tex_coord_index: "D3DTSS_TCI_CAMERASPACEPOSITION",
                    texture_transform_flags: "D3DTTFF_COUNT2",
                }
            }
            MappingType::NormalGradient => {
                // Gradient from the dot product of the vertex normal and the projection direction
                let mut m = Matrix4x4::identity();
                m.row[0].set(0.0, 0.0, 0.0, self.gradient_u_coord);
                let n = self.view_space_projection_normal;
                m.row[1].set(n.x, n.y, n.z, 0.0);
                MapperDeviceState {
                    texture_transform: m,
                    tex_coord_index: "D3DTSS_TCI_CAMERASPACENORMAL",
                    texture_transform_flags: "D3DTTFF_COUNT2",
                }
            }
        }
    }

    /// `MatrixMapperClass::Calculate_Texture_Matrix` (matrixmapper.cpp:211): `tex_matrix = ViewToPixel`.
    pub fn calculate_texture_matrix(&self) -> Matrix4x4 {
        self.view_to_pixel
    }
}

/// The inner mapper of a `CompositeMatrixMapperClass` needs only its texture matrix
/// (`TextureMapperClass::Calculate_Texture_Matrix`); a compile-time contract. A NULL inner mapper is `None`.
pub trait TextureMatrixSource {
    /// `InternalMapper->Calculate_Texture_Matrix(int_mat)`.
    fn calculate_texture_matrix(&self) -> Matrix4x4;
}

/// `class CompositeMatrixMapperClass : public MatrixMapperClass` (matrixmapper.h): composes another mapper's
/// texture matrix with its own.
#[derive(Clone, Debug)]
pub struct CompositeMatrixMapperClass<M: TextureMatrixSource> {
    pub base: MatrixMapperClass,
    pub internal_mapper: Option<M>,
}

impl<M: TextureMatrixSource> CompositeMatrixMapperClass<M> {
    /// `CompositeMatrixMapperClass::CompositeMatrixMapperClass(internal_mapper, stage)` (matrixmapper.cpp:221).
    pub fn new(internal_mapper: Option<M>, stage: u32) -> Self {
        CompositeMatrixMapperClass { base: MatrixMapperClass::new(stage as i32), internal_mapper }
    }

    /// The matrix `tmp` of `Apply`/`Calculate_Texture_Matrix`: rows 0, 1 and 3 of ViewToPixel, with row 3 twice
    /// (row 2 = row 3).
    fn composite_tmp(&self) -> Matrix4x4 {
        let mut tmp = Matrix4x4::identity();
        tmp.row[0] = self.base.view_to_pixel.row[0];
        tmp.row[1] = self.base.view_to_pixel.row[1];
        tmp.row[2] = self.base.view_to_pixel.row[3];
        tmp.row[3] = self.base.view_to_pixel.row[3];
        tmp
    }

    /// `CompositeMatrixMapperClass::Apply` (matrixmapper.cpp:249): with an inner mapper, ViewToPixel is replaced
    /// for the duration of the base `Apply` by `int_mat * tmp`, then restored.
    pub fn apply_state(&mut self, uv_array_index: i32) -> MapperDeviceState {
        if let Some(inner) = &self.internal_mapper {
            let int_mat = inner.calculate_texture_matrix();
            let view_to_pixel_copy = self.base.view_to_pixel;
            let tmp = self.composite_tmp();
            self.base.view_to_pixel = matrix4x4_multiply(&int_mat, &tmp);
            let state = self.base.apply_state(uv_array_index);
            self.base.view_to_pixel = view_to_pixel_copy;
            state
        } else {
            self.base.apply_state(uv_array_index)
        }
    }

    /// `CompositeMatrixMapperClass::Calculate_Texture_Matrix` (matrixmapper.cpp:272).
    pub fn calculate_texture_matrix(&self) -> Matrix4x4 {
        if let Some(inner) = &self.internal_mapper {
            let int_mat = inner.calculate_texture_matrix();
            let tmp = self.composite_tmp();
            matrix4x4_multiply(&int_mat, &tmp)
        } else {
            self.base.calculate_texture_matrix()
        }
    }
}
