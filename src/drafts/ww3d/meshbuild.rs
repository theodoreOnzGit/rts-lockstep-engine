//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshbuild.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshbuild.h
//!   (UniqueArrayClass / HashCalculatorClass from WWLib/uarray.h, hashcalc.h, which the face de-duplication uses)
//!
//! Notes for the differential pass:
//! - `qsort` is not stable and its tie order is the C runtime's. Upstream's runtime was MSVC's; the drafts use a
//!   stable sort (`sort_by`). Faces with equal texture/material keys, and verts with equal bone/material keys, may
//!   therefore come out in a different order than the original binary produced. Flagged, not resolved.
//! - `(int)float` casts of inf/NaN (a zero `Extent` in the hash insert) are undefined behaviour in C++ and give
//!   `INT_MIN` on x86; [`cvt_trunc`] reproduces that, where Rust's `as i32` would saturate.
//! - `WorldInfoClass` (smoothing across neighbouring meshes, `Are_Meshes_Smoothed`, `Get_Shared_Vertex_Normal`) is
//!   not in the verified port; `world_info` is a placeholder trait-free closure-less flag. See [`WorldInfoStub`].
//! - `Compute_Tangent_Basis` is declared in meshbuild.h but defined nowhere in the upstream tree: not translated.

use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;

/// `const float EPSILON = 0.0001f;` (meshbuild.cpp:68).
const EPSILON: f32 = 0.0001f32;

/// `MeshBuilderClass::MAX_PASSES`.
pub const MAX_PASSES: usize = 4;
/// `MeshBuilderClass::MAX_STAGES`.
pub const MAX_STAGES: usize = 8;
/// `MeshBuilderClass::STATE_ACCEPTING_INPUT`.
pub const STATE_ACCEPTING_INPUT: i32 = 0;
/// `MeshBuilderClass::STATE_MESH_PROCESSED`.
pub const STATE_MESH_PROCESSED: i32 = 1;

/// x86 `cvttss2si` semantics of the C cast `(int)x`: out of range and NaN give `INT_MIN` ("integer indefinite").
fn cvt_trunc(x: f32) -> i32 {
    if x.is_nan() || x >= 2147483648.0f32 || x < -2147483648.0f32 {
        i32::MIN
    } else {
        x as i32
    }
}

/// Placeholder for `WorldInfoClass *WorldInfo` (worldinfo.h, not in the verified port). Only the two calls
/// `Compute_Vertex_Normals` makes are modelled, as plain data the owner fills in.
#[derive(Clone, Debug, Default)]
pub struct WorldInfoStub {
    /// `WorldInfoClass::Are_Meshes_Smoothed`.
    pub meshes_smoothed: bool,
    /// Results of `WorldInfoClass::Get_Shared_Vertex_Normal(position, smooth_group)` are supplied by this function
    /// pointer (a plain `fn`, not a trait object).
    pub shared_vertex_normal: Option<fn(&Vector3, i32) -> Vector3>,
}

/// `MeshBuilderClass::VertClass` (meshbuild.h).
#[derive(Clone, Debug)]
pub struct VertClass {
    pub position: Vector3,
    pub normal: Vector3,
    pub sm_group: i32,
    pub id: i32,
    pub bone_index: i32,
    pub max_vert_col_index: i32,
    pub tex_coord: [[Vector2; MAX_STAGES]; MAX_PASSES],
    pub diffuse_color: [Vector3; MAX_PASSES],
    pub specular_color: [Vector3; MAX_PASSES],
    pub diffuse_illumination: [Vector3; MAX_PASSES],
    pub alpha: [f32; MAX_PASSES],
    pub vertex_material_index: [i32; MAX_PASSES],
    pub attribute0: i32,
    pub attribute1: i32,
    /// Set up by the mesh builder: smooth bits that were on in all faces that contributed to this final vertex.
    pub shared_sm_group: i32,
    pub unique_index: i32,
    pub shade_index: i32,
    /// `VertClass *NextHash`, as an index into the unique-vertex array.
    pub next_hash: Option<usize>,
}

impl VertClass {
    /// `VertClass::VertClass` -> `Reset` (meshbuild.cpp:200).
    pub fn new() -> Self {
        let mut v = VertClass {
            position: Vector3::new(0.0, 0.0, 0.0),
            normal: Vector3::new(0.0, 0.0, 0.0),
            sm_group: 0,
            id: 0,
            bone_index: 0,
            max_vert_col_index: 0,
            tex_coord: [[Vector2::new(0.0, 0.0); MAX_STAGES]; MAX_PASSES],
            diffuse_color: [Vector3::new(1.0, 1.0, 1.0); MAX_PASSES],
            specular_color: [Vector3::new(1.0, 1.0, 1.0); MAX_PASSES],
            diffuse_illumination: [Vector3::new(0.0, 0.0, 0.0); MAX_PASSES],
            alpha: [1.0; MAX_PASSES],
            vertex_material_index: [-1; MAX_PASSES],
            attribute0: 0,
            attribute1: 0,
            shared_sm_group: 0,
            unique_index: 0,
            shade_index: 0,
            next_hash: None,
        };
        v.reset();
        v
    }

    /// `MeshBuilderClass::VertClass::Reset` (meshbuild.cpp:200).
    pub fn reset(&mut self) {
        self.position.set(0.0, 0.0, 0.0);
        self.normal.set(0.0, 0.0, 0.0);
        self.sm_group = 0;
        self.id = 0;
        self.max_vert_col_index = 0;
        for pass in 0..MAX_PASSES {
            self.diffuse_color[pass].set(1.0, 1.0, 1.0);
            self.specular_color[pass].set(1.0, 1.0, 1.0);
            self.diffuse_illumination[pass].set(0.0, 0.0, 0.0);
            self.alpha[pass] = 1.0;
            self.vertex_material_index[pass] = -1;
            for stage in 0..MAX_STAGES {
                self.tex_coord[pass][stage].set(0.0, 0.0);
            }
        }
        self.bone_index = 0;
        self.attribute0 = 0;
        self.attribute1 = 0;
        self.unique_index = 0;
        self.shade_index = 0;
        self.next_hash = None;
    }
}

impl Default for VertClass {
    fn default() -> Self {
        Self::new()
    }
}

