//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdsubmesh.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdsubmesh.h
//!
//! `ShdSubMeshClass`: "a collection of polygons who all use the same shader". Upstream derives from
//! `MeshGeometryClass`; the verified port has no such class (only `client::mesh_cast::MeshGeometry`, the part
//! `Cast_Ray` reads), so the geometry this class touches is held here under upstream's field names
//! (`PolyCount`, `VertexCount`, `Poly`, `Vertex`, `VertexNorm`, `BoundBoxMin`, ...). The `ShareBufferClass<T>`
//! reference-counted buffers are `Vec<T>` (sharing is the owner's business; `operator=` clones them).
//!
//! The shader instance is generic: `S` is the owner's shader-instance type (the `Shd6*`/`Shd7*`/`Shd8*` classes behind
//! `ShdInterfaceClass *`), reached through [`ShdShaderInstance`], which lists the virtuals this class and the renderer
//! call. `ShdDefManagerClass::Load_Shader` is `super::shddefmanager`.
//!
//! Not translated: `Get_Deformed_Vertices` (it forwards to `MeshGeometryClass::get_deformed_vertices`, which the port
//! has as `formats::w3d::meshmdl::MeshModel::get_deformed_vertices`; the owner of the HTree calls that with this
//! sub-mesh's vertices), `Create_Decal`/`Delete_Decal` (declared, never defined upstream), `Reset` (declared, never
//! defined upstream).

use super::shdchunk::SHDDEF_CLASSID_LEGACYW3D;
use super::shddef::ShdDefClass;
use super::shddefmanager::{ShdDefLoad, ShdDefManagerClass};
use super::shdlegacyw3d::Shd6LegacyW3DClass;
use super::handles::TextureHandle;
use crate::formats::w3d::mesh::mesh_flags;
use crate::formats::w3d::shader::ShaderClass;
use crate::formats::w3d::w3d_file::*;
use crate::formats::w3d::W3dResult;
use crate::ww::chunkio::ChunkLoad;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;

/// `MAX_TEXTURE_STAGES` (dx8wrapper.h).
pub const MAX_TEXTURE_STAGES: usize = super::shdchunk::MAX_TEXTURE_STAGES;
/// `MeshMatDescClass::MAX_TEX_STAGES`.
pub const MAX_TEX_STAGES: usize = 2;

/// `WW3DErrorType`: `Ok(())` is `WW3D_ERROR_OK`; `Err(ShdLoadFailed)` is `WW3D_ERROR_LOAD_FAILED`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShdLoadFailed;

/// Result of the loaders: a failed read is `WW3D_ERROR_LOAD_FAILED` (`Ok(Err(..))`); a chunk-io error is the outer
/// `Err`.
pub type Ww3dErrorType = W3dResult<Result<(), ShdLoadFailed>>;

/// D3D FVF bits (d3d8types.h) `Define_FVF` builds.
pub const D3DFVF_XYZ: u32 = 0x002;
pub const D3DFVF_NORMAL: u32 = 0x010;
pub const D3DFVF_DIFFUSE: u32 = 0x040;
pub const D3DFVF_SPECULAR: u32 = 0x080;
pub const D3DFVF_TEX1: u32 = 0x100;
pub const D3DFVF_TEX2: u32 = 0x200;
pub const D3DFVF_TEX3: u32 = 0x300;
pub const D3DFVF_TEX4: u32 = 0x400;
pub const D3DFVF_TEX5: u32 = 0x500;
pub const D3DFVF_TEX6: u32 = 0x600;
pub const D3DFVF_TEX7: u32 = 0x700;
pub const D3DFVF_TEX8: u32 = 0x800;
/// `dynamic_fvf_type` (dx8fvf.h): `D3DFVF_XYZ|D3DFVF_NORMAL|D3DFVF_TEX1|D3DFVF_DIFFUSE`.
pub const DYNAMIC_FVF_TYPE: u32 = D3DFVF_XYZ | D3DFVF_NORMAL | D3DFVF_TEX1 | D3DFVF_DIFFUSE;

