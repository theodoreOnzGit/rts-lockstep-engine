//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shd8bumpdiff.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shd8bumpdiff.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shd8bumpdiff_constants.h (the vertex shader constant registers)
//!
//! `Shd8BumpDiffClass`: the version-8 implementation of the bump diffuse shader. It copies its parameters out of the definition
//! (`ShdBumpDiffDefClass`) at construction, builds the vertex stream, and (upstream) sets D3D vertex/pixel shader state in
//! `Apply_Shared`/`Apply_Instance`. Translated: the construction data, `Get_Pass_Count`,
//! `Get_Vertex_Stream_Count`/`Get_Vertex_Size`, `Copy_Vertex_Stream`, `Pass_Selection` and `Setup_Self_Shadow_Info`. Device-only, not translated:
//! `Init`/`Shutdown` (create/destroy the D3D shaders from the `.vsh`/`.psh` code), `Apply_Shared`, `Apply_Instance`
//! (`Set_Vertex_Shader`, `Set_Pixel_Shader`, `Set_Vertex_Shader_Constant`, `Set_Texture`, `ShdHWVertexShader::Light`).
//! The textures are asset-manager handles supplied by the caller.

use super::handles::TextureHandle;
use super::shdchunk::*;
use super::shdbumpdiff::ShdBumpDiffDefClass;
use super::sortingrenderer::{d3dx_mul, Mtx4};
use crate::ww::vector4::Vector4;
use crate::ww::matrix3d::Matrix3D;

/// `class Shd8BumpDiffClass`.
#[derive(Clone, Debug)]
pub struct Shd8BumpDiffClass {
    pub texture: Option<TextureHandle>,
    pub normal_map: Option<TextureHandle>,
    pub ambient: Vector4,
    pub diffuse: Vector4,
    pub bumpiness: Vector4,
    /// `Self_Shadow_Transform` (set by `Setup_Self_Shadow_Info`).
    pub self_shadow_transform: Mtx4,
}

impl Shd8BumpDiffClass {
    /// `Shd8BumpDiffClass::Shd8BumpDiffClass(def)`: copies the colours (alpha 0.0) and bumpiness out of the definition.
    /// `get_texture(name)` is `WW3DAssetManager::Get_Texture`.
    pub fn new<F: FnMut(&str) -> TextureHandle>(def: &ShdBumpDiffDefClass, mut get_texture: F) -> Self {
        let texture = Some(get_texture(def.get_texture_name()));
        let normal_map = Some(get_texture(def.get_bump_map_name()));
        let a = def.get_ambient();
        let d = def.get_diffuse();
        let db = def.get_diffuse_bumpiness();
        Shd8BumpDiffClass {
            texture,
            normal_map,
            ambient: Vector4 { x: a.x, y: a.y, z: a.z, w: 0.0 },
            diffuse: Vector4 { x: d.x, y: d.y, z: d.z, w: 0.0 },
            bumpiness: Vector4 { x: db.x, y: db.y, z: 0.0, w: 0.0 },
            self_shadow_transform: [[0.0; 4]; 4],
        }
    }

    /// `Get_Pass_Count`.
    pub fn get_pass_count(&self) -> i32 {
        2
    }

    /// `Get_Vertex_Stream_Count`.
    pub fn get_vertex_stream_count(&self) -> u32 {
        1
    }

    /// `Get_Vertex_Size(stream)`: `sizeof` of the vertex layout.
    pub fn get_vertex_size(&self, _stream: u32) -> usize {
        std::mem::size_of::<VertexFormatXYZNDUV1TG3>()
    }

    /// `Copy_Vertex_Stream(stream, dest_buffer, vss, vertex_count)`.
    pub fn copy_vertex_stream(&self, vss: &VertexStreamStruct, vertex_count: usize) -> Vec<VertexFormatXYZNDUV1TG3> {
        copy_vertex_stream_xyzn_duv1_tg3(vss, vertex_count)
    }

    /// `Pass_Selection(mesh, rinfo, pass)`: with a shadow map being applied only pass 1 runs (and sets up the
    /// self-shadow transform, by `setup`); otherwise only pass 0 runs.
    pub fn pass_selection<F: FnOnce(&mut Self)>(&mut self, applying_shadow_map: bool, pass: i32, setup: F) -> bool {
        if applying_shadow_map {
            if pass == 1 {
                // just do once
                setup(self);
            }
            return pass == 1;
        }
        pass == 0
    }

    /// `Setup_Self_Shadow_Info(mesh, rinfo)`: `Self_Shadow_Transform = view2tex * (world * view)` with
    /// `view2tex = sb_mat * tex_mat`. `tex_mat` is the projector mapper's `Get_Texture_Transform()`;
    /// `shadow_map` is `(width, height, depth_bits)` of `DX8Wrapper::Get_Shadow_Map(0)` (`None`: returns, leaving the
    /// transform unchanged, as upstream does); `world` is `mesh.Get_Transform()`, `view` is
    /// `rinfo.Camera.Get_View_Matrix()` (both `Matrix3D`, widened by `Matrix4x4::Init`).
    pub fn setup_self_shadow_info(&mut self, tex_mat: &Mtx4, shadow_map: Option<(u32, u32, u32)>, world: &Matrix3D, view: &Matrix3D) {
        let (w, h, bits) = match shadow_map {
            Some(s) => s,
            None => return,
        };
        let off_x = 0.5f32 + (0.5f32 / w as f32);
        let off_y = 0.5f32 + (0.5f32 / h as f32);
        // 0xFFFFFFFF>>(32-bits): the x86 shift count is taken modulo 32 (bits == 0 shifts by 32 -> 0)
        let range = (0xFFFF_FFFFu32 >> ((32u32.wrapping_sub(bits)) & 31)) as f32;
        let bias = 0.0f32;
        let sb_mat: Mtx4 = [
            [0.5, 0.0, 0.0, 0.0],
            [0.0, -0.5, 0.0, 0.0],
            [0.0, 0.0, range, 0.0],
            [off_x, off_y, bias, 1.0],
        ];
        let view2tex = d3dx_mul(&sb_mat, tex_mat);
        let init = |m: &Matrix3D| -> Mtx4 {
            [
                [m.row[0][0], m.row[0][1], m.row[0][2], m.row[0][3]],
                [m.row[1][0], m.row[1][1], m.row[1][2], m.row[1][3]],
                [m.row[2][0], m.row[2][1], m.row[2][2], m.row[2][3]],
                [0.0, 0.0, 0.0, 1.0],
            ]
        };
        let vw_mat = d3dx_mul(&init(world), &init(view));
        self.self_shadow_transform = d3dx_mul(&view2tex, &vw_mat);
    }
}
