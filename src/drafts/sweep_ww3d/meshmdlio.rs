//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshmdlio.cpp
//!
//! The load-time post-processing of `MeshModelClass` that the verified `src/formats/w3d/meshmdl.rs` documents as
//! "off in the game" (`Activate_Fog_On_Load`, `MungeSortOnLoad`, `OverbrightModifyOnLoad` are all false), plus the
//! legacy `read_scg` and `MeshLoadContextClass::Get_Texcoord_Array`.
//!
//! **Excluded: the 22 `MeshModelClass::write_*` functions** (`write_chunks`, `write_user_text`, `write_triangles`,
//! `write_vertices`, `write_vertex_normals`, `write_vertex_shade_indices`, `write_vertex_influences`,
//! `write_material_info`, `write_shaders`, `write_vertex_materials`, `write_textures`, `write_material_pass`,
//! `write_vertex_material_ids`, `write_shader_ids`, `write_scg`, `write_dig`, `write_dcg`, `write_texture_stage`,
//! `write_texture_ids`, `write_stage_texcoords`). They are reached only through `MeshModelClass::Save_W3D`, which only
//! the 3ds Max exporter (`Tools/WW3D/max2w3d`) calls: editor-only, the game never writes a mesh.

use crate::formats::w3d::shader::{
    ShaderClass, StaticSortCategory, DSTBLEND_ONE, DSTBLEND_ONE_MINUS_SRC_ALPHA, DSTBLEND_SRC_ALPHA, DSTBLEND_SRC_COLOR, DSTBLEND_ZERO,
    FOG_ENABLE, FOG_SCALE_FRAGMENT, GRADIENT_MODULATE, GRADIENT_MODULATE2X, SORT_LEVEL_BIN1, SORT_LEVEL_BIN2, SORT_LEVEL_BIN3,
    SORT_LEVEL_NONE, SRCBLEND_ONE, SRCBLEND_SRC_ALPHA,
};

// shader.h bit layout: the port's own masks and shifts (`crate::formats::w3d::mesh::shader_bits`).
use crate::formats::w3d::mesh::shader_bits::{
    MASK_DSTBLEND, MASK_PRIGRADIENT, MASK_SECGRADIENT, MASK_TEXTURING, SHIFT_DSTBLEND, SHIFT_PRIGRADIENT, SHIFT_SECGRADIENT,
    SHIFT_TEXTURING,
};
/// `ShaderClass::TEXTURING_DISABLE` / `TEXTURING_ENABLE` / `SECONDARY_GRADIENT_DISABLE`.
const TEXTURING_DISABLE: u32 = 0;
const TEXTURING_ENABLE: u32 = 1;
const SECONDARY_GRADIENT_DISABLE: u32 = 0;

fn set_primary_gradient(s: &mut ShaderClass, x: u32) {
    s.0 &= !MASK_PRIGRADIENT;
    s.0 |= x << SHIFT_PRIGRADIENT;
}
fn set_texturing(s: &mut ShaderClass, x: u32) {
    s.0 &= !MASK_TEXTURING;
    s.0 |= x << SHIFT_TEXTURING;
}
fn set_dst_blend_func(s: &mut ShaderClass, x: u32) {
    s.0 &= !MASK_DSTBLEND;
    s.0 |= x << SHIFT_DSTBLEND;
}
fn get_secondary_gradient_bits(s: ShaderClass) -> u32 {
    (s.0 & MASK_SECGRADIENT) >> SHIFT_SECGRADIENT
}

/// The material description state `post_process_fog`, the sort functions and `modify_for_overbright` read and write
/// (`MeshMatDescClass`: `PassCount`, `Shader[pass]`, `ShaderArray[pass]`, `Texture[pass][0]`, `UVSource`).
/// Textures and vertex materials are opaque ids; `None` is NULL.
#[derive(Clone, Debug, Default)]
pub struct MatDescView {
    pub pass_count: usize,
    pub shader: Vec<ShaderClass>,
    /// `ShaderArray[pass]`: `None` is NULL.
    pub shader_array: Vec<Option<Vec<ShaderClass>>>,
    /// `Texture[pass][0]`.
    pub texture0: Vec<Option<usize>>,
    /// `TextureArray[pass][0]` (per-polygon texture ids).
    pub texture_array0: Vec<Option<Vec<Option<usize>>>>,
    /// `MaterialArray[pass]` element UV source for stage 0, per vertex (`None` is a NULL array).
    pub material_array_uv_source0: Vec<Option<Vec<i32>>>,
    /// `Material[pass]->Get_UV_Source(0)`.
    pub material_uv_source0: Vec<i32>,
}