/// The shader-instance virtuals (`ShdInterfaceClass` and its derived classes) that `ShdSubMeshClass`,
/// `ShdMeshClass` and the renderer call.
pub trait ShdShaderInstance {
    /// `Peek_Definition()->Get_Class_ID()`.
    fn definition_class_id(&self) -> Option<u32>;
    /// `Get_Class_ID()`.
    fn class_id(&self) -> u32;
    /// `Get_Pass_Count()`.
    fn pass_count(&self) -> i32;
    /// `Is_Opaque()`.
    fn is_opaque(&self) -> bool;
    /// `Greater_Than(s, pass)`.
    fn greater_than(&self, s: &Self, pass: i32) -> bool;
    /// `Similar_Enough(s, pass)`.
    fn similar_enough(&self, s: &Self, pass: i32) -> bool;
    /// `Get_Texture_Count()`.
    fn texture_count(&self) -> usize;
    /// `Peek_Texture(i)`.
    fn peek_texture(&self, i: usize) -> Option<TextureHandle>;
    /// `Use_HW_Vertex_Processing()`.
    fn use_hw_vertex_processing(&self) -> bool;
    /// `Get_Vertex_Stream_Count()`.
    fn vertex_stream_count(&self) -> u32;
    /// `Get_Vertex_Size(stream)`.
    fn vertex_size(&self, stream: u32) -> usize;
    /// `Copy_Vertex_Stream(stream, dest_buffer, vss, vertex_count)`: the bytes of the vertex layout (the shader files'
    /// `copy_vertex_stream_*` functions, serialised little-endian in struct order).
    fn copy_vertex_stream(&self, stream: u32, vss: &super::shdchunk::VertexStreamStruct, vertex_count: usize) -> Vec<u8>;
    /// `Pass_Selection(mesh, rinfo, pass)`; a shader may update its own state (`Shd8*`: the self-shadow transform).
    fn pass_selection(&mut self, mesh: &super::shdmesh::ShdMeshRenderState, pass: i32) -> bool;
    /// `Set_FVF(fvf)` of `Shd6LegacyW3DClass` (the "Danger! assuming Shd6LegacyW3DClass" cast); a no-op for any other.
    fn set_legacy_fvf(&mut self, fvf: u32);
}

/// What `Init_From_Legacy_Mesh_Model` and `Define_FVF` read of the legacy `MeshModelClass` (the port's
/// `formats::w3d::meshmdl::MeshModel` implements the owner's side of this contract).
pub trait LegacyMeshModel {
    fn get_polygon_count(&self) -> i32;
    fn get_pass_count(&self) -> i32;
    fn get_vertex_count(&self) -> usize;
    /// `Get_Flag(f)`, `f` from [`mesh_flags`].
    fn get_flag(&self, f: u32) -> bool;
    fn has_shader_array(&self, pass: i32) -> bool;
    fn has_material_array(&self, pass: i32) -> bool;
    fn has_texture_array(&self, pass: i32, stage: usize) -> bool;
    fn get_shader(&self, poly: i32, pass: i32) -> ShaderClass;
    fn get_single_shader(&self, pass: i32) -> ShaderClass;
    /// `Get_Polygon_Array()[poly]`.
    fn polygon(&self, poly: i32) -> [u32; 3];
    /// `Peek_Material(vidx, pass)` / `Get_Material` (a handle; `None` is NULL).
    fn peek_material(&self, vidx: u32, pass: i32) -> Option<usize>;
    fn get_single_material(&self, pass: i32) -> Option<usize>;
    /// `Peek_Texture(poly, pass, stage)` / `Get_Texture`.
    fn peek_texture(&self, poly: i32, pass: i32, stage: usize) -> Option<TextureHandle>;
    fn get_single_texture(&self, pass: i32, stage: usize) -> Option<TextureHandle>;
    /// `Get_UV_Array_Count()`.
    fn get_uv_array_count(&self) -> usize;
    /// `Get_UV_Array_By_Index(i)`.
    fn get_uv_array_by_index(&self, i: usize) -> Option<Vec<Vector2>>;
    /// `Get_DCG_Array(0)`.
    fn get_dcg_array0(&self) -> Option<Vec<u32>>;
    /// `Get_Color_Array(i, false)` is non-NULL.
    fn has_color_array(&self, i: usize) -> bool;
    /// `Needs_Vertex_Normals()`.
    fn needs_vertex_normals(&self) -> bool;
    /// `*(MeshGeometryClass*)this = *(MeshGeometryClass*)model`: the geometry (see [`ShdSubMeshGeometry`]).
    fn geometry(&self) -> ShdSubMeshGeometry;
}

