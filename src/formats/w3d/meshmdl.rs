// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshmdlio.cpp
//   (MeshModelClass::install_materials, post_process)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshmatdesc.cpp, .h
//   (Post_Load_Process, Configure_Material, Peek_Material, Peek_Texture,
//   Get_Shader, Disable_Backface_Culling)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshgeometry.cpp
//   (MeshGeometryClass::get_deformed_vertices, both overloads)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/vp.cpp
//   (VectorProcessorClass::Transform, the non-SSE path: mulVector3Array)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/dx8renderer.cpp
//   (the split table's Peek_Texture, Peek_Material, Peek_Shader, and the
//   per-pass texture-category split of DX8FVFCategoryContainer::
//   Generate_Texture_Categories / Insert_To_Texture_Category)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, by Greg Hjelstrom, Jani Penttinen and others)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The render side of a loaded W3D mesh (`MeshModelClass` after
//! `Load_W3D`'s last two steps): [`MeshModel`]. What the loader read
//! ([`super::mesh::Mesh`]) is finished as upstream finishes it, then split
//! into the polygon groups the DX8 renderer draws, with the skin's
//! deformation by a posed hierarchy.
//!
//! # As upstream reads it
//!
//! - **`install_materials`** runs `MeshMatDescClass::Post_Load_Process` on the
//!   default material description, with lighting on unless the mesh is
//!   pre-lit per vertex (`PRELIT_VERTEX`). For each pass: a pass without a
//!   vertex material gets a new `VertexMaterialClass()`; each material the
//!   pass uses is configured (`Configure_Material`: diffuse colour source
//!   from the pass's DCG source, emissive from its DIG source, lighting, and
//!   the UV array of stages 0 and 1, `-1` meaning 0). With vertex colour
//!   arrays, the material colours are folded into the DCG array: DCG times
//!   DIG when both are used (then the DIG source is dropped), and DCG times
//!   the one material colour in use (diffuse, diffuse and ambient, ambient,
//!   or emissive), whose colour source then becomes `COLOR1`. Lighting is
//!   turned off on emissive-only materials when every pass asks for it.
//! - **`post_process`**: a two-sided mesh's shaders (single and per
//!   polygon) lose backface culling. Fog activation, static sort levels and
//!   overbright modulation are off in the game (`Activate_Fog_On_Load`,
//!   `MungeSortOnLoad` and `OverbrightModifyOnLoad` are all false), so they
//!   are not run.
//! - **What is drawn** ([`MeshModel::polygon_groups`]): per pass, each
//!   polygon's two stage textures, vertex material (the material of the
//!   polygon's **first vertex**) and shader, as the DX8 renderer's split
//!   table peeks them; polygons with the same three go into one group, the
//!   groups in the order their first polygon comes.
//! - **Skins** ([`MeshModel::get_deformed_vertices`]): each vertex (and its
//!   normal) transformed by the posed hierarchy's transform of its bone, in
//!   runs of vertices on the same bone, the normal by the same matrix with
//!   its translation zeroed (not renormalised).
//!
//! # Upstream quirks that are kept
//!
//! - Vertex materials are shared by index between passes (upstream shares
//!   the pointers): configuring one for a later pass overwrites what an
//!   earlier pass set, so the last pass that uses it decides its colour
//!   sources, lighting and UV sources.
//! - `Post_Load_Process` peeks `Peek_Material(pass, 0)` (the arguments
//!   swapped) before its first per-vertex loop; the value is not used, but
//!   the read is checked.
//! - In the colour-folding loop, the "ambient only" and "emissive only"
//!   cases multiply by `mtl_ambient` / `mtl_emissive` of the **previous**
//!   loop (the inner block redeclares only the diffuse and opacity), so by
//!   the material of the pass's last vertex.
//! - "Lighting off for emissive-only" needs `set_lighting_to_false`, which
//!   only a multi-pass mesh with a non-emissive-only folded pass clears.
//! - Normals of skins are transformed, not renormalised.
//! - Groups compare materials by index; upstream compares them by CRC
//!   (`Equal_Material`), so two distinct materials with equal contents share
//!   a group there and not here. The drawn result is the same.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined`]: a pass count over 4 (`Post_Load_Process` indexes the
//! four-pass arrays); a vertex, polygon or bone index outside its array; a
//! material colour read before any material was (`mtl_diffuse` and the rest
//! are uninitialised when the first vertex has no material); a null material
//! dereferenced; a skin without bone links.
//!
//! # What is left out
//!
//! - The alternate material description (`install_alternate_material_desc`,
//!   `Init_Alternate`): [`MeshModel::has_alternate`] says when the file had
//!   one. Upstream's `Post_Load_Process` on it would also reconfigure the
//!   vertex materials it shares with the default description.
//! - N-patches (`Init_For_NPatch_Rendering`), the gap filler, and the
//!   renderer's polygon sorting.
//!
//! # Representation
//!
//! Arrays are expanded from the loader's run-length [`super::mesh::Arr`]s;
//! vertex materials and textures are indices into
//! [`MeshModel::vertex_materials`] and [`MeshModel::textures`].

