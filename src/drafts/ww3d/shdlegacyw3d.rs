//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdlegacyw3d.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdlegacyw3d.h
//!
//! The legacy-W3D shader wraps an ordinary W3D material (up to 4 passes of shader + vertex material + 2 textures)
//! so old content can ride the shader pipeline. Reused from the port: `W3dShaderStruct` / `W3dVertexMaterialStruct`
//! (`formats::w3d::w3d_file`, `from_bytes`) and `convert_shader` (`formats::w3d::mesh`). Not translated: `Save` /
//! `Save_Variables` (need `ChunkSaveClass`), `Init`/`Shutdown` (empty), `Apply_Shared`/`Apply_Instance`
//! (`Set_Shader`, `Set_Material`, `Set_Texture`, texture-stage states: device), and the `VertexMaterialClass` parsing
//! (`Parse_W3dVertexMaterialStruct`, `Parse_Mapping_Args`) which the instance leaves to the caller's closure.

use super::handles::{TextureHandle, VertexMaterialHandle};
use super::shdchunk::*;
use super::shddef::ShdDefClass;
use crate::formats::w3d::shader::ShaderClass;
use crate::formats::w3d::w3d_file::{W3dShaderStruct, W3dVertexMaterialStruct};
use crate::formats::w3d::W3dResult;
use crate::ww::chunkio::ChunkLoad;

/// `MeshMatDescClass::MAX_PASSES` / `MAX_TEX_STAGES`.
pub const MAX_PASSES: usize = 4;
pub const MAX_TEX_STAGES: usize = 2;

/// `SHDLEGACY_CHUNKID_VARIABLES` and the texture-name / mapper-args chunks that follow it consecutively.
pub const SHDLEGACY_CHUNKID_VARIABLES: u32 = 0x1107_0958;
pub const SHDLEGACY_CHUNKID_TEXTURE_NAME00: u32 = SHDLEGACY_CHUNKID_VARIABLES + 1;
pub const SHDLEGACY_CHUNKID_TEXTURE_NAME31: u32 = SHDLEGACY_CHUNKID_VARIABLES + 8;
pub const SHDLEGACY_CHUNKID_MAPPERARGS00: u32 = SHDLEGACY_CHUNKID_VARIABLES + 9;
pub const SHDLEGACY_CHUNKID_MAPPERARGS31: u32 = SHDLEGACY_CHUNKID_VARIABLES + 16;

/// Micro-chunk ids (`VARID_*`, counted from `VARID_PASS_COUNT = 0`).
pub const VARID_PASS_COUNT: u32 = 0;
pub const VARID_SHADER0: u32 = 1; // ..3
pub const VARID_MATERIAL0: u32 = 5; // ..8
pub const VARID_TEXTURE_ATTRIBUTES00: u32 = 9; // ..16
pub const VARID_MAPCHANNEL00: u32 = 17; // ..24
pub const VARID_AMBIENT_COLOR: u32 = 25;
pub const VARID_DIFFUSE_COLOR: u32 = 26;

/// `class ShdLegacyW3DDefClass` (shdlegacyw3d.h).
#[derive(Clone, Debug)]
pub struct ShdLegacyW3DDefClass {
    pub base: ShdDefClass,
    pub pass_count: i32,
    pub shaders: [W3dShaderStruct; MAX_PASSES],
    pub materials: [W3dVertexMaterialStruct; MAX_PASSES],
    pub texture_names: [[String; MAX_TEX_STAGES]; MAX_PASSES],
    pub mapper_args: [[String; MAX_TEX_STAGES]; MAX_PASSES],
    pub texture_attributes: [[i32; MAX_TEX_STAGES]; MAX_PASSES],
    pub map_channel: [[i32; MAX_TEX_STAGES]; MAX_PASSES],
}

impl ShdLegacyW3DDefClass {
    /// `ShdLegacyW3DDefClass::ShdLegacyW3DDefClass()` (shdlegacyw3d.cpp:75): everything zero, `PassCount` 0.
    pub fn new() -> Self {
        ShdLegacyW3DDefClass {
            base: ShdDefClass::new(SHDDEF_CLASSID_LEGACYW3D),
            pass_count: 0,
            shaders: [W3dShaderStruct::from_bytes(&[0; 16]); MAX_PASSES],
            materials: [W3dVertexMaterialStruct::from_bytes(&[0; 32]); MAX_PASSES],
            texture_names: Default::default(),
            mapper_args: Default::default(),
            texture_attributes: [[0; MAX_TEX_STAGES]; MAX_PASSES],
            map_channel: [[0; MAX_TEX_STAGES]; MAX_PASSES],
        }
    }