/// `MeshBuilderClass::FaceClass` (meshbuild.h).
#[derive(Clone, Debug)]
pub struct FaceClass {
    pub verts: [VertClass; 3],
    pub sm_group: i32,
    pub index: i32,
    pub attributes: i32,
    pub texture_index: [[i32; MAX_STAGES]; MAX_PASSES],
    pub shader_index: [i32; MAX_PASSES],
    pub surface_type: u32,
    /// Set by builder: index of addition.
    pub add_index: i32,
    /// Set by builder: "optimized" vertex indices.
    pub vert_idx: [i32; 3],
    /// Set by builder: face normal.
    pub normal: Vector3,
    /// Set by builder: plane distance.
    pub dist: f32,
}

impl FaceClass {
    /// `FaceClass::FaceClass` -> `Reset`.
    pub fn new() -> Self {
        let mut f = FaceClass {
            verts: [VertClass::new(), VertClass::new(), VertClass::new()],
            sm_group: 0,
            index: 0,
            attributes: 0,
            texture_index: [[-1; MAX_STAGES]; MAX_PASSES],
            shader_index: [-1; MAX_PASSES],
            surface_type: 0,
            add_index: 0,
            vert_idx: [0; 3],
            normal: Vector3::new(0.0, 0.0, 0.0),
            dist: 0.0,
        };
        f.reset();
        f
    }

    /// `MeshBuilderClass::FaceClass::Reset` (meshbuild.cpp:420).
    pub fn reset(&mut self) {
        for i in 0..3 {
            self.verts[i].reset();
            self.vert_idx[i] = 0;
        }
        self.sm_group = 0;
        self.index = 0;
        self.attributes = 0;
        self.surface_type = 0;
        for pass in 0..MAX_PASSES {
            for stage in 0..MAX_STAGES {
                self.texture_index[pass][stage] = -1;
            }
            self.shader_index[pass] = -1;
        }
        self.add_index = 0;
        self.normal.set(0.0, 0.0, 0.0);
        self.dist = 0.0;
    }

    /// `MeshBuilderClass::FaceClass::Compute_Plane` (meshbuild.cpp:448): non-`ALLOW_TEMPORARIES` branch.
    pub fn compute_plane(&mut self) {
        let a = self.verts[1].position - self.verts[0].position;
        let b = self.verts[2].position - self.verts[0].position;
        self.normal = Vector3::normalized_cross_product(&a, &b);
        self.dist = Vector3::dot_product(&self.normal, &self.verts[0].position);
    }

    /// `MeshBuilderClass::FaceClass::Is_Degenerate` (meshbuild.cpp:468).
    pub fn is_degenerate(&self) -> bool {
        for v0 in 0..3 {
            for v1 in (v0 + 1)..3 {
                if self.vert_idx[v0] == self.vert_idx[v1] {
                    return true;
                }
                if self.verts[v0].position.x == self.verts[v1].position.x
                    && self.verts[v0].position.y == self.verts[v1].position.y
                    && self.verts[v0].position.z == self.verts[v1].position.z
                {
                    return true;
                }
            }
        }
        false
    }
}

impl Default for FaceClass {
    fn default() -> Self {
        Self::new()
    }
}

/// `MeshBuilderClass::MeshStatsStruct` (meshbuild.h).
#[derive(Clone, Debug)]
pub struct MeshStatsStruct {
    pub has_texture: [[bool; MAX_STAGES]; MAX_PASSES],
    pub has_shader: [bool; MAX_PASSES],
    pub has_vertex_material: [bool; MAX_PASSES],
    pub has_per_poly_texture: [[bool; MAX_STAGES]; MAX_PASSES],
    pub has_per_poly_shader: [bool; MAX_PASSES],
    pub has_per_vertex_material: [bool; MAX_PASSES],
    pub has_diffuse_color: [bool; MAX_PASSES],
    pub has_specular_color: [bool; MAX_PASSES],
    pub has_diffuse_illumination: [bool; MAX_PASSES],
    pub has_tex_coords: [[bool; MAX_STAGES]; MAX_PASSES],
    pub uv_split_count: i32,
    pub strip_count: i32,
    pub max_strip_length: i32,
    pub avg_strip_length: f32,
}

impl MeshStatsStruct {
    /// All false / zero (`MeshStatsStruct::Reset`, meshbuild.cpp:495).
    pub fn new() -> Self {
        MeshStatsStruct {
            has_texture: [[false; MAX_STAGES]; MAX_PASSES],
            has_shader: [false; MAX_PASSES],
            has_vertex_material: [false; MAX_PASSES],
            has_per_poly_texture: [[false; MAX_STAGES]; MAX_PASSES],
            has_per_poly_shader: [false; MAX_PASSES],
            has_per_vertex_material: [false; MAX_PASSES],
            has_diffuse_color: [false; MAX_PASSES],
            has_specular_color: [false; MAX_PASSES],
            has_diffuse_illumination: [false; MAX_PASSES],
            has_tex_coords: [[false; MAX_STAGES]; MAX_PASSES],
            uv_split_count: 0,
            strip_count: 0,
            max_strip_length: 0,
            avg_strip_length: 0.0,
        }
    }

