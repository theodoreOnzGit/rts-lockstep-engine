//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/matinfo.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/matinfo.h
//!
//! Textures and vertex materials are handles ([`super::handles`]); a lookup by name takes the owner's name
//! function (`Get_Texture_Name`, `VertexMaterialClass::Get_Name`) as a closure. The mesh's material
//! descriptor (`MeshMatDescClass`, `MeshModelClass`) is reached through the [`MeshMatDesc`] contract.

use super::handles::{TextureHandle, VertexMaterialHandle};
use crate::formats::w3d::shader::ShaderClass;

/// `MeshMatDescClass::MAX_TEX_STAGES` (meshmatdesc.h).
pub const MESH_MAX_TEX_STAGES: usize = 2;

/// What `MaterialRemapperClass::Remap_Mesh` and `MaterialCollectorClass::Collect_Materials` ask of a mesh's
/// `MeshMatDescClass` (or `MeshModelClass`, which derives from it), as a compile-time contract.
pub trait MeshMatDesc {
    fn get_pass_count(&self) -> usize;
    fn get_vertex_count(&self) -> usize;
    fn get_polygon_count(&self) -> usize;
    fn has_material_array(&self, pass: usize) -> bool;
    fn peek_material(&self, vert_index: usize, pass: usize) -> Option<VertexMaterialHandle>;
    fn peek_single_material(&self, pass: usize) -> Option<VertexMaterialHandle>;
    fn has_shader_array(&self, pass: usize) -> bool;
    fn get_shader(&self, poly_index: usize, pass: usize) -> ShaderClass;
    fn get_single_shader(&self, pass: usize) -> ShaderClass;
    fn has_texture_array(&self, pass: usize, stage: usize) -> bool;
    fn peek_texture(&self, poly_index: usize, pass: usize, stage: usize) -> Option<TextureHandle>;
    fn peek_single_texture(&self, pass: usize, stage: usize) -> Option<TextureHandle>;
    fn set_material(&mut self, vert_index: usize, vmat: Option<VertexMaterialHandle>, pass: usize);
    fn set_single_material(&mut self, vmat: Option<VertexMaterialHandle>, pass: usize);
    fn set_texture(&mut self, poly_index: usize, tex: Option<TextureHandle>, pass: usize, stage: usize);
    fn set_single_texture(&mut self, tex: Option<TextureHandle>, pass: usize, stage: usize);
}

/// `class MaterialInfoClass` (matinfo.h): holds the references to every material a mesh uses. Vertex material
/// slots may be NULL (`Add_Vertex_Material` accepts NULL), textures may not.
#[derive(Clone, Debug, Default)]
pub struct MaterialInfoClass {
    vertex_materials: Vec<Option<VertexMaterialHandle>>,
    textures: Vec<TextureHandle>,
}

impl MaterialInfoClass {
    /// `MaterialInfoClass::MaterialInfoClass()` (matinfo.cpp:34).
    pub fn new() -> Self {
        Self::default()
    }

    /// `MaterialInfoClass::MaterialInfoClass(const MaterialInfoClass &src)` (matinfo.cpp:38) / `Clone`
    /// (matinfo.cpp:57): upstream **clones every vertex material** (`src.VertexMaterials[mi]->Clone()`) and
    /// shares the textures (`Add_Ref`). `clone_vertex_material` is the owner's `VertexMaterialClass::Clone`.
    pub fn clone_from_src<F: FnMut(VertexMaterialHandle) -> VertexMaterialHandle>(src: &MaterialInfoClass, mut clone_vertex_material: F) -> Self {
        let mut me = MaterialInfoClass::new();
        for mi in 0..src.vertex_materials.len() {
            // upstream dereferences the slot: a NULL slot is a crash there
            let vmat = clone_vertex_material(src.vertex_materials[mi].expect("matinfo.cpp:42 clones a NULL vertex material"));
            me.vertex_materials.push(Some(vmat));
        }
        for ti in 0..src.textures.len() {
            me.textures.push(src.textures[ti]); // tex->Add_Ref()
        }
        me
    }

    /// `Reset` (matinfo.h): `Free()`.
    pub fn reset(&mut self) {
        self.free();
    }

