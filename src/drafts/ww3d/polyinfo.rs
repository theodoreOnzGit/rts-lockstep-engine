//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/polyinfo.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/polyinfo.h

use super::handles::{TextureHandle, VertexMaterialHandle};
use crate::formats::w3d::shader::ShaderClass;

/// `class PolygonInfoClass` (polyinfo.h): a texture, a vertex material and a shader. Upstream owns a heap copy of
/// the shader (`W3DNEW ShaderClass(*shader)`); here it is held by value.
#[derive(Clone, Debug, Default)]
pub struct PolygonInfoClass {
    texture: Option<TextureHandle>,
    vertex_material: Option<VertexMaterialHandle>,
    shader: Option<ShaderClass>,
}

impl PolygonInfoClass {
    /// `PolygonInfoClass::PolygonInfoClass(texture, vertexMaterial, shader)` (polyinfo.h): starts empty, then `Set`.
    pub fn new(texture: Option<TextureHandle>, vertex_material: Option<VertexMaterialHandle>, shader: Option<ShaderClass>) -> Self {
        let mut p = PolygonInfoClass { texture: None, vertex_material: None, shader: None };
        p.set(texture, vertex_material, shader);
        p
    }

    /// `Peek_Texture` (polyinfo.h).
    pub fn peek_texture(&self) -> Option<TextureHandle> {
        self.texture
    }

    /// `Peek_Vertex_Material` (polyinfo.h).
    pub fn peek_vertex_material(&self) -> Option<VertexMaterialHandle> {
        self.vertex_material
    }

    /// `Peek_Shader` (polyinfo.h).
    pub fn peek_shader(&self) -> Option<&ShaderClass> {
        self.shader.as_ref()
    }

    /// `PolygonInfoClass::Set_Texture` (polyinfo.cpp:26): Add_Ref the new, Release_Ref the old (not modelled).
    pub fn set_texture(&mut self, texture: Option<TextureHandle>) {
        self.texture = texture;
    }

    /// `PolygonInfoClass::Set_Vertex_Material` (polyinfo.cpp:34).
    pub fn set_vertex_material(&mut self, vertex_material: Option<VertexMaterialHandle>) {
        self.vertex_material = vertex_material;
    }

    /// `PolygonInfoClass::Set_Shader` (polyinfo.cpp:44): deletes the old copy and makes a new one of `*shader`.
    /// Upstream dereferences `shader` unconditionally (a NULL is a crash); here `None` clears it.
    pub fn set_shader(&mut self, shader: Option<&ShaderClass>) {
        self.shader = shader.copied();
    }

    /// `PolygonInfoClass::Set` (polyinfo.h).
    pub fn set(&mut self, texture: Option<TextureHandle>, vertex_material: Option<VertexMaterialHandle>, shader: Option<ShaderClass>) {
        self.set_texture(texture);
        self.set_vertex_material(vertex_material);
        self.set_shader(shader.as_ref());
    }
}