    /// `MeshBuilderClass::MeshStatsStruct::Reset` (meshbuild.cpp:495).
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for MeshStatsStruct {
    fn default() -> Self {
        Self::new()
    }
}

/// `WingedEdgeStruct` (meshbuild.h): `Next` is an index into the edge table.
#[derive(Clone, Copy, Debug)]
struct WingedEdgeStruct {
    material_idx: i32,
    next: Option<usize>,
    vertex: [i32; 2],
    poly: [i32; 2],
}

/// `WingedEdgePolyStruct`: indices into the edge table.
#[derive(Clone, Copy, Debug)]
struct WingedEdgePolyStruct {
    edge: [usize; 3],
}

/// `FaceHasherClass` (meshbuild.cpp:83): `Items_Match`, `Compute_Hash`, `Num_Hash_Bits` = 10.
fn face_items_match(a: &FaceClass, b: &FaceClass) -> bool {
    a.vert_idx[0] == b.vert_idx[0] && a.vert_idx[1] == b.vert_idx[1] && a.vert_idx[2] == b.vert_idx[2]
}

/// `FaceHasherClass::Compute_Hash`: `(int)(v0*12345.6f + v1*1714.38484f + v2*27561.3f)&1023`.
fn face_compute_hash(item: &FaceClass) -> usize {
    let h = item.vert_idx[0] as f32 * 12345.6f32 + item.vert_idx[1] as f32 * 1714.38484f32
        + item.vert_idx[2] as f32 * 27561.3f32;
    (cvt_trunc(h) & 1023) as usize
}

/// `UniqueArrayClass<FaceClass>::Add` (uarray.h) with the `FaceHasherClass`: the first item with matching vertex
/// indices wins. Returns the final list of unique faces in insertion order.
fn unique_faces(faces: &[FaceClass]) -> Vec<FaceClass> {
    // Num_Hash_Bits() == 10 -> 1024 buckets; NO_ITEM = 0xFFFFFFFF.
    let mut table: Vec<Option<usize>> = vec![None; 1 << 10];
    let mut items: Vec<(FaceClass, Option<usize>)> = Vec::new();
    for f in faces {
        let hash = face_compute_hash(f);
        let mut idx = table[hash];
        let mut found = false;
        while let Some(i) = idx {
            if face_items_match(&items[i].0, f) {
                found = true;
                break;
            }
            idx = items[i].1;
        }
        if !found {
            let index = items.len();
            items.push((f.clone(), table[hash]));
            table[hash] = Some(index);
        }
    }
    items.into_iter().map(|(f, _)| f).collect()
}

/// `VertexArrayClass::HASH_TABLE_SIZE`.
const HASH_TABLE_SIZE: usize = 4096;

/// `class VertexArrayClass` (meshbuild.cpp:121): the unique-vertex array with smoothing-group aware welding.
struct VertexArrayClass {
    vert_count: i32,
    uv_splits: i32,
    verts: Vec<VertClass>,
    hash_table: Vec<Option<usize>>,
    match_normals: i32,
    center: Vector3,
    extent: Vector3,
}

impl VertexArrayClass {
    /// `VertexArrayClass::VertexArrayClass(maxsize, match_normals)`.
    fn new(maxsize: usize, match_normals: i32) -> Self {
        assert!(maxsize > 0);
        VertexArrayClass {
            vert_count: 0,
            uv_splits: 0,
            verts: vec![VertClass::new(); maxsize],
            hash_table: vec![None; HASH_TABLE_SIZE],
            match_normals,
            center: Vector3::new(0.0, 0.0, 0.0),
            extent: Vector3::new(1.0, 1.0, 1.0),
        }
    }

    /// `VertexArrayClass::Set_Bounds`.
    fn set_bounds(&mut self, minv: &Vector3, maxv: &Vector3) {
        self.extent = (*maxv - *minv) / 2.0f32;
        self.center = (*maxv + *minv) / 2.0f32;
    }

    /// `compute_hash(float x, float y)`: 6 bits of x, 6 bits of y.
    fn compute_hash(x: f32, y: f32) -> u32 {
        let ix = cvt_trunc(x) & 0x0000_003F;
        let iy = cvt_trunc(y) & 0x0000_003F;
        ((iy << 6) | ix) as u32
    }

    /// `VertexArrayClass::Submit_Vertex` (meshbuild.cpp:170).
    fn submit_vertex(&mut self, vert: &VertClass) -> i32 {
        let mut lasthash: u32 = 0xFFFF_FFFF;
        let mut shadeindex: u32 = 0xFFFF_FFFF;

        // aja/ehc 19991005 - Handle the case where an extent is zero.
        let xstart: f64 = if (self.extent.x as f64).abs() > EPSILON as f64 {
            ((vert.position.x - self.center.x) / self.extent.x) as f64
        } else {
            self.center.x as f64
        };
        let ystart: f64 = if (self.extent.y as f64).abs() > EPSILON as f64 {
            ((vert.position.y - self.center.y) / self.extent.y) as f64
        } else {
            self.center.y as f64
        };

        let eps = EPSILON as f64;
        let mut x = xstart - eps;
        while x <= xstart + eps + 0.0000001 {
            let mut y = ystart - eps;
            while y <= ystart + eps + 0.000001 {
                let hash = Self::compute_hash(x as f32, y as f32);
                if hash != lasthash {
                    let mut test_vert = self.hash_table[hash as usize];
                    while let Some(tv) = test_vert {
                        if self.verts_shading_match(vert, &self.verts[tv]) && shadeindex == 0xFFFF_FFFF {
                            shadeindex = self.verts[tv].unique_index as u32;
                            // mask the "master" smoothed vertex's smoothing group with our face's smoothing group.
                            self.verts[shadeindex as usize].shared_sm_group &= vert.sm_group;
                        }
                        if verts_match(&mut self.uv_splits, self.match_normals, vert, &self.verts[tv]) {
                            return self.verts[tv].unique_index;
                        }
                        test_vert = self.verts[tv].next_hash;
                    }
                }
                lasthash = hash;
                y += eps;
            }
            x += eps;
        }

        // Not found, add to the end of the array
        let newindex = self.vert_count as usize;
        self.vert_count += 1;

        self.verts[newindex] = vert.clone();
        self.verts[newindex].unique_index = newindex as i32;
        if shadeindex == 0xFFFF_FFFF {
            self.verts[newindex].shade_index = newindex as i32;
            // This is a new vertex, so store the face's smoothing group into SharedSmGroup
            self.verts[newindex].shared_sm_group = self.verts[newindex].sm_group;
        } else {
            self.verts[newindex].shade_index = shadeindex as i32;
        }

        // And add to the hash table (no zero-extent guard here, as upstream)
        let hx = (vert.position.x - self.center.x) / self.extent.x;
        let hy = (vert.position.y - self.center.y) / self.extent.y;
        let hash = Self::compute_hash(hx, hy);
        self.verts[newindex].next_hash = self.hash_table[hash as usize];
        self.hash_table[hash as usize] = Some(newindex);

        newindex as i32
    }

}

/// `VertexArrayClass::Verts_Match` (meshbuild.cpp:262): a free function so it can bump `UVSplits` while the vertex
/// array is borrowed. Increments `uv_splits` on a UV-only mismatch.
fn verts_match(uv_splits: &mut i32, match_normals: i32, v0: &VertClass, v1: &VertClass) -> bool {
    {
        if v0.id != v1.id {
            return false;
        }
        let dp = (v0.position - v1.position).length();
        if dp > EPSILON {
            return false;
        }
        if match_normals == 0 {
            let smgroup_match = (v0.sm_group & v1.sm_group) != 0 || v0.sm_group == v1.sm_group;
            if !smgroup_match {
                return false;
            }
        } else {
            let dn = (v0.normal - v1.normal).length();
            if dn > EPSILON {
                return false;
            }
        }
        for pass in 0..MAX_PASSES {
            if v0.diffuse_color[pass] != v1.diffuse_color[pass] {
                return false;
            }
            if v0.specular_color[pass] != v1.specular_color[pass] {
                return false;
            }
            if v0.diffuse_illumination[pass] != v1.diffuse_illumination[pass] {
                return false;
            }
            if v0.alpha[pass] != v1.alpha[pass] {
                return false;
            }
            if v0.vertex_material_index[pass] != v1.vertex_material_index[pass] {
                return false;
            }
        }
        for pass in 0..MAX_PASSES {
            for stage in 0..MAX_STAGES {
                if v0.tex_coord[pass][stage] != v1.tex_coord[pass][stage] {
                    *uv_splits += 1;
                    return false;
                }
            }
        }
        true
    }
}

impl VertexArrayClass {
    /// `VertexArrayClass::Verts_Shading_Match` (meshbuild.cpp:312).
    fn verts_shading_match(&self, v0: &VertClass, v1: &VertClass) -> bool {
        let dv = (v0.position - v1.position).length();
        let smgroup_match = (v0.sm_group & v1.sm_group) != 0 || v0.sm_group == v1.sm_group;
        dv < EPSILON && smgroup_match && v0.id == v1.id
    }