    /// `Vertex_Material_Count` (matinfo.h).
    pub fn vertex_material_count(&self) -> usize {
        self.vertex_materials.len()
    }

    /// `Texture_Count` (matinfo.h).
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    /// `MaterialInfoClass::Add_Vertex_Material` (matinfo.h).
    pub fn add_vertex_material(&mut self, vmat: Option<VertexMaterialHandle>) -> usize {
        let index = self.vertex_materials.len();
        self.vertex_materials.push(vmat);
        index
    }

    /// `MaterialInfoClass::Add_Texture` (matinfo.cpp:68).
    pub fn add_texture(&mut self, tex: TextureHandle) -> usize {
        let index = self.textures.len();
        self.textures.push(tex);
        index
    }

    /// `MaterialInfoClass::Get_Vertex_Material_Index` (matinfo.h): case-insensitive (`stricmp`), -1 if absent.
    /// Upstream dereferences each slot for its name; a NULL slot is a crash there, here it is skipped.
    pub fn get_vertex_material_index<F: Fn(VertexMaterialHandle) -> String>(&self, name: &str, vmat_name: F) -> i32 {
        for (i, v) in self.vertex_materials.iter().enumerate() {
            if let Some(v) = v {
                if name.eq_ignore_ascii_case(&vmat_name(*v)) {
                    return i as i32;
                }
            }
        }
        -1
    }

    /// `MaterialInfoClass::Get_Texture_Index` (matinfo.cpp:80): case-insensitive, -1 if absent.
    pub fn get_texture_index<F: Fn(TextureHandle) -> String>(&self, name: &str, texture_name: F) -> i32 {
        for (i, t) in self.textures.iter().enumerate() {
            if name.eq_ignore_ascii_case(&texture_name(*t)) {
                return i as i32;
            }
        }
        -1
    }

    /// `Get_Vertex_Material(index)` / `Peek_Vertex_Material(index)` (matinfo.h).
    pub fn peek_vertex_material(&self, index: usize) -> Option<VertexMaterialHandle> {
        assert!(index < self.vertex_materials.len());
        self.vertex_materials[index]
    }

    /// `Get_Vertex_Material(name)` / `Peek_Vertex_Material(name)` (matinfo.h).
    pub fn peek_vertex_material_by_name<F: Fn(VertexMaterialHandle) -> String>(&self, name: &str, vmat_name: F) -> Option<VertexMaterialHandle> {
        let index = self.get_vertex_material_index(name, vmat_name);
        if index == -1 {
            None
        } else {
            self.peek_vertex_material(index as usize)
        }
    }

    /// `Replace_Material` (matinfo.h): `REF_PTR_SET(VertexMaterials[index], newMaterial)`.
    pub fn replace_material(&mut self, index: usize, new_material: Option<VertexMaterialHandle>) {
        self.vertex_materials[index] = new_material;
    }

    /// `Reset_Texture_Mappers` (matinfo.h): `VertexMaterials[i]->Reset_Mappers()` for each slot (a NULL slot
    /// is a crash upstream). `f` is the owner's `Reset_Mappers`.
    pub fn reset_texture_mappers<F: FnMut(VertexMaterialHandle)>(&self, mut f: F) {
        for v in self.vertex_materials.iter().flatten() {
            f(*v);
        }
    }

    /// `Make_Vertex_Materials_Unique` (matinfo.h): `VertexMaterials[i]->Make_Unique()` for each.
    pub fn make_vertex_materials_unique<F: FnMut(VertexMaterialHandle)>(&self, mut f: F) {
        for v in self.vertex_materials.iter().flatten() {
            f(*v);
        }
    }

    /// `Has_Time_Variant_Texture_Mappers` (matinfo.h): any `Are_Mappers_Time_Variant()`.
    pub fn has_time_variant_texture_mappers<F: Fn(VertexMaterialHandle) -> bool>(&self, are_mappers_time_variant: F) -> bool {
        for v in self.vertex_materials.iter().flatten() {
            if are_mappers_time_variant(*v) {
                return true;
            }
        }
        false
    }

    /// `Get_Texture(index)` / `Peek_Texture(index)` (matinfo.cpp:92, matinfo.h).
    pub fn peek_texture(&self, index: usize) -> TextureHandle {
        assert!(index < self.textures.len());
        self.textures[index]
    }

