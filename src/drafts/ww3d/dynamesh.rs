//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/dynamesh.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/dynamesh.h
//!
//! Design notes:
//! - `DynamicMeshModel` derives from `MeshGeometryClass` and owns a `MeshMatDescClass` (meshgeometry.cpp,
//!   meshmatdesc.cpp: not in this area and not in the verified port). The draft stores the arrays those classes
//!   would hold as plain `Vec`s on [`DynamicMeshModel`] and implements [`MeshMatDesc`] (matinfo.rs) over them.
//!   `MeshGeometryClass::Compute_Plane_Equations/Compute_Vertex_Normals/Compute_Bounds` are called by the Dynamic
//!   model with the poly/vert counts temporarily swapped to the dynamic counts; that swap is kept and the base call
//!   is a comment (`// base: ...`), to be wired to the real `MeshGeometryClass` once it is translated.
//! - `DynamicScreenMeshClass` overrides four virtuals (`Location`, `Move_Vertex`, `Reset`, `Flip_Face`). Rather than
//!   a trait object it is the `screen_aspect: Some(aspect)` variant of [`DynamicMeshClass`], and each of the four
//!   dispatches on it.
//! - `Render` builds a list of [`DynamicDraw`] records, the draw calls upstream issues, keeping its run-splitting
//!   loop; the DX8 vertex/index buffer fill and `Set_*` calls are device-only.
//! - `DX8Wrapper::Convert_Color_Clamp(Vector4)` is `Convert_Color` with each channel clamped to 0..1 first
//!   (dx8wrapper.cpp); [`convert_color_clamp`] reproduces that over the port's `dx8_convert_color`.

use super::handles::{TextureHandle, VertexMaterialHandle};
use super::matinfo::{MaterialInfoClass, MeshMatDesc};
use crate::formats::w3d::mesh::dx8_convert_color;
use crate::formats::w3d::shader::{ShaderClass, SORT_LEVEL_NONE};
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;
use crate::ww::vector4::Vector4;
use crate::ww::wwmath;

/// `MeshMatDescClass::MAX_PASSES`.
pub const MAX_PASSES: usize = 4;
/// `MeshMatDescClass::MAX_TEX_STAGES`.
pub const MAX_TEX_STAGES: usize = 2;
/// `MeshMatDescClass::MAX_COLOR_ARRAYS`.
pub const MAX_COLOR_ARRAYS: usize = 2;
/// `MeshMatDescClass::MAX_UV_ARRAYS` = `MAX_PASSES * MAX_TEX_STAGES`.
pub const MAX_UV_ARRAYS: usize = MAX_PASSES * MAX_TEX_STAGES;

/// `MeshGeometryClass::FlagsType` (meshgeometry.h).
pub const DIRTY_BOUNDS: u32 = 0x0000_0001;
pub const DIRTY_PLANES: u32 = 0x0000_0002;
pub const DIRTY_VNORMALS: u32 = 0x0000_0004;
pub const SORT: u32 = 0x0000_0010;
pub const DISABLE_BOUNDING_BOX: u32 = 0x0000_0020;
pub const DISABLE_BOUNDING_SPHERE: u32 = 0x0000_0040;

/// `DynamicMeshClass::TRI_MODE_STRIPS` / `TRI_MODE_FANS`.
pub const TRI_MODE_STRIPS: i32 = 0;
pub const TRI_MODE_FANS: i32 = 1;

/// `DX8Wrapper::Convert_Color_Clamp(const Vector4 &)` (dx8wrapper.cpp): clamp each channel to `[0,1]`, then pack as
/// `Convert_Color` does.
pub fn convert_color_clamp(c: &Vector4) -> u32 {
    dx8_convert_color([
        wwmath::clamp(c.x, 0.0, 1.0),
        wwmath::clamp(c.y, 0.0, 1.0),
        wwmath::clamp(c.z, 0.0, 1.0),
        wwmath::clamp(c.w, 0.0, 1.0),
    ])
}

/// One draw call `DynamicMeshModel::Render` would issue.
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicDraw {
    /// `BUFFER_TYPE_DYNAMIC_SORTING` (goes to `SortingRendererClass::Insert_Triangles`) or `DX8Wrapper::Draw_Triangles`.
    pub sorting: bool,
    pub pass: usize,
    /// `start_tri_idx * 3` (index buffer start).
    pub start_index: u32,
    pub polygon_count: u32,
    pub min_vertex_index: u32,
    pub vertex_count: u32,
    /// The state set for this run: texture stage 0 / 1, material, shader.
    pub texture0: Option<TextureHandle>,
    pub texture1: Option<TextureHandle>,
    pub material: Option<VertexMaterialHandle>,
    pub shader: ShaderClass,
}

/// `class DynamicMeshModel` (dynamesh.h): geometry and the `MeshMatDescClass` arrays, as plain vectors.
#[derive(Clone, Debug)]
pub struct DynamicMeshModel {
    /// `MeshGeometryClass::PolyCount` / `VertexCount`: the MAXIMUM counts.
    poly_count: usize,
    vertex_count: usize,
    flags: u32,
    dynamic_mesh_pnum: i32,
    dynamic_mesh_vnum: i32,
    verts: Vec<Vector3>,
    vert_normals: Vec<Vector3>,
    polys: Vec<[u32; 3]>,
    /// `MatDesc->Get_Color_Array(i)`: per-vertex packed colors, allocated on first use.
    colors: [Option<Vec<u32>>; MAX_COLOR_ARRAYS],
    uvs: [Option<Vec<Vector2>>; MAX_UV_ARRAYS],
    pass_count: usize,
    single_texture: [[Option<TextureHandle>; MAX_TEX_STAGES]; MAX_PASSES],
    single_shader: [ShaderClass; MAX_PASSES],
    single_material: [Option<VertexMaterialHandle>; MAX_PASSES],
    texture_array: [[Option<Vec<Option<TextureHandle>>>; MAX_TEX_STAGES]; MAX_PASSES],
    material_array: [Option<Vec<Option<VertexMaterialHandle>>>; MAX_PASSES],
    shader_array: [Option<Vec<ShaderClass>>; MAX_PASSES],
    mat_info: MaterialInfoClass,
}