    /// `VertexArrayClass::Propogate_Shared_Smooth_Groups` (meshbuild.cpp:326).
    fn propogate_shared_smooth_groups(&mut self) {
        for i in 0..self.vert_count as usize {
            if self.verts[i].shade_index != i as i32 {
                let s = self.verts[i].shade_index as usize;
                self.verts[i].shared_sm_group = self.verts[s].shared_sm_group;
            }
        }
    }
}

/// `tex_compare` (meshbuild.cpp:1700): texture, then vertex material index of vert 0.
fn tex_compare(f0: &FaceClass, f1: &FaceClass, pass: usize, stage: usize) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if f0.texture_index[pass][stage] < f1.texture_index[pass][stage] {
        return Ordering::Less;
    }
    if f0.texture_index[pass][stage] > f1.texture_index[pass][stage] {
        return Ordering::Greater;
    }
    if f0.verts[0].vertex_material_index[pass] < f1.verts[0].vertex_material_index[pass] {
        return Ordering::Less;
    }
    if f0.verts[0].vertex_material_index[pass] > f1.verts[0].vertex_material_index[pass] {
        return Ordering::Greater;
    }
    Ordering::Equal
}

/// `face_material_compare` (meshbuild.cpp:1690): texture index of pass 0, stage 0. Declared static upstream and
/// never used by the builder; kept for completeness.
pub fn face_material_compare(f0: &FaceClass, f1: &FaceClass) -> std::cmp::Ordering {
    f0.texture_index[0][0].cmp(&f1.texture_index[0][0])
}

/// `vertex_compare` (meshbuild.cpp:1760): bone index, then vertex material of pass 0.
fn vertex_compare(v0: &VertClass, v1: &VertClass) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if v0.bone_index < v1.bone_index {
        return Ordering::Less;
    }
    if v0.bone_index > v1.bone_index {
        return Ordering::Greater;
    }
    if v0.vertex_material_index[0] < v1.vertex_material_index[0] {
        return Ordering::Less;
    }
    if v0.vertex_material_index[0] > v1.vertex_material_index[0] {
        return Ordering::Greater;
    }
    Ordering::Equal
}

/// `class MeshBuilderClass` (meshbuild.h).
#[derive(Clone, Debug)]
pub struct MeshBuilderClass {
    state: i32,
    pass_count: i32,
    face_count: i32,
    faces: Vec<FaceClass>,
    input_vert_count: i32,
    vert_count: i32,
    verts: Vec<VertClass>,
    cur_face: i32,
    world_info: Option<WorldInfoStub>,
    stats: MeshStatsStruct,
    poly_order_pass: usize,
    poly_order_stage: usize,
    alloc_face_count: i32,
    alloc_face_growth: i32,
}

impl MeshBuilderClass {
    /// `MeshBuilderClass::MeshBuilderClass(pass_count, face_count_guess, face_count_growth_rate)` (meshbuild.cpp:520).
    /// Upstream's defaults are 1, 255, 64.
    pub fn new(pass_count: i32, face_count_guess: i32, face_count_growth_rate: i32) -> Self {
        let mut m = MeshBuilderClass {
            state: STATE_ACCEPTING_INPUT,
            pass_count,
            face_count: 0,
            faces: Vec::new(),
            input_vert_count: 0,
            vert_count: 0,
            verts: Vec::new(),
            cur_face: 0,
            world_info: None,
            stats: MeshStatsStruct::new(),
            poly_order_pass: 0,
            poly_order_stage: 0,
            alloc_face_count: 0,
            alloc_face_growth: 0,
        };
        m.reset(pass_count, face_count_guess, face_count_growth_rate);
        m
    }

    /// `MeshBuilderClass::Free` (meshbuild.cpp:555).
    fn free(&mut self) {
        self.faces = Vec::new();
        self.verts = Vec::new();
        self.face_count = 0;
        self.vert_count = 0;
        self.alloc_face_count = 0;
        self.alloc_face_growth = 0;
    }

    /// `MeshBuilderClass::Reset` (meshbuild.cpp:582).
    pub fn reset(&mut self, passcount: i32, face_count_guess: i32, face_count_growth_rate: i32) {
        self.free();
        self.state = STATE_ACCEPTING_INPUT;
        self.pass_count = passcount;
        self.alloc_face_count = face_count_guess;
        self.alloc_face_growth = face_count_growth_rate;
        self.faces = vec![FaceClass::new(); self.alloc_face_count as usize];
        self.cur_face = 0;
        self.stats.reset();
    }

    /// `MeshBuilderClass::Set_World_Info` (meshbuild.h).
    pub fn set_world_info(&mut self, world_info: Option<WorldInfoStub>) {
        self.world_info = world_info;
    }

    /// `MeshBuilderClass::Peek_World_Info`.
    pub fn peek_world_info(&self) -> Option<&WorldInfoStub> {
        self.world_info.as_ref()
    }

    /// `MeshBuilderClass::Set_Polygon_Ordering_Channel` (meshbuild.h).
    pub fn set_polygon_ordering_channel(&mut self, pass: usize, texstage: usize) {
        assert!(pass < MAX_PASSES);
        assert!(texstage < MAX_STAGES);
        self.poly_order_pass = pass;
        self.poly_order_stage = texstage;
    }

    pub fn get_pass_count(&self) -> i32 {
        self.pass_count
    }

    pub fn get_vertex_count(&self) -> i32 {
        assert!(self.state == STATE_MESH_PROCESSED);
        self.vert_count
    }

