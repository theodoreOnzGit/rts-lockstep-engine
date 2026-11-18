//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//! PARTLY DIFFERENTIAL-TESTED (#215, tests/differential_draft_shapes.rs): `render_ring`, `vis_render_ring` and `RingMeshClass::Generate` are NOT tested; the other members match (20,000 cases each).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/ringobj.cpp
//!
//! `RingRenderObjClass` members no earlier pass translated: the shared LOD mesh arrays and their cost table, the LOD
//! value array, the material set-up, the vertex / index buffer fill of `render_ring`, the default-value readers, the
//! cached box and `RingMeshClass::Set_Tiling`. `RingMeshClass::Generate` and the DX8 draw calls are [`RingHost`].
//! `vis_render_ring` is `WWASSERT(0)` upstream (never called): translated as the assert.

use crate::formats::w3d::part_buf::{AT_MAX_LOD, AT_MIN_LOD};
use crate::logic::modules::plant;

/// `RING_NUM_LOD`, `RING_LOWEST_LOD`, `RING_HIGHEST_LOD` (ringobj.h:96-98).
pub const RING_NUM_LOD: usize = 20;
pub const RING_LOWEST_LOD: f32 = 10.0;
pub const RING_HIGHEST_LOD: f32 = 50.0;

/// The `RingMeshClass` arrays `render_ring` reads.
#[derive(Clone, Debug, Default)]
pub struct RingMesh {
    pub slices: i32,
    pub tile_count: i32,
    pub vertex_ct: usize,
    pub face_ct: usize,
    pub vtx: Vec<[f32; 3]>,
    pub vtx_normal: Vec<[f32; 3]>,
    pub vtx_uv: Vec<[f32; 2]>,
    pub tri_poly: Vec<[u16; 3]>,
}

impl RingMesh {
    /// `RingMeshClass::Set_Tiling` (ringobj.cpp:1457): re-derive the U coordinates of the vertex pairs.
    pub fn set_tiling(&mut self, count: i32) {
        if self.tile_count != count || plant(11742) {
            let old_tile_count = self.tile_count;
            self.tile_count = count;
            let u_inc = if plant(11743) { old_tile_count as f32 / self.slices as f32 } else { self.tile_count as f32 / self.slices as f32 };
            let mut u_value = if plant(11745) { u_inc } else { 0.0f32 };
            let mut index = 0;
            while index < self.vertex_ct {
                self.vtx_uv[index] = [u_value, 0.0];
                self.vtx_uv[index + 1] = [u_value, 1.0];
                u_value += u_inc;
                index += if plant(11744) { 1 } else { 2 };
            }
        }
    }
}

/// One vertex as `VertexFormatXYZNDUV2` (position, normal, diffuse, uv1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexXyzNduv2 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub diffuse: u32,
    pub u1: f32,
    pub v1: f32,
}

/// The shared static tables (`RingMeshArray`, `RingLODCosts`, `Ring_Array_Valid`).
#[derive(Clone, Debug)]
pub struct RingShared {
    pub mesh_array: Vec<RingMesh>,
    pub lod_costs: [f32; RING_NUM_LOD + 1],
    pub array_valid: bool,
}

impl Default for RingShared {
    fn default() -> Self {
        RingShared { mesh_array: vec![RingMesh::default(); RING_NUM_LOD], lod_costs: [0.0; RING_NUM_LOD + 1], array_valid: false }
    }
}

/// What the ring code calls out: `RingMeshClass::Generate(1.0, size)` and the draw path.
pub trait RingHost {
    /// `RingMeshArray[i].Generate(1.0f, size)`; the mesh is written into `shared.mesh_array[i]`.
    fn generate_ring_mesh(&mut self, mesh: &mut RingMesh, size: f32);
    /// `DX8Wrapper::Convert_Color(color, alpha)`.
    fn convert_color(&self, color: [f32; 3], alpha: f32) -> u32;
    /// Set shader / texture / material, set the vertex and index buffers, then draw or insert into the sorting renderer.
    fn draw_ring(&mut self, verts: &[VertexXyzNduv2], inds: &[u16], sort: bool, face_ct: usize, vertex_ct: usize);
}

impl RingShared {
    /// `RingRenderObjClass::Generate_Shared_Mesh_Arrays` (ringobj.cpp:367): build the 20 LOD meshes once, from 10 to 50
    /// segments in equal steps, with the polygon count of each as its cost (`RingLODCosts[0]` is the null LOD).
    pub fn generate_shared_mesh_arrays<H: RingHost>(&mut self, host: &mut H) {
        if !self.array_valid {
            let mut size = RING_LOWEST_LOD;
            let mut step = RING_HIGHEST_LOD - RING_LOWEST_LOD;
            step /= RING_NUM_LOD as f32;
            self.lod_costs[0] = if plant(11728) { 0.00001 } else { 0.000001 };
            for i in 0..RING_NUM_LOD {
                if plant(11727) {
                    size += step;
                }
                host.generate_ring_mesh(&mut self.mesh_array[i], size);
                self.lod_costs[if plant(11729) { i } else { i + 1 }] = self.mesh_array[i].face_ct as f32;
                if !plant(11727) {
                    size += step;
                }
            }
            self.array_valid = !plant(11730);
        }
    }