use super::htree::HTree;
use super::mesh::{dx8_convert_color, dx8_unpack_color, mesh_flags, Arr, Mesh, Texture, MAX_PASSES, MAX_TEX_STAGES};
use super::shader::ShaderClass;
use super::vertmaterial::{color_source, VertexMaterialState, D3DMCS_COLOR1, D3DMCS_MATERIAL};
use super::{W3dError, W3dResult, W3dUndefined};
use crate::ww::vector3::Vector3;

/// `VertexMaterialClass::MATERIAL`, as a DCG/DIG source.
const MATERIAL: u32 = 0;
/// The largest vertex or polygon count drawn (a guard against headers that
/// claim more than any real model; upstream would allocate them).
pub const MAX_DRAWN_COUNT: usize = 1 << 22;

/// Vertices and normals, as `get_deformed_vertices` gives them.
pub type Deformed = (Vec<[f32; 3]>, Vec<[f32; 3]>);

/// A material's diffuse, opacity, ambient and emissive.
type Colours = ([f32; 3], f32, [f32; 3], [f32; 3]);

/// One pass of the material description, expanded.
#[derive(Clone, Debug, PartialEq)]
pub struct PassDesc {
    /// `Material[pass]`.
    pub material: Option<usize>,
    /// `MaterialArray[pass]` (per vertex).
    pub material_array: Option<Vec<Option<usize>>>,
    /// `Shader[pass]`.
    pub shader: ShaderClass,
    /// `ShaderArray[pass]` (per polygon).
    pub shader_array: Option<Vec<ShaderClass>>,
    /// `Texture[pass][stage]`.
    pub texture: [Option<usize>; MAX_TEX_STAGES],
    /// `TextureArray[pass][stage]` (per polygon).
    pub texture_array: [Option<Vec<Option<usize>>>; MAX_TEX_STAGES],
    /// `UVSource[pass][stage]`.
    pub uv_source: [i32; MAX_TEX_STAGES],
    /// `DCGSource[pass]`.
    pub dcg_source: u32,
    /// `DIGSource[pass]`.
    pub dig_source: u32,
}

/// `MeshModelClass` after `install_materials` and `post_process`: what the
/// renderer reads.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshModel {
    /// The mesh's full name (`CONTAINER.MESH`).
    pub name: Vec<u8>,
    /// `MeshGeometryClass` flags ([`mesh_flags`]).
    pub flags: u32,
    /// `W3dAttributes` (the header's).
    pub w3d_attributes: u32,
    pub sort_level: i8,
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub polys: Vec<[u16; 3]>,
    /// `VertexBoneLink` (skins).
    pub bone_links: Option<Vec<u16>>,
    /// `PassCount`.
    pub pass_count: usize,
    pub passes: Vec<PassDesc>,
    /// `UV[]`: the installed texture-coordinate arrays.
    pub uv: Vec<Vec<[f32; 2]>>,
    /// `ColorArray[0]` (DCG) and `[1]` (DIG), packed ARGB.
    pub color_array: [Option<Vec<u32>>; 2],
    /// The context's vertex materials (and those `Post_Load_Process` created).
    pub vertex_materials: Vec<VertexMaterialState>,
    /// The context's textures (names and settings).
    pub textures: Vec<Texture>,
    /// The file had an alternate material description (not installed).
    pub has_alternate: bool,
}

/// One group of polygons drawn with the same state (a DX8 texture category's
/// share of the mesh).
#[derive(Clone, Debug, PartialEq)]
pub struct PolygonGroup {
    pub pass: usize,
    /// The stage textures (indices into [`MeshModel::textures`]).
    pub textures: [Option<usize>; MAX_TEX_STAGES],
    /// The vertex material (index into [`MeshModel::vertex_materials`]).
    pub material: Option<usize>,
    pub shader: ShaderClass,
    /// The polygons (indices into [`MeshModel::polys`]), in order.
    pub polygons: Vec<u32>,
}