    pub fn get_face_count(&self) -> i32 {
        assert!(self.state == STATE_MESH_PROCESSED);
        self.face_count
    }

    pub fn get_vertex(&self, index: usize) -> &VertClass {
        assert!(self.state == STATE_MESH_PROCESSED);
        assert!((index as i32) < self.vert_count);
        &self.verts[index]
    }

    pub fn get_face(&self, index: usize) -> &FaceClass {
        assert!(self.state == STATE_MESH_PROCESSED);
        assert!((index as i32) < self.face_count);
        &self.faces[index]
    }

    pub fn get_vertex_mut(&mut self, index: usize) -> &mut VertClass {
        assert!(self.state == STATE_MESH_PROCESSED);
        assert!((index as i32) < self.vert_count);
        &mut self.verts[index]
    }

    pub fn get_face_mut(&mut self, index: usize) -> &mut FaceClass {
        assert!(self.state == STATE_MESH_PROCESSED);
        assert!((index as i32) < self.face_count);
        &mut self.faces[index]
    }

    pub fn get_mesh_stats(&self) -> &MeshStatsStruct {
        assert!(self.state == STATE_MESH_PROCESSED);
        &self.stats
    }

    /// `MeshBuilderClass::Add_Face` (meshbuild.cpp:600). Returns the index the face was stored at.
    pub fn add_face(&mut self, face: &FaceClass) -> i32 {
        assert!(self.state == STATE_ACCEPTING_INPUT);
        assert!(self.cur_face <= self.alloc_face_count);
        if self.cur_face == self.alloc_face_count {
            self.grow_face_array();
        }
        let cf = self.cur_face as usize;
        self.faces[cf] = face.clone();
        self.faces[cf].compute_plane();
        self.faces[cf].add_index = self.cur_face;
        for i in 0..3 {
            self.faces[cf].verts[i].sm_group = self.faces[cf].sm_group;
        }
        self.cur_face += 1;
        self.cur_face - 1
    }

    /// `MeshBuilderClass::Build_Mesh` (meshbuild.cpp:640).
    pub fn build_mesh(&mut self, compute_normals: bool) {
        assert!(self.state == STATE_ACCEPTING_INPUT);
        self.state = STATE_MESH_PROCESSED;
        self.face_count = self.cur_face;
        self.optimize_mesh(compute_normals);
    }

    /// `MeshBuilderClass::Compute_Face_Normals` (meshbuild.cpp:670).
    pub fn compute_face_normals(&mut self) {
        for faceidx in 0..self.face_count as usize {
            self.faces[faceidx].compute_plane();
        }
    }

    /// `MeshBuilderClass::Verify_Face_Normals` (meshbuild.cpp:688).
    pub fn verify_face_normals(&self) -> bool {
        let mut ok = true;
        for faceidx in 0..self.face_count as usize {
            let f = &self.faces[faceidx];
            let v0 = &self.verts[f.vert_idx[0] as usize];
            let v1 = &self.verts[f.vert_idx[1] as usize];
            let v2 = &self.verts[f.vert_idx[2] as usize];
            let normal = Vector3::normalized_cross_product(&(v1.position - v0.position), &(v2.position - v0.position));
            let dn = (f.normal - normal).length();
            if dn > 0.0000001f32 {
                ok = false;
            }
        }
        ok
    }

    /// `MeshBuilderClass::Compute_Vertex_Normals` (meshbuild.cpp:720).
    fn compute_vertex_normals(&mut self) {
        for vertidx in 0..self.vert_count as usize {
            self.verts[vertidx].normal.set(0.0, 0.0, 0.0);
        }
        for faceidx in 0..self.face_count as usize {
            for facevertidx in 0..3 {
                let vertindex = self.faces[faceidx].vert_idx[facevertidx] as usize;
                let shadeindex = self.verts[vertindex].shade_index as usize;
                let n = self.faces[faceidx].normal;
                self.verts[shadeindex].normal += n;
            }
        }
        // Smooth this mesh with neighboring meshes!
        if let Some(wi) = &self.world_info {
            if wi.meshes_smoothed {
                for vertidx in 0..self.vert_count as usize {
                    if self.verts[vertidx].shade_index == vertidx as i32 {
                        if let Some(f) = wi.shared_vertex_normal {
                            let add = f(&self.verts[vertidx].position, self.verts[vertidx].shared_sm_group);
                            self.verts[vertidx].normal += add;
                        }
                    }
                }
            }
        }
        // Propogate the accumulated normals to all of the other verts which share them
        for vertidx in 0..self.vert_count as usize {
            let shadeindex = self.verts[vertidx].shade_index as usize;
            self.verts[vertidx].normal = self.verts[shadeindex].normal;
            self.verts[vertidx].normal.normalize();
        }
    }

    /// `MeshBuilderClass::Remove_Degenerate_Faces` (meshbuild.cpp:790).
    fn remove_degenerate_faces(&mut self) {
        let mut candidates: Vec<FaceClass> = Vec::new();
        for faceidx in 0..self.face_count as usize {
            if !self.faces[faceidx].is_degenerate() {
                candidates.push(self.faces[faceidx].clone());
            }
        }
        let unique = unique_faces(&candidates);
        self.face_count = unique.len() as i32;
        self.alloc_face_count = unique.len() as i32;
        self.cur_face = self.face_count;
        self.faces = unique;
    }