impl DynamicMeshModel {
    /// `DynamicMeshModel::DynamicMeshModel(max_polys, max_verts)` (dynamesh.cpp:33) / the `mat_info` overload
    /// (`mat_info = None` builds `NEW_REF(MaterialInfoClass, ())`).
    pub fn new(max_polys: usize, max_verts: usize, mat_info: Option<MaterialInfoClass>) -> Self {
        let mut m = DynamicMeshModel {
            poly_count: 0,
            vertex_count: 0,
            flags: 0,
            dynamic_mesh_pnum: 0,
            dynamic_mesh_vnum: 0,
            verts: Vec::new(),
            vert_normals: Vec::new(),
            polys: Vec::new(),
            colors: [None, None],
            uvs: Default::default(),
            pass_count: 1,
            single_texture: [[None; MAX_TEX_STAGES]; MAX_PASSES],
            single_shader: [ShaderClass(0); MAX_PASSES],
            single_material: [None; MAX_PASSES],
            texture_array: Default::default(),
            material_array: Default::default(),
            shader_array: Default::default(),
            mat_info: mat_info.unwrap_or_default(),
        };
        // MatDesc->Set_Polygon_Count / Set_Vertex_Count; Reset_Geometry(max_polys, max_verts)
        m.reset_geometry(max_polys, max_verts);
        m
    }

    /// `MeshGeometryClass::Reset_Geometry(polycount, vertcount)` (meshgeometry.cpp, not in this area): reallocates
    /// the vertex and polygon arrays to the maximum counts and marks everything dirty. Minimal version so the draft
    /// is self-consistent; to be replaced by the real base class.
    fn reset_geometry(&mut self, polycount: usize, vertcount: usize) {
        self.poly_count = polycount;
        self.vertex_count = vertcount;
        self.verts = vec![Vector3::new(0.0, 0.0, 0.0); vertcount];
        self.vert_normals = vec![Vector3::new(0.0, 0.0, 0.0); vertcount];
        self.polys = vec![[0; 3]; polycount];
        self.flags |= DIRTY_BOUNDS | DIRTY_PLANES | DIRTY_VNORMALS;
    }

