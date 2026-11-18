//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/sortingrenderer.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/sortingrenderer.h
//!
//! The sorting logic is translated; the DX8 calls are not. Upstream snapshots `DX8Wrapper::Get_Render_State` into
//! each node (shader, material, textures, lights, vertex/index buffers); here a node carries an opaque
//! `render_state` index into the owner's snapshots plus the pieces the sort needs: the world and view matrices,
//! the buffer kinds and the source vertex/index data (shared `Arc`s in place of the buffer objects).
//! Upstream's globals (`sorted_list`, `clean_list`, `overlapping_nodes`, `temp_index_array`, the default counts) are
//! fields of [`SortingRendererClass`].
//!
//! Flagged for the differential pass:
//! - the quicksort in `Sort` is hand written and unstable; it is translated as written (not `sort_by`), so tie
//!   order matches. The "TODO: temp fix until I find out who is passing me NaN" bounds are kept;
//! - `D3DXMATRIX * D3DXMATRIX` is a float row-vector product; `D3DXVec3Transform` is `v * M` with w = 1;
//! - `Insert_Triangles` casts nothing: `min_vertex_index`, `vertex_count` etc. are `unsigned short`.

use crate::ww::vector3::Vector3;
use std::sync::Arc;

/// `DEFAULT_SORTING_POLY_COUNT` initial value: `(count * 3)` must be less than 65536.
pub const DEFAULT_SORTING_POLY_COUNT_INIT: u32 = 16384;
/// `DEFAULT_SORTING_VERTEX_COUNT` initial value: must be less than 65536.
pub const DEFAULT_SORTING_VERTEX_COUNT_INIT: u32 = 32768;
/// `MAX_OVERLAPPING_NODES`.
pub const MAX_OVERLAPPING_NODES: usize = 4096;

/// A row-major 4x4 matrix as `D3DXMATRIX` / `Matrix4x4` (row-vector convention).
pub type Mtx4 = [[f32; 4]; 4];

/// `D3DXMATRIX operator*` (float): `C[i][j] = sum_k A[i][k] * B[k][j]`.
pub fn d3dx_mul(a: &Mtx4, b: &Mtx4) -> Mtx4 {
    let mut c = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            c[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] + a[i][3] * b[3][j];
        }
    }
    c
}

/// `D3DXVec3Transform(&out, &v, &m)`: `(v, 1) * m`; returns the four components.
pub fn d3dx_vec3_transform(v: &Vector3, m: &Mtx4) -> [f32; 4] {
    [
        m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z + m[3][0],
        m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z + m[3][1],
        m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z + m[3][2],
        m[0][3] * v.x + m[1][3] * v.y + m[2][3] * v.z + m[3][3],
    ]
}

/// `ShortVectorIStruct` (sortingrenderer.cpp:55).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShortVectorIStruct {
    pub i: u16,
    pub j: u16,
    pub k: u16,
}

/// `TempIndexStruct` (sortingrenderer.cpp:62): a triangle, the node it came from and its depth.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TempIndexStruct {
    pub tri: ShortVectorIStruct,
    pub idx: u16,
    pub z: f32,
}

/// `InsertionSort(begin, end)` (sortingrenderer.cpp:80) over `a[begin..end]`, ordering by `z` (`>` as upstream).
fn insertion_sort(a: &mut [TempIndexStruct], begin: usize, end: usize) {
    let mut iter = begin + 1;
    while iter < end {
        let val = a[iter];
        let mut insert = iter;
        while insert != begin && a[insert - 1].z > val.z {
            a[insert] = a[insert - 1];
            insert -= 1;
        }
        a[insert] = val;
        iter += 1;
    }
}

/// `Sort(begin, end)` (sortingrenderer.cpp:98): median-of-three quicksort with an insertion sort below 17 items.
pub fn sort(a: &mut [TempIndexStruct], begin: usize, end: usize) {
    // `end` is one past the last element; callers pass (0, len).
    let diff = end as i64 - begin as i64;
    if diff <= 16 {
        insertion_sort(a, begin, end);
        return;
    }
    let mid = begin + (diff / 2) as usize;
    a.swap(mid, begin + 1);
    if a[begin + 1].z > a[end - 1].z {
        a.swap(begin + 1, end - 1);
    }
    if a[begin].z > a[end - 1].z {
        a.swap(begin, end - 1);
    } // end[-1] has the largest element
    if a[begin + 1].z > a[begin].z {
        a.swap(begin + 1, begin);
    } // begin[0] has the middle element and begin[1] has the smallest element

    let begin1 = begin + 1; // TODO upstream: temp fix until NaN source found
    let end1 = end - 1;
    let mut left = begin + 1;
    let mut right = end - 1;
    loop {
        // Scan up to find element >= than partition
        left += 1;
        while left < end1 && a[left].z < a[begin].z {
            left += 1;
        }
        // Scan down to find element <= than partition
        right -= 1;
        while right > begin1 && a[right].z > a[begin].z {
            right -= 1;
        }
        if right < left {
            break; // Pointers crossed. Partitioning completed.
        }
        a.swap(left, right); // Exchange elements.
    }
    a.swap(begin, right); // Insert partition element
    if (right - begin) as i64 > end as i64 - (right as i64 + 1) {
        sort(a, right + 1, end);
        sort(a, begin, right);
    } else {
        sort(a, begin, right);
        sort(a, right + 1, end);
    }
}