    // The copy constructor (shdlegacyw3d.cpp:89) copies `PassCount` and the texture names only (shaders, materials,
    // attributes and channels are zeroed, mapper args are NOT copied): reproduced by [`Self::clone_def`].
    /// `ShdLegacyW3DDefClass::ShdLegacyW3DDefClass(const &)`.
    pub fn clone_def(that: &ShdLegacyW3DDefClass) -> Self {
        let mut d = Self::new();
        d.base = that.base.clone();
        d.pass_count = that.pass_count;
        for pass in 0..(that.pass_count.max(0) as usize).min(MAX_PASSES) {
            for stage in 0..MAX_TEX_STAGES {
                d.texture_names[pass][stage] = that.texture_names[pass][stage].clone();
            }
        }
        d
    }

    pub fn requires_normals(&self) -> bool {
        true
    }
    pub fn requires_tangent_space_vectors(&self) -> bool {
        false
    }
    pub fn requires_sorting(&self) -> bool {
        false
    }
    pub fn static_sort_index(&self) -> i32 {
        0
    }
    pub fn is_valid_config(&self) -> bool {
        true
    }

    pub fn get_pass_count(&self) -> i32 {
        self.pass_count
    }
    pub fn get_shader(&self, pass: usize) -> &W3dShaderStruct {
        &self.shaders[pass]
    }
    pub fn get_material(&self, pass: usize) -> &W3dVertexMaterialStruct {
        &self.materials[pass]
    }
    pub fn get_texture_name(&self, pass: usize, stage: usize) -> &str {
        &self.texture_names[pass][stage]
    }
    pub fn get_mapper_args(&self, pass: usize, stage: usize) -> &str {
        &self.mapper_args[pass][stage]
    }
    pub fn get_map_channel(&self, pass: usize, stage: usize) -> i32 {
        self.map_channel[pass][stage]
    }

    /// `Uses_UV_Channel(i)` (shdlegacyw3d.cpp:121): does any stage of any used pass map from channel `i`.
    pub fn uses_uv_channel(&self, i: i32) -> bool {
        for pass in 0..self.pass_count.max(0) as usize {
            for stage in 0..MAX_TEX_STAGES {
                if self.map_channel[pass][stage] == i {
                    return true;
                }
            }
        }
        false
    }

    fn read_string(cload: &mut ChunkLoad<'_>) -> W3dResult<String> {
        let len = cload.cur_chunk_length()?;
        let mut buf = vec![0u8; len as usize];
        cload.read(&mut buf, len)?;
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
    }

    /// `ShdLegacyW3DDefClass::Load` (shdlegacyw3d.cpp:178): the base chunk, then variables / texture-name /
    /// mapper-arg chunks. The pass/stage of a name chunk is `(id - first) / 2`, `(id - first) % 2`.
    pub fn load(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        self.base.load(cload)?;
        while cload.open_chunk()? {
            let id = cload.cur_chunk_id()?;
            if id == SHDLEGACY_CHUNKID_VARIABLES {
                self.load_variables(cload)?;
            } else if (SHDLEGACY_CHUNKID_TEXTURE_NAME00..=SHDLEGACY_CHUNKID_TEXTURE_NAME31).contains(&id) {
                let offset = (id - SHDLEGACY_CHUNKID_TEXTURE_NAME00) as usize;
                self.texture_names[offset / 2][offset % 2] = Self::read_string(cload)?;
            } else if (SHDLEGACY_CHUNKID_MAPPERARGS00..=SHDLEGACY_CHUNKID_MAPPERARGS31).contains(&id) {
                let offset = (id - SHDLEGACY_CHUNKID_MAPPERARGS00) as usize;
                self.mapper_args[offset / 2][offset % 2] = Self::read_string(cload)?;
            }
            cload.close_chunk()?;
        }
        Ok(true)
    }