    /// `Get_Texture(name)` / `Peek_Texture(name)` (matinfo.h).
    pub fn peek_texture_by_name<F: Fn(TextureHandle) -> String>(&self, name: &str, texture_name: F) -> Option<TextureHandle> {
        let index = self.get_texture_index(name, texture_name);
        if index == -1 {
            None
        } else {
            Some(self.peek_texture(index as usize))
        }
    }

    /// `Replace_Texture` (matinfo.h).
    pub fn replace_texture(&mut self, index: usize, new_texture: TextureHandle) {
        self.textures[index] = new_texture;
    }

    /// `MaterialInfoClass::Free` (matinfo.cpp:123): releases and deletes everything. (The commented-out
    /// `Set_Texture_Reduction_Factor` / `Process_Texture_Reduction` above it are dead upstream.)
    fn free(&mut self) {
        self.vertex_materials.clear();
        self.textures.clear();
    }
}

/// `class MaterialRemapperClass` (matinfo.h): maps pointers into a source material info to the same slots of an
/// identical destination, remembering the last hit because meshes are sorted by material.
#[derive(Clone, Debug)]
pub struct MaterialRemapperClass {
    texture_remaps: Vec<(TextureHandle, TextureHandle)>,
    vertex_material_remaps: Vec<(Option<VertexMaterialHandle>, Option<VertexMaterialHandle>)>,
    src_vertex_material_count: usize,
    src_texture_count: usize,
    last_src_vmat: Option<VertexMaterialHandle>,
    last_dest_vmat: Option<VertexMaterialHandle>,
    last_src_tex: Option<TextureHandle>,
    last_dest_tex: Option<TextureHandle>,
}

impl MaterialRemapperClass {
    /// `MaterialRemapperClass::MaterialRemapperClass` (matinfo.cpp:150): both infos must hold the same numbers
    /// of textures and vertex materials (WWASSERT).
    pub fn new(src: &MaterialInfoClass, dest: &MaterialInfoClass) -> Self {
        assert!(src.texture_count() == dest.texture_count());
        assert!(src.vertex_material_count() == dest.vertex_material_count());

        let mut vertex_material_remaps = Vec::new();
        if src.vertex_material_count() > 0 {
            for i in 0..src.vertex_material_count() {
                vertex_material_remaps.push((src.peek_vertex_material(i), dest.peek_vertex_material(i)));
            }
        }
        let mut texture_remaps = Vec::new();
        if src.texture_count() > 0 {
            for i in 0..src.texture_count() {
                texture_remaps.push((src.peek_texture(i), dest.peek_texture(i)));
            }
        }
        MaterialRemapperClass {
            texture_remaps,
            vertex_material_remaps,
            src_vertex_material_count: src.vertex_material_count(),
            src_texture_count: src.texture_count(),
            last_src_vmat: None,
            last_dest_vmat: None,
            last_src_tex: None,
            last_dest_tex: None,
        }
    }

    /// `MaterialRemapperClass::Remap_Texture` (matinfo.cpp:195): NULL maps to NULL; a texture not found is
    /// `WWASSERT(0)` and NULL.
    pub fn remap_texture(&mut self, src: Option<TextureHandle>) -> Option<TextureHandle> {
        let src_h = src?;
        if src == self.last_src_tex {
            return self.last_dest_tex;
        }
        for i in 0..self.texture_remaps.len() {
            if self.texture_remaps[i].0 == src_h {
                self.last_src_tex = src;
                self.last_dest_tex = Some(self.texture_remaps[i].1);
                return Some(self.texture_remaps[i].1);
            }
        }
        debug_assert!(false); // uh-oh didn't find the texture
        None
    }

    /// `MaterialRemapperClass::Remap_Vertex_Material` (matinfo.cpp:215).
    pub fn remap_vertex_material(&mut self, src: Option<VertexMaterialHandle>) -> Option<VertexMaterialHandle> {
        let src_h = src?;
        if src == self.last_src_vmat {
            return self.last_dest_vmat;
        }
        for i in 0..self.vertex_material_remaps.len() {
            if self.vertex_material_remaps[i].0 == Some(src_h) {
                self.last_src_vmat = src;
                self.last_dest_vmat = self.vertex_material_remaps[i].1;
                return self.vertex_material_remaps[i].1;
            }
        }
        debug_assert!(false); // uh-oh didn't find the material
        None
    }