/// A vertex of `VertexFormatXYZNDUV2` as far as sorting reads it: the position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SortVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// `SortingNodeStruct` (sortingrenderer.cpp:150) with its `RenderStateStruct` reduced to what the sort needs.
#[derive(Clone, Debug)]
pub struct SortingNodeStruct {
    /// Index of the owner's `RenderStateStruct` snapshot (`DX8Wrapper::Get_Render_State`).
    pub render_state: usize,
    /// `sorting_state.world` / `.view`.
    pub world: Mtx4,
    pub view: Mtx4,
    /// `index_buffer_type` and `vertex_buffer_types[0]` are `BUFFER_TYPE_SORTING` or `BUFFER_TYPE_DYNAMIC_SORTING`.
    pub sorting_buffers: bool,
    /// `SortingVertexBufferClass::VertexBuffer`, `vba_offset`, `index_base_offset`.
    pub src_vertices: Arc<Vec<SortVertex>>,
    pub vba_offset: usize,
    pub index_base_offset: usize,
    /// `SortingIndexBufferClass::index_buffer`, `iba_offset`.
    pub src_indices: Arc<Vec<u16>>,
    pub iba_offset: usize,
    pub bounding_sphere_center: Vector3,
    pub bounding_sphere_radius: f32,
    pub transformed_center: Vector3,
    pub start_index: u16,
    pub polygon_count: u16,
    pub min_vertex_index: u16,
    pub vertex_count: u16,
}

/// A draw `Flush` would issue, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum SortedDraw {
    /// A node whose buffers are not sorting buffers: `Set_Render_State` + `Draw_Triangles(start, polys, min_v, count)`.
    Immediate { render_state: usize, start_index: u32, polygon_count: u32, min_vertex_index: u32, vertex_count: u32 },
    /// A run of consecutive sorted triangles from one node: `Apply_Render_State` + `Draw_Triangles`.
    Sorted { render_state: usize, start_index: u32, polygon_count: u32, min_vertex_index: u32, vertex_count: u32 },
}

/// The merged vertex/index data the pooled nodes were copied into.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SortingPoolOutput {
    pub vertices: Vec<SortVertex>,
    pub triangles: Vec<ShortVectorIStruct>,
}

/// `class SortingRendererClass` with the file-static state as fields.
#[derive(Clone, Debug)]
pub struct SortingRendererClass {
    pub enable_triangle_draw: bool,
    default_sorting_poly_count: u32,
    default_sorting_vertex_count: u32,
    /// `sorted_list` (descending transformed Z).
    sorted_list: Vec<SortingNodeStruct>,
    overlapping_nodes: Vec<SortingNodeStruct>,
    overlapping_vertex_count: u32,
    overlapping_polygon_count: u32,
}

impl Default for SortingRendererClass {
    fn default() -> Self {
        SortingRendererClass {
            enable_triangle_draw: true, // `bool SortingRendererClass::_EnableTriangleDraw=true`
            default_sorting_poly_count: DEFAULT_SORTING_POLY_COUNT_INIT,
            default_sorting_vertex_count: DEFAULT_SORTING_VERTEX_COUNT_INIT,
            sorted_list: Vec::new(),
            overlapping_nodes: Vec::new(),
            overlapping_vertex_count: 0,
            overlapping_polygon_count: 0,
        }
    }
}

impl SortingRendererClass {
    pub fn new() -> Self {
        Self::default()
    }

    /// `SortingRendererClass::SetMinVertexBufferSize` (sortingrenderer.cpp:45): the poly count is half the vertex
    /// count ("typically have 2:1 vertex:triangle ratio").
    pub fn set_min_vertex_buffer_size(&mut self, val: u32) {
        self.default_sorting_vertex_count = val;
        self.default_sorting_poly_count = val / 2;
    }