    /// `MeshGeometryClass::Get_Flag` / `Set_Flag`.
    pub fn get_flag(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    pub fn set_flag(&mut self, flag: u32, onoff: bool) {
        if onoff {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    pub fn get_polygon_count(&self) -> usize {
        self.poly_count
    }

    pub fn get_vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// `DynamicMeshModel::Compute_Plane_Equations` (dynamesh.cpp:103): counts swapped to the dynamic counts around
    /// the base call.
    pub fn compute_plane_equations(&mut self) {
        let (old_poly, old_vert) = (self.poly_count, self.vertex_count);
        self.poly_count = self.dynamic_mesh_pnum as usize;
        self.vertex_count = self.dynamic_mesh_vnum as usize;
        // base: MeshGeometryClass::Compute_Plane_Equations(planes) — meshgeometry.cpp, not yet translated
        self.poly_count = old_poly;
        self.vertex_count = old_vert;
    }

    /// `DynamicMeshModel::Compute_Vertex_Normals` (dynamesh.cpp:121).
    pub fn compute_vertex_normals(&mut self) {
        let (old_poly, old_vert) = (self.poly_count, self.vertex_count);
        self.poly_count = self.dynamic_mesh_pnum as usize;
        self.vertex_count = self.dynamic_mesh_vnum as usize;
        // base: MeshGeometryClass::Compute_Vertex_Normals(vnorms) — meshgeometry.cpp, not yet translated
        self.poly_count = old_poly;
        self.vertex_count = old_vert;
    }

    /// `DynamicMeshModel::Compute_Bounds` (dynamesh.cpp:138).
    pub fn compute_bounds(&mut self) {
        let (old_poly, old_vert) = (self.poly_count, self.vertex_count);
        self.poly_count = self.dynamic_mesh_pnum as usize;
        self.vertex_count = self.dynamic_mesh_vnum as usize;
        // base: MeshGeometryClass::Compute_Bounds(verts) — meshgeometry.cpp, not yet translated
        self.poly_count = old_poly;
        self.vertex_count = old_vert;
    }

    /// `DynamicMeshModel::Reset` (dynamesh.cpp:154): `MatDesc->Reset(polycount, vertcount, 1)` drops every
    /// material array and puts the pass count back to 1; the material info is replaced by a fresh one.
    pub fn reset(&mut self) {
        self.set_counts(0, 0);
        let polycount = self.get_polygon_count();
        let vertcount = self.get_vertex_count();
        self.reset_geometry(polycount, vertcount);
        self.pass_count = 1;
        self.colors = [None, None];
        self.uvs = Default::default();
        self.single_texture = [[None; MAX_TEX_STAGES]; MAX_PASSES];
        self.single_shader = [ShaderClass(0); MAX_PASSES];
        self.single_material = [None; MAX_PASSES];
        self.texture_array = Default::default();
        self.material_array = Default::default();
        self.shader_array = Default::default();
        self.mat_info = MaterialInfoClass::new();
    }

    /// `DynamicMeshModel::Set_Counts`.
    pub fn set_counts(&mut self, pnum: i32, vnum: i32) {
        self.dynamic_mesh_pnum = pnum;
        self.dynamic_mesh_vnum = vnum;
    }

    /// `DynamicMeshModel::Get_Color_Array(i)` = `MeshMatDescClass::Get_Color_Array(i, create = true)`.
    pub fn get_color_array(&mut self, color_array_index: usize) -> &mut Vec<u32> {
        let n = self.vertex_count;
        self.colors[color_array_index].get_or_insert_with(|| vec![0xFFFF_FFFF; n])
    }

    /// `DynamicMeshModel::Get_UV_Array(i)` = `MeshMatDescClass::Get_UV_Array_By_Index(i, create = true)`.
    pub fn get_uv_array(&mut self, uv_array_index: usize) -> &mut Vec<Vector2> {
        let n = self.vertex_count;
        self.uvs[uv_array_index].get_or_insert_with(|| vec![Vector2::new(0.0, 0.0); n])
    }

    pub fn set_pass_count(&mut self, passes: usize) {
        self.pass_count = passes;
    }

    pub fn get_pass_count(&self) -> usize {
        self.pass_count
    }

    /// `DynamicMeshModel::Set_Single_Shader`.
    pub fn set_single_shader(&mut self, shader: ShaderClass, pass: usize) {
        self.single_shader[pass] = shader;
    }

    /// `DynamicMeshModel::Set_Shader(pidx, shader, pass)` allocates the shader array on first use.
    pub fn set_shader(&mut self, pidx: usize, shader: ShaderClass, pass: usize) {
        let n = self.poly_count;
        let single = self.single_shader[pass];
        self.shader_array[pass].get_or_insert_with(|| vec![single; n])[pidx] = shader;
    }

    /// `DynamicMeshModel::Peek_Material_Info` / `Set_Material_Info`.
    pub fn peek_material_info(&self) -> &MaterialInfoClass {
        &self.mat_info
    }

    pub fn peek_material_info_mut(&mut self) -> &mut MaterialInfoClass {
        &mut self.mat_info
    }

    pub fn set_material_info(&mut self, mat_info: MaterialInfoClass) {
        self.mat_info = mat_info;
    }

    /// `DynamicMeshModel::Get_Non_Const_Polygon_Array` (dynamesh.h).
    pub fn get_non_const_polygon_array(&mut self) -> &mut Vec<[u32; 3]> {
        &mut self.polys
    }

    /// `DynamicMeshModel::Get_Non_Const_Vertex_Normal_Array`: recomputes when `DIRTY_VNORMALS` is set.
    pub fn get_non_const_vertex_normal_array(&mut self) -> &mut Vec<Vector3> {
        if self.get_flag(DIRTY_VNORMALS) {
            self.compute_vertex_normals();
        }
        &mut self.vert_normals
    }

    pub fn get_vertex_array(&mut self) -> &mut Vec<Vector3> {
        &mut self.verts
    }

    /// `DynamicMeshModel::Initialize_Texture_Array` (dynamesh.cpp:300). Upstream passes stage 0 to
    /// `Get_Texture_Array` whatever `stage` is, and loops to `PolyCount`; both kept.
    pub fn initialize_texture_array(&mut self, pass: usize, _stage: usize, texture: Option<TextureHandle>) {
        let n = self.poly_count;
        let arr = self.texture_array[pass][0].get_or_insert_with(|| vec![None; n]);
        for lp in 0..n {
            arr[lp] = texture;
        }
    }

    /// `DynamicMeshModel::Initialize_Material_Array` (dynamesh.cpp:308).
    pub fn initialize_material_array(&mut self, pass: usize, vmat: Option<VertexMaterialHandle>) {
        let n = self.vertex_count;
        let arr = self.material_array[pass].get_or_insert_with(|| vec![None; n]);
        for lp in 0..n {
            arr[lp] = vmat;
        }
    }

    /// `DynamicMeshModel::Render` (dynamesh.cpp:166): the draw calls, in order. `sorting_enabled` is
    /// `WW3D::Is_Sorting_Enabled()`; `sphere` is the bounding sphere handed to the sorting renderer (carried by
    /// the caller; not stored here). The vertex/index buffer fill is device-only.
    pub fn render(&self, sorting_enabled: bool) -> Vec<DynamicDraw> {
        let mut draws = Vec::new();
        let sorting = self.get_flag(SORT) && sorting_enabled;
        // upstream: DynamicVBAccessClass / DynamicIBAccessClass fills, Set_Vertex_Buffer, Set_Index_Buffer — device-only

        let pnum = self.dynamic_mesh_pnum;
        let vnum = self.dynamic_mesh_vnum;
        let tris = &self.polys;

        for pass in 0..self.pass_count {
            let mut min_vert_idx: u16 = (vnum - 1) as u16;
            let mut max_vert_idx: u16 = 0;
            let mut start_tri_idx: u16 = 0;
            let mut cur_tri_idx: u16 = 0;
            let mut done = false;

            let texture_array0 = self.texture_array[pass][0].as_ref();
            let texture_array1 = self.texture_array[pass][1].as_ref();
            let material_array = self.material_array[pass].as_ref();
            let shader_array = self.shader_array[pass].as_ref();

            // Set the DX8 state to the first triangle's state
            let mut tex0 = match texture_array0 {
                Some(a) => a[0],
                None => self.single_texture[pass][0],
            };
            let mut tex1 = match texture_array1 {
                Some(a) => a[0],
                None => self.single_texture[pass][1],
            };
            let mut mat = match material_array {
                Some(a) => a[tris[0][0] as usize],
                None => self.single_material[pass],
            };
            let mut shader = match shader_array {
                Some(a) => a[0],
                None => self.single_shader[pass],
            };

            // If no texture, shader or material arrays for this pass just draw and go to next pass
            if texture_array0.is_none() && texture_array1.is_none() && material_array.is_none() && shader_array.is_none() {
                draws.push(DynamicDraw {
                    sorting,
                    pass,
                    start_index: 0,
                    polygon_count: pnum as u32,
                    min_vertex_index: 0,
                    vertex_count: vnum as u32,
                    texture0: tex0,
                    texture1: tex1,
                    material: mat,
                    shader,
                });
                continue;
            }

            while !done {
                // Add vertex indices of tri[cur_tri_idx] to min_vert_idx, max_vert_idx
                let tri = tris[cur_tri_idx as usize];
                let min_idx = tri[0].min(tri[1]).min(tri[2]) as u16;
                let max_idx = tri[0].max(tri[1]).max(tri[2]) as u16;
                min_vert_idx = min_vert_idx.min(min_idx);
                max_vert_idx = max_vert_idx.max(max_idx);

                // Check the next triangle to see if the current run has ended.
                let next_tri_idx: u16 = cur_tri_idx + 1;
                done = next_tri_idx as i32 >= pnum;
                let (mut texture_changed, mut texture1_changed, mut material_changed, mut shader_changed) =
                    (false, false, false, false);
                if !done {
                    let (c, n) = (cur_tri_idx as usize, next_tri_idx as usize);
                    texture_changed = texture_array0.map_or(false, |a| a[c] != a[n]);
                    texture1_changed = texture_array1.map_or(false, |a| a[c] != a[n]);
                    material_changed =
                        material_array.map_or(false, |a| a[tris[c][0] as usize] != a[tris[n][0] as usize]);
                    shader_changed = shader_array.map_or(false, |a| a[c] != a[n]);
                }

                // If run ends (mesh ends or state changes) draw, reset indices, set state for next run.
                // (texture1_changed alone does not end a run upstream: it is not in this condition.)
                if done || texture_changed || material_changed || shader_changed {
                    draws.push(DynamicDraw {
                        sorting,
                        pass,
                        start_index: start_tri_idx as u32 * 3,
                        polygon_count: 1 + cur_tri_idx as u32 - start_tri_idx as u32,
                        min_vertex_index: min_vert_idx as u32,
                        vertex_count: 1 + max_vert_idx as u32 - min_vert_idx as u32,
                        texture0: tex0,
                        texture1: tex1,
                        material: mat,
                        shader,
                    });
                    start_tri_idx = next_tri_idx;
                    min_vert_idx = (vnum - 1) as u16;
                    max_vert_idx = 0;
                    if !done {
                        let n = next_tri_idx as usize;
                        if texture_changed {
                            tex0 = texture_array0.unwrap()[n];
                        }
                        if texture1_changed {
                            tex1 = texture_array1.unwrap()[n];
                        }
                        if material_changed {
                            mat = material_array.unwrap()[tris[n][0] as usize];
                        }
                        if shader_changed {
                            shader = shader_array.unwrap()[n];
                        }
                    }
                }
                cur_tri_idx = next_tri_idx;
            }
        }
        draws
    }
}

impl MeshMatDesc for DynamicMeshModel {
    fn get_pass_count(&self) -> usize {
        self.pass_count
    }
    fn get_vertex_count(&self) -> usize {
        self.vertex_count
    }
    fn get_polygon_count(&self) -> usize {
        self.poly_count
    }
    fn has_material_array(&self, pass: usize) -> bool {
        self.material_array[pass].is_some()
    }
    fn peek_material(&self, vert_index: usize, pass: usize) -> Option<VertexMaterialHandle> {
        match &self.material_array[pass] {
            Some(a) => a[vert_index],
            None => self.single_material[pass],
        }
    }
    fn peek_single_material(&self, pass: usize) -> Option<VertexMaterialHandle> {
        self.single_material[pass]
    }
    fn has_shader_array(&self, pass: usize) -> bool {
        self.shader_array[pass].is_some()
    }
    fn get_shader(&self, poly_index: usize, pass: usize) -> ShaderClass {
        match &self.shader_array[pass] {
            Some(a) => a[poly_index],
            None => self.single_shader[pass],
        }
    }
    fn get_single_shader(&self, pass: usize) -> ShaderClass {
        self.single_shader[pass]
    }
    fn has_texture_array(&self, pass: usize, stage: usize) -> bool {
        self.texture_array[pass][stage].is_some()
    }
    fn peek_texture(&self, poly_index: usize, pass: usize, stage: usize) -> Option<TextureHandle> {
        match &self.texture_array[pass][stage] {
            Some(a) => a[poly_index],
            None => self.single_texture[pass][stage],
        }
    }
    fn peek_single_texture(&self, pass: usize, stage: usize) -> Option<TextureHandle> {
        self.single_texture[pass][stage]
    }
    fn set_material(&mut self, vert_index: usize, vmat: Option<VertexMaterialHandle>, pass: usize) {
        let n = self.vertex_count;
        let single = self.single_material[pass];
        self.material_array[pass].get_or_insert_with(|| vec![single; n])[vert_index] = vmat;
    }
    fn set_single_material(&mut self, vmat: Option<VertexMaterialHandle>, pass: usize) {
        self.single_material[pass] = vmat;
    }
    fn set_texture(&mut self, poly_index: usize, tex: Option<TextureHandle>, pass: usize, stage: usize) {
        let n = self.poly_count;
        let single = self.single_texture[pass][stage];
        self.texture_array[pass][stage].get_or_insert_with(|| vec![single; n])[poly_index] = tex;
    }
    fn set_single_texture(&mut self, tex: Option<TextureHandle>, pass: usize, stage: usize) {
        self.single_texture[pass][stage] = tex;
    }
}

/// `class DynamicMeshClass` (dynamesh.h) and, when `screen_aspect` is `Some`, `DynamicScreenMeshClass`.
#[derive(Clone, Debug)]
pub struct DynamicMeshClass {
    pub model: DynamicMeshModel,
    vertex_material_idx: [i32; MAX_PASSES],
    multi_vertex_material: [bool; MAX_PASSES],
    texture_idx: [i32; MAX_PASSES],
    multi_texture: [bool; MAX_PASSES],
    cur_vertex_color: [Vector4; MAX_COLOR_ARRAYS],
    multi_vertex_color: [bool; MAX_COLOR_ARRAYS],
    poly_count: i32,
    vert_count: i32,
    tri_vertex_count: i32,
    fan_vertex: i32,
    tri_mode: i32,
    sort_level: i32,
    /// `DynamicScreenMeshClass::Aspect`: `Some` makes this a `DynamicScreenMeshClass`.
    screen_aspect: Option<f32>,
    /// `RenderObjClass::Transform` translation (only `Set_Position` of the screen mesh touches it here).
    pub position: Vector3,
}

impl DynamicMeshClass {
    /// `DynamicMeshClass::DynamicMeshClass(max_poly, max_vert)` / `(.., mat_info)` (dynamesh.cpp:380 / 407).
    pub fn new(max_poly: usize, max_vert: usize, mat_info: Option<MaterialInfoClass>) -> Self {
        DynamicMeshClass {
            model: DynamicMeshModel::new(max_poly, max_vert, mat_info),
            vertex_material_idx: [-1; MAX_PASSES],
            multi_vertex_material: [false; MAX_PASSES],
            texture_idx: [-1; MAX_PASSES],
            multi_texture: [false; MAX_PASSES],
            cur_vertex_color: [Vector4 { x: 1.0, y: 1.0, z: 1.0, w: 1.0 }; MAX_COLOR_ARRAYS],
            multi_vertex_color: [false; MAX_COLOR_ARRAYS],
            poly_count: 0,
            vert_count: 0,
            tri_vertex_count: 0,
            fan_vertex: 0,
            tri_mode: TRI_MODE_STRIPS,
            sort_level: SORT_LEVEL_NONE,
            screen_aspect: None,
            position: Vector3::new(0.0, 0.0, 0.0),
        }
    }

    /// `DynamicScreenMeshClass::DynamicScreenMeshClass(max_poly, max_vert, aspect = 1.0f)` (dynamesh.h).
    pub fn new_screen(max_poly: usize, max_vert: usize, aspect: f32) -> Self {
        let mut m = Self::new(max_poly, max_vert, None);
        m.screen_aspect = Some(aspect);
        m
    }

    // DynamicMeshClass::DynamicMeshClass(const DynamicMeshClass &) (dynamesh.cpp:434) and Clone() are `Clone`; the
    // model copy upstream goes through MaterialRemapperClass to clone the material info, which `Clone` of the owned
    // MaterialInfoClass does not do for ref-counted materials (handles are shared).

    /// `DynamicMeshClass::Get_Num_Polys`.
    pub fn get_num_polys(&self) -> i32 {
        self.poly_count
    }

    /// `DynamicMeshClass::Get_Num_Vertices`.
    pub fn get_num_vertices(&self) -> i32 {
        self.vert_count
    }

    pub fn get_sort_level(&self) -> i32 {
        self.sort_level
    }

    /// `DynamicMeshClass::Set_Sort_Level`: a non-NONE level disables per-mesh sorting. (`SortLevel` is a `char`.)
    pub fn set_sort_level(&mut self, level: i32) {
        self.sort_level = level as i8 as i32;
        if level != SORT_LEVEL_NONE {
            self.disable_sort();
        }
    }

    pub fn set_pass_count(&mut self, passes: usize) {
        assert!(self.poly_count == 0);
        self.model.set_pass_count(passes);
    }

    pub fn get_pass_count(&self) -> usize {
        self.model.get_pass_count()
    }

    /// `DynamicMeshClass::Set_Shader`.
    pub fn set_shader(&mut self, shader: ShaderClass, pass: usize) -> i32 {
        self.model.set_single_shader(shader, pass);
        0
    }

    // Flag helpers (dynamesh.h)
    pub fn set_dirty_bounds(&mut self) {
        self.model.set_flag(DIRTY_BOUNDS, true);
    }
    pub fn set_dirty_planes(&mut self) {
        self.model.set_flag(DIRTY_PLANES, true);
    }
    pub fn set_dirty_vertex_normals(&mut self) {
        self.model.set_flag(DIRTY_VNORMALS, true);
    }
    pub fn disable_sort(&mut self) {
        self.model.set_flag(SORT, false);
    }
    pub fn enable_sort(&mut self) {
        self.model.set_flag(SORT, true);
    }
    pub fn sort_enabled(&self) -> bool {
        self.model.get_flag(SORT)
    }
    pub fn disable_bounding_box(&mut self) {
        self.model.set_flag(DISABLE_BOUNDING_BOX, true);
    }
    pub fn enable_bounding_box(&mut self) {
        self.model.set_flag(DISABLE_BOUNDING_BOX, false);
    }
    pub fn test_bounding_box(&self) -> bool {
        !self.model.get_flag(DISABLE_BOUNDING_BOX)
    }
    pub fn disable_bounding_sphere(&mut self) {
        self.model.set_flag(DISABLE_BOUNDING_SPHERE, true);
    }
    pub fn enable_bounding_sphere(&mut self) {
        self.model.set_flag(DISABLE_BOUNDING_SPHERE, false);
    }
    pub fn test_bounding_sphere(&self) -> bool {
        !self.model.get_flag(DISABLE_BOUNDING_SPHERE)
    }

    /// `DynamicMeshClass::Set_Dirty`.
    pub fn set_dirty(&mut self) {
        self.set_dirty_bounds();
        self.set_dirty_planes();
        self.set_dirty_vertex_normals();
    }

    /// `DynamicMeshClass::Reset_Flags`.
    pub fn reset_flags(&mut self) {
        self.set_dirty();
    }

    /// `DynamicMeshClass::Reset_Native_Mesh`.
    pub fn reset_native_mesh(&mut self) {
        self.model.reset();
    }

    /// `DynamicMeshClass::Reset_Mesh_Counters`.
    pub fn reset_mesh_counters(&mut self) {
        self.model.set_counts(0, 0);
        self.disable_sort();
        self.poly_count = 0;
        self.vert_count = 0;
    }

    /// `DynamicMeshClass::Reset` (dynamesh.h), and the `DynamicScreenMeshClass::Reset` override
    /// (dynamesh.cpp:`DynamicScreenMeshClass::Reset`): the screen variant skips the native-mesh reset and the
    /// index clearing.
    pub fn reset(&mut self) {
        if self.screen_aspect.is_some() {
            self.reset_flags();
            self.reset_mesh_counters();
            return;
        }
        self.reset_flags();
        self.reset_native_mesh();
        self.reset_mesh_counters();
        let mut pass = MAX_PASSES;
        while pass > 0 {
            pass -= 1;
            self.texture_idx[pass] = -1;
            self.vertex_material_idx[pass] = -1;
            self.multi_vertex_material[pass] = false;
        }
    }

    /// `DynamicMeshClass::Resize` (dynamesh.cpp:466).
    pub fn resize(&mut self, max_polys: usize, max_verts: usize) {
        self.reset();
        self.model = DynamicMeshModel::new(max_polys, max_verts, None);
        let mut pass = MAX_PASSES;
        while pass > 0 {
            pass -= 1;
            self.texture_idx[pass] = -1;
            self.vertex_material_idx[pass] = -1;
            self.multi_vertex_material[pass] = false;
        }
    }

    pub fn begin_tri_strip(&mut self) {
        self.tri_vertex_count = 0;
        self.tri_mode = TRI_MODE_STRIPS;
    }

    pub fn begin_tri_fan(&mut self) {
        self.tri_vertex_count = 0;
        self.tri_mode = TRI_MODE_FANS;
        self.fan_vertex = self.vert_count;
    }

    pub fn end_tri_strip(&mut self) {
        self.tri_vertex_count = 0;
    }

    pub fn end_tri_fan(&mut self) {
        self.tri_vertex_count = 0;
    }

    /// `DynamicMeshClass::Location` (dynamesh.cpp:510) and the `DynamicScreenMeshClass::Location` override: the
    /// screen variant remaps `(x, y)` to `(x*2 - 1, Aspect - y*2*Aspect)` and forces z to 0.
    pub fn location(&mut self, x: f32, y: f32, z: f32) {
        if let Some(aspect) = self.screen_aspect {
            self.location_base((x * 2.0) - 1.0, aspect - (y * 2.0 * aspect), 0.0);
        } else {
            self.location_base(x, y, z);
        }
    }

    fn location_base(&mut self, x: f32, y: f32, z: f32) {
        let vc = self.vert_count as usize;
        let loc = self.model.get_vertex_array();
        loc[vc].x = x;
        loc[vc].y = y;
        loc[vc].z = z;
    }

    /// `DynamicMeshClass::Location_Inline`.
    pub fn location_inline(&mut self, x: f32, y: f32, z: f32) {
        self.location_base(x, y, z);
    }

    /// `DynamicMeshClass::Get_Location`.
    pub fn get_location(&mut self, index: usize) -> &mut Vector3 {
        &mut self.model.get_vertex_array()[index]
    }

    /// `DynamicMeshClass::Normal(x, y, z)`.
    pub fn normal(&mut self, x: f32, y: f32, z: f32) {
        let vc = self.vert_count as usize;
        let norms = self.model.get_non_const_vertex_normal_array();
        norms[vc].x = x;
        norms[vc].y = y;
        norms[vc].z = z;
    }

    /// `DynamicMeshClass::Color(r, g, b, a, color_array_index)`.
    pub fn color(&mut self, r: f32, g: f32, b: f32, a: f32, color_array_index: usize) {
        let vc = self.vert_count as usize;
        let packed = convert_color_clamp(&Vector4 { x: r, y: g, z: b, w: a });
        self.model.get_color_array(color_array_index)[vc] = packed;
    }

    /// `DynamicMeshClass::Color(unsigned v, color_array_index)`.
    pub fn color_packed(&mut self, v: u32, color_array_index: usize) {
        let vc = self.vert_count as usize;
        self.model.get_color_array(color_array_index)[vc] = v;
    }

    /// `DynamicMeshClass::Get_Color`.
    pub fn get_color(&mut self, index: usize, color_array_index: usize) -> u32 {
        self.model.get_color_array(color_array_index)[index]
    }

    /// `DynamicMeshClass::UV(u, v, uv_array_index)`.
    pub fn uv(&mut self, u: f32, v: f32, uv_array_index: usize) {
        let vc = self.vert_count as usize;
        let uvs = self.model.get_uv_array(uv_array_index);
        uvs[vc].x = u;
        uvs[vc].y = v;
    }

    /// `DynamicMeshClass::Get_UV`.
    pub fn get_uv(&mut self, index: usize, uv_array_index: usize) -> &mut Vector2 {
        &mut self.model.get_uv_array(uv_array_index)[index]
    }

    /// `DynamicMeshClass::Begin_Vertex` (empty).
    pub fn begin_vertex(&mut self) {}

    /// `DynamicMeshClass::Flip_Face` and the `DynamicScreenMeshClass::Flip_Face` override (inverted).
    fn flip_face(&self) -> bool {
        let base = self.tri_vertex_count & 1 == 0;
        if self.screen_aspect.is_some() {
            !base
        } else {
            base
        }
    }

    /// `DynamicMeshClass::Vertex(x, y, z, u, v)` (dynamesh.h).
    pub fn vertex(&mut self, x: f32, y: f32, z: f32, u: f32, v: f32) -> bool {
        self.begin_vertex();
        self.location(x, y, z);
        self.uv(u, v, 0);
        self.end_vertex()
    }

    /// `DynamicMeshClass::Vertex(Vector2 v)`.
    pub fn vertex_v2(&mut self, v: Vector2) -> bool {
        self.begin_vertex();
        self.location(v.x, v.y, 0.0);
        self.end_vertex()
    }

    /// `DynamicMeshClass::End_Vertex` (dynamesh.cpp:300).
    pub fn end_vertex(&mut self) -> bool {
        // check that we have room for a new vertex
        assert!(self.vert_count < self.model.get_vertex_count() as i32);

        // if we are a multi-material object record the material
        let mut pass = self.get_pass_count();
        while pass > 0 {
            pass -= 1;
            if self.multi_vertex_material[pass] {
                let mat = self.model.peek_material_info().peek_vertex_material(self.vertex_material_idx[pass] as usize);
                MeshMatDesc::set_material(&mut self.model, self.vert_count as usize, mat, pass);
            }
        }

        // if we are multi colored, record the color
        for color_array_index in 0..MAX_COLOR_ARRAYS {
            if self.multi_vertex_color[color_array_index] {
                let vc = self.vert_count as usize;
                let packed = convert_color_clamp(&self.cur_vertex_color[color_array_index]);
                self.model.get_color_array(color_array_index)[vc] = packed;
            }
        }

        // mark this vertex as being complete
        self.vert_count += 1;
        self.tri_vertex_count += 1;

        // if we have 3 or more vertices, add a new poly
        if self.tri_vertex_count >= 3 {
            assert!(self.poly_count < self.model.get_polygon_count() as i32);

            let vc = self.vert_count as u32;
            let pc = self.poly_count as usize;
            let (tri_mode, fan_vertex) = (self.tri_mode, self.fan_vertex as u32);
            let flip = self.flip_face();
            {
                let poly = &mut self.model.get_non_const_polygon_array()[pc];
                if tri_mode == TRI_MODE_STRIPS {
                    poly[0] = vc - 3;
                    poly[1] = vc - 2;
                    poly[2] = vc - 1;
                    // for every other tri, reverse vertex order
                    if flip {
                        poly[1] = vc - 1;
                        poly[2] = vc - 2;
                    }
                } else {
                    poly[0] = fan_vertex;
                    poly[1] = vc - 2;
                    poly[2] = vc - 1;
                }
            }

            // check each pass
            let mut pass = self.get_pass_count();
            while pass > 0 {
                pass -= 1;
                if self.multi_texture[pass] {
                    let tex = self.model.peek_material_info().peek_texture(self.texture_idx[pass] as usize);
                    MeshMatDesc::set_texture(&mut self.model, pc, Some(tex), pass, 0);
                }
            }

            self.poly_count += 1;
            self.model.set_counts(self.poly_count, self.vert_count);
        }
        true
    }

    /// `DynamicMeshClass::Move_Vertex` (dynamesh.cpp:524) and the screen override.
    pub fn move_vertex(&mut self, index: usize, x: f32, y: f32, z: f32) {
        let (x, y, z) = match self.screen_aspect {
            Some(aspect) => ((x * 2.0) - 1.0, aspect - (y * 2.0 * aspect), 0.0),
            None => (x, y, z),
        };
        let loc = self.model.get_vertex_array();
        loc[index].x = x;
        loc[index].y = y;
        loc[index].z = z;
    }

    /// `DynamicMeshClass::Get_Vertex`.
    pub fn get_vertex(&mut self, index: usize) -> (f32, f32, f32) {
        let l = self.model.get_vertex_array()[index];
        (l.x, l.y, l.z)
    }

    /// `DynamicMeshClass::Translate_Vertices` (dynamesh.cpp:548).
    pub fn translate_vertices(&mut self, offset: &Vector3) {
        let n = self.get_num_vertices() as usize;
        let loc = self.model.get_vertex_array();
        for i in 0..n {
            loc[i].x += offset.x;
            loc[i].y += offset.y;
            loc[i].z += offset.z;
        }
        self.set_dirty_bounds();
        self.set_dirty_planes();
    }

    /// `DynamicScreenMeshClass::Set_Position` (dynamesh.cpp): `RenderObjClass::Set_Position(Vector3(v.X*2,
    /// -(v.Y*2*Aspect), 0))`.
    pub fn set_screen_position(&mut self, v: &Vector3) {
        let aspect = self.screen_aspect.unwrap_or(1.0);
        self.position = Vector3::new(v.x * 2.0, -(v.y * 2.0 * aspect), 0.0);
    }

    /// `DynamicScreenMeshClass::Set_Aspect`.
    pub fn set_aspect(&mut self, aspect: f32) {
        self.screen_aspect = Some(aspect);
    }

    /// `DynamicMeshClass::Switch_To_Multi_Vertex_Color` (dynamesh.h).
    fn switch_to_multi_vertex_color(&mut self, color_array_index: usize) {
        let vertex_color = convert_color_clamp(&self.cur_vertex_color[color_array_index]);
        let n = self.vert_count as usize;
        let list = self.model.get_color_array(color_array_index);
        for lp in 0..n {
            list[lp] = vertex_color;
        }
        self.multi_vertex_color[color_array_index] = true;
    }

    /// `DynamicMeshClass::Set_Vertex_Color(const Vector4 &, color_array_index)`.
    pub fn set_vertex_color(&mut self, color: &Vector4, color_array_index: usize) -> i32 {
        if !self.multi_vertex_color[color_array_index] {
            self.switch_to_multi_vertex_color(color_array_index);
        }
        self.cur_vertex_color[color_array_index] = *color;
        0
    }

    /// `DynamicMeshClass::Set_Vertex_Color(const Vector3 &, ..)`: alpha 1.
    pub fn set_vertex_color_v3(&mut self, color: &Vector3, color_array_index: usize) -> i32 {
        self.set_vertex_color(&Vector4 { x: color.x, y: color.y, z: color.z, w: 1.0 }, color_array_index);
        0
    }

    /// `DynamicMeshClass::Change_Vertex_Color` (dynamesh.h).
    pub fn change_vertex_color(&mut self, index: usize, color: &Vector4, color_array_index: usize) {
        if !self.multi_vertex_color[color_array_index] {
            self.switch_to_multi_vertex_color(color_array_index);
        }
        self.cur_vertex_color[color_array_index] = *color;
        let packed = convert_color_clamp(color);
        self.model.get_color_array(color_array_index)[index] = packed;
    }

    /// `DynamicMeshClass::Set_Vertex_Material(int idx, int pass)` (dynamesh.cpp:560).
    pub fn set_vertex_material_idx(&mut self, idx: i32, pass: usize) -> i32 {
        assert!((idx as usize) < self.model.peek_material_info().vertex_material_count());
        self.vertex_material_idx[pass] = idx;
        if !self.multi_vertex_material[pass] {
            let mat = self.model.peek_material_info().peek_vertex_material(idx as usize);
            MeshMatDesc::set_single_material(&mut self.model, mat, pass);
        }
        self.vertex_material_idx[pass]
    }

    /// `DynamicMeshClass::Set_Vertex_Material(VertexMaterialClass *, dont_search, pass)` (dynamesh.cpp:573).
    pub fn set_vertex_material(&mut self, material: VertexMaterialHandle, dont_search: bool, pass: usize) -> i32 {
        let count = self.model.peek_material_info().vertex_material_count();
        let cur = self.vertex_material_idx[pass];
        // Check if same as the last vertex material
        if count > 0 && cur != -1 && self.model.peek_material_info().peek_vertex_material(cur as usize) == Some(material) {
            return cur;
        }
        // we may have just jumped to becoming a multi-vertex-material object.
        if !self.multi_vertex_material[pass]
            && count > 0
            && cur != -1
            && self.model.peek_material_info().peek_vertex_material(cur as usize) != Some(material)
        {
            let mat = self.model.peek_material_info().peek_vertex_material(cur as usize);
            self.model.initialize_material_array(pass, mat);
            self.multi_vertex_material[pass] = true;
        }

        // add the material to the material info if we can't find it in the list.
        if !dont_search {
            let mut found = false;
            for lp in 0..self.model.peek_material_info().vertex_material_count() {
                if self.model.peek_material_info().peek_vertex_material(lp) == Some(material) {
                    self.vertex_material_idx[pass] = lp as i32;
                    found = true;
                    break;
                }
            }
            if !found {
                self.model.peek_material_info_mut().add_vertex_material(Some(material));
                self.vertex_material_idx[pass] = self.model.peek_material_info().vertex_material_count() as i32 - 1;
            }
        } else {
            self.model.peek_material_info_mut().add_vertex_material(Some(material));
            self.vertex_material_idx[pass] = self.model.peek_material_info().vertex_material_count() as i32 - 1;
        }

        if !self.multi_vertex_material[pass] {
            let m = self.model.peek_material_info().peek_vertex_material(self.vertex_material_idx[pass] as usize);
            MeshMatDesc::set_single_material(&mut self.model, m, pass);
        }
        self.vertex_material_idx[pass]
    }

    /// `DynamicMeshClass::Set_Texture(int idx, int pass)` (dynamesh.cpp:632).
    pub fn set_texture_idx(&mut self, idx: i32, pass: usize) -> i32 {
        assert!((idx as usize) < self.model.peek_material_info().texture_count());
        self.texture_idx[pass] = idx;
        if !self.multi_texture[pass] {
            let tex = self.model.peek_material_info().peek_texture(idx as usize);
            MeshMatDesc::set_single_texture(&mut self.model, Some(tex), pass, 0);
        }
        self.texture_idx[pass]
    }

    /// `DynamicMeshClass::Set_Texture(TextureClass *, dont_search, pass)` (dynamesh.cpp:645).
    pub fn set_texture(&mut self, texture: TextureHandle, dont_search: bool, pass: usize) -> i32 {
        let count = self.model.peek_material_info().texture_count();
        let cur = self.texture_idx[pass];
        if count > 0 && cur != -1 && self.model.peek_material_info().peek_texture(cur as usize) == texture {
            return cur;
        }
        if !self.multi_texture[pass]
            && count > 0
            && cur != -1
            && self.model.peek_material_info().peek_texture(cur as usize) != texture
        {
            let tex = self.model.peek_material_info().peek_texture(cur as usize);
            self.model.initialize_texture_array(pass, 0, Some(tex));
            self.multi_texture[pass] = true;
        }

        if !dont_search {
            let mut found = false;
            for lp in 0..self.model.peek_material_info().texture_count() {
                if self.model.peek_material_info().peek_texture(lp) == texture {
                    self.texture_idx[pass] = lp as i32;
                    found = true;
                    break;
                }
            }
            if !found {
                self.model.peek_material_info_mut().add_texture(texture);
                self.texture_idx[pass] = self.model.peek_material_info().texture_count() as i32 - 1;
            }
        } else {
            self.model.peek_material_info_mut().add_texture(texture);
            self.texture_idx[pass] = self.model.peek_material_info().texture_count() as i32 - 1;
        }

        if !self.multi_texture[pass] {
            let tex = self.model.peek_material_info().peek_texture(self.texture_idx[pass] as usize);
            MeshMatDesc::set_single_texture(&mut self.model, Some(tex), pass, 0);
        }
        self.texture_idx[pass]
    }

    /// `DynamicMeshClass::Set_Polygon_Info` (dynamesh.h): shader, vertex material and (if any) texture of a
    /// `PolygonInfoClass`, passed as plain values (`None` texture = `Peek_Texture()` NULL).
    pub fn set_polygon_info(
        &mut self,
        shader: ShaderClass,
        vertex_material: VertexMaterialHandle,
        texture: Option<TextureHandle>,
        dont_search_texture: bool,
        dont_search_vertex_material: bool,
        pass: usize,
    ) {
        self.set_shader(shader, pass);
        self.set_vertex_material(vertex_material, dont_search_vertex_material, pass);
        if let Some(t) = texture {
            self.set_texture(t, dont_search_texture, pass);
        }
    }

    /// `DynamicMeshClass::Render` (dynamesh.cpp:288), the data half: returns `None` for an empty mesh (the
    /// hidden test, the static-sort-list redirect and the frustum overlap test are the render-object base's and the
    /// camera's, see `SortLevel`), otherwise the model's draw calls.
    pub fn render(&self, sorting_enabled: bool) -> Option<Vec<DynamicDraw>> {
        // test for an empty mesh..
        if self.poly_count == 0 {
            return None;
        }
        // upstream: DX8Wrapper::Set_Transform(D3DTS_WORLD, Transform) — device-only, not translated
        Some(self.model.render(sorting_enabled))
    }
}