/// The `MeshGeometryClass` part of a `ShdSubMeshClass`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShdSubMeshGeometry {
    pub name: String,
    pub flags: u32,
    pub poly_count: i32,
    pub vertex_count: i32,
    /// `Poly` (`ShareBufferClass<TriIndex>`).
    pub poly: Vec<[u32; 3]>,
    /// `PolySurfaceType`.
    pub poly_surface_type: Vec<u8>,
    /// `Vertex`.
    pub vertex: Vec<Vector3>,
    /// `VertexNorm` (`get_vert_normals`).
    pub vertex_norm: Vec<Vector3>,
    /// `BoneLinks` (`get_bone_links`, `uint16`).
    pub bone_links: Vec<u16>,
    /// `ShadeIndices` (`get_shade_indices`).
    pub shade_indices: Vec<u32>,
    pub bound_box_min: Vector3,
    pub bound_box_max: Vector3,
    pub bound_sphere_center: Vector3,
    pub bound_sphere_radius: f32,
}

/// `Define_FVF(MeshModelClass*, bool)` (shdsubmesh.cpp): the FVF for a legacy model. `sorting_enabled` is
/// `WW3D::Is_Sorting_Enabled()`. (File-static upstream; kept for the legacy path.)
pub fn define_fvf_model<M: LegacyMeshModel>(mmc: &M, _enable_lighting: bool, sorting_enabled: bool) -> u32 {
    if mmc.get_flag(mesh_flags::SKIN) {
        return DYNAMIC_FVF_TYPE;
    }
    if mmc.get_flag(mesh_flags::SORT) && sorting_enabled {
        return DYNAMIC_FVF_TYPE;
    }
    let mut fvf = D3DFVF_XYZ;
    let tex_coord_count = mmc.get_uv_array_count();
    if mmc.has_color_array(0) {
        fvf |= D3DFVF_DIFFUSE;
    }
    if mmc.has_color_array(1) {
        fvf |= D3DFVF_SPECULAR;
    }
    fvf |= tex_fvf(tex_coord_count);
    if !mmc.needs_vertex_normals() {
        return fvf;
    }
    fvf |= D3DFVF_NORMAL; // Realtime-lit
    fvf
}

/// The `switch (tex_coord_count)` of both `Define_FVF`s: 0 (and any other count) adds nothing; 1..=8 add `D3DFVF_TEXn`.
fn tex_fvf(tex_coord_count: usize) -> u32 {
    match tex_coord_count {
        1 => D3DFVF_TEX1,
        2 => D3DFVF_TEX2,
        3 => D3DFVF_TEX3,
        4 => D3DFVF_TEX4,
        5 => D3DFVF_TEX5,
        6 => D3DFVF_TEX6,
        7 => D3DFVF_TEX7,
        8 => D3DFVF_TEX8,
        _ => 0,
    }
}

/// `class ShdSubMeshClass` (shdsubmesh.h).
#[derive(Clone, Debug)]
pub struct ShdSubMeshClass<S> {
    pub geometry: ShdSubMeshGeometry,
    /// `UV[MAX_TEXTURE_STAGES]` (`ShareBufferClass<Vector2>*`; `None` is NULL).
    pub uv: [Option<Vec<Vector2>>; MAX_TEXTURE_STAGES],
    /// `Diffuse`.
    pub diffuse: Option<Vec<u32>>,
    /// `S`, `T`, `SxT`.
    pub s: Option<Vec<Vector3>>,
    pub t: Option<Vec<Vector3>>,
    pub sxt: Option<Vec<Vector3>>,
    pub first_visible_polygon: i32,
    pub visible_polygon_count: i32,
    pub sorting: bool,
    /// `Shader`.
    pub shader: Option<S>,
}

impl<S> ShdSubMeshClass<S> {
    /// `ShdSubMeshClass::ShdSubMeshClass()` (shdsubmesh.cpp).
    pub fn new() -> Self {
        ShdSubMeshClass {
            geometry: ShdSubMeshGeometry::default(),
            uv: Default::default(),
            diffuse: None,
            s: None,
            t: None,
            sxt: None,
            first_visible_polygon: 0,
            visible_polygon_count: 0,
            sorting: false,
            shader: None,
        }
    }

    /// `Set_Shader(shader)` (`REF_PTR_SET`).
    pub fn set_shader(&mut self, shader: Option<S>) {
        self.shader = shader;
    }

    /// `Peek_Shader()`.
    pub fn peek_shader(&self) -> Option<&S> {
        self.shader.as_ref()
    }

    /// `Get_UV_Array(stage)`.
    pub fn get_uv_array(&self, stage: usize) -> Option<&[Vector2]> {
        self.uv[stage].as_deref()
    }