/// `MeshModelClass` sort/fog-relevant state.
#[derive(Clone, Debug, Default)]
pub struct MeshModelSortState {
    pub def_mat_desc: MatDescView,
    pub vertex_count: usize,
    pub poly_count: usize,
    pub sort_level: i32,
    pub name: String,
}

impl MeshModelSortState {
    fn has_shader_array(&self, pass: usize) -> bool {
        self.def_mat_desc.shader_array.get(pass).map_or(false, |a| a.is_some())
    }

    /// `MeshModelClass::get_sort_flags(int pass)` (meshmdlio.cpp:1777): the bit set of static-sort categories on a pass.
    pub fn get_sort_flags_pass(&self, pass: usize) -> u32 {
        let mut flags = 0u32;
        if self.has_shader_array(pass) {
            if let Some(arr) = &self.def_mat_desc.shader_array[pass] {
                for s in arr {
                    let scat = s.get_ss_category() as u32;
                    flags |= 1 << scat;
                }
            }
        } else {
            let scat = self.def_mat_desc.shader[pass].get_ss_category() as u32;
            flags |= 1 << scat;
        }
        flags
    }

    /// `MeshModelClass::get_sort_flags()` (meshmdlio.cpp:1793): OR over all passes.
    pub fn get_sort_flags(&self) -> u32 {
        let mut flags = 0u32;
        for pass in 0..self.def_mat_desc.pass_count {
            flags |= self.get_sort_flags_pass(pass);
        }
        flags
    }

    /// `MeshModelClass::compute_static_sort_levels` (meshmdlio.cpp:1802).
    pub fn compute_static_sort_levels(&mut self) {
        const SSCAT_OPAQUE_BF: u32 = 1 << (StaticSortCategory::Opaque as u32);
        const SSCAT_ALPHA_TEST_BF: u32 = 1 << (StaticSortCategory::AlphaTest as u32);
        const SSCAT_ADDITIVE_BF: u32 = 1 << (StaticSortCategory::Additive as u32);
        const SSCAT_SCREEN_BF: u32 = 1 << (StaticSortCategory::Screen as u32);
        if self.get_sort_flags_pass(0) == SSCAT_OPAQUE_BF {
            self.sort_level = SORT_LEVEL_NONE;
            return;
        }
        let flags = self.get_sort_flags();
        if flags == (SSCAT_OPAQUE_BF | SSCAT_ALPHA_TEST_BF) {
            self.sort_level = SORT_LEVEL_NONE;
        } else if flags == SSCAT_ADDITIVE_BF {
            self.sort_level = SORT_LEVEL_BIN3;
        } else if flags == SSCAT_SCREEN_BF {
            self.sort_level = SORT_LEVEL_BIN2;
        } else {
            self.sort_level = SORT_LEVEL_BIN1;
        }
    }

    /// `MeshModelClass::modify_for_overbright` (meshmdlio.cpp:1838): every `GRADIENT_MODULATE` primary gradient, in the
    /// single shader and in the per-polygon shaders, becomes `GRADIENT_MODULATE2X`.
    pub fn modify_for_overbright(&mut self) {
        let pass_cnt = self.def_mat_desc.pass_count;
        for pass_idx in 0..pass_cnt {
            let mut shader = self.def_mat_desc.shader[pass_idx];
            if shader.get_primary_gradient() == GRADIENT_MODULATE {
                set_primary_gradient(&mut shader, GRADIENT_MODULATE2X);
                self.def_mat_desc.shader[pass_idx] = shader;
            }
            if self.has_shader_array(pass_idx) {
                let p_cnt = self.poly_count;
                for p_idx in 0..p_cnt {
                    let arr = self.def_mat_desc.shader_array[pass_idx].as_mut().unwrap();
                    let mut shader = arr[p_idx];
                    if shader.get_primary_gradient() == GRADIENT_MODULATE {
                        set_primary_gradient(&mut shader, GRADIENT_MODULATE2X);
                        arr[p_idx] = shader;
                    }
                }
            }
        }
    }

