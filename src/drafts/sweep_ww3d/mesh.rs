//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/mesh.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/mesh.h
//!
//! `MeshClass` members no earlier pass translated: `Get_W3D_Flags`, `Get_User_Text`, `Get_Model`, `Get_Draw_Call_Count` and
//! `Render_Material_Pass`. The verified port draws meshes through `formats::w3d::render` and the client's mesh draw, and
//! holds the model as `formats::w3d::meshmdl`; [`MeshModelView`] is the part of `MeshModelClass` these read, and
//! [`MeshPassHost`] the DX8 / material-pass calls of `Render_Material_Pass` (device-only, not translated: each is one
//! line of the contract).

use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// What `MeshClass` reads of `Model` (`MeshModelClass`).
#[derive(Clone, Debug, Default)]
pub struct MeshModelView {
    /// `W3dAttributes`.
    pub w3d_attributes: u32,
    /// `Get_User_Text()`: `None` is NULL.
    pub user_text: Option<String>,
    /// `PolygonRendererList.Count()`.
    pub polygon_renderer_count: i32,
    /// `MatInfo` (NULL when `None`) with its `Texture_Count()`.
    pub mat_info_texture_count: Option<i32>,
}

/// `MeshClass::Get_W3D_Flags()` (mesh.cpp:359): `Model->W3dAttributes`.
pub fn get_w3d_flags(model: &MeshModelView) -> u32 {
    model.w3d_attributes
}

/// `MeshClass::Get_User_Text()` (mesh.cpp:377): `Model->Get_User_Text()`.
pub fn get_user_text(model: &MeshModelView) -> Option<&str> {
    model.user_text.as_deref()
}

/// `MeshClass::Get_Model()` (mesh.cpp:419): the model with an `Add_Ref` when not NULL. The reference count is the host's
/// (an `Arc` clone); the handle is returned as is.
pub fn get_model<T: Clone>(model: &Option<T>) -> Option<T> {
    model.clone()
}

/// `MeshClass::Get_Draw_Call_Count()` (mesh.cpp:1579): the polygon renderer count if any, else the texture count of the
/// material info if positive ("e.g. dont have prs when sorting"), else 1; 0 for a NULL model.
pub fn get_draw_call_count(model: Option<&MeshModelView>) -> i32 {
    if let Some(model) = model {
        // Prefer to return the number of polygon renderers
        let prcount = model.polygon_renderer_count;
        if prcount > 0 {
            return prcount;
        }

        // Otherwise if we have textures, return the number of textures (e.g. dont have prs when sorting)
        if let Some(tc) = model.mat_info_texture_count {
            if tc > 0 {
                return tc;
            }
        }

        // Otherwise, return 1
        1
    } else {
        0
    }
}

/// The `MeshClass` fields `Render_Material_Pass` reads.
#[derive(Clone, Debug)]
pub struct MeshPassState {
    /// `LightEnvironment != NULL`.
    pub has_light_environment: bool,
    /// `m_materialPassAlphaOverride`.
    pub material_pass_alpha_override: f32,
    /// `m_materialPassEmissiveOverride`.
    pub material_pass_emissive_override: f32,
    /// `Transform`.
    pub transform: Matrix3D,
    /// `BaseVertexOffset`.
    pub base_vertex_offset: i32,
    /// `Model->Get_Flag(MeshModelClass::SKIN)`.
    pub is_skin: bool,
    /// `pass->Get_Cull_Volume() != NULL`.
    pub pass_has_cull_volume: bool,
}