    /// `MaterialRemapperClass::Remap_Mesh` (matinfo.cpp:236): remaps the vertex materials (per vertex or single,
    /// per pass) when the source info has at least one, then the textures (per polygon or single, per pass and
    /// stage) when it has at least one.
    pub fn remap_mesh<S: MeshMatDesc, D: MeshMatDesc>(&mut self, srcmeshmatdesc: &S, destmeshmatdesc: &mut D) {
        // Remap the vertex materials if there is at least one of them
        if self.src_vertex_material_count >= 1 {
            for pass in 0..srcmeshmatdesc.get_pass_count() {
                if srcmeshmatdesc.has_material_array(pass) {
                    for vert_index in 0..srcmeshmatdesc.get_vertex_count() {
                        let src = srcmeshmatdesc.peek_material(vert_index, pass);
                        let r = self.remap_vertex_material(src);
                        destmeshmatdesc.set_material(vert_index, r, pass);
                    }
                } else {
                    let src = srcmeshmatdesc.peek_single_material(pass);
                    let r = self.remap_vertex_material(src);
                    destmeshmatdesc.set_single_material(r, pass);
                }
            }
        }

        // Remap the textures if there is at least one of them
        if self.src_texture_count >= 1 {
            for pass in 0..srcmeshmatdesc.get_pass_count() {
                for stage in 0..MESH_MAX_TEX_STAGES {
                    if srcmeshmatdesc.has_texture_array(pass, stage) {
                        for poly_index in 0..srcmeshmatdesc.get_polygon_count() {
                            let src = srcmeshmatdesc.peek_texture(poly_index, pass, stage);
                            let r = self.remap_texture(src);
                            destmeshmatdesc.set_texture(poly_index, r, pass, stage);
                        }
                    } else {
                        let src = srcmeshmatdesc.peek_single_texture(pass, stage);
                        let r = self.remap_texture(src);
                        destmeshmatdesc.set_single_texture(r, pass, stage);
                    }
                }
            }
        }
    }
}

/// `class MaterialCollectorClass` (matinfo.h): the unique shaders, vertex materials and textures of a mesh.
/// Uniqueness is by identity (handle equality), not contents.
#[derive(Clone, Debug)]
pub struct MaterialCollectorClass {
    shaders: Vec<ShaderClass>,
    vertex_materials: Vec<VertexMaterialHandle>,
    textures: Vec<TextureHandle>,
    last_shader: ShaderClass,
    last_material: Option<VertexMaterialHandle>,
    last_texture: Option<TextureHandle>,
}

impl MaterialCollectorClass {
    /// `MaterialCollectorClass::MaterialCollectorClass` (matinfo.cpp:308): `LastShader = ShaderClass(0xFFFFFFFF)`.
    pub fn new() -> Self {
        MaterialCollectorClass {
            shaders: Vec::new(),
            vertex_materials: Vec::new(),
            textures: Vec::new(),
            last_shader: ShaderClass(0xFFFF_FFFF),
            last_material: None,
            last_texture: None,
        }
    }

    /// `MaterialCollectorClass::Collect_Materials` (matinfo.cpp:320): per pass, the vertex materials (per
    /// vertex or the single one), the shaders (per polygon or the single one) and the textures (per stage, per
    /// polygon or the single one).
    pub fn collect_materials<M: MeshMatDesc>(&mut self, mesh: &M) {
        for pass in 0..mesh.get_pass_count() {
            // Vertex materials (either single or per vertex)
            if mesh.has_material_array(pass) {
                for vert_index in 0..mesh.get_vertex_count() {
                    let mat = mesh.peek_material(vert_index, pass);
                    self.add_vertex_material(mat);
                }
            } else {
                let mat = mesh.peek_single_material(pass);
                self.add_vertex_material(mat);
            }

            // Shaders (single or per poly...)
            if mesh.has_shader_array(pass) {
                for poly_index in 0..mesh.get_polygon_count() {
                    self.add_shader(mesh.get_shader(poly_index, pass));
                }
            } else {
                let sh = mesh.get_single_shader(pass);
                self.add_shader(sh);
            }

            // Textures per pass, per stage (either array or single...)
            for stage in 0..MESH_MAX_TEX_STAGES {
                if mesh.has_texture_array(pass, stage) {
                    for poly_index in 0..mesh.get_polygon_count() {
                        let tex = mesh.peek_texture(poly_index, pass, stage);
                        self.add_texture(tex);
                    }
                } else {
                    let tex = mesh.peek_single_texture(pass, stage);
                    self.add_texture(tex);
                }
            }
        }
    }