    /// `MeshModelClass::post_process_fog` (meshmdlio.cpp:1689): fix up the two-pass "emissive map" and "shiny mask"
    /// effects so fog applies once, then give every other pass the generic `Enable_Fog`.
    pub fn post_process_fog(&mut self) {
        let d = &mut self.def_mat_desc;
        // If two pass... and a single shader on both passes (neither pass has a shader array)
        if d.pass_count == 2 {
            let no_arrays = d.shader_array.get(0).map_or(true, |a| a.is_none()) && d.shader_array.get(1).map_or(true, |a| a.is_none());
            if no_arrays {
                let mut shader0 = d.shader[0];
                let mut shader1 = d.shader[1];
                let emissive_map_effect = d.pass_count == 2
                    && shader0.get_texturing() == TEXTURING_DISABLE
                    && shader0.get_src_blend_func() == SRCBLEND_ONE
                    && shader0.get_dst_blend_func() == DSTBLEND_ZERO
                    && shader0.get_primary_gradient() == GRADIENT_MODULATE
                    && get_secondary_gradient_bits(shader0) == SECONDARY_GRADIENT_DISABLE
                    && shader1.get_texturing() == TEXTURING_ENABLE
                    && shader1.get_src_blend_func() == SRCBLEND_SRC_ALPHA
                    && shader1.get_dst_blend_func() == DSTBLEND_SRC_COLOR;
                if emissive_map_effect {
                    // pass 0 gets a texture now, and fog is set so it applies once
                    set_texturing(&mut shader0, TEXTURING_ENABLE);
                    set_dst_blend_func(&mut shader1, DSTBLEND_ONE);
                    shader0.set_fog_func(FOG_ENABLE);
                    shader1.set_fog_func(FOG_SCALE_FRAGMENT);
                    d.shader[0] = shader0;
                    d.shader[1] = shader1;
                    // copy pass 1 texture / texture array to pass 0
                    d.texture0[0] = d.texture0[1];
                    if d.texture_array0[1].is_some() && d.texture_array0[0].is_none() {
                        let n = self.poly_count;
                        let src = d.texture_array0[1].clone().unwrap();
                        let mut dst = vec![None; n];
                        for i in 0..n {
                            dst[i] = src[i];
                        }
                        d.texture_array0[0] = Some(dst);
                    }
                    // pass 0 takes pass 1's UV source (first vertex material if there is an array)
                    let uv_source = match &d.material_array_uv_source0[1] {
                        Some(a) => a[0],
                        None => d.material_uv_source0[1],
                    };
                    match &mut d.material_array_uv_source0[0] {
                        Some(a) => {
                            for i in 0..self.vertex_count {
                                a[i] = uv_source;
                            }
                        }
                        None => d.material_uv_source0[0] = uv_source,
                    }
                    return;
                }
                let shiny_mask_effect = shader0.get_src_blend_func() == SRCBLEND_ONE
                    && shader0.get_dst_blend_func() == DSTBLEND_ZERO
                    && shader1.get_src_blend_func() == SRCBLEND_ONE
                    && (shader1.get_dst_blend_func() == DSTBLEND_SRC_ALPHA || shader1.get_dst_blend_func() == DSTBLEND_ONE_MINUS_SRC_ALPHA);
                if shiny_mask_effect {
                    shader0.set_fog_func(FOG_SCALE_FRAGMENT);
                    shader1.set_fog_func(FOG_ENABLE);
                    d.shader[0] = shader0;
                    d.shader[1] = shader1;
                    return;
                }
            }
        }
        // not one of the special two-pass combinations: generic per-pass fix-up
        for pass in 0..d.pass_count {
            d.shader[pass].enable_fog();
            if let Some(arr) = d.shader_array.get_mut(pass).and_then(|a| a.as_mut()) {
                for tri in 0..arr.len() {
                    arr[tri].enable_fog();
                }
            }
        }
    }
}

/// `MeshModelClass::read_scg` (meshmdlio.cpp:1367): "new dx8 code no longer supports this chunk" (2/9/2001); the body is
/// `return WW3D_ERROR_OK` without reading, so the chunk is skipped. `true` is `WW3D_ERROR_OK`.
pub fn read_scg() -> bool {
    true
}

/// `MeshModelClass::clone_materials` (meshmdlio.cpp:1904): the whole body is a commented-out `MaterialInfoClass`
/// copy and `MaterialRemapperClass::Remap_Mesh`, i.e. in the release build it does nothing but run the remapper over
/// the material descriptions. The remapper is the host's: `remap` stands in for
/// `remapper.Remap_Mesh(srcmesh.CurMatDesc, CurMatDesc)`.
pub fn clone_materials(remap: impl FnOnce()) {
    remap();
}

/// `MeshLoadContextClass` texcoord storage.
#[derive(Clone, Debug, Default)]
pub struct MeshLoadContextClass {
    /// `Header.NumVertices`.
    pub num_vertices: usize,
    /// `TexCoords` (`W3dTexCoordStruct[]`, U then V); `None` is NULL.
    pub tex_coords: Option<Vec<[f32; 2]>>,
}

impl MeshLoadContextClass {
    /// `MeshLoadContextClass::Get_Texcoord_Array` (meshmdlio.cpp:1999): allocated on first use, `NumVertices` long.
    pub fn get_texcoord_array(&mut self) -> &mut Vec<[f32; 2]> {
        if self.tex_coords.is_none() {
            self.tex_coords = Some(vec![[0.0; 2]; self.num_vertices]);
        }
        self.tex_coords.as_mut().unwrap()
    }
}
