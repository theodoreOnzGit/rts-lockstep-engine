//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdcubemap.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdcubemap.h
//!
//! The definition (`ShdCubeMapDefClass`): its editable parameters (the `NAMED_*_PARAM` registrations are the level editor's and are
//! not translated), `Load`, and the requirements the renderer asks of it. Not translated: `Save`
//! (needs `ChunkSaveClass`), `Is_Valid_Config` (always true), `Create` (builds the device shader: see the `shd*`
//! shader files, which take the definition by reference).

use super::shdchunk::*;
use super::shddef::{ShdDefClass, ShdVersion};
use crate::formats::w3d::W3dResult;
use crate::ww::chunkio::ChunkLoad;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;

/// `CHUNKID_VARIABLES` of this shader (shdcubemap.cpp).
pub const CHUNKID_VARIABLES: u32 = 0x1649_0470;
/// `VARID_TEXTURE_NAME`.
pub const VARID_TEXTURE_NAME: u32 = 0x00;
/// `VARID_AMBIENT_COLOR`.
pub const VARID_AMBIENT_COLOR: u32 = 0x01;
/// `VARID_DIFFUSE_COLOR`.
pub const VARID_DIFFUSE_COLOR: u32 = 0x02;
/// `VARID_SPECULAR_COLOR`.
pub const VARID_SPECULAR_COLOR: u32 = 0x03;

/// `class ShdCubeMapDefClass` (shdcubemap.h).
#[derive(Clone, Debug)]
pub struct ShdCubeMapDefClass {
    pub base: ShdDefClass,
    pub texture_name: String,
    pub ambient: Vector3,
    pub diffuse: Vector3,
    pub specular: Vector3,
}

impl ShdCubeMapDefClass {
    /// `ShdCubeMapDefClass::ShdCubeMapDefClass()`: the `SHDDEF_CLASSID_CUBEMAP` base and the default parameter values.
    pub fn new() -> Self {
        ShdCubeMapDefClass {
            base: ShdDefClass::new(SHDDEF_CLASSID_CUBEMAP),
            texture_name: String::new(),
            ambient: Vector3::new(1.0, 1.0, 1.0),
            diffuse: Vector3::new(1.0, 1.0, 1.0),
            specular: Vector3::new(0.0, 0.0, 0.0),
        }
    }

    /// The shader's name in the editor / `REGISTER_SHDDEF`.
    pub const NAME: &'static str = "Cube Map";

    /// `Requires_Normals`.
    pub fn requires_normals(&self) -> bool {
        true
    }

    /// `Requires_Tangent_Space_Vectors`.
    pub fn requires_tangent_space_vectors(&self) -> bool {
        false
    }

    /// `Requires_Sorting`.
    pub fn requires_sorting(&self) -> bool {
        false
    }

    /// `Static_Sort_Index`.
    pub fn static_sort_index(&self) -> i32 {
        0
    }

    /// `Is_Valid_Config`: always true.
    pub fn is_valid_config(&self) -> bool {
        true
    }

    /// `Get_TextureName`.
    pub fn get_texture_name(&self) -> &str {
        &self.texture_name
    }

    /// `Get_Ambient`.
    pub fn get_ambient(&self) -> &Vector3 {
        &self.ambient
    }

    /// `Get_Diffuse`.
    pub fn get_diffuse(&self) -> &Vector3 {
        &self.diffuse
    }

    /// `Get_Specular`.
    pub fn get_specular(&self) -> &Vector3 {
        &self.specular
    }

    /// `ShdCubeMapDefClass::Load` (shdcubemap.cpp): the base class's chunk, then one `CHUNKID_VARIABLES` chunk of micro-chunks;
    /// unknown chunks and micro-chunks are skipped. Always true.
    pub fn load(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        self.base.load(cload)?;
        while cload.open_chunk()? {
            if cload.cur_chunk_id()? == CHUNKID_VARIABLES {
                while cload.open_micro_chunk()? {
                    match cload.cur_micro_chunk_id() {
                        VARID_TEXTURE_NAME => self.texture_name = read_micro_wwstring(cload)?,
                        VARID_AMBIENT_COLOR => { read_micro_vector3(cload, &mut self.ambient)?; }
                        VARID_DIFFUSE_COLOR => { read_micro_vector3(cload, &mut self.diffuse)?; }
                        VARID_SPECULAR_COLOR => { read_micro_vector3(cload, &mut self.specular)?; }
                        _ => {}
                    }
                    cload.close_micro_chunk()?;
                }
            }
            cload.close_chunk()?;
        }
        Ok(true)
    }
}