    /// `Get_Diffuse_Array()`.
    pub fn get_diffuse_array(&self) -> Option<&[u32]> {
        self.diffuse.as_deref()
    }

    /// `Get_Tangent_Basis_S_Array()`.
    pub fn get_tangent_basis_s_array(&self) -> Option<&[Vector3]> {
        self.s.as_deref()
    }

    /// `Get_Tangent_Basis_T_Array()`.
    pub fn get_tangent_basis_t_array(&self) -> Option<&[Vector3]> {
        self.t.as_deref()
    }

    /// `Get_Tangent_Basis_SxT_Array()`.
    pub fn get_tangent_basis_sxt_array(&self) -> Option<&[Vector3]> {
        self.sxt.as_deref()
    }

    /// `Get_First_Visible_Polygon()`.
    pub fn get_first_visible_polygon(&self) -> i32 {
        self.first_visible_polygon
    }

    /// `Get_Visible_Polygon_Count()`.
    pub fn get_visible_polygon_count(&self) -> i32 {
        self.visible_polygon_count
    }

    /// `Is_Sorting()`.
    pub fn is_sorting(&self) -> bool {
        self.sorting
    }

    /// `Get_Flag(f)` (`MeshGeometryClass`).
    pub fn get_flag(&self, f: u32) -> bool {
        self.geometry.flags & f != 0
    }

    /// `Set_Flag(f, onoff)` (`MeshGeometryClass`).
    pub fn set_flag(&mut self, f: u32, onoff: bool) {
        if onoff {
            self.geometry.flags |= f;
        } else {
            self.geometry.flags &= !f;
        }
    }

    /// `Get_Polygon_Count()`.
    pub fn get_polygon_count(&self) -> i32 {
        self.geometry.poly_count
    }

    /// `Get_Vertex_Count()`.
    pub fn get_vertex_count(&self) -> i32 {
        self.geometry.vertex_count
    }

    /// `Set_Name(name)` (`MeshGeometryClass`).
    pub fn set_name(&mut self, name: &str) {
        self.geometry.name = name.to_string();
    }

    /// `Get_Name()`.
    pub fn get_name(&self) -> &str {
        &self.geometry.name
    }

    /// `Get_Bounding_Sphere` (`MeshGeometryClass`): centre and radius.
    pub fn get_bounding_sphere(&self) -> (Vector3, f32) {
        (self.geometry.bound_sphere_center, self.geometry.bound_sphere_radius)
    }

    /// `Get_Bounding_Box` (`MeshGeometryClass`): min and max corners.
    pub fn get_bounding_box(&self) -> (Vector3, Vector3) {
        (self.geometry.bound_box_min, self.geometry.bound_box_max)
    }
}

impl<S: Clone> ShdSubMeshClass<S> {
    /// `ShdSubMeshClass::ShdSubMeshClass(that)` / `operator=` (shdsubmesh.cpp): copies the visible range, `Sorting`
    /// (the copy constructor starts with `Sorting(false)` and then `*this = that` copies it), the shader reference,
    /// the buffers and the geometry.
    pub fn assign(&mut self, that: &ShdSubMeshClass<S>) {
        self.first_visible_polygon = that.first_visible_polygon;
        self.visible_polygon_count = that.visible_polygon_count;
        self.sorting = that.sorting;
        self.shader = that.shader.clone();
        self.uv = that.uv.clone();
        self.s = that.s.clone();
        self.t = that.t.clone();
        self.sxt = that.sxt.clone();
        self.diffuse = that.diffuse.clone();
        self.geometry = that.geometry.clone();
    }
}

impl<S: ShdShaderInstance> ShdSubMeshClass<S> {
    /// `Define_FVF(ShdSubMeshClass*, bool)` (shdsubmesh.cpp): the FVF for this sub-mesh. `sorting_enabled` is
    /// `WW3D::Is_Sorting_Enabled()`.
    pub fn define_fvf(&self, _enable_lighting: bool, sorting_enabled: bool) -> u32 {
        if self.get_flag(mesh_flags::SKIN) {
            return DYNAMIC_FVF_TYPE;
        }
        if self.get_flag(mesh_flags::SORT) && sorting_enabled {
            return DYNAMIC_FVF_TYPE;
        }
        let mut fvf = D3DFVF_XYZ;
        let mut tex_coord_count = 0usize;
        while tex_coord_count < MAX_TEX_STAGES {
            if self.get_uv_array(tex_coord_count).is_none() {
                break;
            }
            tex_coord_count += 1;
        }
        if self.get_diffuse_array().is_some() {
            fvf |= D3DFVF_DIFFUSE;
        }
        fvf |= tex_fvf(tex_coord_count);
        if !self.geometry.vertex_norm.is_empty() {
            fvf |= D3DFVF_NORMAL; // Realtime-lit
        }
        fvf
    }

