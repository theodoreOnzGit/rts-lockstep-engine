//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shd6bumpspec.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shd6bumpspec.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shd6bumpspec_constants.h (the vertex shader constant registers)
//!
//! `Shd6BumpSpecClass`: the version-6 implementation of the bump specular shader. It copies its parameters out of the definition
//! (`ShdBumpSpecDefClass`) at construction, builds the vertex stream, and (upstream) sets D3D vertex/pixel shader state in
//! `Apply_Shared`/`Apply_Instance`. Translated: the construction data, `Get_Pass_Count`,
//! `Get_Vertex_Stream_Count`/`Get_Vertex_Size`, `Copy_Vertex_Stream`. Device-only, not translated:
//! `Init`/`Shutdown` (create/destroy the D3D shaders from the `.vsh`/`.psh` code), `Apply_Shared`, `Apply_Instance`
//! (`Set_Vertex_Shader`, `Set_Pixel_Shader`, `Set_Vertex_Shader_Constant`, `Set_Texture`, `ShdHWVertexShader::Light`).
//! The textures are asset-manager handles supplied by the caller.

use super::handles::TextureHandle;
use super::shdchunk::*;
use super::shdbumpspec::ShdBumpSpecDefClass;
use crate::ww::vector4::Vector4;

/// `class Shd6BumpSpecClass`.
#[derive(Clone, Debug)]
pub struct Shd6BumpSpecClass {
    pub texture: Option<TextureHandle>,
    pub ambient: Vector4,
    pub diffuse: Vector4,
    pub specular: Vector4,
}

impl Shd6BumpSpecClass {
    /// `Shd6BumpSpecClass::Shd6BumpSpecClass(def)`: copies the colours (alpha 1.0) and bumpiness out of the definition.
    /// `get_texture(name)` is `WW3DAssetManager::Get_Texture`.
    pub fn new<F: FnMut(&str) -> TextureHandle>(def: &ShdBumpSpecDefClass, mut get_texture: F) -> Self {
        let texture = Some(get_texture(def.get_texture_name()));
        let a = def.get_ambient();
        let d = def.get_diffuse();
        let sp = def.get_specular();
        Shd6BumpSpecClass {
            texture,
            ambient: Vector4 { x: a.x, y: a.y, z: a.z, w: 1.0 },
            diffuse: Vector4 { x: d.x, y: d.y, z: d.z, w: 1.0 },
            specular: Vector4 { x: sp.x, y: sp.y, z: sp.z, w: 1.0 },
        }
    }

    /// `Get_Pass_Count`.
    pub fn get_pass_count(&self) -> i32 {
        1
    }

    /// `Get_Vertex_Stream_Count`.
    pub fn get_vertex_stream_count(&self) -> u32 {
        1
    }

    /// `Get_Vertex_Size(stream)`: `sizeof` of the vertex layout.
    pub fn get_vertex_size(&self, _stream: u32) -> usize {
        std::mem::size_of::<VertexFormatXYZNDUV1>()
    }

    /// `Copy_Vertex_Stream(stream, dest_buffer, vss, vertex_count)`.
    pub fn copy_vertex_stream(&self, vss: &VertexStreamStruct, vertex_count: usize) -> Vec<VertexFormatXYZNDUV1> {
        copy_vertex_stream_xyzn_duv1(vss, vertex_count)
    }
}