    /// `MeshBuilderClass::Compute_Mesh_Stats` (meshbuild.cpp:818).
    fn compute_mesh_stats(&mut self) {
        let mut tex_index = [[0i32; MAX_STAGES]; MAX_PASSES];
        let mut shader_index = [0i32; MAX_PASSES];
        let mut vmat_index = [0i32; MAX_PASSES];

        self.stats.reset();

        for pass in 0..MAX_PASSES {
            for stage in 0..MAX_STAGES {
                tex_index[pass][stage] = self.faces[0].texture_index[pass][stage];
            }
            shader_index[pass] = self.faces[0].shader_index[pass];
            vmat_index[pass] = self.verts[0].vertex_material_index[pass];
        }

        let fc = self.face_count as usize;
        let vc = self.vert_count as usize;
        for pass in 0..MAX_PASSES {
            for stage in 0..MAX_STAGES {
                for face_index in 0..fc {
                    if self.faces[face_index].texture_index[pass][stage] != tex_index[pass][stage] {
                        self.stats.has_per_poly_texture[pass][stage] = true;
                        break;
                    }
                }
                for vert_index in 0..vc {
                    if self.verts[vert_index].tex_coord[pass][stage] != Vector2::new(0.0, 0.0) {
                        self.stats.has_tex_coords[pass][stage] = true;
                        break;
                    }
                }
            }
            for face_index in 0..fc {
                if self.faces[face_index].shader_index[pass] != shader_index[pass] {
                    self.stats.has_per_poly_shader[pass] = true;
                    break;
                }
            }
            for vert_index in 0..vc {
                if self.verts[vert_index].vertex_material_index[pass] != vmat_index[pass] {
                    self.stats.has_per_vertex_material[pass] = true;
                    break;
                }
            }
            for vert_index in 0..vc {
                if self.verts[vert_index].diffuse_color[pass] != Vector3::new(1.0, 1.0, 1.0)
                    || self.verts[vert_index].alpha[pass] != 1.0f32
                {
                    self.stats.has_diffuse_color[pass] = true;
                    break;
                }
            }
            for vert_index in 0..vc {
                if self.verts[vert_index].specular_color[pass] != Vector3::new(1.0, 1.0, 1.0) {
                    self.stats.has_specular_color[pass] = true;
                    break;
                }
            }
            for vert_index in 0..vc {
                if self.verts[vert_index].diffuse_illumination[pass] != Vector3::new(0.0, 0.0, 0.0) {
                    self.stats.has_diffuse_illumination[pass] = true;
                    break;
                }
            }
            for stage in 0..MAX_STAGES {
                for face_index in 0..fc {
                    if self.faces[face_index].texture_index[pass][stage] != -1 {
                        self.stats.has_texture[pass][stage] = true;
                        break;
                    }
                }
            }
            for face_index in 0..fc {
                if self.faces[face_index].shader_index[pass] != -1 {
                    self.stats.has_shader[pass] = true;
                    break;
                }
            }
            for vert_index in 0..vc {
                if self.verts[vert_index].vertex_material_index[pass] != -1 {
                    self.stats.has_vertex_material[pass] = true;
                    break;
                }
            }
        }
    }

    /// `MeshBuilderClass::Compute_Bounding_Box` (meshbuild.cpp:935): returns (min, max).
    pub fn compute_bounding_box(&self) -> (Vector3, Vector3) {
        let mut set_min = self.verts[0].position;
        let mut set_max = self.verts[0].position;
        for i in 0..self.vert_count as usize {
            let p = &self.verts[i].position;
            if p.x < set_min.x {
                set_min.x = p.x;
            }
            if p.y < set_min.y {
                set_min.y = p.y;
            }
            if p.z < set_min.z {
                set_min.z = p.z;
            }
            if p.x > set_max.x {
                set_max.x = p.x;
            }
            if p.y > set_max.y {
                set_max.y = p.y;
            }
            if p.z > set_max.z {
                set_max.z = p.z;
            }
        }
        (set_min, set_max)
    }

    /// `MeshBuilderClass::Compute_Bounding_Sphere` (meshbuild.cpp:965), Graphics Gems I p.301: returns
    /// (center, radius). The span and radius arithmetic is `double`, as upstream; `sqrt` is `f64::sqrt` (correctly
    /// rounded, matching the C `sqrt`).
    pub fn compute_bounding_sphere(&self) -> (Vector3, f32) {
        let p0 = self.verts[0].position;
        let (mut xmin, mut xmax, mut ymin, mut ymax, mut zmin, mut zmax) = (p0, p0, p0, p0, p0, p0);

        // FIRST PASS: finding the 6 minima and maxima points
        for i in 1..self.vert_count as usize {
            let p = self.verts[i].position;
            if p.x < xmin.x {
                xmin = p;
            }
            if p.x > xmax.x {
                xmax = p;
            }
            if p.y < ymin.y {
                ymin = p;
            }
            if p.y > ymax.y {
                ymax = p;
            }
            if p.z < zmin.z {
                zmin = p;
            }
            if p.z > zmax.z {
                zmax = p;
            }
        }

        fn span(a: &Vector3, b: &Vector3) -> f64 {
            let dx = (a.x - b.x) as f64;
            let dy = (a.y - b.y) as f64;
            let dz = (a.z - b.z) as f64;
            dx * dx + dy * dy + dz * dz
        }
        // dx = xmax.X - xmin.X is a float subtraction widened to double, as written above.
        let xspan = span(&xmax, &xmin);
        let yspan = span(&ymax, &ymin);
        let zspan = span(&zmax, &zmin);

        let mut dia1 = xmin;
        let mut dia2 = xmax;
        let mut maxspan = xspan;
        if yspan > maxspan {
            maxspan = yspan;
            dia1 = ymin;
            dia2 = ymax;
        }
        if zspan > maxspan {
            dia1 = zmin;
            dia2 = zmax;
        }
        let _ = maxspan;

        let mut center = Vector3::new(
            (dia1.x + dia2.x) / 2.0f32,
            (dia1.y + dia2.y) / 2.0f32,
            (dia1.z + dia2.z) / 2.0f32,
        );
        let dx = (dia2.x - center.x) as f64;
        let dy = (dia2.y - center.y) as f64;
        let dz = (dia2.z - center.z) as f64;
        let mut radsqr = dx * dx + dy * dy + dz * dz;
        let mut radius = radsqr.sqrt();

        // SECOND PASS: increment current sphere if any points fall outside of it.
        for i in 0..self.vert_count as usize {
            let p = self.verts[i].position;
            let dx = (p.x - center.x) as f64;
            let dy = (p.y - center.y) as f64;
            let dz = (p.z - center.z) as f64;
            let testrad2 = dx * dx + dy * dy + dz * dz;
            if testrad2 > radsqr {
                let testrad = testrad2.sqrt();
                radius = (radius + testrad) / 2.0;
                radsqr = radius * radius;
                let oldtonew = testrad - radius;
                // center.X is float: the double expression is narrowed on assignment.
                center.x = ((radius * center.x as f64 + oldtonew * p.x as f64) / testrad) as f32;
                center.y = ((radius * center.y as f64 + oldtonew * p.y as f64) / testrad) as f32;
                center.z = ((radius * center.z as f64 + oldtonew * p.z as f64) / testrad) as f32;
            }
        }
        (center, radius as f32)
    }