    /// `ShdSubMeshClass::Init_From_Legacy_Mesh_Model(model, first_polygon)` (shdsubmesh.cpp). `sorting_enabled` is
    /// `WW3D::Is_Sorting_Enabled()`; `make_shader` turns the `Shd6LegacyW3DClass` this builds into the owner's shader
    /// instance (upstream: `shader_def->Create()` on a fresh `ShdLegacyW3DDefClass`, then the casts).
    pub fn init_from_legacy_mesh_model<M, F>(&mut self, model: &M, first_polygon: i32, sorting_enabled: bool, make_shader: F)
    where
        M: LegacyMeshModel,
        F: FnOnce(Shd6LegacyW3DClass) -> S,
    {
        debug_assert!(first_polygon < model.get_polygon_count());
        self.sorting = model.get_flag(mesh_flags::SORT) && sorting_enabled;

        // Figure out how many polygons we can use
        let mut active_polygon_count = 0i32;
        let mut arrays_used = false;
        let mut pass = 0;
        while pass < model.get_pass_count() && !arrays_used {
            if model.has_shader_array(pass) || model.has_material_array(pass) {
                arrays_used = true;
                break;
            }
            for stage in 0..MAX_TEX_STAGES {
                if model.has_texture_array(pass, stage) {
                    arrays_used = true;
                    break;
                }
            }
            pass += 1;
        }

        // If any arrays are used by the mesh, count how many polygons we have that share properties.
        if arrays_used {
            let mut stop = false;
            active_polygon_count = 0;
            let mut poly = first_polygon;
            while poly < model.get_polygon_count() && !stop {
                for pass in 0..model.get_pass_count() {
                    if model.has_shader_array(pass) {
                        if model.get_shader(first_polygon, pass) != model.get_shader(poly, pass) {
                            stop = true;
                            break;
                        }
                    }
                    if model.has_material_array(pass) {
                        let first_tri = model.polygon(first_polygon);
                        let cur_tri = model.polygon(poly);
                        if model.peek_material(first_tri[0], pass) != model.peek_material(cur_tri[0], pass)
                            || model.peek_material(first_tri[1], pass) != model.peek_material(cur_tri[1], pass)
                            || model.peek_material(first_tri[2], pass) != model.peek_material(cur_tri[2], pass)
                        {
                            stop = true;
                            break;
                        }
                    }
                    for stage in 0..MAX_TEX_STAGES {
                        if model.has_texture_array(pass, stage) {
                            if model.peek_texture(first_polygon, pass, stage) != model.peek_texture(poly, pass, stage) {
                                stop = true;
                                break;
                            }
                        }
                    }
                }
                // upstream increments after the pass loop even when `stop` was set inside it
                active_polygon_count += 1;
                poly += 1;
            }
        }

        self.first_visible_polygon = first_polygon;
        self.visible_polygon_count = active_polygon_count;
        if self.first_visible_polygon != 0 && self.visible_polygon_count == 0 {
            self.visible_polygon_count = model.get_polygon_count() - self.first_visible_polygon;
        }
        debug_assert!(self.first_visible_polygon + self.visible_polygon_count <= model.get_polygon_count());

        // Now that the definition is initialized, create a shader instance (the one def/instance pair of a legacy mesh)
        let mut legacy = Shd6LegacyW3DClass {
            pass_count: model.get_pass_count(),
            shaders: [ShaderClass(0); super::shdlegacyw3d::MAX_PASSES],
            materials: [None; super::shdlegacyw3d::MAX_PASSES],
            textures: [[None; super::shdlegacyw3d::MAX_TEX_STAGES]; super::shdlegacyw3d::MAX_PASSES],
            class_id: SHDDEF_CLASSID_LEGACYW3D,
            fvf: define_fvf_model(model, true, sorting_enabled),
        };
        for pass in 0..model.get_pass_count() {
            let legacy_shader = if model.has_shader_array(pass) {
                model.get_shader(first_polygon, pass)
            } else {
                model.get_single_shader(pass)
            };
            let legacy_material = if model.has_material_array(pass) {
                let first_tri = model.polygon(first_polygon);
                model.peek_material(first_tri[0], pass)
            } else {
                model.get_single_material(pass)
            };
            for stage in 0..MAX_TEX_STAGES {
                let texture = if model.has_texture_array(pass, stage) {
                    model.peek_texture(first_polygon, pass, stage)
                } else {
                    model.get_single_texture(pass, stage)
                };
                if texture.is_some() {
                    // upstream: Set_Texture(pass, stage, texture); texture->Release_Ref() (ref counts are the owner's)
                    legacy.textures[pass as usize][stage] = texture;
                }
            }
            legacy.shaders[pass as usize] = legacy_shader;
            legacy.materials[pass as usize] = legacy_material;
        }
        self.shader = Some(make_shader(legacy));

        // *(MeshGeometryClass*)this = *(MeshGeometryClass*)model;  (the CullTree's Set_Mesh(this) is the owner's)
        self.geometry = model.geometry();

        // Copy texture coordinates
        let uv_array_count = model.get_uv_array_count();
        debug_assert!(uv_array_count <= MAX_TEXTURE_STAGES);
        for uvi in 0..uv_array_count {
            if let Some(uv) = model.get_uv_array_by_index(uvi) {
                // memcpy(UV[uvi]->Get_Array(), uv, sizeof(Vector2)*model->Get_Vertex_Count())
                let n = model.get_vertex_count();
                self.uv[uvi] = Some(uv[..n].to_vec());
            }
        }

        // Copy vertex color array
        if let Some(diffuse) = model.get_dcg_array0() {
            let n = model.get_vertex_count();
            self.diffuse = Some(diffuse[..n].to_vec());
        }
    }