/// The calls of `Render_Material_Pass`.
pub trait MeshPassHost {
    /// `DX8Wrapper::Set_Light_Environment(LightEnvironment)`.
    fn set_light_environment(&mut self);
    /// `MaterialPassClass::Is_Per_Polygon_Culling_Enabled()`.
    fn is_per_polygon_culling_enabled(&self) -> bool;
    /// `pass->Peek_Material()`: whether the pass has a vertex material.
    fn pass_has_material(&self) -> bool;
    /// `mat->Get_Opacity()`.
    fn material_get_opacity(&self) -> f32;
    /// `mat->Set_Opacity(opacity)`.
    fn material_set_opacity(&mut self, opacity: f32);
    /// `mat->Get_Emissive(&emissive)`.
    fn material_get_emissive(&self) -> Vector3;
    /// `mat->Set_Emissive(emissive)`.
    fn material_set_emissive(&mut self, emissive: &Vector3);
    /// `pass->Install_Materials()`.
    fn pass_install_materials(&mut self);
    /// `pass->UnInstall_Materials()`.
    fn pass_uninstall_materials(&mut self);
    /// `DX8Wrapper::Set_Index_Buffer(ib, 0)`.
    fn set_index_buffer(&mut self);
    /// `DX8Wrapper::Set_World_Identity()`.
    fn set_world_identity(&mut self);
    /// `DX8Wrapper::Set_Transform(D3DTS_WORLD, Transform)`.
    fn set_world_transform(&mut self, transform: &Matrix3D);
    /// `for each DX8PolygonRendererClass in Model->PolygonRendererList: if (Get_Pass() == 0) Render(BaseVertexOffset)`.
    fn render_pass0_polygon_renderers(&mut self, base_vertex_offset: i32);
    /// The per-polygon-culling branch, from `temp_apt.Delete_All(false)` to the `Draw_Triangles` and the
    /// `UnInstall_Materials()`: the active polygon table from the cull volume against the model's transform inverse
    /// (`Get_Orthogonal_Inverse`, `OBBoxClass::Transform`, `Basis.Get_Z_Vector` negated, `Generate_Rigid_APT` with the
    /// cull tree if `Model->Has_Cull_Tree()`), the dynamic index buffer of the APT's triangles
    /// (`BUFFER_TYPE_DYNAMIC_SORTING` when the model is `SORT` and `WW3D::Is_Sorting_Enabled()`), and
    /// `Draw_Triangles(0, temp_apt.Count(), min_v, max_v-min_v+1)`: all device work, not translated.
    fn render_culled_polygons(&mut self);
}

/// `MeshClass::Render_Material_Pass(pass, ib)` (mesh.cpp:823): renders the model once with an extra material pass.
/// "Added to allow dynamic opacity on additional render passed without having to create a new material pass per object
/// instance. -MW": the pass's (possibly shared) material has its opacity / emissive overridden for the draw and restored
/// afterwards.
pub fn render_material_pass<H: MeshPassHost>(state: &MeshPassState, host: &mut H) {
    let mut old_opacity: f32 = -1.0;
    let mut old_emissive = Vector3::new(-1.0, -1.0, -1.0);

    if state.has_light_environment {
        host.set_light_environment();
    }

    if state.is_skin {
        // In the case of skin meshes, we need to render our polys with the identity transform
        apply_overrides(state, host, &mut old_opacity, &mut old_emissive);
        host.pass_install_materials();
        host.set_index_buffer();

        host.set_world_identity();

        host.render_pass0_polygon_renderers(state.base_vertex_offset);

        restore_overrides(host, old_opacity, &old_emissive);
        // MW: Need uninstall custom materials in case they leave D3D in unknown state
        host.pass_uninstall_materials();
    } else if state.pass_has_cull_volume && host.is_per_polygon_culling_enabled() {
        // Generate the APT, spew its triangles into a dynamic index buffer and draw (only when the APT is not empty;
        // the host's `render_culled_polygons` includes the `if (temp_apt.Count() > 0)`).
        host.render_culled_polygons();
    } else {
        // Normal mesh case, render polys with this mesh's transform
        apply_overrides(state, host, &mut old_opacity, &mut old_emissive);
        host.pass_install_materials();
        host.set_index_buffer();

        host.set_world_transform(&state.transform);

        host.render_pass0_polygon_renderers(state.base_vertex_offset);

        restore_overrides(host, old_opacity, &old_emissive);
        // MW: Need uninstall custom materials in case they leave D3D in unknown state
        host.pass_uninstall_materials();
    }
}

/// The two `if (m_materialPassAlphaOverride != 1.0f)` / `if (m_materialPassEmissiveOverride != 1.0f)` blocks.
fn apply_overrides<H: MeshPassHost>(state: &MeshPassState, host: &mut H, old_opacity: &mut f32, old_emissive: &mut Vector3) {
    if state.material_pass_alpha_override != 1.0 {
        if host.pass_has_material() {
            *old_opacity = host.material_get_opacity();
            host.material_set_opacity(state.material_pass_alpha_override);
        }
    }
    if state.material_pass_emissive_override != 1.0 {
        if host.pass_has_material() {
            *old_emissive = host.material_get_emissive();
            let scaled = *old_emissive * state.material_pass_emissive_override;
            host.material_set_emissive(&scaled);
        }
    }
}

/// "opacity was modified for this mesh instance, so need to restore the material setting which may be shared among other
/// instances", and the same for the emissive colour.
fn restore_overrides<H: MeshPassHost>(host: &mut H, old_opacity: f32, old_emissive: &Vector3) {
    if old_opacity >= 0.0 {
        host.material_set_opacity(old_opacity);
    }
    if old_emissive.x >= 0.0 {
        host.material_set_emissive(old_emissive);
    }
}
