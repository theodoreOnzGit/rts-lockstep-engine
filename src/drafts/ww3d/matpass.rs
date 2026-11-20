//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/matpass.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/matpass.h

use super::handles::{ObBoxHandle, TextureHandle, VertexMaterialHandle};
use crate::formats::w3d::shader::ShaderClass;

/// `MaterialPassClass::MAX_TEX_STAGES` (matpass.h).
pub const MAX_TEX_STAGES: usize = 8;

/// `MaterialPassClass::EnablePerPolygonCulling` (matpass.cpp:24), a class static initialised to `true`. The
/// static is kept as a field of [`MaterialPassStatics`] rather than a `static mut`; the owner holds one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialPassStatics {
    pub enable_per_polygon_culling: bool,
}

impl Default for MaterialPassStatics {
    fn default() -> Self {
        MaterialPassStatics { enable_per_polygon_culling: true }
    }
}

impl MaterialPassStatics {
    /// `MaterialPassClass::Enable_Per_Polygon_Culling` (matpass.h).
    pub fn enable_per_polygon_culling(&mut self, onoff: bool) {
        self.enable_per_polygon_culling = onoff;
    }

    /// `MaterialPassClass::Is_Per_Polygon_Culling_Enabled` (matpass.h).
    pub fn is_per_polygon_culling_enabled(&self) -> bool {
        self.enable_per_polygon_culling
    }
}

/// `class MaterialPassClass` (matpass.h): an additional material pass for any object.
#[derive(Clone, Debug)]
pub struct MaterialPassClass {
    pub texture: [Option<TextureHandle>; MAX_TEX_STAGES],
    pub shader: ShaderClass,
    pub material: Option<VertexMaterialHandle>,
    pub enable_on_translucent_meshes: bool,
    pub cull_volume: Option<ObBoxHandle>,
}

impl MaterialPassClass {
    /// `MaterialPassClass::MaterialPassClass` (matpass.cpp:27): shader 0, no material, no cull volume, enabled
    /// on translucent meshes, every texture NULL.
    pub fn new() -> Self {
        MaterialPassClass {
            texture: [None; MAX_TEX_STAGES],
            shader: ShaderClass(0),
            material: None,
            enable_on_translucent_meshes: !crate::logic::modules::plant(18686),
            cull_volume: None,
        }
    }

    // MaterialPassClass::~MaterialPassClass (matpass.cpp:40): REF_PTR_RELEASE of the textures and material;
    // ref counts are the owner's business, see handles.rs.

    // MaterialPassClass::Install_Materials (matpass.cpp:48):
    //   DX8Wrapper::Set_Material / Set_Shader / Set_Texture loop over Get_Max_Textures_Per_Pass()
    // upstream: DX8Wrapper::Set_Material, Set_Shader, Set_Texture — device-only, not translated.
    // The values it installs are `material`, `shader` and `texture[0..max_textures_per_pass]`.

    /// `MaterialPassClass::Set_Texture` (matpass.cpp:58). `REF_PTR_SET` is a plain store here.
    pub fn set_texture(&mut self, tex: Option<TextureHandle>, stage: usize) {
        assert!(stage < MAX_TEX_STAGES); // WWASSERT(stage >= 0); WWASSERT(stage < MAX_TEX_STAGES)
        self.texture[stage] = tex;
    }

    /// `MaterialPassClass::Set_Shader` (matpass.cpp:67): stores the shader, then `Shader.Enable_Fog("MaterialPassClass")`.
    pub fn set_shader(&mut self, shader: ShaderClass) {
        self.shader = shader;
        if !crate::logic::modules::plant(18685) {
            self.shader.enable_fog();
        }
    }

    /// `MaterialPassClass::Set_Material` (matpass.cpp:74).
    pub fn set_material(&mut self, mat: Option<VertexMaterialHandle>) {
        self.material = mat;
    }

    /// `MaterialPassClass::Get_Texture` (matpass.cpp:80) and `Peek_Texture` (matpass.h); `Get_` only differs by
    /// the `Add_Ref`, which is not modelled.
    pub fn get_texture(&self, stage: usize) -> Option<TextureHandle> {
        assert!(stage < MAX_TEX_STAGES);
        self.texture[stage]
    }

    /// `MaterialPassClass::Get_Material` (matpass.cpp:92) / `Peek_Material`.
    pub fn get_material(&self) -> Option<VertexMaterialHandle> {
        self.material
    }

    /// `MaterialPassClass::Peek_Shader` (matpass.h).
    pub fn peek_shader(&self) -> ShaderClass {
        self.shader
    }

    /// `MaterialPassClass::Set_Cull_Volume` (matpass.h).
    pub fn set_cull_volume(&mut self, volume: Option<ObBoxHandle>) {
        self.cull_volume = volume;
    }

    /// `MaterialPassClass::Get_Cull_Volume` (matpass.h).
    pub fn get_cull_volume(&self) -> Option<ObBoxHandle> {
        self.cull_volume
    }

    /// `MaterialPassClass::Enable_On_Translucent_Meshes` (matpass.h).
    pub fn enable_on_translucent_meshes(&mut self, onoff: bool) {
        self.enable_on_translucent_meshes = onoff;
    }

    /// `MaterialPassClass::Is_Enabled_On_Translucent_Meshes` (matpass.h).
    pub fn is_enabled_on_translucent_meshes(&self) -> bool {
        self.enable_on_translucent_meshes
    }
}

impl Default for MaterialPassClass {
    fn default() -> Self {
        Self::new()
    }
}