    /// `MeshBuilderClass::Optimize_Mesh` (meshbuild.cpp:1100).
    fn optimize_mesh(&mut self, compute_normals: bool) {
        let match_normals = if compute_normals { 0 } else { 1 };
        let fc = self.face_count as usize;
        let mut unique_verts = VertexArrayClass::new(fc * 3, match_normals);

        // Find the min and max of the array of vertices
        let mut minv = self.faces[0].verts[0].position;
        let mut maxv = self.faces[0].verts[0].position;
        for faceidx in 0..fc {
            for facevertidx in 0..3 {
                minv.update_min(&self.faces[faceidx].verts[facevertidx].position);
                maxv.update_max(&self.faces[faceidx].verts[facevertidx].position);
            }
        }
        unique_verts.set_bounds(&minv, &maxv);

        // Build the array of unique vertices
        for faceidx in 0..fc {
            for facevertidx in 0..3 {
                let v = self.faces[faceidx].verts[facevertidx].clone();
                self.faces[faceidx].vert_idx[facevertidx] = unique_verts.submit_vertex(&v);
            }
        }

        unique_verts.propogate_shared_smooth_groups();

        self.vert_count = unique_verts.vert_count;
        self.verts = unique_verts.verts[..self.vert_count as usize].to_vec();

        // Build a new array of faces, removing degenerate faces
        self.remove_degenerate_faces();

        // Recompute the face normals just in case they changed a little due to vertex welding.
        self.compute_face_normals();
        if compute_normals {
            self.compute_vertex_normals();
        }

        self.compute_mesh_stats();
        self.stats.uv_split_count = unique_verts.uv_splits;

        // Sort the polys by texture (Texture_Compare_Funcs[PolyOrderPass][PolyOrderStage]); stable, see module doc.
        let (pp, ps) = (self.poly_order_pass, self.poly_order_stage);
        let fcount = self.face_count as usize;
        self.faces[..fcount].sort_by(|a, b| tex_compare(a, b, pp, ps));

        // Sort the vertices by bone index and vertex material
        self.sort_vertices();

        // Strip optimize the mesh
        self.strip_optimize_mesh();
        let _ = self.verify_face_normals();
    }

    /// `MeshBuilderClass::Strip_Optimize_Mesh` (meshbuild.cpp:1230).
    fn strip_optimize_mesh(&mut self) {
        let face_count = self.face_count as usize;
        let mut edgehash: [Option<usize>; 512] = [None; 512];
        let mut edgetab: Vec<WingedEdgeStruct> = vec![
            WingedEdgeStruct { material_idx: 0, next: None, vertex: [0, 0], poly: [0, 0] };
            face_count * 3
        ];
        let mut pedges: Vec<WingedEdgePolyStruct> = vec![WingedEdgePolyStruct { edge: [0; 3] }; face_count];

        let mut newpolys: Vec<FaceClass> = vec![FaceClass::new(); face_count];
        let mut newmat: Vec<i32> = vec![0; face_count];
        let mut premap: Vec<i32> = vec![-1; face_count];
        let mut vtimestamp: Vec<i32> = vec![-1; self.vert_count as usize];

        let mut vcount: i32 = 0;
        let mut polysinserted: usize = 0;
        let mut lastmat: i32 = 0;
        let mut edgecount: usize = 0;
        let pp = self.poly_order_pass;
        let ps = self.poly_order_stage;

        // Build the edge table for the mesh (Oct 20,1998: uses TextureIndex[PolyOrderPass][PolyOrderStage])
        for i in 0..face_count {
            let mat = self.faces[i].texture_index[pp][ps];
            for j in 0..3usize {
                let v0 = j;
                let v1i = if v0 < 2 { v0 + 1 } else { 0 };
                let mut a = self.faces[i].vert_idx[v0];
                let mut b = self.faces[i].vert_idx[v1i];
                // order the vertices before hashing (hash depends on vert order)
                if a > b {
                    std::mem::swap(&mut a, &mut b);
                }
                let hash = ((a + b * 119) & 511) as usize;

                let mut edge = edgehash[hash];
                while let Some(e) = edge {
                    if edgetab[e].vertex[0] == a && edgetab[e].vertex[1] == b && edgetab[e].material_idx == mat {
                        break;
                    }
                    edge = edgetab[e].next;
                }

                let e = match edge {
                    Some(e) => {
                        edgetab[e].poly[1] = i as i32;
                        e
                    }
                    None => {
                        let e = edgecount;
                        edgecount += 1;
                        edgetab[e].next = edgehash[hash];
                        edgehash[hash] = Some(e);
                        edgetab[e].vertex[0] = a;
                        edgetab[e].vertex[1] = b;
                        edgetab[e].poly[0] = i as i32;
                        edgetab[e].poly[1] = -1;
                        edgetab[e].material_idx = mat;
                        e
                    }
                };
                pedges[i].edge[j] = e;
            }
        }

        // Insert polygons into a new polygon list until all have been inserted, making long strips.
        while polysinserted < face_count {
            let mut startpoly: i32 = -1;
            let mut bestc: i32 = 1 << 29;
            let startpass = 0;

            for findpass in startpass..2 {
                for i in 0..face_count {
                    if premap[i] == -1 {
                        // if material mismatch, cannot choose poly (except in pass 1)
                        if findpass == 0 && self.faces[i].texture_index[pp][ps] != lastmat {
                            continue;
                        }
                        let mut c: i32 = 0;
                        for j in 0..3 {
                            if edgetab[pedges[i].edge[j]].poly[1] >= 0 {
                                c += vcount + 1;
                            }
                        }
                        for j in 0..3 {
                            c += vcount - vtimestamp[self.faces[i].vert_idx[j] as usize];
                        }
                        if c < bestc {
                            bestc = c;
                            startpoly = i as i32;
                        }
                    }
                }
                if startpoly != -1 {
                    break;
                }
            }

            // track the fact that we created a new strip:
            self.stats.strip_count += 1;

            let sp = startpoly as usize;
            lastmat = self.faces[sp].texture_index[pp][ps];
            newmat[polysinserted] = self.faces[sp].texture_index[pp][ps];

            let mut found_shared_edge = false;
            newpolys[polysinserted] = self.faces[sp].clone();
            let newpoly_idx = polysinserted;

            let mut edge_index = 0;
            while edge_index < 3 && !found_shared_edge {
                let mut side_index = 0;
                while side_index < 2 && !found_shared_edge {
                    let ed = edgetab[pedges[sp].edge[edge_index]];
                    let poly = ed.poly[side_index];
                    if poly != -1 && poly != startpoly && premap[poly as usize] == -1 {
                        // find vert which is not on the final edge
                        let mut first_vert = -1;
                        for vidx in 0..3 {
                            let vi = newpolys[newpoly_idx].vert_idx[vidx];
                            if vi != ed.vertex[0] && vi != ed.vertex[1] {
                                first_vert = vi;
                                break;
                            }
                        }
                        assert!(first_vert != -1);
                        // rotate the vertex indices until first_vert is the index of VertIdx[0]
                        while newpolys[newpoly_idx].vert_idx[0] != first_vert {
                            let vi = &mut newpolys[newpoly_idx].vert_idx;
                            let tmp = vi[0];
                            vi[0] = vi[1];
                            vi[1] = vi[2];
                            vi[2] = tmp;
                        }
                        found_shared_edge = true;
                        break;
                    }
                    side_index += 1;
                }
                edge_index += 1;
            }

            // if a shared edge wasn't found, just copy the poly
            if !found_shared_edge {
                newpolys[polysinserted] = self.faces[sp].clone();
            }

            premap[sp] = polysinserted as i32;
            polysinserted += 1;

            for i in 0..3 {
                let v = self.faces[sp].vert_idx[i] as usize;
                if vtimestamp[v] == -1 {
                    vtimestamp[v] = vcount;
                    vcount += 1;
                }
            }

            // If we have no shared edges, this is a lone poly, start another strip
            if edgetab[pedges[sp].edge[0]].poly[1] == -1
                && edgetab[pedges[sp].edge[1]].poly[1] == -1
                && edgetab[pedges[sp].edge[2]].poly[1] == -1
            {
                continue;
            }

            // Build the strip starting from the polygon chosen in the previous loop
            let mut v_fifo = [newpolys[newpoly_idx].vert_idx[1], newpolys[newpoly_idx].vert_idx[2]];
            let mut scnt: i32 = 0;
            let mut nextpoly: i32 = startpoly;

            while nextpoly != -1 {
                let cur = nextpoly as usize;
                nextpoly = -1;

                // `goto found` becomes a labelled search.
                'search: for i in 0..3 {
                    let e = edgetab[pedges[cur].edge[i]];
                    if (e.vertex[0] == v_fifo[0] && e.vertex[1] == v_fifo[1])
                        || (e.vertex[1] == v_fifo[0] && e.vertex[0] == v_fifo[1])
                    {
                        for j in 0..2 {
                            if e.poly[j] > -1 && premap[e.poly[j] as usize] == -1 {
                                nextpoly = e.poly[j];
                                break 'search;
                            }
                        }
                    }
                }
                if nextpoly == -1 {
                    // couldn't find another poly, break from loop
                    break;
                }
                let np = nextpoly as usize;

                // now, find the new vertex (two verts are on the edge, find the third)
                let mut nw = -1i32;
                for i in 0..3 {
                    if self.faces[np].vert_idx[i] != v_fifo[0] && self.faces[np].vert_idx[i] != v_fifo[1] {
                        nw = i as i32;
                        break;
                    }
                }
                assert!(nw != -1);
                let new_vindex = self.faces[np].vert_idx[nw as usize];

                newmat[polysinserted] = self.faces[np].texture_index[pp][ps];

                newpolys[polysinserted].vert_idx[0] = v_fifo[0];
                newpolys[polysinserted].vert_idx[1] = v_fifo[1];
                newpolys[polysinserted].vert_idx[2] = new_vindex;

                // if we are on an even triangle, swap the vertex ordering
                if scnt & 1 == 0 {
                    newpolys[polysinserted].vert_idx.swap(0, 1);
                }

                v_fifo[0] = v_fifo[1];
                v_fifo[1] = new_vindex;

                if vtimestamp[new_vindex as usize] == -1 {
                    vtimestamp[new_vindex as usize] = vcount;
                    vcount += 1;
                }

                premap[np] = polysinserted as i32;
                polysinserted += 1;
                scnt += 1;
            }

            // scnt+1 is the number of polys that were added to the strip
            self.stats.avg_strip_length += (scnt + 1) as f32;
            if scnt + 1 > self.stats.max_strip_length {
                self.stats.max_strip_length = scnt + 1;
            }
        }

