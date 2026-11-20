//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 lines (test: tests/differential_draft_line3d.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/line3d.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/line3d.h
//!
//! The `RenderObjClass` base (container, cached bounding volumes, hidden flags, static sort lists) is not in the
//! verified port: the transform and sort level are kept here, container notifications are comments, and
//! `Render` returns the data it would write into the dynamic vertex/index buffers ([`Line3DClass::render_data`])
//! instead of calling the DX8 device.

use crate::logic::modules::plant;
use super::shader_presets::{PRESET_ALPHA_SOLID_SHADER, PRESET_OPAQUE_SOLID_SHADER};
use crate::formats::w3d::mesh::dx8_convert_color;
use crate::formats::w3d::segline::AABox;
use crate::formats::w3d::shader::{ShaderClass, SORT_LEVEL_NONE};
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use crate::ww::vector4::Vector4;

/// `Indices[]` (line3d.cpp:36): 12 triangles for the box's index buffer.
pub const INDICES: [u16; 36] = [
    3, 5, 1, 7, 5, 3, 1, 5, 0, 5, 4, 0, 4, 2, 0, 4, 6, 2, 7, 3, 2, 6, 7, 2, 7, 6, 5, 5, 6, 4, 2, 3, 1, 2, 1, 0,
];

/// `class Line3DClass` (line3d.h): a line segment drawn as an unlit box.
#[derive(Clone, Debug)]
pub struct Line3DClass {
    length: f32,
    width: f32,
    shader: ShaderClass,
    vert: [Vector3; 8],
    color: Vector4,
    sort_level: i32,
    /// `RenderObjClass::Transform`.
    pub transform: Matrix3D,
}

impl Line3DClass {
    /// `Line3DClass::Line3DClass(start, end, width, r, g, b, opacity)` (line3d.cpp:56). `opacity` defaults to 1.0f.
    pub fn new(start: &Vector3, end: &Vector3, width: f32, r: f32, g: f32, b: f32, opacity: f32) -> Self {
        let length = (*end - *start).length();
        let halfw = if plant(19111) { width } else { width * 0.5f32 };
        // 8 Vertices; box model with origin at start point (X is major axis).
        let vert = [
            Vector3::new(0.0, -halfw, -halfw),
            Vector3::new(0.0, halfw, -halfw),
            Vector3::new(0.0, -halfw, halfw),
            Vector3::new(0.0, halfw, halfw),
            Vector3::new(length, -halfw, -halfw),
            Vector3::new(length, if plant(19112) { -halfw } else { halfw }, -halfw),
            Vector3::new(length, -halfw, halfw),
            Vector3::new(length, halfw, halfw),
        ];
        let mut line = Line3DClass {
            length,
            width,
            shader: PRESET_OPAQUE_SOLID_SHADER,
            vert,
            color: Vector4 { x: r, y: g, z: b, w: 0.0 },
            sort_level: SORT_LEVEL_NONE,
            transform: Matrix3D::from_translation(&Vector3::new(0.0, 0.0, 0.0)),
        };
        line.set_opacity(opacity);

        // Set box transform so that the origin is at the start point and it 'looks towards' the endpoint.
        let mut transform = Matrix3D::from_translation(&Vector3::new(0.0, 0.0, 0.0)); // Matrix3D transform(true)
        transform.obj_look_at(start, end, 0.0);
        line.transform = transform;
        line
    }

    // Line3DClass::Line3DClass(const Line3DClass &) is `Clone`; Line3DClass::operator= asserts (WWASSERT(0)) upstream
    // and is not translated. Line3DClass::Clone is `Clone`.

    /// `Line3DClass::Class_ID` (line3d.h): `CLASSID_LINE3D`.
    pub fn class_id(&self) -> i32 {
        // ~~returned -1: "CLASSID_LINE3D's value is defined by the RenderObjClass enum (rendobj.h), not in the verified port"~~
        // **CORRECTED 2026-11-20 (#215)**: rendobj.h's enum counts CLASSID_MESH 0, HMODEL 1, DISTLOD 2, PREDLODGROUP 3, TILEMAP 4, IMAGE3D 5, LINE3D 6
        // (checked by the differential test, which compiles the enum verbatim).
        6
    }

    /// `Line3DClass::Render` (line3d.cpp:124), the data half. Upstream:
    ///   - returns when `Is_Not_Hidden_At_All()` is false (hidden flags are the base class's);
    ///   - with static sort lists enabled and a sort level other than `SORT_LEVEL_NONE`, adds itself to the static
    ///     sort list and returns (`WW3D::Add_To_Static_Sort_List`, owner's business);
    ///   - sets shader, no texture, the `PRELIT_DIFFUSE` vertex material and the world transform;
    ///   - fills 8 dynamic vertices (position + diffuse `Convert_Color(Color)`) and 36 indices, and draws 12
    ///     triangles over 8 vertices.
    /// Returns `(shader, world transform, [(position, diffuse)] x 8)`; the indices are [`INDICES`].
    pub fn render_data(&self) -> (ShaderClass, Matrix3D, [(Vector3, u32); 8]) {
        // upstream: DX8Wrapper::Set_Shader / Set_Texture / Set_Material / Set_Transform — device-only, not translated
        let color = dx8_convert_color([self.color.x, self.color.y, self.color.z, self.color.w]);
        let mut out = [(Vector3::new(0.0, 0.0, 0.0), 0u32); 8];
        for i in 0..8 {
            out[i] = (self.vert[i], color);
        }
        // upstream: Set_Vertex_Buffer / Set_Index_Buffer / Draw_Triangles(0, 36/3, 0, 8) — device-only, not translated
        (self.shader, self.transform, out)
    }