    /// `ShdSubMeshClass::Load_W3D(cload)` (shdsubmesh.cpp). `defs` is the shader definition manager;
    /// `create_shader` is `shddef->Create()`; `sorting_enabled` unused by upstream here; `legacy_is_opaque` is read
    /// through the shader. Returns upstream's `WW3D_ERROR_*`.
    pub fn load_w3d<D, F>(&mut self, cload: &mut ChunkLoad<'_>, defs: &ShdDefManagerClass<D>, create_shader: F) -> Ww3dErrorType
    where
        D: ShdDefLoad,
        F: FnOnce(D) -> S,
    {
        // read header
        cload.open_chunk()?;
        let mut hb = [0u8; 56];
        if cload.read_into(&mut hb)? != 56 {
            return Ok(Err(ShdLoadFailed));
        }
        let hdr = W3dShdSubMeshHeaderStruct::from_bytes(&hb);
        cload.close_chunk()?;

        // process header
        let g = &mut self.geometry;
        g.poly_count = hdr.num_tris as i32;
        g.vertex_count = hdr.num_vertices as i32;
        if g.poly_count != 0 && g.vertex_count != 0 {
            let pc = g.poly_count as usize;
            let vc = g.vertex_count as usize;
            g.poly = vec![[0u32; 3]; pc];
            g.poly_surface_type = vec![0u8; pc];
            g.vertex = vec![Vector3::new(0.0, 0.0, 0.0); vc];
            // OPTIMIZE_VNORM_RAM is off: VertexNorm is allocated here
            g.vertex_norm = vec![Vector3::new(0.0, 0.0, 0.0); vc];
        }
        g.bound_box_min = Vector3::new(hdr.box_min.x, hdr.box_min.y, hdr.box_min.z);
        g.bound_box_max = Vector3::new(hdr.box_max.x, hdr.box_max.y, hdr.box_max.z);
        g.bound_sphere_center = Vector3::new(hdr.sph_center.x, hdr.sph_center.y, hdr.sph_center.z);
        g.bound_sphere_radius = hdr.sph_radius;

        // upstream ignores read_chunks' result here
        let _ = self.read_chunks(cload, defs, create_shader)?;

        let (def_class, is_opaque) = match &self.shader {
            Some(s) => (s.definition_class_id(), s.is_opaque()),
            // upstream dereferences Shader->Peek_Definition() unchecked (a mesh with no shader chunk crashes)
            None => (None, true),
        };
        if def_class == Some(SHDDEF_CLASSID_LEGACYW3D) {
            let fvf = self.define_fvf(true, true);
            if let Some(s) = self.shader.as_mut() {
                // Danger! We're assuming Shd6LegacyW3DClass is the only type created by legacy shader!!!
                s.set_legacy_fvf(fvf);
            }
            if !is_opaque {
                self.set_flag(mesh_flags::SORT, true);
            }
        }
        Ok(Ok(()))
    }

    /// `read_chunks(cload)`: opens each chunk until none are left and dispatches on its id; the first error ends it.
    fn read_chunks<D, F>(&mut self, cload: &mut ChunkLoad<'_>, defs: &ShdDefManagerClass<D>, create_shader: F) -> Ww3dErrorType
    where
        D: ShdDefLoad,
        F: FnOnce(D) -> S,
    {
        let mut create_shader = Some(create_shader);
        while cload.open_chunk()? {
            let error = match cload.cur_chunk_id()? {
                W3D_CHUNK_SHDSUBMESH_VERTICES => self.read_vertices(cload)?,
                W3D_CHUNK_SHDSUBMESH_VERTEX_NORMALS => self.read_vertex_normals(cload)?,
                W3D_CHUNK_SHDSUBMESH_UV0 => self.read_uv(cload, 0)?,
                W3D_CHUNK_SHDSUBMESH_UV1 => self.read_uv(cload, 1)?,
                W3D_CHUNK_SHDSUBMESH_TANGENT_BASIS_S => self.read_tangent_basis(cload, 0)?,
                W3D_CHUNK_SHDSUBMESH_TANGENT_BASIS_T => self.read_tangent_basis(cload, 1)?,
                W3D_CHUNK_SHDSUBMESH_TANGENT_BASIS_SXT => self.read_tangent_basis(cload, 2)?,
                W3D_CHUNK_SHDSUBMESH_TRIANGLES => self.read_triangles(cload)?,
                W3D_CHUNK_SHDSUBMESH_VERTEX_SHADE_INDICES => self.read_vertex_shade_indices(cload)?,
                W3D_CHUNK_SHDSUBMESH_SHADER => match create_shader.take() {
                    Some(f) => self.read_shader(cload, defs, f)?,
                    None => Ok(()),
                },
                W3D_CHUNK_SHDSUBMESH_VERTEX_INFLUENCES => self.read_vertex_influences(cload)?,
                _ => Ok(()),
            };
            cload.close_chunk()?;
            if error.is_err() {
                return Ok(error);
            }
        }
        Ok(Ok(()))
    }

    /// `read_vertices`: `Get_Vertex_Count()` `W3dVectorStruct`s into `Vertex`.
    fn read_vertices(&mut self, cload: &mut ChunkLoad<'_>) -> Ww3dErrorType {
        for i in 0..self.geometry.vertex_count as usize {
            let mut b = [0u8; 12];
            if cload.read_into(&mut b)? != 12 {
                return Ok(Err(ShdLoadFailed));
            }
            let v = read_vec3(&b);
            self.geometry.vertex[i] = v;
        }
        Ok(Ok(()))
    }

    /// `read_vertex_normals`: `VertexCount` `W3dVectorStruct`s into the vertex normals.
    fn read_vertex_normals(&mut self, cload: &mut ChunkLoad<'_>) -> Ww3dErrorType {
        for i in 0..self.geometry.vertex_count as usize {
            let mut b = [0u8; 12];
            if cload.read_into(&mut b)? != 12 {
                return Ok(Err(ShdLoadFailed));
            }
            self.geometry.vertex_norm[i] = read_vec3(&b);
        }
        Ok(Ok(()))
    }

    /// `read_uv0` (`stage` 0) / `read_uv1` (`stage` 1): `VertexCount` `Vector2`s into a fresh zeroed buffer.
    fn read_uv(&mut self, cload: &mut ChunkLoad<'_>, stage: usize) -> Ww3dErrorType {
        let n = self.geometry.vertex_count as usize;
        let mut buf = vec![Vector2::new(0.0, 0.0); n];
        self.uv[stage] = Some(Vec::new()); // NEW_REF replaces the old buffer before the reads
        for i in 0..n {
            let mut b = [0u8; 8];
            if cload.read_into(&mut b)? != 8 {
                self.uv[stage] = Some(buf);
                return Ok(Err(ShdLoadFailed));
            }
            buf[i] = Vector2::new(f32::from_le_bytes([b[0], b[1], b[2], b[3]]), f32::from_le_bytes([b[4], b[5], b[6], b[7]]));
        }
        self.uv[stage] = Some(buf);
        Ok(Ok(()))
    }

    /// `read_tangent_basis_s` (`which` 0), `_t` (1), `_sxt` (2): `VertexCount` `Vector3`s into a fresh zeroed buffer.
    fn read_tangent_basis(&mut self, cload: &mut ChunkLoad<'_>, which: usize) -> Ww3dErrorType {
        let n = self.geometry.vertex_count as usize;
        let mut buf = vec![Vector3::new(0.0, 0.0, 0.0); n];
        let mut failed = false;
        for i in 0..n {
            let mut b = [0u8; 12];
            if cload.read_into(&mut b)? != 12 {
                failed = true;
                break;
            }
            buf[i] = read_vec3(&b);
        }
        match which {
            0 => self.s = Some(buf),
            1 => self.t = Some(buf),
            _ => self.sxt = Some(buf),
        }
        Ok(if failed { Err(ShdLoadFailed) } else { Ok(()) })
    }

    /// `read_vertex_influences`: `VertexCount` bytes, each read into the next `uint16` of `BoneLinks` (upstream reads
    /// `sizeof(uint8)` into a `uint16*`, so only the low byte is written); sets the `SKIN` flag.
    fn read_vertex_influences(&mut self, cload: &mut ChunkLoad<'_>) -> Ww3dErrorType {
        let n = self.geometry.vertex_count as usize;
        if self.geometry.bone_links.len() < n {
            self.geometry.bone_links.resize(n, 0);
        }
        for i in 0..n {
            let mut b = [0u8; 1];
            if cload.read_into(&mut b)? != 1 {
                return Ok(Err(ShdLoadFailed));
            }
            // low byte only; the high byte keeps what get_bone_links() allocated (zero)
            self.geometry.bone_links[i] = (self.geometry.bone_links[i] & 0xff00) | u16::from(b[0]);
        }
        self.set_flag(mesh_flags::SKIN, true);
        Ok(Ok(()))
    }

    /// `read_triangles`: `Get_Polygon_Count()` triples of `unsigned short` into `Poly`; clears `DIRTY_PLANES` first
    /// (the plane and surface-type reads are commented out upstream).
    fn read_triangles(&mut self, cload: &mut ChunkLoad<'_>) -> Ww3dErrorType {
        self.set_flag(mesh_flags::DIRTY_PLANES, false);
        for i in 0..self.geometry.poly_count as usize {
            let mut b = [0u8; 6];
            if cload.read_into(&mut b)? != 6 {
                return Ok(Err(ShdLoadFailed));
            }
            self.geometry.poly[i] = [
                u32::from(u16::from_le_bytes([b[0], b[1]])),
                u32::from(u16::from_le_bytes([b[2], b[3]])),
                u32::from(u16::from_le_bytes([b[4], b[5]])),
            ];
        }
        Ok(Ok(()))
    }

    /// `read_vertex_shade_indices`: `Get_Vertex_Count()` `uint32`s into `get_shade_indices(true)`.
    fn read_vertex_shade_indices(&mut self, cload: &mut ChunkLoad<'_>) -> Ww3dErrorType {
        let n = self.geometry.vertex_count as usize;
        if self.geometry.shade_indices.len() < n {
            self.geometry.shade_indices.resize(n, 0);
        }
        for i in 0..n {
            let mut b = [0u8; 4];
            if cload.read_into(&mut b)? != 4 {
                return Ok(Err(ShdLoadFailed));
            }
            self.geometry.shade_indices[i] = u32::from_le_bytes(b);
        }
        Ok(Ok(()))
    }

    /// `read_shader`: `ShdDefManagerClass::Load_Shader(cload, &shddef)`, then `Shader = shddef->Create()`. Upstream
    /// dereferences a NULL `shddef` for an unknown class id; here the shader is left unset.
    fn read_shader<D, F>(&mut self, cload: &mut ChunkLoad<'_>, defs: &ShdDefManagerClass<D>, create_shader: F) -> Ww3dErrorType
    where
        D: ShdDefLoad,
        F: FnOnce(D) -> S,
    {
        let shddef = defs.load_shader(cload)?;
        if let Some(d) = shddef {
            self.shader = Some(create_shader(d));
        }
        Ok(Ok(()))
    }
}

impl<S> Default for ShdSubMeshClass<S> {
    fn default() -> Self {
        Self::new()
    }
}

fn read_vec3(b: &[u8; 12]) -> Vector3 {
    let f = |i: usize| f32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    Vector3::new(f(0), f(4), f(8))
}

/// Unused import guard: `ShdDefClass` is the base of the definitions `ShdDefLoad` loads.
#[allow(dead_code)]
type _ShdDefBase = ShdDefClass;