fn expand<T: Copy + PartialEq>(a: &Option<Arr<T>>) -> W3dResult<Option<Vec<T>>> {
    match a {
        Some(a) if a.len() > MAX_DRAWN_COUNT => Err(W3dError::Unsupported("a mesh array too large to draw")),
        Some(a) => Ok(Some(a.to_vec())),
        None => Ok(None),
    }
}

fn idx(v: Option<u32>) -> Option<usize> {
    v.map(|i| i as usize)
}

impl MeshModel {
    /// The loaded mesh, finished as `MeshModelClass::Load_W3D` finishes it:
    /// `install_materials` (the default description's `Post_Load_Process`)
    /// and `post_process`.
    pub fn from_mesh(mesh: &Mesh) -> W3dResult<MeshModel> {
        let d = &mesh.default_mat;
        if !(0..=MAX_PASSES as i32).contains(&d.pass_count) {
            return Err(W3dUndefined::IndexOutOfRange.into());
        }
        let mut passes = Vec::with_capacity(MAX_PASSES);
        for p in &d.passes {
            let texture_array = [
                expand(&p.texture_array[0])?.map(|v| v.into_iter().map(idx).collect()),
                expand(&p.texture_array[1])?.map(|v| v.into_iter().map(idx).collect()),
            ];
            passes.push(PassDesc {
                material: idx(p.material),
                material_array: expand(&p.material_array)?.map(|v| v.into_iter().map(idx).collect()),
                shader: ShaderClass(p.shader),
                shader_array: expand(&p.shader_array)?.map(|v| v.into_iter().map(ShaderClass).collect()),
                texture: [idx(p.texture[0]), idx(p.texture[1])],
                texture_array,
                uv_source: p.uv_source,
                dcg_source: p.dcg_source,
                dig_source: p.dig_source,
            });
        }
        let mut uv = Vec::with_capacity(d.uv.len());
        for u in &d.uv {
            uv.push(expand(&Some(u.uvs.clone()))?.unwrap_or_default());
        }
        let alt = &mesh.alternate_mat;
        let has_alternate = alt.passes.iter().any(|p| {
            p.material.is_some()
                || p.material_array.is_some()
                || p.shader != 0
                || p.shader_array.is_some()
                || p.texture.iter().any(|t| t.is_some())
                || p.texture_array.iter().any(|t| t.is_some())
        }) || !alt.uv.is_empty()
            || alt.color_array.iter().any(|c| c.is_some());
        let mut m = MeshModel {
            name: mesh.name.clone(),
            flags: mesh.flags,
            w3d_attributes: mesh.w3d_attributes,
            sort_level: mesh.sort_level,
            vertices: expand(&mesh.vertices)?.unwrap_or_default(),
            normals: expand(&mesh.normals)?.unwrap_or_default(),
            polys: expand(&mesh.polys)?.unwrap_or_default(),
            bone_links: expand(&mesh.bone_links)?,
            pass_count: d.pass_count as usize,
            passes,
            uv,
            color_array: [expand(&d.color_array[0])?, expand(&d.color_array[1])?],
            vertex_materials: mesh.vertex_materials.iter().map(VertexMaterialState::from_w3d).collect(),
            textures: mesh.textures.clone(),
            has_alternate,
        };
        // install_materials: vertex-lit models need the lighting turned off
        let lighting_enabled = m.flags & mesh_flags::PRELIT_VERTEX == 0;
        m.post_load_process(lighting_enabled, mesh.vertex_count.max(0) as usize)?;
        m.post_process();
        Ok(m)
    }

    /// The vertex count (`Get_Vertex_Count()`).
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// The polygon count.
    pub fn poly_count(&self) -> usize {
        self.polys.len()
    }

    /// `Get_Flag(f)`.
    pub fn get_flag(&self, f: u32) -> bool {
        self.flags & f != 0
    }

    /// `MeshMatDescClass::Peek_Material(vidx, pass)`.
    pub fn peek_material(&self, vidx: usize, pass: usize) -> W3dResult<Option<usize>> {
        let p = self.passes.get(pass).ok_or(W3dUndefined::IndexOutOfRange)?;
        match &p.material_array {
            Some(a) => a.get(vidx).copied().ok_or(W3dUndefined::IndexOutOfRange.into()),
            None => Ok(p.material),
        }
    }

    /// `MeshMatDescClass::Peek_Texture(pidx, pass, stage)`.
    pub fn peek_texture(&self, pidx: usize, pass: usize, stage: usize) -> W3dResult<Option<usize>> {
        let p = self.passes.get(pass).ok_or(W3dUndefined::IndexOutOfRange)?;
        match &p.texture_array[stage] {
            Some(a) => a.get(pidx).copied().ok_or(W3dUndefined::IndexOutOfRange.into()),
            None => Ok(p.texture[stage]),
        }
    }