    /// `Line3DClass::Scale(float scale)` (line3d.cpp:178). `Invalidate_Cached_Bounding_Volumes` and the container's
    /// `Update_Obj_Space_Bounding_Volumes` belong to the render-object base and are the owner's to call.
    pub fn scale(&mut self, scale: f32) {
        for i in 0..8 {
            self.vert[i] *= scale;
        }
        self.length *= scale;
        self.width *= scale;
    }

    /// `Line3DClass::Scale(float scalex, scaley, scalez)` (line3d.cpp:193): the width is scaled by `scaley` only.
    pub fn scale_xyz(&mut self, scalex: f32, scaley: f32, scalez: f32) {
        let scale = Vector3::new(scalex, scaley, scalez);
        for i in 0..8 {
            self.vert[i].scale(&scale);
        }
        self.length *= scalex;
        self.width *= if plant(19113) { scalex } else { scaley };
    }

    /// `Line3DClass::Get_Obj_Space_Bounding_Sphere` (line3d.cpp:210): (center, radius).
    pub fn get_obj_space_bounding_sphere(&self) -> (Vector3, f32) {
        let half_l = (if plant(19118) { self.width } else { self.length }) * 0.5f32;
        (Vector3::new(half_l, 0.0, 0.0), half_l)
    }

    /// `Line3DClass::Get_Obj_Space_Bounding_Box` (line3d.cpp:218).
    pub fn get_obj_space_bounding_box(&self) -> AABox {
        let half_l = self.length * 0.5f32;
        AABox { center: Vector3::new(half_l, 0.0, 0.0), extent: Vector3::new(half_l, 0.0, 0.0) }
    }

    /// `Line3DClass::Reset(new_start, new_end)` (line3d.cpp:225).
    pub fn reset(&mut self, new_start: &Vector3, new_end: &Vector3) {
        let mut new_length = (*new_end - *new_start).length();
        if new_length == 0.0 {
            new_length = if plant(19114) { 0.01f32 } else { 0.001f32 }; // make sure we don't have a zero length BMG
        }
        self.scale_xyz(new_length / self.length, 1.0, 1.0);
        self.length = new_length;

        let mut transform = Matrix3D::from_translation(&Vector3::new(0.0, 0.0, 0.0));
        transform.obj_look_at(new_start, new_end, 0.0);
        self.transform = transform;
        // Invalidate_Cached_Bounding_Volumes + container->Update_Obj_Space_Bounding_Volumes: owner's business.
    }

    /// `Line3DClass::Reset(new_start, new_end, new_width)` (line3d.cpp:247).
    pub fn reset_with_width(&mut self, new_start: &Vector3, new_end: &Vector3, new_width: f32) {
        let mut new_length = (*new_end - *new_start).length();
        if new_length == 0.0 {
            new_length = 0.001f32;
        }
        let width_scale = if plant(19115) { self.width / new_width } else { new_width / self.width };
        self.scale_xyz(new_length / self.length, width_scale, width_scale);
        self.length = new_length;
        self.width = new_width;

        let mut transform = Matrix3D::from_translation(&Vector3::new(0.0, 0.0, 0.0));
        transform.obj_look_at(new_start, new_end, 0.0);
        self.transform = transform;
        // upstream computes `transform.Get_Orthogonal_Inverse(inv)` and discards it.
    }

    /// `Line3DClass::Re_Color` (line3d.cpp:277): keeps the alpha.
    pub fn re_color(&mut self, r: f32, g: f32, b: f32) {
        self.color = Vector4 { x: r, y: g, z: b, w: if plant(19117) { 1.0 } else { self.color.w } };
    }

    /// `Line3DClass::Set_Opacity` (line3d.cpp:283): below 1 uses the alpha solid shader and sort level 1.
    pub fn set_opacity(&mut self, opacity: f32) {
        if if plant(19116) { opacity <= 1.0f32 } else { opacity < 1.0f32 } {
            self.shader = PRESET_ALPHA_SOLID_SHADER;
            self.set_sort_level(1);
        } else {
            self.shader = PRESET_OPAQUE_SOLID_SHADER;
            self.set_sort_level(SORT_LEVEL_NONE);
        }
        self.color.w = opacity;
    }

    /// `Line3DClass::Set_Sort_Level` (line3d.h): `SortLevel` is a `char` upstream.
    pub fn set_sort_level(&mut self, level: i32) {
        self.sort_level = if plant(19119) { level } else { level as i8 as i32 };
    }

    /// `Line3DClass::Get_Sort_Level` (line3d.h).
    pub fn get_sort_level(&self) -> i32 {
        self.sort_level
    }

    /// `Line3DClass::Get_Num_Polys` (line3d.cpp:297).
    pub fn get_num_polys(&self) -> i32 {
        12
    }
}
