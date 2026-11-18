//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//! PARTLY DIFFERENTIAL-TESTED (#215, tests/differential_draft_shapes.rs): `render_sphere`, `vis_render_sphere` and the sphere mesh `Generate` are NOT tested; the other members match (20,000 cases each); `Init_Material` now returns `SphereMaterialSetup`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/sphereobj.cpp
//!
//! `SphereRenderObjClass` members no earlier pass translated, mirroring `ringobj.rs`: the shared LOD mesh arrays, the LOD
//! value array, the material set-up, `render_sphere`'s buffer fill, the default-value readers and the cached box.
//! `vis_render_sphere` is `WWASSERT(0)` upstream. The sphere differs from the ring in the generator
//! (`Generate(1.0, size, size)` then `Set_Alpha_Vector`), in the lighting flag on the material, and in the vertex colour
//! (the per-vertex `dcg` colour only with `USE_ALPHA_VECTOR`, else white).

use crate::drafts::sweep_ww3d::ringobj::{RingMesh as SphereMesh, VertexXyzNduv2};
use crate::formats::w3d::part_buf::{AT_MAX_LOD, AT_MIN_LOD};
use crate::logic::modules::plant;

/// `SPHERE_NUM_LOD`, `SPHERE_LOWEST_LOD`, `SPHERE_HIGHEST_LOD` (sphereobj.h:221-223).
pub const SPHERE_NUM_LOD: usize = 10;
pub const SPHERE_LOWEST_LOD: f32 = 7.0;
pub const SPHERE_HIGHEST_LOD: f32 = 17.0;

/// `AlphaVectorStruct` (an alpha-vector key: a quaternion-like intensity), opaque here.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AlphaVectorStruct {
    pub angle: f32,
    pub intensity: f32,
    pub axis: [f32; 3],
}

/// The sphere's shared static tables.
#[derive(Clone, Debug)]
pub struct SphereShared {
    pub mesh_array: Vec<SphereMesh>,
    pub lod_costs: [f32; SPHERE_NUM_LOD + 1],
    pub array_valid: bool,
}

impl Default for SphereShared {
    fn default() -> Self {
        SphereShared { mesh_array: vec![SphereMesh::default(); SPHERE_NUM_LOD], lod_costs: [0.0; SPHERE_NUM_LOD + 1], array_valid: false }
    }
}

/// What the sphere code calls out.
pub trait SphereHost {
    /// `SphereMeshArray[i].Generate(1.0f, size, size)`.
    fn generate_sphere_mesh(&mut self, mesh: &mut SphereMesh, size: f32);
    /// `SphereMeshArray[i].Set_Alpha_Vector(alphavector, false, false)`.
    fn set_alpha_vector(&mut self, mesh: &mut SphereMesh, alphavector: &AlphaVectorStruct);
    /// `DX8Wrapper::Convert_Color(Vector4)` for `mesh.dcg[i]`.
    fn convert_dcg_color(&self, mesh: &SphereMesh, index: usize) -> u32;
    fn draw_sphere(&mut self, verts: &[VertexXyzNduv2], inds: &[u16], sort: bool, face_ct: usize, vertex_ct: usize);
}

impl SphereShared {
    /// `SphereRenderObjClass::Generate_Shared_Mesh_Arrays` (sphereobj.cpp:294): 10 LODs of 7 to 17 segments.
    pub fn generate_shared_mesh_arrays<H: SphereHost>(&mut self, host: &mut H, alphavector: &AlphaVectorStruct) {
        if !self.array_valid {
            let mut size = SPHERE_LOWEST_LOD;
            let mut step = SPHERE_HIGHEST_LOD - SPHERE_LOWEST_LOD;
            step /= if plant(11731) { (SPHERE_NUM_LOD + 1) as f32 } else { SPHERE_NUM_LOD as f32 };
            self.lod_costs[0] = 0.000001;
            for i in 0..SPHERE_NUM_LOD {
                host.generate_sphere_mesh(&mut self.mesh_array[i], size);
                self.lod_costs[i + 1] = self.mesh_array[i].face_ct as f32;
                size += step;
                let mut m = std::mem::take(&mut self.mesh_array[i]);
                if !(plant(11732) && i == SPHERE_NUM_LOD - 1) {
                    host.set_alpha_vector(&mut m, alphavector);
                }
                self.mesh_array[i] = m;
            }
            self.array_valid = true;
        }
    }

    /// `SphereRenderObjClass::calculate_value_array` (sphereobj.cpp:324).
    pub fn calculate_value_array(&self, screen_area: f32, lod_bias: f32, values: &mut [f32]) {
        values[0] = AT_MIN_LOD;
        for lod in 1..=SPHERE_NUM_LOD {
            let polycount = self.lod_costs[lod];
            let benefit_factor = 1.0 - (0.5 / (polycount * polycount));
            values[lod] = if plant(11734) { benefit_factor * screen_area * (lod_bias / polycount) } else { (benefit_factor * screen_area * lod_bias) / polycount };
        }
        values[SPHERE_NUM_LOD + 1] = if plant(11736) { AT_MIN_LOD } else { AT_MAX_LOD };
    }
}