    /// `MaterialCollectorClass::Reset` (matinfo.cpp:366). Note upstream does not reset the `Last*` caches.
    pub fn reset(&mut self) {
        self.textures.clear();
        self.vertex_materials.clear();
        self.shaders.clear();
    }

    /// `MaterialCollectorClass::Add_Texture` (matinfo.cpp:381).
    pub fn add_texture(&mut self, tex: Option<TextureHandle>) {
        let t = match tex {
            Some(t) => t,
            None => return,
        };
        if tex == self.last_texture {
            return;
        }
        if self.find_texture(t) != -1 {
            return;
        }
        self.textures.push(t);
        self.last_texture = tex;
    }

    /// `MaterialCollectorClass::Add_Shader` (matinfo.cpp:397).
    pub fn add_shader(&mut self, shader: ShaderClass) {
        if shader == self.last_shader {
            return;
        }
        if self.find_shader(&shader) != -1 {
            return;
        }
        self.shaders.push(shader);
        self.last_shader = shader;
    }

    /// `MaterialCollectorClass::Add_Vertex_Material` (matinfo.cpp:410).
    pub fn add_vertex_material(&mut self, vmat: Option<VertexMaterialHandle>) {
        let v = match vmat {
            Some(v) => v,
            None => return,
        };
        if vmat == self.last_material {
            return;
        }
        if self.find_vertex_material(v) != -1 {
            return;
        }
        self.vertex_materials.push(v);
        self.last_material = vmat;
    }

    /// `Get_Shader_Count` (matinfo.cpp:426).
    pub fn get_shader_count(&self) -> usize {
        self.shaders.len()
    }

    /// `Get_Vertex_Material_Count` (matinfo.cpp:431).
    pub fn get_vertex_material_count(&self) -> usize {
        self.vertex_materials.len()
    }

    /// `Get_Texture_Count` (matinfo.cpp:436).
    pub fn get_texture_count(&self) -> usize {
        self.textures.len()
    }

    /// `Peek_Shader` (matinfo.cpp:441).
    pub fn peek_shader(&self, i: usize) -> ShaderClass {
        self.shaders[i]
    }

    /// `Peek_Texture` (matinfo.cpp:446).
    pub fn peek_texture(&self, i: usize) -> TextureHandle {
        self.textures[i]
    }

    /// `Peek_Vertex_Material` (matinfo.cpp:451).
    pub fn peek_vertex_material(&self, i: usize) -> VertexMaterialHandle {
        self.vertex_materials[i]
    }

    /// `Find_Shader` (matinfo.cpp:456).
    pub fn find_shader(&self, shader: &ShaderClass) -> i32 {
        for (si, s) in self.shaders.iter().enumerate() {
            if s == shader {
                return si as i32;
            }
        }
        -1
    }

    /// `Find_Texture` (matinfo.cpp:467).
    pub fn find_texture(&self, tex: TextureHandle) -> i32 {
        for (ti, t) in self.textures.iter().enumerate() {
            if *t == tex {
                return ti as i32;
            }
        }
        -1
    }

    /// `Find_Vertex_Material` (matinfo.cpp:478).
    pub fn find_vertex_material(&self, mat: VertexMaterialHandle) -> i32 {
        for (vi, v) in self.vertex_materials.iter().enumerate() {
            if *v == mat {
                return vi as i32;
            }
        }
        -1
    }
}

impl Default for MaterialCollectorClass {
    fn default() -> Self {
        Self::new()
    }
}