impl Default for ShdCubeMapDefClass {
    fn default() -> Self {
        Self::new()
    }
}

// ---- the instance class (added in the second pass; the definition above was drafted first) ----

use super::handles::TextureHandle;
use crate::ww::vector4::Vector4;

/// `class Shd6CubeMapClass` (shdcubemap.h): the version-6 (fixed function) implementation. It copies its colours out of the definition
/// at construction and fills a `D3DMATERIAL8`. Translated: construction, `Get_Pass_Count` (1), `Get_Texture_Count` (1),
/// `Peek_Texture`, `Use_HW_Vertex_Processing` (true), `Get_Vertex_Stream_Count` (1), `Get_Vertex_Size`,
/// `Copy_Vertex_Stream`. Device-only, not translated: `Init`/`Shutdown` (empty), `Apply_Shared`, `Apply_Instance`
/// (`DX8Wrapper::Set_DX8_Texture_Stage_State`, `Set_Vertex_Shader`, `Set_DX8_Render_State`, `Set_DX8_Material`, `Set_Texture`).
#[derive(Clone, Debug)]
pub struct Shd6CubeMapClass {
    /// `ShdInterfaceClass(def, SHDDEF_CLASSID_CUBEMAP)`.
    pub class_id: u32,
    /// `Texture`.
    pub texture: Option<TextureHandle>,
    /// `Ambient` (alpha 1.0).
    pub ambient: Vector4,
    /// `Diffuse` (alpha 1.0).
    pub diffuse: Vector4,
    /// `Specular` (alpha 1.0).
    pub specular: Vector4,
    /// `Material`: the `D3DMATERIAL8` colours the constructor fills (the rest is zeroed).
    pub material: D3DMaterial8Colors,
}

impl Shd6CubeMapClass {
    /// `Shd6CubeMapClass::Shd6CubeMapClass(def)`. `get_texture(name, cube_map)` is `WW3DAssetManager::Get_Texture(name, MIP_LEVELS_ALL,
    /// WW3D_FORMAT_UNKNOWN, true, TextureBaseClass::TEX_CUBEMAP)` (the cube-map flag is passed as `true`).
    pub fn new<F: FnMut(&str, bool) -> TextureHandle>(def: &ShdCubeMapDefClass, mut get_texture: F) -> Self {
        let texture = Some(get_texture(def.get_texture_name(), true));
        let a = *def.get_ambient();
        let d = *def.get_diffuse();
        let s = *def.get_specular();
        Shd6CubeMapClass {
            class_id: SHDDEF_CLASSID_CUBEMAP,
            texture,
            ambient: Vector4 { x: a.x, y: a.y, z: a.z, w: 1.0 },
            diffuse: Vector4 { x: d.x, y: d.y, z: d.z, w: 1.0 },
            specular: Vector4 { x: s.x, y: s.y, z: s.z, w: 1.0 },
            // Material->Ambient/Diffuse/Specular = a/d/s, Power = 20
            material: D3DMaterial8Colors { ambient: [a.x, a.y, a.z], diffuse: [d.x, d.y, d.z], specular: [s.x, s.y, s.z], power: 20.0 },
        }
    }

    /// `Get_Pass_Count`.
    pub fn get_pass_count(&self) -> i32 {
        1
    }

    /// `Get_Texture_Count`.
    pub fn get_texture_count(&self) -> usize {
        1
    }

    /// `Peek_Texture(idx)`.
    pub fn peek_texture(&self, _idx: usize) -> Option<TextureHandle> {
        None
    }

    /// `Use_HW_Vertex_Processing`.
    pub fn use_hw_vertex_processing(&self) -> bool {
        true
    }

    /// `Get_Vertex_Stream_Count`.
    pub fn get_vertex_stream_count(&self) -> u32 {
        1
    }

    /// `Get_Vertex_Size(stream)`.
    pub fn get_vertex_size(&self, _stream: u32) -> usize {
        std::mem::size_of::<VertexFormatXYZNDCUBEMAP>()
    }

    /// `Copy_Vertex_Stream(stream, dest_buffer, vss, vertex_count)`: see `shdchunk::copy_vertex_stream_cubemap`.
    pub fn copy_vertex_stream(&self, vss: &VertexStreamStruct, vertex_count: usize) -> Vec<VertexFormatXYZNDCUBEMAP> {
        copy_vertex_stream_cubemap(vss, vertex_count)
    }
}