    /// `ShdLegacyW3DDefClass::Load_Variables` (shdlegacyw3d.cpp:296).
    fn load_variables(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        while cload.open_micro_chunk()? {
            let id = cload.cur_micro_chunk_id();
            if id == VARID_PASS_COUNT {
                let mut b = [0u8; 4];
                cload.read_into(&mut b)?;
                self.pass_count = i32::from_le_bytes(b);
            } else if (VARID_SHADER0..VARID_SHADER0 + 4).contains(&id) {
                let mut b = [0u8; 16];
                cload.read_into(&mut b)?;
                self.shaders[(id - VARID_SHADER0) as usize] = W3dShaderStruct::from_bytes(&b);
            } else if (VARID_MATERIAL0..VARID_MATERIAL0 + 4).contains(&id) {
                let mut b = [0u8; 32];
                cload.read_into(&mut b)?;
                self.materials[(id - VARID_MATERIAL0) as usize] = W3dVertexMaterialStruct::from_bytes(&b);
            } else if (VARID_TEXTURE_ATTRIBUTES00..VARID_TEXTURE_ATTRIBUTES00 + 8).contains(&id) {
                let offset = (id - VARID_TEXTURE_ATTRIBUTES00) as usize;
                let mut b = [0u8; 4];
                cload.read_into(&mut b)?;
                self.texture_attributes[offset / 2][offset % 2] = i32::from_le_bytes(b);
            } else if (VARID_MAPCHANNEL00..VARID_MAPCHANNEL00 + 8).contains(&id) {
                let offset = (id - VARID_MAPCHANNEL00) as usize;
                let mut b = [0u8; 4];
                cload.read_into(&mut b)?;
                self.map_channel[offset / 2][offset % 2] = i32::from_le_bytes(b);
            }
            cload.close_micro_chunk()?;
        }
        Ok(true)
    }
}

impl Default for ShdLegacyW3DDefClass {
    fn default() -> Self {
        Self::new()
    }
}

/// `class Shd6LegacyW3DClass` (shdlegacyw3d.h): the instance, one shader / material / two textures per pass.
#[derive(Clone, Debug)]
pub struct Shd6LegacyW3DClass {
    pub pass_count: i32,
    pub shaders: [ShaderClass; MAX_PASSES],
    pub materials: [Option<VertexMaterialHandle>; MAX_PASSES],
    pub textures: [[Option<TextureHandle>; MAX_TEX_STAGES]; MAX_PASSES],
    pub class_id: u32,
    /// `FVF`: the vertex format code (set by the mesh builder; `Get_Vertex_Size` is its size).
    pub fvf: u32,
}

impl Shd6LegacyW3DClass {
    /// `Shd6LegacyW3DClass::Shd6LegacyW3DClass(def)` (shdlegacyw3d.cpp:350). `make_material(pass)` builds the
    /// `VertexMaterialClass` (`Parse_W3dVertexMaterialStruct`, `Parse_Mapping_Args` with the two mapper-arg
    /// strings or NULL when empty, `Set_UV_Source(0/1, map channel)`, `Set_Lighting(true)`); `get_texture(name)` is the
    /// asset manager; an empty texture name leaves the stage NULL.
    pub fn new<M, T>(def: &ShdLegacyW3DDefClass, mut make_material: M, mut get_texture: T) -> Self
    where
        M: FnMut(usize) -> VertexMaterialHandle,
        T: FnMut(&str) -> TextureHandle,
    {
        let mut s = Shd6LegacyW3DClass {
            pass_count: def.get_pass_count(),
            shaders: [ShaderClass(0); MAX_PASSES],
            materials: [None; MAX_PASSES],
            textures: [[None; MAX_TEX_STAGES]; MAX_PASSES],
            class_id: SHDDEF_CLASSID_LEGACYW3D,
            fvf: 0,
        };
        for pass in 0..s.pass_count.max(0) as usize {
            s.shaders[pass] = ShaderClass(crate::formats::w3d::mesh::convert_shader(def.get_shader(pass)));
            s.materials[pass] = Some(make_material(pass));
            for stage in 0..MAX_TEX_STAGES {
                let tex_name = def.get_texture_name(pass, stage);
                s.textures[pass][stage] = if !tex_name.is_empty() { Some(get_texture(tex_name)) } else { None };
            }
        }
        s
    }

    pub fn get_pass_count(&self) -> i32 {
        self.pass_count
    }

    /// `Greater_Than(s, pass)` (shdlegacyw3d.cpp:430): orders by class id, then first texture, then material, then
    /// shader bits (handles compared as indices: upstream compares pointers).
    pub fn greater_than(&self, s: &Shd6LegacyW3DClass, pass: usize) -> bool {
        if s.class_id != self.class_id {
            return s.class_id > self.class_id;
        }
        if s.textures[pass][0] != self.textures[pass][0] {
            return s.textures[pass][0] > self.textures[pass][0];
        }
        if s.materials[pass] != self.materials[pass] {
            return s.materials[pass] > self.materials[pass];
        }
        if s.shaders[pass].0 != self.shaders[pass].0 {
            return s.shaders[pass].0 > self.shaders[pass].0;
        }
        false
    }

    /// `Is_Opaque` (shdlegacyw3d.h:140): the first pass's destination blend is `DSTBLEND_ZERO`.
    pub fn is_opaque(&self) -> bool {
        self.shaders[0].get_dst_blend_func() == crate::formats::w3d::shader::DSTBLEND_ZERO
    }

    pub fn get_vertex_stream_count(&self) -> u32 {
        1
    }
}
