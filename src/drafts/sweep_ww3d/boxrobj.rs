//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/boxrobj.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/boxrobj.h
//!
//! The drawing and cached-box members of the box render objects that no earlier pass translated:
//! `BoxRenderObjClass::Set_Box_Display_Mask`, `::Get_Box_Display_Mask`, `::render_box`, `::vis_render_box`,
//! `AABoxRenderObjClass::update_cached_box`, `OBBoxRenderObjClass::update_cached_box` and `::Get_Box`.
//! The verified `formats::w3d::boxrobj` only loads the `W3dBoxStruct` prototype.
//!
//! `DisplayMask` and `IsInitted` are class statics upstream; here they are the fields of [`BoxRenderState`], which the
//! caller keeps one of. `render_box` writes the vertex and index data the DX8 dynamic buffers receive into a
//! [`BoxDrawData`] (positions, normals, diffuse colour, the 36 indices) and leaves the buffer locking, the
//! `DX8Wrapper::Set_Material` / `Set_Shader` / `Set_Texture` / `Set_Index_Buffer` / `Set_Vertex_Buffer` / `Draw_Triangles`
//! calls to the host (device-only, not translated).

use crate::drafts::wwlib::aabox::AABoxClass;
use crate::drafts::wwlib::obbox::OBBoxClass;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath::WWMATH_OOSQRT3;

/// `NUM_BOX_VERTS`.
pub const NUM_BOX_VERTS: usize = 8;
/// `NUM_BOX_FACES`.
pub const NUM_BOX_FACES: usize = 12;

/// `_BoxFaces` (boxrobj.cpp:113): "Face Connectivity".
pub const BOX_FACES: [[u16; 3]; NUM_BOX_FACES] = [
    [0, 1, 2], // +z faces
    [0, 2, 3],
    [4, 7, 6], // -z faces
    [4, 6, 5],
    [0, 3, 7], // +x faces
    [0, 7, 4],
    [1, 5, 6], // -x faces
    [1, 6, 2],
    [4, 5, 1], // +y faces
    [4, 1, 0],
    [3, 2, 6], // -y faces
    [3, 6, 7],
];

/// `_BoxVerts` (boxrobj.cpp:130): "Vertex Positions as a function of the box extents".
pub const BOX_VERTS: [[f32; 3]; NUM_BOX_VERTS] = [
    [1.0, 1.0, 1.0], // +z ring of 4 verts
    [-1.0, 1.0, 1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, -1.0], // -z ring of 4 verts;
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
];

/// `_BoxVertexNormals` (boxrobj.cpp:144).
pub const BOX_VERTEX_NORMALS: [[f32; 3]; NUM_BOX_VERTS] = [
    [WWMATH_OOSQRT3, WWMATH_OOSQRT3, WWMATH_OOSQRT3],
    [-WWMATH_OOSQRT3, WWMATH_OOSQRT3, WWMATH_OOSQRT3],
    [-WWMATH_OOSQRT3, -WWMATH_OOSQRT3, WWMATH_OOSQRT3],
    [WWMATH_OOSQRT3, -WWMATH_OOSQRT3, WWMATH_OOSQRT3],
    [WWMATH_OOSQRT3, WWMATH_OOSQRT3, -WWMATH_OOSQRT3],
    [-WWMATH_OOSQRT3, WWMATH_OOSQRT3, -WWMATH_OOSQRT3],
    [-WWMATH_OOSQRT3, -WWMATH_OOSQRT3, -WWMATH_OOSQRT3],
    [WWMATH_OOSQRT3, -WWMATH_OOSQRT3, -WWMATH_OOSQRT3],
];

/// `BoxRenderObjClass::IsInitted` and `::DisplayMask` (class statics).
#[derive(Clone, Copy, Debug, Default)]
pub struct BoxRenderState {
    pub is_initted: bool,
    pub display_mask: i32,
}

impl BoxRenderState {
    /// `BoxRenderObjClass::Set_Box_Display_Mask(mask)` (boxrobj.cpp:408).
    pub fn set_box_display_mask(&mut self, mask: i32) {
        self.display_mask = mask;
    }

    /// `BoxRenderObjClass::Get_Box_Display_Mask()` (boxrobj.cpp:426).
    pub fn get_box_display_mask(&self) -> i32 {
        self.display_mask
    }
}

/// One vertex of the `VertexFormatXYZNDUV2` buffer `render_box` fills (position, normal, diffuse).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoxVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub diffuse: u32,
}

/// What `render_box` puts in the dynamic vertex and index buffers.
#[derive(Clone, Debug)]
pub struct BoxDrawData {
    pub vertices: [BoxVertex; NUM_BOX_VERTS],
    /// `NUM_BOX_FACES * 3` indices.
    pub indices: [u16; NUM_BOX_FACES * 3],
}

/// The calls out of `render_box` / `vis_render_box`.
pub trait BoxRenderHost {
    /// `Get_Collision_Type()`.
    fn get_collision_type(&self) -> i32;
    /// `DX8Wrapper::Convert_Color(Color, Opacity)`.
    fn convert_color(&self, color: &Vector3, opacity: f32) -> u32;
    /// Submit the buffers: `Set_Material(_BoxMaterial)`, `Set_Shader(_BoxShader)`, `Set_Texture(0,NULL)`,
    /// `Set_Index_Buffer`, `Set_Vertex_Buffer`, then `Draw_Triangles(buffer_type, 0, NUM_BOX_FACES, 0, NUM_BOX_VERTS)`.
    fn draw_triangles(&mut self, data: &BoxDrawData);
    /// `rinfo.VisRasterizer->Render_Triangles(verts, NUM_BOX_VERTS, _BoxFaces, NUM_BOX_FACES, Get_Bounding_Box())`.
    fn vis_render_triangles(&mut self, verts: &[Vector3; NUM_BOX_VERTS], faces: &[[u16; 3]; NUM_BOX_FACES]);
}