        // Use the premap array to get the rest of the info into the new face table.
        for i in 0..face_count {
            let old_idx = i;
            let new_idx = premap[i] as usize;
            for pass in 0..MAX_PASSES {
                for stage in 0..MAX_STAGES {
                    newpolys[new_idx].texture_index[pass][stage] = self.faces[old_idx].texture_index[pass][stage];
                }
                newpolys[new_idx].shader_index[pass] = self.faces[old_idx].shader_index[pass];
            }
            newpolys[new_idx].sm_group = self.faces[old_idx].sm_group;
            newpolys[new_idx].index = self.faces[old_idx].index;
            newpolys[new_idx].attributes = self.faces[old_idx].attributes;
            newpolys[new_idx].add_index = self.faces[old_idx].add_index;
            newpolys[new_idx].normal = self.faces[old_idx].normal;
            newpolys[new_idx].dist = self.faces[old_idx].dist;
            newpolys[new_idx].surface_type = self.faces[old_idx].surface_type;
            // just checking
            assert!(newmat[new_idx] == self.faces[old_idx].texture_index[pp][ps]);
        }

        self.faces = newpolys;
        self.alloc_face_count = self.face_count;

        // finish the computation of the average strip length
        self.stats.avg_strip_length /= self.stats.strip_count as f32;
    }

    /// `MeshBuilderClass::Grow_Face_Array` (meshbuild.cpp:1590).
    fn grow_face_array(&mut self) {
        self.alloc_face_count += self.alloc_face_growth;
        self.faces.resize(self.alloc_face_count as usize, FaceClass::new());
    }

    /// `MeshBuilderClass::Sort_Vertices` (meshbuild.cpp:1610).
    fn sort_vertices(&mut self) {
        let vc = self.vert_count as usize;
        // qsort(Verts, VertCount, sizeof(VertClass), vertex_compare); stable here, see module doc.
        self.verts[..vc].sort_by(vertex_compare);

        // Build a vertex remap table (table[old_index] = new_index)
        let mut vertex_remap_table = vec![0i32; vc];
        for vi in 0..vc {
            vertex_remap_table[self.verts[vi].unique_index as usize] = vi as i32;
        }
        // Remap the faces' vertex indices
        for fi in 0..self.face_count as usize {
            for vi in 0..3 {
                let old_index = self.faces[fi].vert_idx[vi] as usize;
                self.faces[fi].vert_idx[vi] = vertex_remap_table[old_index];
            }
        }
    }
}