    /// `SortingRendererClass::Insert_Triangles(sphere, start_index, polygon_count, min_vertex_index, vertex_count)`
    /// (sortingrenderer.cpp:213), the sorting-enabled path (when `WW3D::Is_Sorting_Enabled()` is false upstream
    /// draws immediately: the caller does that). `node` arrives with the render state snapshot, matrices and
    /// buffers filled in; this fills in the draw range, computes `transformed_center` and inserts it keeping
    /// descending Z.
    pub fn insert_triangles(
        &mut self,
        mut state: SortingNodeStruct,
        bounding_sphere_center: Vector3,
        bounding_sphere_radius: f32,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
    ) {
        assert!(state.sorting_buffers);
        state.bounding_sphere_center = bounding_sphere_center;
        state.bounding_sphere_radius = bounding_sphere_radius;
        state.start_index = start_index;
        state.polygon_count = polygon_count;
        state.min_vertex_index = min_vertex_index;
        state.vertex_count = vertex_count;
        self.finish_insert(state);
    }

    /// `Insert_Triangles(start_index, polygon_count, min_vertex_index, vertex_count)` without a sphere: a
    /// zero-radius sphere at the origin.
    pub fn insert_triangles_no_sphere(
        &mut self,
        state: SortingNodeStruct,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
    ) {
        self.insert_triangles(state, Vector3::new(0.0, 0.0, 0.0), 0.0, start_index, polygon_count, min_vertex_index, vertex_count);
    }

    /// `SortingRendererClass::Insert_VolumeParticle` (sortingrenderer.cpp:480): as `Insert_Triangles` with the
    /// polygon and vertex counts multiplied by `layer_count` (`unsigned short` arithmetic).
    pub fn insert_volume_particle(
        &mut self,
        mut state: SortingNodeStruct,
        bounding_sphere_center: Vector3,
        bounding_sphere_radius: f32,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
        layer_count: u16,
    ) {
        assert!(state.sorting_buffers);
        state.bounding_sphere_center = bounding_sphere_center;
        state.bounding_sphere_radius = bounding_sphere_radius;
        state.start_index = start_index;
        state.min_vertex_index = min_vertex_index;
        state.polygon_count = polygon_count.wrapping_mul(layer_count);
        state.vertex_count = vertex_count.wrapping_mul(layer_count);
        self.finish_insert(state);
    }

    /// The shared tail of both inserts: the transformed centre and the ordered insertion.
    fn finish_insert(&mut self, mut state: SortingNodeStruct) {
        let mtx = d3dx_mul(&state.world, &state.view);
        let t = d3dx_vec3_transform(&state.bounding_sphere_center, &mtx);
        state.transformed_center = Vector3::new(t[0], t[1], t[2]);

        // Walk the list: insert before the first node whose Z is smaller; else append.
        let pos = self.sorted_list.iter().position(|n| state.transformed_center.z > n.transformed_center.z);
        match pos {
            Some(p) => self.sorted_list.insert(p, state),
            None => self.sorted_list.push(state),
        }
    }

    /// `SortingRendererClass::Insert_To_Sorting_Pool` (sortingrenderer.cpp:278). Over `MAX_OVERLAPPING_NODES` the
    /// node is dropped (upstream releases its refs and asserts).
    fn insert_to_sorting_pool(&mut self, state: SortingNodeStruct) {
        if self.overlapping_nodes.len() >= MAX_OVERLAPPING_NODES {
            debug_assert!(false);
            return;
        }
        self.overlapping_vertex_count += state.vertex_count as u32;
        self.overlapping_polygon_count += state.polygon_count as u32;
        self.overlapping_nodes.push(state);
    }