    /// `MeshMatDescClass::Get_Shader(pidx, pass)`.
    pub fn get_shader(&self, pidx: usize, pass: usize) -> W3dResult<ShaderClass> {
        let p = self.passes.get(pass).ok_or(W3dUndefined::IndexOutOfRange)?;
        match &p.shader_array {
            Some(a) => a.get(pidx).copied().ok_or(W3dUndefined::IndexOutOfRange.into()),
            None => Ok(p.shader),
        }
    }

    fn vmat(&mut self, m: usize) -> W3dResult<&mut VertexMaterialState> {
        self.vertex_materials.get_mut(m).ok_or(W3dUndefined::IndexOutOfRange.into())
    }

    /// `Configure_Material(mtl, pass, lighting_enabled)`.
    fn configure_material(&mut self, m: usize, pass: usize, lighting_enabled: bool) -> W3dResult<()> {
        let p = self.passes[pass].clone();
        let mtl = self.vmat(m)?;
        mtl.diffuse_color_source = color_source(p.dcg_source);
        mtl.emissive_color_source = color_source(p.dig_source);
        mtl.use_lighting = lighting_enabled;
        for stage in 0..MAX_TEX_STAGES {
            let src = if p.uv_source[stage] == -1 { 0 } else { p.uv_source[stage] };
            mtl.uv_source[stage] = src;
        }
        Ok(())
    }

    /// The colours `Post_Load_Process` reads with `Get_Diffuse`,
    /// `Get_Opacity`, `Get_Ambient`, `Get_Emissive`: a null material is
    /// dereferenced (undefined).
    fn colours(&self, m: Option<usize>) -> W3dResult<Colours> {
        let m = m.ok_or(W3dUndefined::NullPointer)?;
        let v = self.vertex_materials.get(m).ok_or(W3dUndefined::IndexOutOfRange)?;
        Ok((v.diffuse, v.opacity, v.ambient, v.emissive))
    }