/// The corner positions as a function of the box: `center + _BoxVerts[i] * extent`, per axis.
fn compute_box_verts(center: &Vector3, extent: &Vector3) -> [Vector3; NUM_BOX_VERTS] {
    let mut verts = [Vector3::new(0.0, 0.0, 0.0); NUM_BOX_VERTS];
    for ivert in 0..NUM_BOX_VERTS {
        verts[ivert].x = center.x + BOX_VERTS[ivert][0] * extent.x;
        verts[ivert].y = center.y + BOX_VERTS[ivert][1] * extent.y;
        verts[ivert].z = center.z + BOX_VERTS[ivert][2] * extent.z;
    }
    verts
}

/// `BoxRenderObjClass::render_box(rinfo, center, extent)` (boxrobj.cpp:444): nothing unless initialised and the box's
/// collision type is in the display mask. Writes the 8 vertices (location, normal, diffuse) and 36 indices into the
/// dynamic buffers and draws the 12 triangles.
pub fn render_box<H: BoxRenderHost>(
    state: &BoxRenderState,
    host: &mut H,
    color: &Vector3,
    opacity: f32,
    center: &Vector3,
    extent: &Vector3,
) {
    if !state.is_initted {
        return;
    }
    if (state.display_mask & host.get_collision_type()) != 0 {
        // compute the vertex positions
        let verts = compute_box_verts(center, extent);

        // Dump the box vertices into the sorting dynamic vertex buffer.
        let color = host.convert_color(color, opacity);

        let mut vertices = [BoxVertex::default(); NUM_BOX_VERTS];
        for i in 0..NUM_BOX_VERTS {
            vertices[i] = BoxVertex {
                // Locations
                x: verts[i].x,
                y: verts[i].y,
                z: verts[i].z,
                // Normals
                nx: BOX_VERTEX_NORMALS[i][0],
                ny: BOX_VERTEX_NORMALS[i][1],
                nz: BOX_VERTEX_NORMALS[i][2],
                // Colors
                diffuse: color,
            };
        }

        // Dump the faces into the sorting dynamic index buffer.
        let mut indices = [0u16; NUM_BOX_FACES * 3];
        for i in 0..NUM_BOX_FACES {
            indices[3 * i] = BOX_FACES[i][0];
            indices[3 * i + 1] = BOX_FACES[i][1];
            indices[3 * i + 2] = BOX_FACES[i][2];
        }

        // upstream: DynamicVBAccessClass / DynamicIBAccessClass locks, DX8Wrapper::Set_Material / Set_Shader /
        // Set_Texture / Set_Index_Buffer / Set_Vertex_Buffer / Draw_Triangles — device-only, not translated
        // (`Get_Obj_Space_Bounding_Sphere(sphere)` is computed and unused upstream)
        host.draw_triangles(&BoxDrawData { vertices, indices });
    }
}

/// `BoxRenderObjClass::vis_render_box(rinfo, center, extent)` (boxrobj.cpp:536): the same corners, rendered into the
/// visibility rasteriser (no display-mask test, only `IsInitted`).
pub fn vis_render_box<H: BoxRenderHost>(state: &BoxRenderState, host: &mut H, center: &Vector3, extent: &Vector3) {
    if !state.is_initted {
        return;
    }

    // compute the vertex positions
    let verts = compute_box_verts(center, extent);

    // render!
    host.vis_render_triangles(&verts, &BOX_FACES);
}

/// `AABoxRenderObjClass::update_cached_box()` (boxrobj.cpp:786): `CachedBox.Center = Transform.Get_Translation() +
/// ObjSpaceCenter; CachedBox.Extent = ObjSpaceExtent;`.
pub fn aabox_update_cached_box(cached_box: &mut AABoxClass, transform: &Matrix3D, obj_space_center: &Vector3, obj_space_extent: &Vector3) {
    cached_box.center = transform.get_translation() + *obj_space_center;
    cached_box.extent = *obj_space_extent;
}

/// `OBBoxRenderObjClass::update_cached_box()` (boxrobj.cpp:1168): the centre is `ObjSpaceCenter` transformed by
/// `Transform`, the extent is `ObjSpaceExtent`, and the basis is the rotation of `Transform` (`Basis.Set(Transform)`).
pub fn obbox_update_cached_box(cached_box: &mut OBBoxClass, transform: &Matrix3D, obj_space_center: &Vector3, obj_space_extent: &Vector3) {
    cached_box.center = Matrix3D::transform_vector(transform, obj_space_center);
    cached_box.extent = *obj_space_extent;
    cached_box.basis.set_matrix3d(transform);
}

/// `OBBoxRenderObjClass::Get_Box()` (boxrobj.cpp:1338): `Validate_Transform(); update_cached_box(); return CachedBox;`.
/// `validate_transform` is the host's `RenderObjClass::Validate_Transform` (it refreshes `Transform` from the container).
pub fn obbox_get_box<'a, F: FnOnce() -> Matrix3D>(
    cached_box: &'a mut OBBoxClass,
    validate_transform: F,
    obj_space_center: &Vector3,
    obj_space_extent: &Vector3,
) -> &'a mut OBBoxClass {
    let transform = validate_transform();
    obbox_update_cached_box(cached_box, &transform, obj_space_center, obj_space_extent);
    cached_box
}