    /// `RingRenderObjClass::calculate_value_array` (ringobj.cpp:394): the LOD value (benefit per polygon) table.
    pub fn calculate_value_array(&self, screen_area: f32, lod_bias: f32, values: &mut [f32]) {
        values[0] = AT_MIN_LOD;
        for lod in 1..=(if plant(11735) { RING_NUM_LOD - 1 } else { RING_NUM_LOD }) {
            let polycount = self.lod_costs[lod];
            let benefit_factor = if plant(11733) { 1.0 - (0.5 / polycount) } else { 1.0 - (0.5 / (polycount * polycount)) };
            values[lod] = (benefit_factor * screen_area * lod_bias) / polycount;
        }
        values[RING_NUM_LOD + 1] = AT_MAX_LOD; // Post-inc value will flag max LOD.
    }
}

/// `RingRenderObjClass::Init_Material` (ringobj.cpp:418): the ring's vertex material: emissive white, opacity 0.25, no
/// shininess; the shader is `ShaderClass::_PresetAlphaShader`. Returns (ambient, diffuse, specular, emissive, opacity,
/// shininess).
pub fn init_material() -> ([f32; 3], [f32; 3], [f32; 3], [f32; 3], f32, f32) {
    ([0.0; 3], [0.0; 3], [0.0; 3], [1.0; 3], if plant(11746) { 0.5 } else { 0.25 }, 0.0)
}

/// `RingRenderObjClass::render_ring` (ringobj.cpp:521): `ring` is `RingMeshArray[CurrentLOD - 1]`. The vertices take one
/// colour: `Alpha * Color` with alpha 1 when the destination blend is `DSTBLEND_ONE`, else `Color` with `Alpha`; texture
/// coordinates are written only with a texture. `sort` is `dst_blend != ZERO && alpha_test disabled && sorting enabled`
/// (the third term is cut from this copy of the line; the caller computes the whole expression).
pub fn render_ring<H: RingHost>(host: &mut H, current_lod: usize, ring: &RingMesh, has_texture: bool, dst_blend_is_one: bool, color: [f32; 3], alpha: f32, sort: bool) {
    if current_lod == 0 {
        debug_assert!(false);
        return;
    }
    let packed = if dst_blend_is_one {
        host.convert_color([alpha * color[0], alpha * color[1], alpha * color[2]], 1.0)
    } else {
        host.convert_color(color, alpha)
    };
    let mut verts = Vec::with_capacity(ring.vertex_ct);
    for i in 0..ring.vertex_ct {
        let mut v = VertexXyzNduv2 {
            x: ring.vtx[i][0],
            y: ring.vtx[i][1],
            z: ring.vtx[i][2],
            nx: ring.vtx_normal[i][0],
            ny: ring.vtx_normal[i][1],
            nz: ring.vtx_normal[i][2],
            diffuse: packed,
            ..Default::default()
        };
        if has_texture {
            v.u1 = ring.vtx_uv[i][0];
            v.v1 = ring.vtx_uv[i][1];
        }
        verts.push(v);
    }
    let mut inds = Vec::with_capacity(ring.face_ct * 3);
    for i in 0..ring.face_ct {
        inds.push(ring.tri_poly[i][0]);
        inds.push(ring.tri_poly[i][1]);
        inds.push(ring.tri_poly[i][2]);
    }
    host.draw_ring(&verts, &inds, sort, ring.face_ct, ring.vertex_ct);
}

/// `RingRenderObjClass::vis_render_ring` (ringobj.cpp:617): `WWASSERT(0)`, never reached.
pub fn vis_render_ring() {
    debug_assert!(false);
}

/// `RingRenderObjClass::Get_Default_Color` (ringobj.cpp:752): the first colour key, else the current colour.
pub fn get_default_color(first_key: Option<[f32; 3]>, color: [f32; 3]) -> [f32; 3] {
    if plant(11749) {
        return color;
    }
    first_key.unwrap_or(color)
}

/// `RingRenderObjClass::Get_Default_Alpha` (ringobj.cpp:778).
pub fn get_default_alpha(first_key: Option<f32>, alpha: f32) -> f32 {
    first_key.unwrap_or(alpha)
}

/// `RingRenderObjClass::Get_Default_Inner_Scale` (ringobj.cpp:804).
pub fn get_default_inner_scale(first_key: Option<[f32; 2]>, inner_scale: [f32; 2]) -> [f32; 2] {
    first_key.unwrap_or(inner_scale)
}

/// `RingRenderObjClass::Get_Default_Outer_Scale` (ringobj.cpp:830).
pub fn get_default_outer_scale(first_key: Option<[f32; 2]>, outer_scale: [f32; 2]) -> [f32; 2] {
    first_key.unwrap_or(outer_scale)
}

/// `RingRenderObjClass::update_cached_box` (ringobj.cpp:919): `(centre, extent)` from the translation and the object-space
/// centre and extent.
pub fn update_cached_box(translation: [f32; 3], obj_space_center: [f32; 3], obj_space_extent: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    (
        [translation[0] + obj_space_center[0], translation[1] + if plant(11737) { obj_space_center[0] } else { obj_space_center[1] }, translation[2] + obj_space_center[2]],
        obj_space_extent,
    )
}