    /// `MeshMatDescClass::Post_Load_Process(lighting_enabled, parent)`.
    fn post_load_process(&mut self, lighting_enabled: bool, vertex_count: usize) -> W3dResult<()> {
        let nz = |c: [f32; 3]| c[0] != 0.0 || c[1] != 0.0 || c[2] != 0.0;
        let mut set_lighting_to_false = true;
        for pass in 0..self.pass_count {
            // If this pass doesn't have a vertex material, create one
            if self.passes[pass].material.is_none() && self.passes[pass].material_array.is_none() {
                self.vertex_materials.push(VertexMaterialState::default());
                self.passes[pass].material = Some(self.vertex_materials.len() - 1);
            }
            // Configure the materials to source the uv coordinates and colors
            if let Some(m) = self.passes[pass].material {
                self.configure_material(m, pass, lighting_enabled)?;
            } else {
                let mut prev = None;
                self.peek_material(pass, 0)?; // Peek_Material(pass,0): read, not used
                for vidx in 0..vertex_count {
                    let mtl = self.peek_material(vidx, pass)?;
                    if mtl != prev {
                        if let Some(m) = mtl {
                            self.configure_material(m, pass, lighting_enabled)?;
                            prev = mtl;
                        }
                    }
                }
            }

            if self.color_array[0].is_none() && self.color_array[1].is_none() {
                continue;
            }

            let mut single_diffuse = [0.0f32; 3];
            let mut single_ambient = [0.0f32; 3];
            let mut single_emissive = [0.0f32; 3];
            let mut single_opacity = 1.0f32;
            let (mut diffuse_used, mut ambient_used, mut emissive_used) = (false, false, false);
            // (single_*_used and opacity_used are computed upstream and never read)
            let mut mtl_diffuse: Option<[f32; 3]> = None;
            let mut mtl_ambient: Option<[f32; 3]> = None;
            let mut mtl_emissive: Option<[f32; 3]> = None;

            let mtl = self.peek_material(0, pass)?;
            if mtl.is_some() {
                let (d, o, a, e) = self.colours(mtl)?;
                single_diffuse = d;
                single_opacity = o;
                single_ambient = a;
                single_emissive = e;
                diffuse_used |= nz(d);
                ambient_used |= nz(a);
                emissive_used |= nz(e);
            }
            let _ = (single_diffuse, single_ambient, single_emissive, single_opacity);
            let mut prev = None;
            for vidx in 0..vertex_count {
                let mtl = self.peek_material(vidx, pass)?;
                if mtl != prev {
                    prev = mtl;
                    let (d, _, a, e) = self.colours(mtl)?;
                    mtl_diffuse = Some(d);
                    mtl_ambient = Some(a);
                    mtl_emissive = Some(e);
                }
                let un = W3dUndefined::UninitialisedRead;
                diffuse_used |= nz(mtl_diffuse.ok_or(un)?);
                ambient_used |= nz(mtl_ambient.ok_or(un)?);
                emissive_used |= nz(mtl_emissive.ok_or(un)?);
                // (the opacity is initialised: 1.0 until a material is read)
            }

            // If both DCG and DIG arrays are submitted, multiply them together to DCG channel
            let (dcg, dig) = (self.passes[pass].dcg_source, self.passes[pass].dig_source);
            if dcg != MATERIAL && dig != MATERIAL && self.color_array[0].is_some() && self.color_array[1].is_some() {
                for vidx in 0..vertex_count {
                    let e = self.color_array[1].as_ref().unwrap().get(vidx).copied().ok_or(W3dUndefined::IndexOutOfRange)?;
                    let c = self.color_array[0].as_mut().unwrap().get_mut(vidx).ok_or(W3dUndefined::IndexOutOfRange)?;
                    let mut diffuse = dx8_unpack_color(*c);
                    let emissive = dx8_unpack_color(e);
                    diffuse[0] *= emissive[0];
                    diffuse[1] *= emissive[1];
                    diffuse[2] *= emissive[2];
                    *c = dx8_convert_color(diffuse);
                }
            }
            self.passes[pass].dig_source = MATERIAL; // DIG channel no more

            if dcg != MATERIAL && self.color_array[0].is_some() {
                let mut prev = None;
                let mut inner_diffuse: Option<[f32; 3]> = None;
                let mut inner_opacity = 1.0f32;
                for vidx in 0..vertex_count {
                    let mtl = self.peek_material(vidx, pass)?;
                    if mtl != prev {
                        prev = mtl;
                        let (d, o, _, _) = self.colours(mtl)?;
                        inner_diffuse = Some(d);
                        inner_opacity = o;
                    }
                    let fold = |s: &mut Self, by: Option<[f32; 3]>| -> W3dResult<()> {
                        let by = by.ok_or(W3dUndefined::UninitialisedRead)?;
                        let c = s.color_array[0].as_mut().unwrap().get_mut(vidx).ok_or(W3dUndefined::IndexOutOfRange)?;
                        let mut diffuse = dx8_unpack_color(*c);
                        diffuse[0] *= by[0];
                        diffuse[1] *= by[1];
                        diffuse[2] *= by[2];
                        diffuse[3] *= inner_opacity;
                        *c = dx8_convert_color(diffuse);
                        Ok(())
                    };
                    let sources = |s: &mut Self, amb: u32, dif: u32, emi: u32| -> W3dResult<()> {
                        let m = mtl.ok_or(W3dUndefined::NullPointer)?;
                        let v = s.vmat(m)?;
                        v.ambient_color_source = amb;
                        v.diffuse_color_source = dif;
                        v.emissive_color_source = emi;
                        Ok(())
                    };
                    if diffuse_used && !ambient_used && !emissive_used {
                        fold(self, inner_diffuse)?;
                        sources(self, D3DMCS_MATERIAL, D3DMCS_COLOR1, D3DMCS_MATERIAL)?;
                    }
                    if diffuse_used && ambient_used && !emissive_used {
                        fold(self, inner_diffuse)?;
                        sources(self, D3DMCS_COLOR1, D3DMCS_COLOR1, D3DMCS_MATERIAL)?;
                    }
                    if !diffuse_used && ambient_used && !emissive_used {
                        // the outer loop's mtl_ambient (kept)
                        fold(self, mtl_ambient)?;
                        sources(self, D3DMCS_COLOR1, D3DMCS_MATERIAL, D3DMCS_MATERIAL)?;
                    }
                    if !diffuse_used && !ambient_used && emissive_used {
                        // the outer loop's mtl_emissive (kept)
                        fold(self, mtl_emissive)?;
                        sources(self, D3DMCS_MATERIAL, D3DMCS_COLOR1, D3DMCS_MATERIAL)?;
                    } else if self.pass_count != 1 {
                        set_lighting_to_false = false;
                    }
                }
            }
        }

        // Set lighting to false if requested in all passes...
        if set_lighting_to_false {
            for pass in 0..self.pass_count {
                let (mut diffuse_used, mut ambient_used, mut emissive_used) = (false, false, false);
                let nz = |c: [f32; 3]| c[0] != 0.0 || c[1] != 0.0 || c[2] != 0.0;
                let mtl = self.peek_material(0, pass)?;
                if mtl.is_some() {
                    let (d, _, a, e) = self.colours(mtl)?;
                    diffuse_used |= nz(d);
                    ambient_used |= nz(a);
                    emissive_used |= nz(e);
                }
                let mut prev = None;
                let mut cur: Option<[[f32; 3]; 3]> = None;
                for vidx in 0..vertex_count {
                    let mtl = self.peek_material(vidx, pass)?;
                    if mtl != prev {
                        prev = mtl;
                        let (d, _, a, e) = self.colours(mtl)?;
                        cur = Some([d, a, e]);
                    }
                    let [d, a, e] = cur.ok_or(W3dUndefined::UninitialisedRead)?;
                    diffuse_used |= nz(d);
                    ambient_used |= nz(a);
                    emissive_used |= nz(e);
                }
                if self.passes[pass].dcg_source != MATERIAL && self.color_array[0].is_some() {
                    let mut prev = None;
                    for vidx in 0..vertex_count {
                        let mtl = self.peek_material(vidx, pass)?;
                        if mtl != prev {
                            prev = mtl;
                            if !diffuse_used && !ambient_used && emissive_used {
                                let m = mtl.ok_or(W3dUndefined::NullPointer)?;
                                self.vmat(m)?.use_lighting = false;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// `MeshModelClass::post_process()`, with the game's settings (no fog
    /// activation, no static sort munging, no overbright).
    fn post_process(&mut self) {
        // turn off backface culling if the mesh is supposed to be two-sided
        if self.get_flag(mesh_flags::TWO_SIDED) {
            // DefMatDesc->Disable_Backface_Culling()
            for pass in 0..self.pass_count {
                let p = &mut self.passes[pass];
                p.shader.set_cull_mode(0);
                if let Some(a) = &mut p.shader_array {
                    for s in a.iter_mut() {
                        s.set_cull_mode(0);
                    }
                }
            }
        }
    }

    /// The polygon groups the DX8 renderer draws, pass by pass: each
    /// polygon's textures, material (its first vertex's) and shader as the
    /// split table peeks them; equal triples share a group.
    pub fn polygon_groups(&self) -> W3dResult<Vec<PolygonGroup>> {
        let mut out: Vec<PolygonGroup> = Vec::new();
        for pass in 0..self.pass_count {
            let first = out.len();
            for (i, poly) in self.polys.iter().enumerate() {
                let textures = [self.peek_texture(i, pass, 0)?, self.peek_texture(i, pass, 1)?];
                let material = if self.passes[pass].material_array.is_some() {
                    self.peek_material(usize::from(poly[0]), pass)?
                } else {
                    self.passes[pass].material
                };
                let shader = self.get_shader(i, pass)?;
                match out[first..].iter_mut().find(|g| g.textures == textures && g.material == material && g.shader == shader) {
                    Some(g) => g.polygons.push(i as u32),
                    None => out.push(PolygonGroup { pass, textures, material, shader, polygons: vec![i as u32] }),
                }
            }
        }
        Ok(out)
    }

    /// `Get_UV_Array_By_Index(i)`.
    pub fn uv_array(&self, i: usize) -> Option<&[[f32; 2]]> {
        self.uv.get(i).map(|v| v.as_slice())
    }

    /// `MeshGeometryClass::get_deformed_vertices(dst_vert, dst_norm, htree)`:
    /// each vertex and normal moved by its bone's transform in the posed
    /// `htree`.
    pub fn get_deformed_vertices(&self, htree: &HTree) -> W3dResult<Deformed> {
        let n = self.vertex_count();
        let bonelink = self.bone_links.as_ref().ok_or(W3dUndefined::NullPointer)?;
        if bonelink.len() < n || self.normals.len() < n {
            return Err(W3dUndefined::IndexOutOfRange.into());
        }
        let mut dst_vert = vec![[0.0f32; 3]; n];
        let mut dst_norm = vec![[0.0f32; 3]; n];
        let v3 = |a: [f32; 3]| Vector3::new(a[0], a[1], a[2]);
        let mut vi = 0;
        while vi < n {
            let tm = htree.get_transform(i32::from(bonelink[vi]))?;
            // Make a copy so we can set the translation to zero
            let mut mytm = tm;
            let idx = bonelink[vi];
            let mut cnt = vi;
            while cnt < n && bonelink[cnt] == idx {
                cnt += 1;
            }
            // VectorProcessorClass::Transform: mtx.mulVector3Array(src, dst, count)
            for (dst, src) in dst_vert[vi..cnt].iter_mut().zip(&self.vertices[vi..cnt]) {
                let r = mytm.mul_vector3(&v3(*src));
                *dst = [r.x, r.y, r.z];
            }
            mytm.set_translation(&Vector3::new(0.0, 0.0, 0.0));
            for (dst, src) in dst_norm[vi..cnt].iter_mut().zip(&self.normals[vi..cnt]) {
                let r = mytm.mul_vector3(&v3(*src));
                *dst = [r.x, r.y, r.z];
            }
            vi = cnt;
        }
        Ok((dst_vert, dst_norm))
    }

    /// The share of the polygons whose winding (`(b - a) x (c - a)`) points
    /// the same way as the sum of their vertex normals: a check of which
    /// face is the front, for a renderer that culls by winding. `None`
    /// without polygons.
    pub fn winding_agrees_with_normals(&self) -> Option<f32> {
        if self.polys.is_empty() || self.normals.len() < self.vertices.len() {
            return None;
        }
        let mut agree = 0usize;
        let mut counted = 0usize;
        for p in &self.polys {
            let [a, b, c] = p.map(usize::from);
            if a >= self.vertices.len() || b >= self.vertices.len() || c >= self.vertices.len() {
                continue;
            }
            let (va, vb, vc) = (self.vertices[a], self.vertices[b], self.vertices[c]);
            let e1 = [vb[0] - va[0], vb[1] - va[1], vb[2] - va[2]];
            let e2 = [vc[0] - va[0], vc[1] - va[1], vc[2] - va[2]];
            let cross = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let ns = [a, b, c].iter().fold([0.0f32; 3], |s, &i| [s[0] + self.normals[i][0], s[1] + self.normals[i][1], s[2] + self.normals[i][2]]);
            let dot = cross[0] * ns[0] + cross[1] * ns[1] + cross[2] * ns[2];
            counted += 1;
            if dot > 0.0 {
                agree += 1;
            }
        }
        (counted > 0).then(|| agree as f32 / counted as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::w3d::htree::Pivot;
    use crate::formats::w3d::mesh::{self as m, MeshLoadOptions};
    use crate::formats::w3d::synth::{MeshSpec, StageSpec};
    use crate::ww::chunkio::{ChunkLoad, GameFile};
    use crate::ww::matrix3d::Matrix3D;

    fn load(bytes: &[u8]) -> Mesh {
        let mut c = ChunkLoad::new(GameFile::new(bytes).unwrap());
        assert!(c.open_chunk().unwrap());
        m::load_w3d(&mut c, &MeshLoadOptions::default()).unwrap().unwrap()
    }

    /// Predictions, written before the first run, for a two-triangle quad
    /// with one vertex material (diffuse 255,128,0), one alpha-blended
    /// textured shader and one texture `quad.tga` on stage 0:
    /// - one pass, one group of both polygons, texture 0, material 0;
    /// - `Configure_Material` sets diffuse/emissive sources to MATERIAL,
    ///   lighting on, stage 0 on UV array 0 (its UV source) and stage 1 on
    ///   array 0 (`-1` -> 0);
    /// - the UV array is (U, 1 - V);
    /// - not two-sided, so the shader keeps culling.
    #[test]
    fn a_textured_quad_is_one_group() {
        let spec = MeshSpec::quad(b"QUAD", b"BOX");
        let mesh = load(&spec.to_w3d());
        let model = MeshModel::from_mesh(&mesh).unwrap();
        assert_eq!(model.pass_count, 1);
        let g = model.polygon_groups().unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!((g[0].textures, g[0].material, g[0].polygons.clone()), ([Some(0), None], Some(0), vec![0, 1]));
        assert_eq!(model.textures[0].name, b"quad.tga");
        let vm = &model.vertex_materials[0];
        assert_eq!((vm.diffuse_color_source, vm.emissive_color_source, vm.use_lighting), (D3DMCS_MATERIAL, D3DMCS_MATERIAL, true));
        assert_eq!(&vm.uv_source[..2], &[0, 0]);
        assert_eq!(vm.diffuse, [1.0, 128.0 / 255.0, 0.0]);
        assert_eq!(model.uv_array(0).unwrap()[2], [1.0, 0.0], "vertex 2 at (1, 1) -> (1, 1 - 1)");
        assert_eq!(g[0].shader.get_cull_mode(), 1);
        assert_eq!(model.winding_agrees_with_normals(), Some(1.0));
    }

    /// Predictions: a two-sided mesh loses culling on its shader; a pass with
    /// no vertex material gets a new default one (index 1, after the file's
    /// one), configured with lighting on; a pre-lit-vertex mesh configures
    /// with lighting off.
    #[test]
    fn two_sided_and_missing_materials() {
        let mut spec = MeshSpec::quad(b"QUAD", b"BOX");
        spec.attributes |= crate::formats::w3d::w3d_file::W3D_MESH_FLAG_TWO_SIDED;
        spec.vertex_material_on_pass = false;
        let model = MeshModel::from_mesh(&load(&spec.to_w3d())).unwrap();
        assert_eq!(model.passes[0].shader.get_cull_mode(), 0);
        assert_eq!(model.passes[0].material, Some(1));
        assert!(model.vertex_materials[1].use_lighting);
        assert_eq!(model.vertex_materials[1].diffuse, [1.0; 3]);
    }

    /// Predictions for a skin of four vertices on bones 1, 1, 2, 1 of a tree
    /// whose pivot 1 is translated by (10, 0, 0) and pivot 2 rotated 90
    /// degrees about z: vertex (1, 0, 0) on bone 1 goes to (11, 0, 0), its
    /// normal (0, 0, 1) stays (0, 0, 1) (translation dropped); vertex
    /// (1, 0, 0) on bone 2 goes to (0, 1, 0) (within rounding of
    /// sin/cos 90), its normal (1, 0, 0) to (0, 1, 0); a bone index past the
    /// tree is undefined.
    #[test]
    fn skin_vertices_follow_their_bones() {
        let mut spec = MeshSpec::quad(b"SKIN", b"BOX");
        spec.attributes |= crate::formats::w3d::w3d_file::W3D_MESH_FLAG_GEOMETRY_TYPE_SKIN;
        spec.vertices = vec![[1.0, 0.0, 0.0]; 4];
        spec.normals = vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        spec.bones = Some(vec![1, 1, 2, 1]);
        let model = MeshModel::from_mesh(&load(&spec.to_w3d())).unwrap();
        assert!(model.get_flag(mesh_flags::SKIN));
        let mut t1 = Matrix3D::IDENTITY;
        t1.translate(10.0, 0.0, 0.0);
        let r2 = Matrix3D::from_axis_angle(&Vector3::new(0.0, 0.0, 1.0), std::f32::consts::FRAC_PI_2);
        let mut tree = HTree { name: [0; 16], num_pivots: 3, pivots: vec![Pivot::default(); 3], scale_factor: 1.0 };
        tree.pivots[1].transform = t1;
        tree.pivots[2].transform = r2;
        let (v, n) = model.get_deformed_vertices(&tree).unwrap();
        assert_eq!(v[0], [11.0, 0.0, 0.0]);
        assert_eq!(n[0], [0.0, 0.0, 1.0]);
        assert!((v[2][0]).abs() < 1e-6 && (v[2][1] - 1.0).abs() < 1e-6, "{:?}", v[2]);
        assert!((n[2][1] - 1.0).abs() < 1e-6, "{:?}", n[2]);
        assert_eq!(v[3], [11.0, 0.0, 0.0]);
        let small = HTree { name: [0; 16], num_pivots: 2, pivots: vec![Pivot::default(); 2], scale_factor: 1.0 };
        assert_eq!(model.get_deformed_vertices(&small), Err(W3dUndefined::IndexOutOfRange.into()));
    }

    /// Predictions for vertex colours: a DCG array of mid grey (0x80808080)
    /// with the material diffuse (255, 128, 0) and nothing else used: the
    /// colour is multiplied by the diffuse (R 0x80, G 0x40, B 0), alpha by
    /// the opacity 1, and the material's diffuse source becomes COLOR1.
    #[test]
    fn vertex_colours_fold_the_diffuse() {
        let mut spec = MeshSpec::quad(b"QUAD", b"BOX");
        spec.dcg = Some(vec![0x8080_8080; 4]);
        spec.ambient = [0, 0, 0];
        let model = MeshModel::from_mesh(&load(&spec.to_w3d())).unwrap();
        let c = model.color_array[0].as_ref().unwrap()[0];
        assert_eq!(c >> 24, 0x80);
        assert_eq!((c >> 16) & 0xff, 0x80);
        assert_eq!((c >> 8) & 0xff, 0x40);
        assert_eq!(c & 0xff, 0);
        assert_eq!(model.vertex_materials[0].diffuse_color_source, D3DMCS_COLOR1);
        let _ = StageSpec::default();
    }
}