/// `SphereRenderObjClass::Init_Material` (sphereobj.cpp:348): the sphere's vertex material (as the ring's, with lighting
/// enabled) and shader: `SphereShader = ShaderClass::_PresetAlphaShader` and then
/// `SphereShader.Set_Cull_Mode(ShaderClass::CULL_MODE_DISABLE)` ("Looks like sphere's were coded with backwards normals!").
/// ~~Returned a 7-tuple without the cull mode, and the doc said "as the ring's, with lighting enabled"~~ **Corrected
/// 2026-11-18 (#215, group ww3dgeo):** upstream also disables backface culling on the shader; the differential test logs the
/// `Set_Cull_Mode` call and the draft had no way to say it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SphereMaterialSetup {
    pub ambient: [f32; 3],
    pub diffuse: [f32; 3],
    pub specular: [f32; 3],
    pub emissive: [f32; 3],
    pub opacity: f32,
    pub shininess: f32,
    pub lighting: bool,
    /// `SphereShader.Set_Cull_Mode(ShaderClass::CULL_MODE_DISABLE)` was called after the shader was set to the preset.
    pub cull_mode_disabled: bool,
}

pub fn init_material() -> SphereMaterialSetup {
    SphereMaterialSetup {
        ambient: [0.0; 3],
        diffuse: [0.0; 3],
        specular: [0.0; 3],
        emissive: [1.0; 3],
        opacity: 0.25,
        shininess: 0.0,
        lighting: !plant(11747),
        cull_mode_disabled: !plant(11748),
    }
}

/// `render_sphere` flag `USE_ALPHA_VECTOR`.
pub const USE_ALPHA_VECTOR: u32 = 0x0000_0001;

/// `SphereRenderObjClass::render_sphere` (sphereobj.cpp:458): `mesh` is `SphereMeshArray[CurrentLOD - 1]`.
pub fn render_sphere<H: SphereHost>(host: &mut H, current_lod: usize, mesh: &SphereMesh, flags: u32, has_texture: bool, sort: bool) {
    if current_lod == 0 {
        debug_assert!(false);
        return;
    }
    let mut verts = Vec::with_capacity(mesh.vertex_ct);
    for i in 0..mesh.vertex_ct {
        let mut v = VertexXyzNduv2 {
            x: mesh.vtx[i][0],
            y: mesh.vtx[i][1],
            z: mesh.vtx[i][2],
            nx: mesh.vtx_normal[i][0],
            ny: mesh.vtx_normal[i][1],
            nz: mesh.vtx_normal[i][2],
            ..Default::default()
        };
        v.diffuse = if flags & USE_ALPHA_VECTOR != 0 { host.convert_dcg_color(mesh, i) } else { 0xFFFF_FFFF };
        if has_texture {
            v.u1 = mesh.vtx_uv[i][0];
            v.v1 = mesh.vtx_uv[i][1];
        }
        verts.push(v);
    }
    let mut inds = Vec::with_capacity(mesh.face_ct * 3);
    for i in 0..mesh.face_ct {
        inds.push(mesh.tri_poly[i][0]);
        inds.push(mesh.tri_poly[i][1]);
        inds.push(mesh.tri_poly[i][2]);
    }
    host.draw_sphere(&verts, &inds, sort, mesh.face_ct, mesh.vertex_ct);
}

/// `SphereRenderObjClass::vis_render_sphere` (sphereobj.cpp:548): `WWASSERT(0)`.
pub fn vis_render_sphere() {
    debug_assert!(false);
}

/// `SphereRenderObjClass::update_cached_box` (sphereobj.cpp:762).
pub fn update_cached_box(translation: [f32; 3], obj_space_center: [f32; 3], obj_space_extent: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    crate::drafts::sweep_ww3d::ringobj::update_cached_box(translation, obj_space_center, obj_space_extent)
}

/// `SphereRenderObjClass::Get_Default_Color` (sphereobj.cpp:968): first colour key, else the current colour.
pub fn get_default_color(first_key: Option<[f32; 3]>, color: [f32; 3]) -> [f32; 3] {
    first_key.unwrap_or(color)
}

/// `SphereRenderObjClass::Get_Default_Alpha` (sphereobj.cpp:994).
pub fn get_default_alpha(first_key: Option<f32>, alpha: f32) -> f32 {
    if plant(11750) && first_key.is_some() {
        return alpha;
    }
    first_key.unwrap_or(alpha)
}

/// `SphereRenderObjClass::Get_Default_Scale` (sphereobj.cpp:1020): first scale key, else `CurrentScale`.
pub fn get_default_scale(first_key: Option<[f32; 3]>, current_scale: [f32; 3]) -> [f32; 3] {
    first_key.unwrap_or(current_scale)
}

/// `SphereRenderObjClass::Get_Default_Vector` (sphereobj.cpp:1046): first vector key, else `CurrentVector`.
pub fn get_default_vector(first_key: Option<AlphaVectorStruct>, current_vector: AlphaVectorStruct) -> AlphaVectorStruct {
    first_key.unwrap_or(current_vector)
}