    /// `SortingRendererClass::Flush_Sorting_Pool` (sortingrenderer.cpp:330). Builds the merged vertex array, sorts
    /// every pooled triangle by depth and returns the draws plus the merged data. The vertex/index buffer
    /// allocation sizing (`DynamicVBAccessClass`) and `Apply_Render_State` are device-only.
    pub fn flush_sorting_pool(&mut self) -> (SortingPoolOutput, Vec<SortedDraw>) {
        let mut out = SortingPoolOutput::default();
        let mut draws = Vec::new();
        if self.overlapping_nodes.is_empty() {
            return (out, draws);
        }

        // Get_Temp_Index_Array(count): at least DEFAULT_SORTING_POLY_COUNT entries; only the first
        // overlapping_polygon_count are used.
        let mut tis: Vec<TempIndexStruct> = vec![TempIndexStruct::default(); self.overlapping_polygon_count as usize];

        let mut polygon_array_offset: usize = 0;
        let mut vertex_array_offset: usize = 0;
        for node_id in 0..self.overlapping_nodes.len() {
            let state = &mut self.overlapping_nodes[node_id];
            let base = state.vba_offset + state.index_base_offset + state.min_vertex_index as usize;
            let vc = state.vertex_count as usize;
            out.vertices.extend_from_slice(&state.src_vertices[base..base + vc]);

            let mtx = d3dx_mul(&state.world, &state.view);
            let ibase = state.start_index as usize + state.iba_offset;
            let straight_z = mtx[0][2] == 0.0 && mtx[1][2] == 0.0 && mtx[3][2] == 0.0 && mtx[2][2] == 1.0;

            for i in 0..state.polygon_count as usize {
                let idx1 = state.src_indices[ibase + i * 3].wrapping_sub(state.min_vertex_index);
                let idx2 = state.src_indices[ibase + i * 3 + 1].wrapping_sub(state.min_vertex_index);
                let idx3 = state.src_indices[ibase + i * 3 + 2].wrapping_sub(state.min_vertex_index);
                debug_assert!((idx1 as u16) < state.vertex_count && (idx2 as u16) < state.vertex_count && (idx3 as u16) < state.vertex_count);
                let v1 = state.src_vertices[base + idx1 as usize];
                let v2 = state.src_vertices[base + idx2 as usize];
                let v3 = state.src_vertices[base + idx3 as usize];

                let array_index = i + polygon_array_offset;
                let tis_ptr = &mut tis[array_index];
                tis_ptr.tri.i = idx1.wrapping_add(vertex_array_offset as u16);
                tis_ptr.tri.j = idx2.wrapping_add(vertex_array_offset as u16);
                tis_ptr.tri.k = idx3.wrapping_add(vertex_array_offset as u16);
                tis_ptr.idx = node_id as u16;
                tis_ptr.z = if straight_z {
                    (v1.z + v2.z + v3.z) / 3.0f32
                } else {
                    (mtx[0][2] * (v1.x + v2.x + v3.x) + mtx[1][2] * (v1.y + v2.y + v3.y) + mtx[2][2] * (v1.z + v2.z + v3.z))
                        / 3.0f32
                        + mtx[3][2]
                };
            }
            state.min_vertex_index = vertex_array_offset as u16;
            polygon_array_offset += state.polygon_count as usize;
            vertex_array_offset += state.vertex_count as usize;
        }

        let n = tis.len();
        sort(&mut tis, 0, n);

        // (TODO upstream: break up rendering when more than 65536/3 triangles)
        for a in 0..n {
            out.triangles.push(tis[a].tri);
        }

        // Runs of consecutive triangles from the same node
        let mut count_to_render: u32 = 1;
        let mut start_index: u32 = 0;
        let mut node_id = tis[0].idx as usize;
        for i in 1..n {
            if node_id != tis[i].idx as usize {
                let state = &self.overlapping_nodes[node_id];
                draws.push(SortedDraw::Sorted {
                    render_state: state.render_state,
                    start_index: start_index * 3,
                    polygon_count: count_to_render,
                    min_vertex_index: state.min_vertex_index as u32,
                    vertex_count: state.vertex_count as u32,
                });
                count_to_render = 0;
                start_index = i as u32;
                node_id = tis[i].idx as usize;
            }
            count_to_render += 1;
        }
        if count_to_render > 0 {
            let state = &self.overlapping_nodes[node_id];
            draws.push(SortedDraw::Sorted {
                render_state: state.render_state,
                start_index: start_index * 3,
                polygon_count: count_to_render,
                min_vertex_index: state.min_vertex_index as u32,
                vertex_count: state.vertex_count as u32,
            });
        }

        // Release_Refs + clean_list: nodes are simply dropped.
        self.overlapping_nodes.clear();
        self.overlapping_polygon_count = 0;
        self.overlapping_vertex_count = 0;
        (out, draws)
    }

    /// `SortingRendererClass::Flush` (sortingrenderer.cpp:425): pops the sorted list from the front (farthest
    /// first); nodes with sorting buffers go to the pool, the rest draw immediately; then the pool flushes.
    /// `_Enable_Triangle_Draw` toggling and the view/world save/restore are device-only.
    pub fn flush(&mut self) -> (Vec<SortedDraw>, SortingPoolOutput, Vec<SortedDraw>) {
        let mut immediate = Vec::new();
        let list = std::mem::take(&mut self.sorted_list);
        for state in list {
            if state.sorting_buffers {
                self.insert_to_sorting_pool(state);
            } else {
                immediate.push(SortedDraw::Immediate {
                    render_state: state.render_state,
                    start_index: state.start_index as u32,
                    polygon_count: state.polygon_count as u32,
                    min_vertex_index: state.min_vertex_index as u32,
                    vertex_count: state.vertex_count as u32,
                });
            }
        }
        let (pool, sorted) = self.flush_sorting_pool();
        (immediate, pool, sorted)
    }

    /// `SortingRendererClass::Deinit` (sortingrenderer.cpp:468): empties both lists and the temp index array.
    pub fn deinit(&mut self) {
        self.sorted_list.clear();
        self.overlapping_nodes.clear();
    }
}
