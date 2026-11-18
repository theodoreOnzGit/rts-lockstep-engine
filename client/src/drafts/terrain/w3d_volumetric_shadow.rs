//! UNTESTED DRAFT (#202, #204, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DVolumetricShadow.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Shadow/W3DVolumetricShadow.cpp
//!   (`Geometry`, `PolyNeighbor`, `W3DShadowGeometryMesh`, `W3DShadowGeometry`, `W3DShadowGeometryManager` are classes of
//!   that .cpp)
//!
//! Slot tables replace the raw pointer lists (`m_shadowList`/`m_next`, the geometry hash table). A shadow caster's
//! `RenderObjClass` is a [`ShadowRenderObj`] snapshot and the game and device queries are [`VolumetricShadowEnv`]. The
//! shadow volumes built into `W3DBufferManager` slots (`VBM_FVF_XYZ` vertex slots and index slots) keep their vertices and
//! indices in the shadow (`static_vertices`/`static_indices`), since the buffer manager draft owns no device data; the
//! slots, the per-buffer render task lists (`m_renderTaskList`) and the draw ranges are as upstream's. Draw calls
//! (`RenderMeshVolume`, `RenderDynamicMeshVolume`, `renderStencilShadows`) are returned as data.
//!
//! Not translated: the `DO_TERRAIN_SHADOW_VOLUMES` code (`W3DShadowGeometryHeightmapMesh`, `isPatchShadowed`,
//! `loadTerrainShadows`; the macro is not defined anywhere upstream), `RenderMeshVolumeBounds` (`SV_DEBUG_BOUNDS`, not
//! defined), the `SV_DEBUG` and `RECORD_SHADOW_STRIP_STATS` code, the `CNC3` branch of `updateMeshVolume` (not defined;
//! the Generals branch is translated), `ASSUME_NEAR_LIGHTSOURCE` (not defined), `getRenderCost` (debug), and the
//! commented-out `W3DShadowGeometry::init` granny code.
//!
//! Notes on upstream behaviour kept as it is:
//! - `W3DVolumetricShadowManager::addShadow` reads `shadowInfo->m_sizeX` without a NULL check; `W3DShadowManager::addShadow`
//!   can pass NULL (a crash upstream). `add_shadow` reads 0 for `None`.
//! - `SetGeometry` loops over all `MAX_SHADOW_CASTER_MESHES` mesh entries of the geometry (unused ones have zero vertices).
//! - `allocateSilhouette` sizes the silhouette `numVertices * 5` ("HACK, HACK... Should be 2!").
//! - The two edge swaps in `constructVolume`/`constructVolumeVB` swap whole `Int`s: a pair of `Short` indices.
//! - `fabs`, `tan`, `asin`, `sqrt` are the double functions of <math.h> on promoted floats, converted back to `Real`
//!   where the result is stored in one; `abs(diff)` in `buildPolygonNeighbors` is on an `Int`.
//! - `Geometry::m_visibleState` is not set by its constructor (`Unknown` here).

use super::w3d_buffer_manager::{VbmFvfTypes, W3DBufferManager};
use super::w3d_common::{AABoxClass, RenderObjHandle, SphereClass};
use super::w3d_projected_shadow::OverlapType;
use super::w3d_shadow::{Shadow, ShadowRenderObj, ShadowTypeInfo, MAX_SHADOW_LIGHTS};
use super::w3d_terrain_background::{aabox_from_min_max, MinMaxAABoxClass};
use rts_lockstep_engine::common::base_type::{real_to_int_ceil, PI};
use rts_lockstep_engine::ww::matrix3d::{d3dx_matrix_inverse_stand_in, Matrix3D};
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::wwmath;

/// `MAX_SHADOW_CASTER_MESHES`: number of meshes allowed in animated hierarchy (must be <256 since index is a byte).
pub const MAX_SHADOW_CASTER_MESHES: usize = 160;
/// `MAX_SILHOUETTE_EDGES`.
pub const MAX_SILHOUETTE_EDGES: i32 = 1024;
/// `SHADOW_EXTRUSION_BUFFER`: amount to extend shadow volume beyond what's required to hit ground.
pub const SHADOW_EXTRUSION_BUFFER: f32 = 0.1;
/// `AIRBORNE_UNIT_GROUND_DELTA`.
pub const AIRBORNE_UNIT_GROUND_DELTA: f32 = 2.0;
/// `MAX_SHADOW_LENGTH_SCALE_FACTOR`: amount shadow can extend beyond the objects normal bounding sphere.
pub const MAX_SHADOW_LENGTH_SCALE_FACTOR: f32 = 1.0;
/// `MAX_SHADOW_LENGTH_EXTRA_AIRBORNE_SCALE_FACTOR`.
pub const MAX_SHADOW_LENGTH_EXTRA_AIRBORNE_SCALE_FACTOR: f32 = 1.5;
/// `MAX_EXTRUSION_LENGTH`: `512.0f*MAP_XY_FACTOR`.
pub const MAX_EXTRUSION_LENGTH: f32 = 512.0 * 10.0;
/// `MAX_SHADOW_EXTRUSION_UNDER_OBJECT_BEFORE_CLAMP`.
pub const MAX_SHADOW_EXTRUSION_UNDER_OBJECT_BEFORE_CLAMP: f32 = 5.0;
/// `SHADOW_SAMPLING_INTERVAL`: `MAP_XY_FACTOR * 2.0f`.
pub const SHADOW_SAMPLING_INTERVAL: f32 = 10.0 * 2.0;
/// `OVERHANGING_OBJECT_CLAMP_ANGLE`: `80.0f/180.0f*PI`.
pub const OVERHANGING_OBJECT_CLAMP_ANGLE: f32 = 80.0 / 180.0 * PI;
/// `MAX_SHADOW_VOLUME_VERTS`.
pub const MAX_SHADOW_VOLUME_VERTS: usize = 16384;
/// `MAX_POLYGON_NEIGHBORS`: we use nothing but triangles for geometry polygons so we have at most 3 neighbors.
pub const MAX_POLYGON_NEIGHBORS: usize = 3;
/// `NO_NEIGHBOR`: entry value for neighbor when there isn't one.
pub const NO_NEIGHBOR: i16 = -1;
/// `POLY_VISIBLE`: polygon is visible from light.
pub const POLY_VISIBLE: u8 = 0x01;
/// `POLY_PROCESSED`: this poly has been processed.
pub const POLY_PROCESSED: u8 = 0x02;
/// `W3DVolumetricShadow::SHADOW_DYNAMIC`: flag indicating a dynamic shadow caster (animated mesh).
pub const SHADOW_DYNAMIC: i32 = 0x1;

/// `SHADOW_STATIC_VOLUME_VERTEX` (= `SHADOW_DYNAMIC_VOLUME_VERTEX`, `SV_DEBUG` not defined): the vertex passed to D3D.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShadowVolumeVertex {
    /// `x`.
    pub x: f32,
    /// `y`.
    pub y: f32,
    /// `z`.
    pub z: f32,
}

/// The shared dynamic shadow buffers of the file (`shadowVertexBufferD3D`, `shadowIndexBufferD3D`, `nShadowVertsInBuf`,
/// `nShadowStartBatchVertex`, `nShadowIndicesInBuf`, `nShadowStartBatchIndex`, `SHADOW_VERTEX_SIZE`,
/// `SHADOW_INDEX_SIZE`), which the projected terrain shadow also uses.
#[derive(Clone, Debug)]
pub struct ShadowVolumeBuffers {
    /// `shadowVertexBufferD3D` (`SHADOW_VERTEX_SIZE` vertices; empty while released).
    pub vertex_buffer: Vec<ShadowVolumeVertex>,
    /// `shadowIndexBufferD3D` (`SHADOW_INDEX_SIZE` indices; empty while released).
    pub index_buffer: Vec<u16>,
    /// `nShadowVertsInBuf`: model vertices in vertex buffer.
    pub n_shadow_verts_in_buf: i32,
    /// `nShadowStartBatchVertex`.
    pub n_shadow_start_batch_vertex: i32,
    /// `nShadowIndicesInBuf`.
    pub n_shadow_indices_in_buf: i32,
    /// `nShadowStartBatchIndex`.
    pub n_shadow_start_batch_index: i32,
    /// `SHADOW_VERTEX_SIZE`.
    pub shadow_vertex_size: i32,
    /// `SHADOW_INDEX_SIZE`.
    pub shadow_index_size: i32,
}

impl Default for ShadowVolumeBuffers {
    fn default() -> Self {
        Self::new()
    }
}

impl ShadowVolumeBuffers {
    /// The globals' initial values: `SHADOW_VERTEX_SIZE=4096`, `SHADOW_INDEX_SIZE=8192`, counters 0. The buffers are
    /// allocated at their size so the projected terrain shadow can write before `ReAcquireResources` (upstream would
    /// dereference NULL).
    pub fn new() -> Self {
        ShadowVolumeBuffers {
            vertex_buffer: vec![ShadowVolumeVertex::default(); 4096],
            index_buffer: vec![0u16; 8192],
            n_shadow_verts_in_buf: 0,
            n_shadow_start_batch_vertex: 0,
            n_shadow_indices_in_buf: 0,
            n_shadow_start_batch_index: 0,
            shadow_vertex_size: 4096,
            shadow_index_size: 8192,
        }
    }
}

/// `Geometry::VisibleState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisibleState {
    /// `STATE_UNKNOWN` (`CollisionMath::BOTH`).
    Unknown,
    /// `STATE_VISIBLE` (`CollisionMath::INSIDE`).
    Visible,
    /// `STATE_INVISIBLE` (`CollisionMath::OUTSIDE`).
    Invisible,
}

/// `struct Geometry`: a simple structure to hold random geometry (vertices, polygons, etc.). Used to store shadow volumes.
#[derive(Clone, Debug)]
pub struct Geometry {
    /// `m_verts`.
    pub verts: Vec<Vector3>,
    /// `m_indices` (`numPolygons*3`).
    pub indices: Vec<u16>,
    /// `m_numPolygon`.
    pub num_polygon: i32,
    /// `m_numVertex`.
    pub num_vertex: i32,
    /// `m_numActivePolygon`: number of polygons filled with valid data.
    pub num_active_polygon: i32,
    /// `m_numActiveVertex`: number of vertices filled with valid data.
    pub num_active_vertex: i32,
    /// `m_flags`: geometry attribute flags - static vs. dynamic, etc.
    pub flags: i32,
    /// `m_boundingBox`: object space bounding box of shadow volume.
    pub bounding_box: AABoxClass,
    /// `m_boundingSphere`: object space bounding sphere of shadow volume.
    pub bounding_sphere: SphereClass,
    /// `m_visibleState`: flag if this geometry was visible in this frame.
    pub visible_state: VisibleState,
}

impl Default for Geometry {
    fn default() -> Self {
        Self::new()
    }
}

impl Geometry {
    /// `Geometry::Geometry`.
    pub fn new() -> Self {
        Geometry {
            verts: Vec::new(),
            indices: Vec::new(),
            num_polygon: 0,
            num_vertex: 0,
            num_active_polygon: 0,
            num_active_vertex: 0,
            flags: 0,
            bounding_box: AABoxClass::default(),
            bounding_sphere: SphereClass::default(),
            visible_state: VisibleState::Unknown,
        }
    }

    /// `Geometry::Create`.
    pub fn create(&mut self, num_vertices: i32, num_polygons: i32) -> bool {
        if num_vertices != 0 {
            self.verts = vec![Vector3::default(); num_vertices as usize];
        }
        if num_polygons != 0 {
            self.indices = vec![0u16; (num_polygons * 3) as usize];
        }
        self.num_polygon = num_polygons;
        self.num_vertex = num_vertices;
        self.num_active_polygon = 0;
        self.num_active_vertex = 0;
        true
    }

    /// `Geometry::Release`.
    pub fn release(&mut self) {
        self.verts = Vec::new();
        self.indices = Vec::new();
        self.num_active_polygon = 0;
        self.num_polygon = 0;
        self.num_active_vertex = 0;
        self.num_vertex = 0;
    }

    /// `Geometry::SetPolygonIndex`.
    pub fn set_polygon_index(&mut self, poly_id: i32, index_list: [i16; 3]) -> i32 {
        let b = (poly_id * 3) as usize;
        self.indices[b] = index_list[0] as u16;
        self.indices[b + 1] = index_list[1] as u16;
        self.indices[b + 2] = index_list[2] as u16;
        3
    }

    /// `Geometry::SetVertex`.
    pub fn set_vertex(&mut self, vert_id: i32, v: &Vector3) {
        self.verts[vert_id as usize] = *v;
    }

    /// `Geometry::FindVertexInRange`: find a vertex within given range.
    pub fn find_vertex_in_range(&self, start: i32, end: i32, v: &Vector3) -> i32 {
        for i in start..end {
            if (self.verts[i as usize] - *v).length2() == 0.0 {
                return i;
            }
        }
        -1
    }
}

/// `NeighborEdge`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeighborEdge {
    /// `neighborIndex`: index of polygon who is our neighbor, `NO_NEIGHBOR` if there isn't one.
    pub neighbor_index: i16,
    /// `neighborEdgeIndex`: the two vertex indices that represent the shared edge.
    pub neighbor_edge_index: [i16; 2],
}

/// `struct PolyNeighbor`.
#[derive(Clone, Copy, Debug, Default)]
pub struct PolyNeighbor {
    /// `myIndex`: our polygon index so we know who we are.
    pub my_index: i16,
    /// `status`: status flags used when processing neighbors.
    pub status: u8,
    /// `neighbor`.
    pub neighbor: [NeighborEdge; MAX_POLYGON_NEIGHBORS],
}

/// A mesh as `W3DShadowGeometry::initFrom*` reads it (`MeshModelClass`/`ShdSubMeshClass` accessors and flags).
#[derive(Clone, Debug, Default)]
pub struct ShadowMeshSource {
    /// `Get_Vertex_Array`.
    pub vertices: Vec<Vector3>,
    /// `Get_Polygon_Array` (`TriIndex` I, J, K).
    pub polygons: Vec<(u32, u32, u32)>,
    /// `Is_Alpha()`.
    pub is_alpha: bool,
    /// `Is_Translucent()`.
    pub is_translucent: bool,
    /// `Get_Flag(MeshGeometryClass::CAST_SHADOW)`.
    pub cast_shadow: bool,
    /// `Get_Flag(MeshGeometryClass::SKIN)`.
    pub skin: bool,
}

/// A model of the top LOD of an HLOD (`Peek_Lod_Model(top, i)`).
#[derive(Clone, Debug)]
pub enum HlodModelSource {
    /// NULL.
    None,
    /// `CLASSID_MESH`.
    Mesh(ShadowMeshSource),
    /// `CLASSID_SHDMESH`: its sub meshes (each with `CAST_SHADOW` and `SKIN` flags) and `Is_Translucent()`.
    ShdMesh {
        /// `shd_mesh->Is_Translucent()`.
        is_translucent: bool,
        /// The sub meshes.
        sub_meshes: Vec<ShadowMeshSource>,
    },
    /// Another class id.
    Other,
}

/// The render object `W3DShadowGeometryManager::Load_Geom` inspects (`Class_ID()`).
#[derive(Clone, Debug)]
pub enum ShadowGeomSource {
    /// `CLASSID_HLOD`: the models of the top LOD (`Get_LOD_Count()-1`).
    Hlod(Vec<HlodModelSource>),
    /// `CLASSID_MESH`.
    Mesh(ShadowMeshSource),
    /// Unknown render object type.
    Other,
}

/// `class W3DShadowGeometryMesh`: original mesh specific data and geometry, cleaned to remove replicated vertices and
/// cache mesh data needed for faster silhouette computation.
#[derive(Clone, Debug, Default)]
pub struct W3DShadowGeometryMesh {
    /// `m_meshRobjIndex`: index of this mesh within hlod robj.
    pub mesh_robj_index: i32,
    /// `m_verts`: array of vertices.
    pub verts: Vec<Vector3>,
    /// `m_polygonNormals`: array of face normals (`None` until built).
    pub polygon_normals: Option<Vec<Vector3>>,
    /// `m_numVerts`: number of actual vertices after duplicates are removed.
    pub num_verts: i32,
    /// `m_numPolygons`: number of polygons in source geometry.
    pub num_polygons: i32,
    /// `m_polygons`: array of 3 vertex indices per face.
    pub polygons: Vec<(u32, u32, u32)>,
    /// `m_parentVerts`: array of parent vertex indices for each vertex.
    pub parent_verts: Vec<u16>,
    /// `m_polyNeighbors`: the neighbor info indexed by polygon id.
    pub poly_neighbors: Vec<PolyNeighbor>,
    /// `m_numPolyNeighbors`.
    pub num_poly_neighbors: i32,
}

impl W3DShadowGeometryMesh {
    /// `W3DShadowGeometryMesh::W3DShadowGeometryMesh` (W3DVolumetricShadow.cpp:897).
    pub fn new() -> Self {
        Self::default()
    }

    /// `GetNumPolygon`.
    pub fn get_num_polygon(&self) -> i32 {
        self.num_polygons
    }

    /// `GetNumVertex`.
    pub fn get_num_vertex(&self) -> i32 {
        self.num_verts
    }

    /// `GetPolygonNormal`.
    pub fn get_polygon_normal(&self, poly_norm_id: i32) -> Vector3 {
        self.polygon_normals.as_ref().unwrap()[poly_norm_id as usize]
    }

    /// `GetVertex`.
    pub fn get_vertex(&self, vert_id: i32) -> Vector3 {
        self.verts[vert_id as usize]
    }

    /// `GetPolygonIndex`: indices to the 3 vertices of this face (through the parent vertices).
    pub fn get_polygon_index(&self, poly_id: i32) -> [i16; 3] {
        let polyi = self.polygons[poly_id as usize];
        [self.parent_verts[polyi.0 as usize] as i16, self.parent_verts[polyi.1 as usize] as i16, self.parent_verts[polyi.2 as usize] as i16]
    }

    /// `buildPolygonNormal` (computing, no cache).
    fn build_polygon_normal(&self, poly_norm_id: i32) -> Vector3 {
        if let Some(n) = self.polygon_normals.as_ref() {
            return n[poly_norm_id as usize];
        }
        let index_list = self.get_polygon_index(poly_norm_id);
        let v0 = self.get_vertex(index_list[0] as i32);
        let v1 = self.get_vertex(index_list[1] as i32);
        let v2 = self.get_vertex(index_list[2] as i32);

        // compute triangle normal by crossing 2 edges
        let edge1 = v1 - v0;
        let edge2 = v1 - v2;
        Vector3::normalized_cross_product(&edge2, &edge1)
    }

    /// `buildPolygonNormals`.
    pub fn build_polygon_normals(&mut self) {
        if self.polygon_normals.is_none() {
            // need to allocate storage
            let mut temp_vec = vec![Vector3::default(); self.num_polygons as usize];
            for i in 0..self.num_polygons {
                temp_vec[i as usize] = self.build_polygon_normal(i);
            }
            self.polygon_normals = Some(temp_vec);
        }
    }

    /// `W3DShadowGeometryMesh::GetPolyNeighbor` (W3DVolumetricShadow.cpp:923): builds the neighbors on first use.
    pub fn get_poly_neighbor(&mut self, poly_index: i32) -> Option<usize> {
        if self.poly_neighbors.is_empty() {
            self.build_polygon_neighbors();
        }

        // sanity
        if poly_index < 0 || poly_index >= self.num_poly_neighbors {
            return None;
        }

        Some(poly_index as usize)
    }

    /// `W3DShadowGeometryMesh::allocateNeighbors` (W3DVolumetricShadow.cpp:1134).
    pub fn allocate_neighbors(&mut self, num_polys: i32) -> bool {
        // assert( m_numPolyNeighbors == 0 ); assert( m_polyNeighbors == NULL );
        self.poly_neighbors = vec![PolyNeighbor::default(); num_polys as usize];
        // list is now acutally allocated
        self.num_poly_neighbors = num_polys;
        true
    }

    /// `W3DShadowGeometryMesh::deleteNeighbors` (W3DVolumetricShadow.cpp:1162).
    pub fn delete_neighbors(&mut self) {
        if !self.poly_neighbors.is_empty() {
            self.poly_neighbors = Vec::new();
            self.num_poly_neighbors = 0;
        }
    }

    /// `W3DShadowGeometryMesh::buildPolygonNeighbors` (W3DVolumetricShadow.cpp:948): whenever we set a new geometry we want
    /// to build some information about the faces in the new geometry so that we can efficiently traverse across the
    /// surface to neighboring polygons.
    pub fn build_polygon_neighbors(&mut self) {
        // Make sure we have polygon normals BEFORE we need them...
        self.build_polygon_normals();

        // how many polygons are in our geometry
        let num_polys = self.get_num_polygon();

        // if there are no polygons for this geometry then we should have no neighbor information
        if num_polys == 0 {
            // if our geometry has somehow deformed and we used to have polygon neighbor information we should delete it
            // before we bail
            if self.num_poly_neighbors != 0 {
                self.delete_neighbors();
            }

            return; // nothing to see here people, move along
        }

        // in the event that this geometry can deform on the fly or we are building our neighbor information for the very
        // first time ... if our current geometry has a different number of polygons than we had previously calculated we
        // need to delete and reallocate a new storage space for the neighbor information
        if num_polys != self.num_poly_neighbors {
            // delete the old neighbor storage
            self.delete_neighbors();

            // allocate a new pool for neighbor information
            if !self.allocate_neighbors(num_polys) {
                return;
            }
        }

        // initialize all polygon neighbor information to none and assign our own reference to myIndex so we know who we are
        for i in 0..self.num_poly_neighbors as usize {
            // assign our own identification
            self.poly_neighbors[i].my_index = i as i16;

            // assign our neighbors to none
            for j in 0..MAX_POLYGON_NEIGHBORS {
                self.poly_neighbors[i].neighbor[j].neighbor_index = NO_NEIGHBOR;
            }
        }

        // assign polygon data for each of our polygons
        for i in 0..self.num_poly_neighbors {
            // get the indices of the three triangle points for this polygon
            let poly = self.get_polygon_index(i);
            let v_norm = self.get_polygon_normal(i);

            // find the neighbors of this polygon
            for j in 0..self.num_poly_neighbors {
                let mut index1_pos = [0i32; 2]; // positions of shared edge vertices in triangle list. (0,1 or 2)

                // ignore our own polygon
                if i == j {
                    continue;
                }

                // get the vertex index information for this other polygon
                let other_poly = self.get_polygon_index(j);

                // if 2 of the 3 vertex indices are the same then these polygons are neighbors
                //
                // Also check if winding order of vertices on edge is opposite. If vertices are in same order as our
                // polygon, then it's not a valid edge because the neighbor is flipped.

                let mut index1: i32 = -1;
                let mut index2: i32 = -1;
                for a in 0..3i32 {
                    for b in 0..3i32 {
                        if poly[a as usize] == other_poly[b as usize] {
                            if index1 == -1 {
                                index1 = poly[a as usize] as i32; // record matching index1
                                index1_pos[0] = a;
                                index1_pos[1] = b;
                            } else if index2 == -1 {
                                // Check direction of edge in each polygon.  If they are same direction skip it.
                                let diff1 = a - index1_pos[0];
                                let diff2 = b - index1_pos[1];
                                // ((diff1&0x80000000)^((abs(diff1)&2)<<30)): the mask is an unsigned literal
                                let k1 = ((diff1 as u32) & 0x8000_0000) ^ (((diff1.abs() & 2) as u32) << 30);
                                let k2 = ((diff2 as u32) & 0x8000_0000) ^ (((diff2.abs() & 2) as u32) << 30);
                                if k1 != k2 {
                                    let v_other_norm = self.get_polygon_normal(j);

                                    // Check if the 2 polygons face in exactly opposite directions - don't allow this type
                                    // of neighbor.
                                    if wwmath::fabs(Vector3::dot_product(&v_other_norm, &v_norm) + 1.0) <= 0.01 {
                                        continue;
                                    }

                                    index2 = poly[a as usize] as i32; // record matching index2
                                } else {
                                    continue;
                                }
                            } else {
                                // This is the same poly facing opposite direction.
                                index1 = -1;
                                index2 = -1;
                                continue;
                            }
                        }
                    }
                }
                if index1 != -1 && index2 != -1 {
                    // the polygon j is a neighbor of our polygon i, put the j index into the first free neighbor slot for
                    // polygon i
                    for a in 0..MAX_POLYGON_NEIGHBORS {
                        if self.poly_neighbors[i as usize].neighbor[a].neighbor_index == NO_NEIGHBOR {
                            // record the neighbor index
                            self.poly_neighbors[i as usize].neighbor[a].neighbor_index = j as i16;

                            // record the sharded edge vertex indices
                            self.poly_neighbors[i as usize].neighbor[a].neighbor_edge_index[0] = index1 as i16;
                            self.poly_neighbors[i as usize].neighbor[a].neighbor_edge_index[1] = index2 as i16;

                            break; // exit for a
                        }
                    }

                    // error condition, if our counter a is at the max number of neighbors, which is 3 for a triangle mesh,
                    // we did something wrong here because that would mean we found a 4th match! (upstream's report is
                    // commented out)
                }
            }
        }
    }
}

/// `class W3DShadowGeometry`: wraps any shadow casting geometry with additional data needed for efficient shadow volume
/// generation.
#[derive(Clone, Debug)]
pub struct W3DShadowGeometry {
    /// `m_namebuf`: name of model hierarchy.
    pub name: Vec<u8>,
    /// `m_meshList[MAX_SHADOW_CASTER_MESHES]`: collection of meshes for this geometry.
    pub mesh_list: Vec<W3DShadowGeometryMesh>,
    /// `m_meshCount`: number of meshes in hierarchy.
    pub mesh_count: i32,
    /// `m_numTotalsVerts`: number of verts in entire hierarchy.
    pub num_totals_verts: i32,
}

impl Default for W3DShadowGeometry {
    fn default() -> Self {
        Self::new()
    }
}

/// Finds the duplicated vertices of a mesh as every `initFrom*` does: returns `(vertParent, newVertexCount)`. The parent
/// array is initialised to 0xffff (`memset(.., 0xffffffff, ..)`).
fn find_parent_vertices(verts: &[Vector3]) -> (Vec<u16>, i32) {
    let num_verts = verts.len();
    let mut vert_parent = vec![0xffffu16; num_verts];
    let mut new_vertex_count = num_verts as i32;
    // Find all duplicated vertices.
    for j in 0..num_verts {
        if vert_parent[j] != 0xffff {
            continue; // this vertex has already been processed
        }

        let v_curr = verts[j];

        for k in (j + 1)..num_verts {
            let len = v_curr - verts[k];
            if len.length2() == 0.0 {
                // found duplicate vertex
                vert_parent[k] = j as u16;
                new_vertex_count -= 1; // decrease total vertices since duplicate found.
            }
        }
        vert_parent[j] = j as u16; // first instance of new vertex
    }
    (vert_parent, new_vertex_count)
}

impl W3DShadowGeometry {
    /// `W3DShadowGeometry::W3DShadowGeometry` (the members are uninitialised upstream: zero).
    pub fn new() -> Self {
        W3DShadowGeometry { name: Vec::new(), mesh_list: vec![W3DShadowGeometryMesh::new(); MAX_SHADOW_CASTER_MESHES], mesh_count: 0, num_totals_verts: 0 }
    }

    /// `W3DShadowGeometry::Set_Name`.
    pub fn set_name(&mut self, name: &[u8]) {
        let n = name.len().min(2 * 16 - 1);
        self.name = name[..n].to_vec();
    }

    /// `getMeshCount`.
    pub fn get_mesh_count(&self) -> i32 {
        self.mesh_count
    }

    /// `GetNumTotalVertex`.
    pub fn get_num_total_vertex(&self) -> i32 {
        self.num_totals_verts
    }

    /// Fills the next mesh entry from a mesh source (the body both loops share). `Err(())` is upstream's early `return
    /// FALSE` for too many vertices.
    fn add_mesh(&mut self, src: &ShadowMeshSource, mesh_robj_index: i32) -> Result<(), ()> {
        let mc = self.mesh_count as usize;
        // DEBUG_ASSERTCRASH(m_meshCount < MAX_SHADOW_CASTER_MESHES, ("Too many shadow sub-meshes"));
        let geom_mesh = &mut self.mesh_list[mc];
        geom_mesh.mesh_robj_index = mesh_robj_index;
        geom_mesh.num_verts = src.vertices.len() as i32;
        geom_mesh.verts = src.vertices.clone();
        geom_mesh.num_polygons = src.polygons.len() as i32;
        geom_mesh.polygons = src.polygons.clone();

        if geom_mesh.num_verts as usize > MAX_SHADOW_VOLUME_VERTS {
            return Err(()); // too many vertices to process
        }

        // reset index of all vertices; find all duplicated vertices
        let (vert_parent, new_vertex_count) = find_parent_vertices(&geom_mesh.verts);
        geom_mesh.parent_verts = vert_parent;
        geom_mesh.num_verts = new_vertex_count; // adjust actual vertex count to ignore duplicates
        self.num_totals_verts += new_vertex_count;

        // build our neighboring polygon information (commented out upstream: built on first use)
        self.mesh_count += 1;
        Ok(())
    }

    /// `W3DShadowGeometry::initFromHLOD` (W3DVolumetricShadow.cpp:627): initialize the geometry from the top LOD of a W3D
    /// HLOD object.
    pub fn init_from_hlod(&mut self, models: &[HlodModelSource]) -> i32 {
        self.num_totals_verts = 0;

        for (i, model) in models.iter().enumerate() {
            match model {
                HlodModelSource::Mesh(src) => {
                    // transparent meshes that don't have forced shadows will not cast volumetric shadows
                    if (src.is_alpha || src.is_translucent) && !src.cast_shadow {
                        continue;
                    }
                    // skin meshes should never cast a volumetric shadow
                    if src.skin {
                        continue;
                    }
                    if self.add_mesh(src, i as i32).is_err() {
                        return 0;
                    }
                }
                HlodModelSource::ShdMesh { is_translucent, sub_meshes } => {
                    for sub in sub_meshes.iter() {
                        if !sub.cast_shadow {
                            continue; // Only cast shadows from meshes with the shadow flag ENABLED!
                        }

                        // transparent meshes that don't have forced shadows will not cast volumetric shadows
                        if *is_translucent && !sub.cast_shadow {
                            continue;
                        }

                        // skin meshes should never cast a volumetric shadow
                        if sub.skin {
                            continue;
                        }

                        if self.add_mesh(sub, i as i32).is_err() {
                            return 0;
                        }
                    }
                }
                _ => {}
            }
        }

        (self.mesh_count != 0) as i32
    }

    /// `W3DShadowGeometry::initFromMesh` (W3DVolumetricShadow.cpp:787): initialize the geometry from a W3D mesh object.
    pub fn init_from_mesh(&mut self, src: &ShadowMeshSource) -> i32 {
        let mc = self.mesh_count as usize;
        // assert (m_meshCount < MAX_SHADOW_CASTER_MESHES);

        self.mesh_list[mc].mesh_robj_index = -1; // robj is the mesh so no index needed.
        if (src.is_alpha || src.is_translucent) && !src.cast_shadow {
            return 0; // transparent meshes that don't have forced shadows will not cast volumetric shadows
        }

        let geom_mesh = &mut self.mesh_list[mc];
        geom_mesh.num_verts = src.vertices.len() as i32;
        geom_mesh.verts = src.vertices.clone();
        geom_mesh.num_polygons = src.polygons.len() as i32;
        geom_mesh.polygons = src.polygons.clone();

        if geom_mesh.num_verts as usize > MAX_SHADOW_VOLUME_VERTS {
            return 0; // too many vertices to process
        }

        let (vert_parent, new_vertex_count) = find_parent_vertices(&geom_mesh.verts);
        geom_mesh.parent_verts = vert_parent;
        geom_mesh.num_verts = new_vertex_count; // adjust actual vertex count to ignore duplicates

        self.mesh_count = 1;
        self.num_totals_verts = new_vertex_count;

        1
    }

    /// `W3DShadowGeometry::init` (W3DVolumetricShadow.cpp:849): returns TRUE (the granny code is commented out).
    pub fn init(&mut self) -> i32 {
        1
    }
}

/// `class W3DShadowGeometryManager`: manages shadow geometry for each render object.
#[derive(Clone, Debug, Default)]
pub struct W3DShadowGeometryManager {
    /// The geometries (storage; ids stay valid after `Free_All_Geoms`, as the reference counts keep them alive upstream).
    pub storage: Vec<W3DShadowGeometry>,
    /// `GeomPtrTable`: `(name, id)`, keyed with `stricmp`.
    pub geom_ptr_table: Vec<(Vec<u8>, usize)>,
    /// `MissingGeomTable`.
    pub missing_geom_table: Vec<Vec<u8>>,
}

fn eq_no_case(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

impl W3DShadowGeometryManager {
    /// `W3DShadowGeometryManager::W3DShadowGeometryManager`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `Free_All_Geoms` (W3DVolumetricShadow.cpp:3972): release all loaded geometries.
    pub fn free_all_geoms(&mut self) {
        self.geom_ptr_table.clear();
    }

    /// `Peek_Geom`: find geometry in cache.
    pub fn peek_geom(&self, name: &[u8]) -> Option<usize> {
        self.geom_ptr_table.iter().find(|(n, _)| eq_no_case(n, name)).map(|(_, id)| *id)
    }

    /// `Get_Geom`: get geometry from cache (and increment its reference count).
    pub fn get_geom(&self, name: &[u8]) -> Option<usize> {
        self.peek_geom(name)
    }

    /// `Add_Geom`: add geometry to cache.
    pub fn add_geom(&mut self, new_geom: W3DShadowGeometry) -> bool {
        let id = self.storage.len();
        let name = new_geom.name.clone();
        self.storage.push(new_geom);
        self.geom_ptr_table.push((name, id));
        true
    }

    /// `Register_Missing`.
    pub fn register_missing(&mut self, name: &[u8]) {
        self.missing_geom_table.push(name.to_vec());
    }

    /// `Is_Missing`.
    pub fn is_missing(&self, name: &[u8]) -> bool {
        self.missing_geom_table.iter().any(|n| eq_no_case(n, name))
    }

    /// `Load_Geom` (W3DVolumetricShadow.cpp:4047): create shadow geometry from a reference render object. 0 on success, 1
    /// on error.
    pub fn load_geom(&mut self, source: &ShadowGeomSource, name: &[u8]) -> i32 {
        let mut newgeom = W3DShadowGeometry::new();
        newgeom.set_name(name);

        let res = match source {
            ShadowGeomSource::Hlod(models) => newgeom.init_from_hlod(models),
            ShadowGeomSource::Mesh(m) => newgeom.init_from_mesh(m),
            ShadowGeomSource::Other => 0, // unknown render object type
        };

        if res != 1 {
            // load failed!
            return 1;
        } else if self.peek_geom(&newgeom.name).is_some() {
            // duplicate exists!
            return 1;
        } else {
            self.add_geom(newgeom);
        }

        0
    }
}

/// A mesh of the caster as the shadow reads it (`hlod->Peek_Lod_Model(0, i)` or the render object itself):
/// `Get_Transform`, `Get_Bounding_Box`, `Is_Not_Hidden_At_All`.
#[derive(Clone, Copy, Debug)]
pub struct ShadowMeshObj {
    /// `Get_Transform()`.
    pub transform: Matrix3D,
    /// `Get_Bounding_Box()`.
    pub bounding_box: AABoxClass,
    /// `Is_Not_Hidden_At_All()`.
    pub is_not_hidden_at_all: bool,
}

/// What the volumetric shadows ask of the game and the device.
pub trait VolumetricShadowEnv {
    /// `TheGlobalData->m_useShadowVolumes`.
    fn use_shadow_volumes(&self) -> bool;
    /// `DX8Wrapper::Has_Stencil()`.
    fn has_stencil(&self) -> bool;
    /// `TheTerrainLogic->getGroundHeight(x, y)`; `None` when there is no `TheTerrainLogic`.
    fn logic_ground_height(&self, x: f32, y: f32) -> Option<f32>;
    /// `TheTerrainRenderObject->getHeightMapHeight(x, y, NULL)`.
    fn height_map_height(&self, x: f32, y: f32) -> f32;
    /// `TheTerrainRenderObject->getMinHeight()`.
    fn terrain_min_height(&self) -> f32;
    /// `TheTerrainRenderObject->Cast_Ray(raytest)`: `Some((contact point, start_bad))` when it hit.
    fn cast_ray_terrain(&self, from: &Vector3, to: &Vector3) -> Option<(Vector3, bool)>;
    /// `CollisionMath::Overlap_Test(*shadowCameraFrustum, sphere)`.
    fn frustum_sphere_overlap(&self, sphere: &SphereClass) -> OverlapType;
    /// `CollisionMath::Overlap_Test(*shadowCameraFrustum, box)`.
    fn frustum_box_overlap(&self, b: &AABoxClass) -> OverlapType;
    /// `TheTerrainRenderObject->getMaximumVisibleBox(*shadowCameraFrustum, &bbox, TRUE)`.
    fn maximum_visible_box(&self) -> AABoxClass;
    /// The caster's mesh `mesh_robj_index` (`hlod->Peek_Lod_Model(0, index)`, or the render object itself when negative).
    fn mesh_of_robj(&self, robj: RenderObjHandle, mesh_robj_index: i32) -> Option<ShadowMeshObj>;
    /// `TheTacticalView->getOrigin`, `getWidth`, `getHeight`.
    fn tactical_view(&self) -> (i32, i32, i32, i32);
    /// `DX8Wrapper::Get_Current_Caps()->Get_DX8_Caps().PrimitiveMiscCaps & D3DPMISCCAPS_COLORWRITEENABLE`.
    fn supports_color_write_enable(&self) -> bool;
}

/// `W3DVolumetricShadowRenderTask` (a `W3DBufferManager::W3DRenderTask`): the volume to render.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct W3DVolumetricShadowRenderTask {
    /// `m_parentShadow`: main casting object to which this volume belongs (a shadow slot).
    pub parent_shadow: usize,
    /// `m_meshIndex`: mesh index of volume within parent to render.
    pub mesh_index: u8,
    /// `m_lightIndex`: light index of volume within parent to render.
    pub light_index: u8,
}

/// One shadow volume draw: `SetTransform(WORLD, mesh transform)`, `SetIndices(.., base_vertex)` and
/// `DrawIndexedPrimitive(TRIANGLELIST, 0, verts, start_index, polys)`, with the volume's data.
#[derive(Clone, Debug)]
pub struct VolumeDraw {
    /// The mesh transform (the renderer transposes it).
    pub world: Matrix3D,
    /// `SetIndices` base vertex.
    pub base_vertex: i32,
    /// `DrawIndexedPrimitive`'s `startIndex`.
    pub start_index: i32,
    /// Polygon count.
    pub polys: i32,
    /// Vertex count.
    pub verts: i32,
    /// True for `RenderDynamicMeshVolume` (the shared dynamic buffer), false for a static `W3DBufferManager` slot.
    pub dynamic: bool,
    /// The volume's vertices.
    pub vertices: Vec<ShadowVolumeVertex>,
    /// The volume's indices (`polys*3`).
    pub indices: Vec<u16>,
}

/// `renderStencilShadows`' full-viewport quad: four `D3DFVF_XYZRHW | D3DFVF_DIFFUSE` vertices in `TRIANGLESTRIP` order
/// and the stencil mask (`~getStencilShadowMask()`) it is drawn with (func LESSEQUAL, ref 1, keep).
#[derive(Clone, Copy, Debug)]
pub struct StencilQuad {
    /// `v[i].p`: `(x, y, 0, 1)`.
    pub positions: [(f32, f32); 4],
    /// `v[i].color` (the shadow colour).
    pub color: u32,
    /// `D3DRS_STENCILMASK`.
    pub stencil_mask: u32,
}

/// What `renderShadows` draws, in order.
#[derive(Clone, Debug, Default)]
pub struct VolumetricShadowRender {
    /// Dynamic volumes rendered while updating (incrementing pass), in order.
    pub dynamic_update_draws: Vec<VolumeDraw>,
    /// Static volumes, front faces (`STENCILPASS = INCR`, `CULL_CW`).
    pub static_incr_draws: Vec<VolumeDraw>,
    /// Static volumes, back faces (`STENCILPASS = DECRSAT`, `CULL_CCW`).
    pub static_decr_draws: Vec<VolumeDraw>,
    /// Dynamic volumes again (decrementing pass).
    pub dynamic_decr_draws: Vec<VolumeDraw>,
    /// The stencil fill quad (`renderStencilShadows`).
    pub stencil_quad: Option<StencilQuad>,
    /// `numRenderedShadows` (counted, never used further upstream).
    pub num_rendered_shadows: i32,
    /// The stencil function: `true` for `NOTEQUAL` (mask 0x80808080), else `GREATEREQUAL`.
    pub stencil_not_equal: bool,
}

/// `class W3DVolumetricShadow`.
#[derive(Clone, Debug)]
pub struct W3DVolumetricShadow {
    /// The `Shadow` base.
    pub base: Shadow,
    /// `m_next`: for the shadow manager list.
    pub next: Option<usize>,
    /// `m_geometry`: geometry the shadow is derived from (an id of the manager's geometry storage).
    pub geometry: Option<usize>,
    /// `m_robj`: render object belonging to model casting shadow.
    pub robj: Option<ShadowRenderObj>,
    /// `m_shadowLengthScale`: scale factor used to reduce/clamp shadow length.
    pub shadow_length_scale: f32,
    /// `m_robjExtent`: maximum possible horizontal reach of shadow from center of object.
    pub robj_extent: f32,
    /// `m_extraExtrusionPadding`: amount to extrude shadow beyond where it would normally stop (ground level below object).
    pub extra_extrusion_padding: f32,
    /// `m_shadowVolume[MAX_SHADOW_LIGHTS][MAX_SHADOW_CASTER_MESHES]`.
    pub shadow_volume: Vec<Vec<Option<Geometry>>>,
    /// `m_shadowVolumeVB[..][..]`: vertex buffer slots of the buffer manager.
    pub shadow_volume_vb: Vec<Vec<Option<usize>>>,
    /// `m_shadowVolumeIB[..][..]`: index buffer slots of the buffer manager.
    pub shadow_volume_ib: Vec<Vec<Option<usize>>>,
    /// The data of a static volume in its slots (the buffer manager draft owns no device data).
    pub static_vertices: Vec<Vec<Vec<ShadowVolumeVertex>>>,
    /// The indices of a static volume.
    pub static_indices: Vec<Vec<Vec<u16>>>,
    /// `m_shadowVolumeCount[MAX_SHADOW_CASTER_MESHES]`: how many shadows are valid in `m_shadowVolume`.
    pub shadow_volume_count: Vec<i32>,
    /// `m_lightPosHistory[..][..]`.
    pub light_pos_history: Vec<Vec<Vector3>>,
    /// `m_objectXformHistory[..][..]` (a `Matrix4x4` of a `Matrix3D`: the last row is 0,0,0,1).
    pub object_xform_history: Vec<Vec<Matrix3D>>,
    /// `m_silhouetteIndex[MAX_SHADOW_CASTER_MESHES]`: silhouette vertex index list, edges occur as disjoint pairs.
    pub silhouette_index: Vec<Vec<i16>>,
    /// `m_numSilhouetteIndices[..]`: total number of edge indices in the index array (a multiple of two).
    pub num_silhouette_indices: Vec<i16>,
    /// `m_maxSilhouetteEntries[..]`: how big the silhouette index can hold max.
    pub max_silhouette_entries: Vec<i16>,
    /// `m_numIndicesPerMesh[..]`: silhouette indices from each mesh.
    pub num_indices_per_mesh: Vec<i32>,
}

impl Default for W3DVolumetricShadow {
    fn default() -> Self {
        Self::new()
    }
}

/// `cosAngleToCare`: `cos((0.2 * PI) / 180.0)` (double, stored as `Real`).
fn cos_angle_to_care() -> f32 {
    super::double_math::c_cos(((0.2f64 * PI as f64) / 180.0) as f32)
}

impl W3DVolumetricShadow {
    /// `W3DVolumetricShadow::W3DVolumetricShadow` (W3DVolumetricShadow.cpp:1698).
    pub fn new() -> Self {
        let mut base = Shadow::new();
        base.is_enabled = true;
        base.is_invisible_enabled = false;
        let m = MAX_SHADOW_CASTER_MESHES;
        let l = MAX_SHADOW_LIGHTS;
        W3DVolumetricShadow {
            base,
            next: None,
            geometry: None,
            robj: None,
            shadow_length_scale: 0.0,
            robj_extent: 0.0,
            extra_extrusion_padding: 0.0,
            shadow_volume: vec![vec![None; m]; l],
            shadow_volume_vb: vec![vec![None; m]; l],
            shadow_volume_ib: vec![vec![None; m]; l],
            static_vertices: vec![vec![Vec::new(); m]; l],
            static_indices: vec![vec![Vec::new(); m]; l],
            shadow_volume_count: vec![0; m],
            light_pos_history: vec![vec![Vector3::new(0.0, 0.0, 0.0); m]; l],
            object_xform_history: vec![vec![Matrix3D::identity(); m]; l],
            silhouette_index: vec![Vec::new(); m],
            num_silhouette_indices: vec![0; m],
            max_silhouette_entries: vec![0; m],
            num_indices_per_mesh: vec![0; m],
        }
    }

    /// `W3DVolumetricShadow::~W3DVolumetricShadow` (W3DVolumetricShadow.cpp:1737): frees the silhouettes and returns the
    /// volumes' buffer slots to the buffer manager.
    pub fn destroy(&mut self, buffers: &mut W3DBufferManager) {
        // we must free any silhouette data allocated
        for j in 0..MAX_SHADOW_CASTER_MESHES {
            self.delete_silhouette(j as i32);
        }

        // free any shadow volume data
        for i in 0..MAX_SHADOW_LIGHTS {
            for j in 0..MAX_SHADOW_CASTER_MESHES {
                self.shadow_volume[i][j] = None;
                if let Some(s) = self.shadow_volume_vb[i][j].take() {
                    buffers.release_slot_vb(s);
                }
                if let Some(s) = self.shadow_volume_ib[i][j].take() {
                    buffers.release_slot_ib(s);
                }
            }
        }

        self.geometry = None;
        self.robj = None;
    }

    /// `W3DVolumetricShadow::setShadowLengthScale`.
    pub fn set_shadow_length_scale(&mut self, value: f32) {
        self.shadow_length_scale = value;
    }

    /// `W3DVolumetricShadow::setOptimalExtrusionPadding`.
    pub fn set_optimal_extrusion_padding(&mut self, value: f32) {
        self.extra_extrusion_padding = value;
    }

    /// `W3DVolumetricShadow::setRenderObjExtent`.
    pub fn set_render_obj_extent(&mut self, extent: f32) {
        self.robj_extent = extent;
    }

    /// `W3DVolumetricShadow::setRenderObject`.
    pub fn set_render_object(&mut self, robj: ShadowRenderObj) {
        // assert(m_robj==NULL)
        self.robj = Some(robj);
    }

    /// `W3DVolumetricShadow::setLightPosHistory`: updates the last position of light.
    pub fn set_light_pos_history(&mut self, light_index: usize, mesh_index: usize, pos: Vector3) {
        self.light_pos_history[light_index][mesh_index] = pos;
    }

    /// `W3DVolumetricShadow::allocateSilhouette` (W3DVolumetricShadow.cpp:3328).
    pub fn allocate_silhouette(&mut self, mesh_index: i32, num_vertices: i32) -> bool {
        let num_entries = num_vertices * 5; // @todo: HACK, HACK... Should be 2!
        let mi = mesh_index as usize;
        // assert( m_silhouetteIndex[meshIndex] == NULL && m_numSilhouetteIndices[meshIndex] == 0 && numEntries > 0 );
        self.silhouette_index[mi] = vec![0i16; num_entries.max(0) as usize];
        self.num_silhouette_indices[mi] = 0;
        self.max_silhouette_entries[mi] = num_entries as i16;
        true
    }

    /// `W3DVolumetricShadow::deleteSilhouette` (W3DVolumetricShadow.cpp:3361).
    pub fn delete_silhouette(&mut self, mesh_index: i32) {
        let mi = mesh_index as usize;
        self.silhouette_index[mi] = Vec::new();
        self.num_silhouette_indices[mi] = 0;
    }

    /// `W3DVolumetricShadow::resetSilhouette` (W3DVolumetricShadow.cpp:3375).
    pub fn reset_silhouette(&mut self, mesh_index: i32) {
        self.num_silhouette_indices[mesh_index as usize] = 0;
    }

    /// `W3DVolumetricShadow::SetGeometry` (W3DVolumetricShadow.cpp:1765): our geometry has changed, we need to allocate
    /// enough memory for the silhouette data. If silhouette data is present it must be reallocated to accommodate the new
    /// size if smaller.
    pub fn set_geometry(&mut self, geometry: Option<usize>, geoms: &W3DShadowGeometryManager) {
        let mut num_prev_vertices: i16 = 0;
        let mut num_new_vertices: i16 = 0;

        for i in 0..MAX_SHADOW_CASTER_MESHES {
            if let Some(g) = self.geometry {
                num_prev_vertices = geoms.storage[g].mesh_list[i].get_num_vertex() as i16;
            }

            // now many vertices does our new geometry have
            if let Some(g) = geometry {
                num_new_vertices = geoms.storage[g].mesh_list[i].get_num_vertex() as i16;
            }

            // if our new geometry requires more memory allocate it, if it requires less we'll leave it around for future
            // switches in geometry
            if num_new_vertices > num_prev_vertices {
                self.delete_silhouette(i as i32);
                if !self.allocate_silhouette(i as i32, num_new_vertices as i32) {
                    return;
                }
            }
        }

        // assign the new geometry, possible over an old geometry
        self.geometry = geometry;
    }

    /// `W3DVolumetricShadow::allocateShadowVolume` (W3DVolumetricShadow.cpp:3170).
    pub fn allocate_shadow_volume(&mut self, volume_index: i32, mesh_index: i32) -> bool {
        if volume_index < 0 || volume_index >= MAX_SHADOW_LIGHTS as i32 {
            return false;
        }
        let (vi, mi) = (volume_index as usize, mesh_index as usize);

        if self.shadow_volume[vi][mi].is_none() {
            self.shadow_volume[vi][mi] = Some(Geometry::new()); // create the new geometry
            self.shadow_volume_count[mi] += 1;
        }

        // for dynamic volumes the number of polygons is the maximum silhouette entries; vertices twice that
        let num_polygons = self.max_silhouette_entries[mi] as i32;
        let num_vertices = self.max_silhouette_entries[mi] as i32 * 2;
        let sv = self.shadow_volume[vi][mi].as_mut().unwrap();
        if (sv.flags & SHADOW_DYNAMIC) != 0 && !sv.create(num_vertices, num_polygons) {
            self.shadow_volume[vi][mi] = None;
            return false;
        }
        true // success
    }

    /// `W3DVolumetricShadow::deleteShadowVolume` (W3DVolumetricShadow.cpp:3255): delete all volume data.
    pub fn delete_shadow_volume(&mut self, volume_index: i32) {
        if volume_index < 0 || volume_index >= MAX_SHADOW_LIGHTS as i32 {
            return;
        }
        for mesh_index in 0..MAX_SHADOW_CASTER_MESHES {
            if self.shadow_volume[volume_index as usize][mesh_index].is_some() {
                self.shadow_volume[volume_index as usize][mesh_index] = None;
                self.shadow_volume_count[mesh_index] -= 1;
            }
        }
    }

    /// `W3DVolumetricShadow::resetShadowVolume` (W3DVolumetricShadow.cpp:3291): reset shadow volume, returning its slots.
    pub fn reset_shadow_volume(&mut self, volume_index: i32, mesh_index: i32, buffers: &mut W3DBufferManager) {
        if volume_index < 0 || volume_index >= MAX_SHADOW_LIGHTS as i32 {
            return;
        }
        let (vi, mi) = (volume_index as usize, mesh_index as usize);
        if self.shadow_volume[vi][mi].is_some() {
            if let Some(s) = self.shadow_volume_vb[vi][mi].take() {
                buffers.release_slot_vb(s);
            }
            if let Some(s) = self.shadow_volume_ib[vi][mi].take() {
                buffers.release_slot_ib(s);
            }
            self.static_vertices[vi][mi].clear();
            self.static_indices[vi][mi].clear();
            self.shadow_volume[vi][mi].as_mut().unwrap().release();
        }
    }

    /// `W3DVolumetricShadow::addSilhouetteIndices` (W3DVolumetricShadow.cpp:2415).
    fn add_silhouette_indices(&mut self, mesh_index: usize, edge_start: i16, edge_end: i16) {
        // assert( m_numSilhouetteIndices[meshIndex] < m_maxSilhouetteEntries[meshIndex] );
        let n = self.num_silhouette_indices[mesh_index] as usize;
        self.silhouette_index[mesh_index][n] = edge_start;
        self.num_silhouette_indices[mesh_index] += 1;
        let n = self.num_silhouette_indices[mesh_index] as usize;
        self.silhouette_index[mesh_index][n] = edge_end;
        self.num_silhouette_indices[mesh_index] += 1;
    }

    /// `W3DVolumetricShadow::addSilhouetteEdge` (W3DVolumetricShadow.cpp:2262): it has been determined that the polygon
    /// neighbor in the "neighborIndex" of "visible" needs to be added to the silhouette. We add those two vertex indices
    /// in the order they were specified in "visible" to assure that the constructed edge is in counter clockwise order.
    fn add_silhouette_edge(&mut self, mesh_index: usize, geom_mesh: &W3DShadowGeometryMesh, visible: &PolyNeighbor, hidden: &PolyNeighbor) {
        let mut neighbor_index = 0usize;

        for i in 0..MAX_POLYGON_NEIGHBORS {
            if visible.neighbor[i].neighbor_index == hidden.my_index {
                neighbor_index = i;
                break; // exit for
            }
        }

        let visible_index_list = geom_mesh.get_polygon_index(visible.my_index as i32);
        let nei = visible.neighbor[neighbor_index].neighbor_edge_index;

        // the vertex of the visible polygon that is not on the shared edge starts the edge going the other way
        let (edge_start, edge_end);
        if visible_index_list[0] != nei[0] && visible_index_list[0] != nei[1] {
            edge_start = visible_index_list[1];
            edge_end = visible_index_list[2];
        } else if visible_index_list[1] != nei[0] && visible_index_list[1] != nei[1] {
            edge_start = visible_index_list[2];
            edge_end = visible_index_list[0];
        } else {
            edge_start = visible_index_list[0];
            edge_end = visible_index_list[1];
        }

        self.add_silhouette_indices(mesh_index, edge_start, edge_end);
    }

    /// `W3DVolumetricShadow::addNeighborlessEdges` (W3DVolumetricShadow.cpp:2348): the polygon "us" is visible and has at
    /// least one edge with no neighbor, add those edges to the silhouette.
    fn add_neighborless_edges(&mut self, mesh_index: usize, geom_mesh: &W3DShadowGeometryMesh, us: &PolyNeighbor) {
        let vertex_index_list = geom_mesh.get_polygon_index(us.my_index as i32);

        // check each of the three edges of this polygon
        for i in 0..3usize {
            let edge_start = vertex_index_list[i];
            let edge_end = if i == 2 {
                vertex_index_list[0] // wraps to begging of list
            } else {
                vertex_index_list[i + 1]
            };

            // see if this edge is shared with a neighbor
            let mut add_edge = true;
            for j in 0..MAX_POLYGON_NEIGHBORS {
                if us.neighbor[j].neighbor_index != NO_NEIGHBOR {
                    let e = us.neighbor[j].neighbor_edge_index;
                    if (e[0] == edge_start && e[1] == edge_end) || (e[1] == edge_start && e[0] == edge_end) {
                        add_edge = false;
                        break; // exit for j, no need to search on
                    }
                }
            }

            if add_edge {
                self.add_silhouette_indices(mesh_index, edge_start, edge_end);
            }
        }
    }

    /// `W3DVolumetricShadow::buildSilhouette` (W3DVolumetricShadow.cpp:2433): build the silhouette of the mesh as seen from
    /// the light position (object space).
    pub fn build_silhouette(&mut self, mesh_index: i32, light_pos_object: &Vector3, geoms: &mut W3DShadowGeometryManager) {
        let mi = mesh_index as usize;
        let geom_id = self.geometry.unwrap();
        let geom_mesh = &mut geoms.storage[geom_id].mesh_list[mi];

        let mesh_edge_start = self.num_silhouette_indices[mi] as i32; // index to first edge contributed by specific mesh

        let num_polys = geom_mesh.get_num_polygon();

        // build neighbors/normals if needed (GetPolyNeighbor builds them on first use)
        if geom_mesh.poly_neighbors.is_empty() {
            geom_mesh.build_polygon_neighbors();
        }

        // find which polygons are visible from the light
        for i in 0..num_polys {
            let idx = geom_mesh.get_poly_neighbor(i).unwrap();
            geom_mesh.poly_neighbors[idx].status = 0;

            let normal = geom_mesh.get_polygon_normal(i);
            let poly = geom_mesh.get_polygon_index(i);

            // since our light source could be very close to the object and that would change the shadow we are going to
            // say that the light vector is from the light position to one of the vertices in the polygon
            let vertex = geom_mesh.get_vertex(poly[0] as i32);
            let light_vector = vertex - *light_pos_object;

            // dot the light vector with the normal of the polygon to see if the poly is visible from this location
            if Vector3::dot_product(&light_vector, &normal) < 0.0 {
                geom_mesh.poly_neighbors[idx].status |= POLY_VISIBLE;
            }
        }

        // now go through all the polygons again and add silhouette edges
        for i in 0..num_polys {
            let idx = geom_mesh.get_poly_neighbor(i).unwrap();
            let mut visible_neighborless = false;

            for j in 0..MAX_POLYGON_NEIGHBORS {
                let poly_neighbor = geom_mesh.poly_neighbors[idx];
                let mut other_neighbor: Option<PolyNeighbor> = None;
                if poly_neighbor.neighbor[j].neighbor_index != NO_NEIGHBOR {
                    let oidx = geom_mesh.get_poly_neighbor(poly_neighbor.neighbor[j].neighbor_index as i32).unwrap();
                    let o = geom_mesh.poly_neighbors[oidx];
                    other_neighbor = Some(o);

                    // if the other neighbor has been processed, skip it
                    if (o.status & POLY_PROCESSED) != 0 {
                        continue; // for j
                    }
                }

                if (poly_neighbor.status & POLY_VISIBLE) != 0 {
                    match other_neighbor {
                        None => {
                            visible_neighborless = true;
                        }
                        Some(o) => {
                            if (o.status & POLY_VISIBLE) == 0 {
                                let gm = geom_mesh.clone_for_edge();
                                self.add_silhouette_edge(mi, &gm, &poly_neighbor, &o);
                            }
                        }
                    }
                } else if let Some(o) = other_neighbor {
                    if (o.status & POLY_VISIBLE) != 0 {
                        let gm = geom_mesh.clone_for_edge();
                        self.add_silhouette_edge(mi, &gm, &o, &poly_neighbor);
                    }
                }
            }

            // if this polygon was visible and had no neighbor on some edge, add those edges
            if visible_neighborless {
                let pn = geom_mesh.poly_neighbors[idx];
                let gm = geom_mesh.clone_for_edge();
                self.add_neighborless_edges(mi, &gm, &pn);
            }

            // we are now done with this polygon
            geom_mesh.poly_neighbors[idx].status |= POLY_PROCESSED;
        }

        self.num_indices_per_mesh[mi] = self.num_silhouette_indices[mi] as i32 - mesh_edge_start;
    }

    /// Builds one strip's edge-ordering step both `construct*` functions share: finds, after edge `i`, the next edge that
    /// starts where this one ends and swaps it into place (`Int` swap of a pair of `Short`s). Returns the index `k` of the
    /// found edge, `indices_per_mesh` when none.
    fn order_next_edge(silhouette_indices: &mut [i16], i: i32, indices_per_mesh: i32) -> i32 {
        let current_edge_end = silhouette_indices[(i + 1) as usize];
        let mut k = i + 2;
        while k < indices_per_mesh {
            if silhouette_indices[k as usize] == current_edge_end {
                // swap the two edges
                let (a, b) = ((i + 2) as usize, k as usize);
                silhouette_indices.swap(a, b);
                silhouette_indices.swap(a + 1, b + 1);
                break;
            }
            k += 2;
        }
        k
    }

    /// `W3DVolumetricShadow::constructVolume` (W3DVolumetricShadow.cpp:2612): construct the dynamic shadow volume
    /// (in memory) from the silhouette.
    pub fn construct_volume(&mut self, light_pos_object: &Vector3, shadow_extrude_distance: f32, volume_index: i32, mesh_index: i32, geoms: &W3DShadowGeometryManager) {
        if volume_index < 0 || volume_index >= MAX_SHADOW_LIGHTS as i32 {
            return;
        }
        let (vi, mi) = (volume_index as usize, mesh_index as usize);
        if self.shadow_volume[vi][mi].is_none() {
            return;
        }

        let mut vertex_count: i32 = 0;
        let mut polygon_count: i32 = 0;

        let indices_per_mesh = self.num_indices_per_mesh[mi];
        if indices_per_mesh == 0 {
            return; // nothing to draw
        }

        let geom_mesh = &geoms.storage[self.geometry.unwrap()].mesh_list[mi];
        let shadow_volume = self.shadow_volume[vi][mi].as_mut().unwrap();
        shadow_volume.num_active_polygon = 0;
        shadow_volume.num_active_vertex = 0;

        let silhouette_indices = &mut self.silhouette_index[mi];
        let mut strip_start_index = silhouette_indices[0];
        let mut strip_start_vertex: i32 = 0;

        // the first edge's start vertex and its extrusion
        let ev2 = geom_mesh.get_vertex(silhouette_indices[0] as i32);
        let mut extrude2 = (ev2 - *light_pos_object) * shadow_extrude_distance + ev2;
        // (extrude2 = ev2 - light; extrude2 *= dist; extrude2 += ev2: same operations in the same order)
        shadow_volume.set_vertex(vertex_count, &ev2);
        shadow_volume.set_vertex(vertex_count + 1, &extrude2);
        vertex_count = 2;

        let mut last_edge_vertex2_index: i32 = 0;
        let mut last_extrude2_index: i32 = 1;

        let mut i = 0;
        while i < indices_per_mesh {
            let current_edge_end = silhouette_indices[(i + 1) as usize];
            let k = Self::order_next_edge(silhouette_indices, i, indices_per_mesh);

            if k >= indices_per_mesh {
                // reached end of strip. Insert final edge.
                if current_edge_end == strip_start_index {
                    // add end of strip that wraps around to start (forming closed cylinder/shape)
                    shadow_volume.set_polygon_index(polygon_count, [last_edge_vertex2_index as i16, last_extrude2_index as i16, strip_start_vertex as i16]);
                    shadow_volume.set_polygon_index(polygon_count + 1, [strip_start_vertex as i16, last_extrude2_index as i16, (strip_start_vertex + 1) as i16]);
                } else {
                    // add end of strip. Finishes the last 2 polygons.
                    let ev = geom_mesh.get_vertex(current_edge_end as i32);
                    shadow_volume.set_vertex(vertex_count, &ev);

                    shadow_volume.set_polygon_index(polygon_count, [last_edge_vertex2_index as i16, last_extrude2_index as i16, vertex_count as i16]);
                    extrude2 = (ev - *light_pos_object) * shadow_extrude_distance + ev;
                    shadow_volume.set_vertex(vertex_count + 1, &extrude2);

                    shadow_volume.set_polygon_index(polygon_count + 1, [vertex_count as i16, last_extrude2_index as i16, (vertex_count + 1) as i16]);
                    last_edge_vertex2_index = vertex_count;
                    last_extrude2_index = vertex_count + 1;
                    vertex_count += 2;
                }

                if (i + 2) >= indices_per_mesh {
                    // finished with all silhouette edges
                    polygon_count += 2;
                    break; // reached end of all edges
                }

                // start a new strip at the next edge
                let ev = geom_mesh.get_vertex(silhouette_indices[(i + 2) as usize] as i32);
                extrude2 = (ev - *light_pos_object) * shadow_extrude_distance + ev;
                last_edge_vertex2_index = vertex_count;
                last_extrude2_index = vertex_count + 1;
                strip_start_index = silhouette_indices[(i + 2) as usize];
                strip_start_vertex = last_edge_vertex2_index;
                shadow_volume.set_vertex(last_edge_vertex2_index, &ev);
                shadow_volume.set_vertex(last_extrude2_index, &extrude2);
                vertex_count += 2;
                polygon_count += 2;
                i += 2;
                continue;
            } else {
                // continue existing strip by adding extra vertex and extrusion
                let ev = geom_mesh.get_vertex(current_edge_end as i32);
                shadow_volume.set_vertex(vertex_count, &ev);

                shadow_volume.set_polygon_index(polygon_count, [last_edge_vertex2_index as i16, last_extrude2_index as i16, vertex_count as i16]);
                extrude2 = (ev - *light_pos_object) * shadow_extrude_distance + ev;
                shadow_volume.set_vertex(vertex_count + 1, &extrude2);

                shadow_volume.set_polygon_index(polygon_count + 1, [vertex_count as i16, last_extrude2_index as i16, (vertex_count + 1) as i16]);
                last_edge_vertex2_index = vertex_count;
                last_extrude2_index = vertex_count + 1;
                vertex_count += 2;
                polygon_count += 2;
            }
            i += 2;
        }

        shadow_volume.num_active_polygon = polygon_count;
        shadow_volume.num_active_vertex = vertex_count;
    }

    /// `W3DVolumetricShadow::constructVolumeVB` (W3DVolumetricShadow.cpp:2849): construct the static shadow volume into
    /// vertex/index buffer slots of the buffer manager. A first pass counts the vertices and polygons, the second fills.
    pub fn construct_volume_vb(
        &mut self,
        light_pos_object: &Vector3,
        shadow_extrude_distance: f32,
        volume_index: i32,
        mesh_index: i32,
        geoms: &W3DShadowGeometryManager,
        buffers: &mut W3DBufferManager,
    ) {
        if volume_index < 0 || volume_index >= MAX_SHADOW_LIGHTS as i32 {
            return;
        }
        let (vi, mi) = (volume_index as usize, mesh_index as usize);
        if self.shadow_volume[vi][mi].is_none() {
            return;
        }

        let mut vertex_count: i32;
        let mut polygon_count: i32;
        let indices_per_mesh = self.num_indices_per_mesh[mi];
        {
            // initial pass to determine vertex/polygon counts.
            vertex_count = 0;
            polygon_count = 0;
            if indices_per_mesh == 0 {
                return; // nothing to draw
            }

            let silhouette_indices = &mut self.silhouette_index[mi];
            let mut strip_start_index = silhouette_indices[0];
            vertex_count = 2;

            let mut i = 0;
            while i < indices_per_mesh {
                let current_edge_end = silhouette_indices[(i + 1) as usize];
                let k = Self::order_next_edge(silhouette_indices, i, indices_per_mesh);

                if k >= indices_per_mesh {
                    // reached end of strip. Insert final edge.
                    if current_edge_end == strip_start_index {
                        // add end of strip that wraps around to start (forming closed cylinder/shape)
                    } else {
                        // add end of strip. Finishes the last 2 polygons.
                        vertex_count += 2;
                    }

                    if (i + 2) >= indices_per_mesh {
                        // finished with all silhouette edges
                        polygon_count += 2;
                        break; // reached end of all edges
                    }

                    strip_start_index = silhouette_indices[(i + 2) as usize];
                    vertex_count += 2;
                    polygon_count += 2;
                    i += 2;
                    continue;
                } else {
                    // continue existing strip by adding extra vertex and extrusion
                    vertex_count += 2;
                    polygon_count += 2;
                }
                i += 2;
            }
        }

        // DEBUG_ASSERTCRASH(m_shadowVolumeVB[volumeIndex][meshIndex] == NULL, ("Updating Existing Static Vertex Buffer Shadow"));
        let vb_slot = buffers.get_slot_vb(VbmFvfTypes::Xyz, vertex_count);
        self.shadow_volume_vb[vi][mi] = vb_slot;
        // DEBUG_ASSERTCRASH(m_shadowVolume[volumeIndex][meshIndex]->GetNumPolygon() == 0, ("Updating Existing Static Shadow Volume"));
        let ib_slot = buffers.get_slot_ib(polygon_count * 3);
        self.shadow_volume_ib[vi][mi] = ib_slot;

        if ib_slot.is_none() || vb_slot.is_none() {
            // could not allocate storage to hold buffers
            if let Some(s) = ib_slot {
                buffers.release_slot_ib(s);
            }
            if let Some(s) = vb_slot {
                buffers.release_slot_vb(s);
            }
            self.shadow_volume_ib[vi][mi] = None;
            self.shadow_volume_vb[vi][mi] = None;
            return;
        }

        let geom_mesh = &geoms.storage[self.geometry.unwrap()].mesh_list[mi];

        let mut vb: Vec<ShadowVolumeVertex> = Vec::with_capacity(vertex_count as usize);
        let mut ib: Vec<u16> = Vec::with_capacity((polygon_count * 3) as usize);
        let to_v = |v: &Vector3| ShadowVolumeVertex { x: v.x, y: v.y, z: v.z };

        {
            let shadow_volume = self.shadow_volume[vi][mi].as_mut().unwrap();
            shadow_volume.num_active_polygon = polygon_count;
            shadow_volume.num_active_vertex = vertex_count;
        }

        let silhouette_indices = &self.silhouette_index[mi];
        let mut strip_start_index = silhouette_indices[0];
        let mut strip_start_vertex: i32 = 0;

        let ev = geom_mesh.get_vertex(silhouette_indices[0] as i32);
        let mut extrude2 = (ev - *light_pos_object) * shadow_extrude_distance + ev;
        vb.push(to_v(&ev));
        vb.push(to_v(&extrude2));

        vertex_count = 2;
        polygon_count = 0;

        let mut last_edge_vertex2_index: i32 = 0;
        let mut last_extrude2_index: i32 = 1;

        // the quad's two triangles: ib[0] = lastEdgeVertex2, ib[4]=ib[1] = lastExtrude2, ib[3]=ib[2] = edgeVertex2,
        // ib[5] = extrude2
        let push_quad = |ib: &mut Vec<u16>, last_edge: i32, last_extrude: i32, edge_vertex2: i32, extrude: i32| {
            ib.push(last_edge as u16);
            ib.push(last_extrude as u16);
            ib.push(edge_vertex2 as u16);
            ib.push(edge_vertex2 as u16);
            ib.push(last_extrude as u16);
            ib.push(extrude as u16);
        };

        let mut i = 0;
        while i < indices_per_mesh {
            let current_edge_end = silhouette_indices[(i + 1) as usize];
            // (the edges were ordered by the counting pass above)
            if ((i + 2) >= indices_per_mesh) || silhouette_indices[(i + 2) as usize] != current_edge_end {
                // reached end of strip. Insert final edge.
                if current_edge_end == strip_start_index {
                    // add end of strip that wraps around to start (forming closed cylinder/shape)
                    push_quad(&mut ib, last_edge_vertex2_index, last_extrude2_index, strip_start_vertex, strip_start_vertex + 1);
                } else {
                    // add end of strip. Finishes the last 2 polygons.
                    let ev = geom_mesh.get_vertex(current_edge_end as i32);
                    vb.push(to_v(&ev));

                    push_quad(&mut ib, last_edge_vertex2_index, last_extrude2_index, vertex_count, vertex_count + 1);
                    extrude2 = (ev - *light_pos_object) * shadow_extrude_distance + ev;
                    vb.push(to_v(&extrude2));
                    last_edge_vertex2_index = vertex_count;
                    last_extrude2_index = vertex_count + 1;
                    vertex_count += 2;
                }

                if (i + 2) >= indices_per_mesh {
                    // finished with all silhouette edges
                    polygon_count += 2;
                    break; // reached end of all edges
                }

                let evb = geom_mesh.get_vertex(silhouette_indices[(i + 2) as usize] as i32);
                extrude2 = (evb - *light_pos_object) * shadow_extrude_distance + evb;
                last_edge_vertex2_index = vertex_count;
                last_extrude2_index = vertex_count + 1;
                strip_start_index = silhouette_indices[(i + 2) as usize];
                strip_start_vertex = last_edge_vertex2_index;
                vb.push(to_v(&evb));
                vb.push(to_v(&extrude2));
                vertex_count += 2;
                polygon_count += 2;
                i += 2;
                continue;
            } else {
                // continue existing strip by adding extra vertex and extrusion
                let ev = geom_mesh.get_vertex(current_edge_end as i32);
                vb.push(to_v(&ev));

                push_quad(&mut ib, last_edge_vertex2_index, last_extrude2_index, vertex_count, vertex_count + 1);
                extrude2 = (ev - *light_pos_object) * shadow_extrude_distance + ev;
                vb.push(to_v(&extrude2));
                last_edge_vertex2_index = vertex_count;
                last_extrude2_index = vertex_count + 1;
                vertex_count += 2;
                polygon_count += 2;
            }
            i += 2;
        }
        let _ = polygon_count;

        self.static_vertices[vi][mi] = vb;
        self.static_indices[vi][mi] = ib;
    }

    /// `W3DVolumetricShadow::updateOptimalExtrusionPadding` (W3DVolumetricShadow.cpp:1188): use raycasting to figure out a
    /// shadow extrusion length that guarantees that the highest point of the object is extruded long enough to hit some
    /// ground. This is a very slow operation so only do once for static non-moving objects. `light_pos_world0` is
    /// `TheW3DShadowManager->getLightPosWorld(0)`.
    pub fn update_optimal_extrusion_padding(&mut self, light_pos_world0: &Vector3, env: &impl VolumetricShadowEnv) {
        let robj = match self.robj.clone() {
            Some(r) => r,
            None => return,
        };

        // get the light
        let mut light_pos_world = *light_pos_world0;

        // check if object has a limit/clamp on shadow length and adjust light position of necessary.
        if self.shadow_length_scale != 0.0 {
            // Find light's distance from origin in xy plane
            let light_xy_distance = wwmath::sqrt(light_pos_world.x * light_pos_world.x + light_pos_world.y * light_pos_world.y);
            let new_z = light_xy_distance * self.shadow_length_scale;

            if new_z > light_pos_world.z {
                // clamped z component is higher than actual light position allows so adjust it.
                light_pos_world.z = new_z;
            }
        }

        // find maximum shadow length which will not cause any corners of the object's bounding box to cast shadows that
        // drop significantly below the object's base.
        let obj_pos = robj.position;
        let mut last_valid_terrain_point = obj_pos;
        let mut base_ground_height = obj_pos.z;
        let bx = robj.bounding_box;
        let mut corners = [Vector3::default(); 4];

        // Get vertices of top of bounding box since they will generate the longest shadow
        corners[0] = bx.center + bx.extent; // top right corner
        corners[1] = corners[0];
        corners[1].x -= 2.0 * bx.extent.x; // top left corner
        corners[2] = corners[1];
        corners[2].y -= 2.0 * bx.extent.y; // bottom left corner
        corners[3] = corners[2];
        corners[3].x += 2.0 * bx.extent.x; // bottom right corner

        // find the corner that causes the longest shadow projection and clamp light position to make sure it falls on
        // even ground about the same height as the object's base.
        for i in 0..4usize {
            // Cast ray from top volume corners onto ground plane
            let mut light_ray = corners[i] - light_pos_world; // vector light to corner
            light_ray.normalize();

            // find out where this ray intersects terrain.
            let hit = env.cast_ray_terrain(&corners[i], &(corners[i] + light_ray * MAX_EXTRUSION_LENGTH));
            if let Some((contact, start_bad)) = hit {
                if !start_bad {
                    // Found intersection point where shadow has its maximum length. Do a quick search to see if terrain
                    // falls significantly below the height of the object anywhere between the base and the intersection
                    // point. If so, we either need to extend shadow extrusion or make the light angle more vertical.
                    let mut shadow_ray = contact - corners[i]; // vector from object corner to terrain intersection.
                    shadow_ray.z = 0.0; // remove z-component since we'll be sampling along the xy plane.

                    // walk along the shadow/light direction vector looking for large dips - indicating object is on hill
                    // or cliff.
                    let len = shadow_ray.length();
                    let num_steps = real_to_int_ceil(len / SHADOW_SAMPLING_INTERVAL);
                    let step_size = 1.0 / num_steps as f32;

                    let mut t = step_size;
                    for j in 0..num_steps {
                        let mut terrain_point = corners[i] + shadow_ray * t;
                        terrain_point.z = 0.0; // ignore height

                        let terrain_height = env.height_map_height(terrain_point.x, terrain_point.y);
                        if terrain_height < (obj_pos.z - MAX_SHADOW_EXTRUSION_UNDER_OBJECT_BEFORE_CLAMP) {
                            // check if terrain dips more than 10 units under object.
                            if j == 0 {
                                // this is the initial point so object must be right on the edge of a cliff.
                                base_ground_height = terrain_height; // force extrusion all the way down cliff.
                                let tan_angle = super::double_math::c_tan(OVERHANGING_OBJECT_CLAMP_ANGLE); // clamp to about 89 degrees or close to vertical lightpos.
                                self.set_shadow_length_scale(tan_angle); // update the clamp angle to shorted shadow enough so this corner on flat ground.
                                break;
                            }

                            // Find ray from last valid terrain contact point to object box corner.
                            let clamp_ray = corners[i] - last_valid_terrain_point;
                            let mut clamp_angle = super::double_math::c_asin(clamp_ray.z / clamp_ray.length());
                            if clamp_angle >= (PI / 2.0) || clamp_angle <= 0.0 {
                                clamp_angle = OVERHANGING_OBJECT_CLAMP_ANGLE; // clamp to about 89 degrees or close to vertical lightpos.
                            }
                            let tan_angle = super::double_math::c_tan(clamp_angle);
                            if tan_angle > self.shadow_length_scale {
                                self.set_shadow_length_scale(tan_angle); // update the clamp angle to shorted shadow enough so this corner on flat ground.
                            }
                            break;
                        }

                        if terrain_height < base_ground_height {
                            base_ground_height = terrain_height; // point was below object but within safety margin so record it's position.
                            last_valid_terrain_point = terrain_point;
                            last_valid_terrain_point.z = base_ground_height;
                        }

                        t += step_size;
                    }
                }
            }
        }

        self.extra_extrusion_padding = obj_pos.z - base_ground_height + SHADOW_EXTRUSION_BUFFER;

        // DEBUG_ASSERTCRASH(m_extraExtrusionPadding <= (255.0f*MAP_HEIGHT_SCALE), ...)
    }
}

impl W3DShadowGeometryMesh {
    /// The read-only part of the mesh `addSilhouetteEdge`/`addNeighborlessEdges` use (`GetPolygonIndex`): a copy of the
    /// polygon and parent vertex tables, so the silhouette arrays of the shadow can be written while the mesh's neighbor
    /// statuses are borrowed mutably.
    fn clone_for_edge(&self) -> W3DShadowGeometryMesh {
        W3DShadowGeometryMesh { polygons: self.polygons.clone(), parent_verts: self.parent_verts.clone(), ..W3DShadowGeometryMesh::default() }
    }
}

/// `class W3DVolumetricShadowManager`.
pub struct W3DVolumetricShadowManager {
    /// The shadow slot table (`m_shadowList`/`m_next` link slots).
    pub shadows: Vec<Option<W3DVolumetricShadow>>,
    /// `m_shadowList`.
    pub shadow_list: Option<usize>,
    /// `m_dynamicShadowVolumesToRender`: a LIFO list (the head is the last element).
    pub dynamic_shadow_volumes_to_render: Vec<W3DVolumetricShadowRenderTask>,
    /// `m_W3DShadowGeometryManager`.
    pub w3d_shadow_geometry_manager: W3DShadowGeometryManager,
    /// `TheW3DBufferManager` (created by this manager's constructor).
    pub w3d_buffer_manager: Option<W3DBufferManager>,
    /// The `m_renderTaskList` of each vertex buffer of the buffer manager (by buffer index): LIFO lists.
    pub vb_render_task_lists: Vec<Vec<W3DVolumetricShadowRenderTask>>,
    /// The shared dynamic shadow buffers.
    pub buffers: ShadowVolumeBuffers,
    /// `bcX, bcY, bcZ`: rough bounding box around the visible portion of the terrain, useful for quick culling.
    pub bc: Vector3,
    /// `beX, beY, beZ`.
    pub be: Vector3,
}

impl Default for W3DVolumetricShadowManager {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DVolumetricShadowManager {
    /// `W3DVolumetricShadowManager::W3DVolumetricShadowManager` (W3DVolumetricShadow.cpp:3749).
    pub fn new() -> Self {
        W3DVolumetricShadowManager {
            shadows: Vec::new(),
            shadow_list: None,
            dynamic_shadow_volumes_to_render: Vec::new(),
            w3d_shadow_geometry_manager: W3DShadowGeometryManager::new(),
            w3d_buffer_manager: Some(W3DBufferManager::new()),
            vb_render_task_lists: Vec::new(),
            buffers: ShadowVolumeBuffers::new(),
            bc: Vector3::default(),
            be: Vector3::default(),
        }
    }

    /// `W3DVolumetricShadowManager::~W3DVolumetricShadowManager` (W3DVolumetricShadow.cpp:3762).
    pub fn destroy(&mut self) {
        self.release_resources();
        self.w3d_buffer_manager = None;
        // assert(m_shadowList==NULL);
    }

    /// `W3DVolumetricShadowManager::addDynamicShadowTask` (W3DVolumetricShadow.h): queues up a dynamic shadow caster for
    /// rendering - only used internally by shadow system.
    pub fn add_dynamic_shadow_task(&mut self, task: W3DVolumetricShadowRenderTask) {
        self.dynamic_shadow_volumes_to_render.push(task);
    }

    /// `W3DVolumetricShadowManager::init` (W3DVolumetricShadow.cpp:3833).
    pub fn init(&mut self) -> bool {
        true
    }

    /// `W3DVolumetricShadowManager::reset` (W3DVolumetricShadow.cpp:3841).
    pub fn reset(&mut self) {
        // assert (m_shadowList == NULL);
        self.w3d_shadow_geometry_manager.free_all_geoms();
        if let Some(b) = self.w3d_buffer_manager.as_mut() {
            b.free_all_buffers();
        }
        self.vb_render_task_lists.clear();
    }

    /// `W3DVolumetricShadowManager::ReleaseResources` (W3DVolumetricShadow.cpp:3776): releases all W3D/D3D assets before a
    /// reset.
    pub fn release_resources(&mut self) {
        self.buffers.index_buffer = Vec::new();
        self.buffers.vertex_buffer = Vec::new();

        if let Some(b) = self.w3d_buffer_manager.as_mut() {
            b.release_resources();
            self.invalidate_cached_light_positions(); // vertex buffers need to be refilled.
        }
    }

    /// `W3DVolumetricShadowManager::ReAcquireResources` (W3DVolumetricShadow.cpp:3791): (re)allocates all W3D/D3D assets
    /// after a reset.
    pub fn re_acquire_resources(&mut self) -> bool {
        self.release_resources();

        // CreateIndexBuffer(SHADOW_INDEX_SIZE*sizeof(WORD), WRITEONLY|DYNAMIC, INDEX16, POOL_DEFAULT)
        self.buffers.index_buffer = vec![0u16; self.buffers.shadow_index_size as usize];

        if self.buffers.vertex_buffer.is_empty() {
            // Create vertex buffer
            self.buffers.vertex_buffer = vec![ShadowVolumeVertex::default(); self.buffers.shadow_vertex_size as usize];
        }

        if let Some(b) = self.w3d_buffer_manager.as_mut() {
            if !b.re_acquire_resources() {
                return false;
            }
        }

        true
    }

    /// `W3DVolumetricShadowManager::invalidateCachedLightPositions` (W3DVolumetricShadow.cpp:3725): used to cause a
    /// rebuild of all shadow volumes.
    pub fn invalidate_cached_light_positions(&mut self) {
        if self.shadow_list.is_none() {
            return; // there are no shadows to render.
        }

        let vec = Vector3::new(0.0, 0.0, 0.0);
        let mut cur = self.shadow_list;
        while let Some(slot) = cur {
            let shadow = self.shadows[slot].as_mut().unwrap();
            for i in 0..MAX_SHADOW_LIGHTS {
                for mesh_index in 0..MAX_SHADOW_CASTER_MESHES {
                    shadow.set_light_pos_history(i, mesh_index, vec);
                }
            }
            cur = shadow.next;
        }
    }

    /// `W3DVolumetricShadowManager::addShadow` (W3DVolumetricShadow.cpp:3854): adds shadow caster to rendering system.
    /// `source` is the render object's class and models (read only when its geometry is not cached yet).
    pub fn add_shadow(
        &mut self,
        robj: Option<ShadowRenderObj>,
        source: &ShadowGeomSource,
        shadow_info: Option<&ShadowTypeInfo>,
        draw_is_immobile: Option<bool>,
        env: &impl VolumetricShadowEnv,
    ) -> Option<usize> {
        let robj = robj?;
        if !env.has_stencil() || !env.use_shadow_volumes() {
            return None; // right now we require a stencil buffer
        }

        let name = robj.name.clone();
        if name.is_empty() {
            return None;
        }
        let mut sg = self.w3d_shadow_geometry_manager.get_geom(&name);
        if sg.is_none() {
            // did not find a cached copy of the shadow geometry, create a new one
            self.w3d_shadow_geometry_manager.load_geom(source, &name);
            sg = self.w3d_shadow_geometry_manager.get_geom(&name);
            sg?; // could not create the shadow geometry
        }

        let mut shadow = W3DVolumetricShadow::new();

        shadow.set_render_object(robj.clone());
        shadow.set_geometry(sg, &self.w3d_shadow_geometry_manager);

        // robj->Get_Obj_Space_Bounding_Sphere(sphere)
        shadow.set_render_obj_extent(robj.obj_space_bounding_sphere.radius * MAX_SHADOW_LENGTH_SCALE_FACTOR);

        let mut sun_elevation_angle_tan: f32 = 0.0;
        let size_x = shadow_info.map_or(0.0, |i| i.size_x); // upstream dereferences without a NULL check
        if size_x != 0.0 {
            // need to adjust sun elevation for this model in order to limit shadow length
            sun_elevation_angle_tan = super::double_math::c_tan(size_x / 180.0 * PI);
        }

        shadow.set_shadow_length_scale(sun_elevation_angle_tan);

        // draw is NULL, or not KINDOF_IMMOBILE
        if draw_is_immobile != Some(true) {
            shadow.set_optimal_extrusion_padding(SHADOW_EXTRUSION_BUFFER);
        }

        let slot = self.shadows.len();
        shadow.next = self.shadow_list;
        self.shadows.push(Some(shadow));
        self.shadow_list = Some(slot);

        Some(slot)
    }

    /// `W3DVolumetricShadowManager::removeShadow` (W3DVolumetricShadow.cpp:3912): removes the shadows for this hierarchy
    /// from the shadow manager. No further shadows from this caster will be rendered.
    pub fn remove_shadow(&mut self, slot: usize) {
        let mut prev_shadow: Option<usize> = None;
        let mut next_shadow = self.shadow_list;
        while let Some(ns) = next_shadow {
            if ns == slot {
                let next = self.shadows[slot].as_ref().unwrap().next;
                if let Some(ps) = prev_shadow {
                    self.shadows[ps].as_mut().unwrap().next = next;
                } else {
                    self.shadow_list = next;
                }
                let mut s = self.shadows[slot].take().unwrap();
                if let Some(b) = self.w3d_buffer_manager.as_mut() {
                    s.destroy(b);
                }
                break;
            }
            prev_shadow = Some(ns);
            next_shadow = self.shadows[ns].as_ref().unwrap().next;
        }
    }

    /// `W3DVolumetricShadowManager::removeAllShadows` (W3DVolumetricShadow.cpp:3938): removes all shadows from the shadow
    /// manager. No further shadows will be rendered.
    pub fn remove_all_shadows(&mut self) {
        let mut next_shadow = self.shadow_list;
        self.shadow_list = None;

        while let Some(cur) = next_shadow {
            next_shadow = self.shadows[cur].as_ref().and_then(|s| s.next);
            if let Some(mut s) = self.shadows[cur].take() {
                s.next = None;
                if let Some(b) = self.w3d_buffer_manager.as_mut() {
                    s.destroy(b);
                }
            }
        }
    }

    /// `W3DVolumetricShadow::RenderMeshVolume` (W3DVolumetricShadow.cpp:1375): render a static mesh volume from its buffer
    /// slots.
    fn render_mesh_volume(&self, slot: usize, mesh_index: usize, light_index: usize, mesh_xform: &Matrix3D) -> Option<VolumeDraw> {
        let shadow = self.shadows[slot].as_ref().unwrap();
        let geometry = shadow.shadow_volume[light_index][mesh_index].as_ref().unwrap();

        // get geometry requirements
        let num_verts = geometry.num_active_vertex;
        let num_polys = geometry.num_active_polygon;

        // reject shadows with no data
        if num_verts == 0 || num_polys == 0 {
            return None;
        }

        let vb_slot = shadow.shadow_volume_vb[light_index][mesh_index]?;
        let ib_slot = shadow.shadow_volume_ib[light_index][mesh_index]?;
        let bm = self.w3d_buffer_manager.as_ref().unwrap();
        // SetStreamSource(0, the slot's buffer) when it is not the last active one — device-only
        Some(VolumeDraw {
            world: *mesh_xform,
            base_vertex: bm.vertex_buffer_empty_slots[vb_slot].start,
            start_index: bm.index_buffer_empty_slots[ib_slot].start,
            polys: num_polys,
            verts: num_verts,
            dynamic: false,
            vertices: shadow.static_vertices[light_index][mesh_index].clone(),
            indices: shadow.static_indices[light_index][mesh_index].clone(),
        })
    }

    /// `W3DVolumetricShadow::RenderDynamicMeshVolume` (W3DVolumetricShadow.cpp:1435): render a dynamic mesh volume through
    /// the shared dynamic buffers.
    fn render_dynamic_mesh_volume(&mut self, slot: usize, mesh_index: usize, light_index: usize, mesh_xform: &Matrix3D) -> Option<VolumeDraw> {
        let (num_verts, num_polys, vertices, indices) = {
            let shadow = self.shadows[slot].as_ref().unwrap();
            let geometry = shadow.shadow_volume[light_index][mesh_index].as_ref().unwrap();
            let nv = geometry.num_active_vertex;
            let np = geometry.num_active_polygon;
            if nv == 0 || np == 0 {
                return None; // reject shadows with no data
            }
            (
                nv,
                np,
                geometry.verts[..nv as usize].iter().map(|v| ShadowVolumeVertex { x: v.x, y: v.y, z: v.z }).collect::<Vec<_>>(),
                geometry.indices[..(np * 3) as usize].to_vec(),
            )
        };
        let num_index = num_polys * 3;

        let b = &mut self.buffers;
        let vstart: usize;
        if b.n_shadow_verts_in_buf > (b.shadow_vertex_size - num_verts) {
            // check if room for model verts: flush the buffer by drawing the contents and re-locking again (DISCARD)
            b.n_shadow_verts_in_buf = 0;
            b.n_shadow_start_batch_vertex = 0;
            vstart = 0;
        } else {
            vstart = b.n_shadow_verts_in_buf as usize; // NOOVERWRITE
        }
        for (k, v) in vertices.iter().enumerate() {
            b.vertex_buffer[vstart + k] = *v;
        }

        let istart: usize;
        if b.n_shadow_indices_in_buf > (b.shadow_index_size - num_index) {
            b.n_shadow_indices_in_buf = 0;
            b.n_shadow_start_batch_index = 0;
            istart = 0;
        } else {
            istart = b.n_shadow_indices_in_buf as usize;
        }
        for (k, v) in indices.iter().enumerate() {
            b.index_buffer[istart + k] = *v;
        }

        let draw = VolumeDraw {
            world: *mesh_xform,
            base_vertex: b.n_shadow_start_batch_vertex,
            start_index: b.n_shadow_start_batch_index,
            polys: num_polys,
            verts: num_verts,
            dynamic: true,
            vertices,
            indices,
        };

        b.n_shadow_verts_in_buf += num_verts;
        b.n_shadow_start_batch_vertex = b.n_shadow_verts_in_buf;

        b.n_shadow_indices_in_buf += num_index;
        b.n_shadow_start_batch_index = b.n_shadow_indices_in_buf;
        Some(draw)
    }

    /// `W3DVolumetricShadow::RenderVolume` (W3DVolumetricShadow.cpp:1351): renders a specific volume from the model
    /// hierarchy.
    pub fn render_volume(&mut self, slot: usize, mesh_index: usize, light_index: usize, env: &impl VolumetricShadowEnv) -> Option<VolumeDraw> {
        let (robj_handle, mesh_robj_index, dynamic) = {
            let shadow = self.shadows[slot].as_ref().unwrap();
            let geom = &self.w3d_shadow_geometry_manager.storage[shadow.geometry.unwrap()];
            (
                shadow.robj.as_ref().unwrap().handle,
                geom.mesh_list[mesh_index].mesh_robj_index,
                (shadow.shadow_volume[0][mesh_index].as_ref().unwrap().flags & SHADOW_DYNAMIC) != 0,
            )
        };

        let mesh = env.mesh_of_robj(robj_handle, mesh_robj_index)?;
        // (SV_DEBUG_BOUNDS: RenderMeshVolumeBounds is not compiled)
        if dynamic {
            self.render_dynamic_mesh_volume(slot, mesh_index, light_index, &mesh.transform)
        } else {
            self.render_mesh_volume(slot, mesh_index, light_index, &mesh.transform)
        }
    }

    /// `W3DVolumetricShadow::updateMeshVolume` (W3DVolumetricShadow.cpp:1975): `floor_z` is the assumed ground height
    /// below the model. The code will try to extrude shadows just long enough to hit this point in order to reduce fill
    /// rate usage.
    #[allow(clippy::too_many_arguments)]
    fn update_mesh_volume(
        &mut self,
        slot: usize,
        mesh_index: usize,
        light_index: usize,
        mesh_xform: &Matrix3D,
        mesh_box: &AABoxClass,
        floor_z: f32,
        light_pos_world0: &Vector3,
        env: &impl VolumetricShadowEnv,
    ) {
        let mut shadow = self.shadows[slot].take().unwrap();
        let cos_angle_to_care = cos_angle_to_care();

        // Figuring out if mesh has rotated is cheaper (no normalization) than figuring out if light angle has changed. So
        // we divide the 2 tests. Also, our light (sun) almost never moves so no second test needed at all.
        let mut is_mesh_rotating = false; // flag if mesh has rotated since last update. Translation doesn't matter for infinite light source.
        let mut is_light_moving = false; // flag if light has moved since last update.

        let object_to_world = *mesh_xform; // Matrix4x4 objectToWorld(*meshXform)
        let prev_xform = shadow.object_xform_history[light_index][mesh_index];

        // When dealing with infinite light sources, we can assume that the shadow doesn't change much based on object
        // position. Only the orientation to light matters.
        let axis = |m: &Matrix3D, r: usize| Vector3::new(m.row[r].x, m.row[r].y, m.row[r].z);
        let mut cos_angle = wwmath::fabs(Vector3::dot_product(&axis(&prev_xform, 0), &axis(&object_to_world, 0)));
        if cos_angle >= cos_angle_to_care {
            cos_angle = wwmath::fabs(Vector3::dot_product(&axis(&prev_xform, 1), &axis(&object_to_world, 1)));
            if cos_angle >= cos_angle_to_care {
                cos_angle = wwmath::fabs(Vector3::dot_product(&axis(&prev_xform, 2), &axis(&object_to_world, 2)));
                if cos_angle < cos_angle_to_care {
                    is_mesh_rotating = true;
                }
            } else {
                is_mesh_rotating = true;
            }
        } else {
            is_mesh_rotating = true;
        }

        // get the light
        let mut light_pos_world = *light_pos_world0;

        // get the object
        let object_center = mesh_xform.get_translation(); // current mesh position

        // check if object has a limit/clamp on shadow length and adjust light position of necessary.
        if shadow.shadow_length_scale != 0.0 {
            // Find light's distance from origin in xy plane
            let light_xy_distance = wwmath::sqrt(light_pos_world.x * light_pos_world.x + light_pos_world.y * light_pos_world.y);
            let new_z = light_xy_distance * shadow.shadow_length_scale;

            if new_z > light_pos_world.z {
                // clamped z component is higher than actual light position allows so adjust it.
                light_pos_world.z = new_z;
            }
        }

        if light_pos_world != shadow.light_pos_history[light_index][mesh_index] {
            // Light position has moved, see if enough to matter

            // compute vector from the light to the current object position
            let mut to_light = object_center - light_pos_world;
            to_light.normalize();

            // compute vector from the previous light to the object position
            let mut to_prev_light = object_center - shadow.light_pos_history[light_index][mesh_index];
            to_prev_light.normalize();

            let cos_angle = wwmath::fabs(Vector3::dot_product(&to_light, &to_prev_light));
            if cos_angle < cos_angle_to_care {
                // less than 45 degree change
                is_light_moving = true;
            }
        } else if wwmath::fabs(object_center.z - prev_xform.row[2].w) > SHADOW_EXTRUSION_BUFFER {
            // @todo: Find a better way to deal with this - use maximum extrusion once! Also avoid hit for units climbing hills.
            is_light_moving = true; // treat model rising just like rotation since volume needs update for longer extrusion.
        }

        // reconstruct if needed
        if is_light_moving || is_mesh_rotating {
            // transform the light in the world to object space, we care only about the rotation of components for the
            // coordinate system change, not the translations
            let m4 = [
                [object_to_world.row[0].x, object_to_world.row[0].y, object_to_world.row[0].z, object_to_world.row[0].w],
                [object_to_world.row[1].x, object_to_world.row[1].y, object_to_world.row[1].z, object_to_world.row[1].w],
                [object_to_world.row[2].x, object_to_world.row[2].y, object_to_world.row[2].z, object_to_world.row[2].w],
                [0.0, 0.0, 0.0, 1.0],
            ];
            let (w2o, _det) = d3dx_matrix_inverse_stand_in(&m4);

            // find out light position in object space (Matrix4x4::Transform_Vector)
            let light_pos_object = Vector3::new(
                w2o[0][0] * light_pos_world.x + w2o[0][1] * light_pos_world.y + w2o[0][2] * light_pos_world.z + w2o[0][3],
                w2o[1][0] * light_pos_world.x + w2o[1][1] * light_pos_world.y + w2o[1][2] * light_pos_world.z + w2o[1][3],
                w2o[2][0] * light_pos_world.x + w2o[2][1] * light_pos_world.y + w2o[2][2] * light_pos_world.z + w2o[2][3],
            );

            // Updating shadow volumes is expensive, so verify that this volume is even visible.

            // Generate bounding box around shadow volume by extruding AABB corners
            let bx = *mesh_box; // copy current mesh bounding box (will be smaller than shadow box).
            let mut corners = [Vector3::default(); 8];

            // Get vertices of top of bounding box
            corners[0] = bx.center + bx.extent; // top right corner
            corners[1] = corners[0];
            corners[1].x -= 2.0 * bx.extent.x; // top left corner
            corners[2] = corners[1];
            corners[2].y -= 2.0 * bx.extent.y; // bottom left corner
            corners[3] = corners[2];
            corners[3].x += 2.0 * bx.extent.x; // bottom right corner

            // Project top volume corners onto ground plane
            let mut light_ray = corners[0] - light_pos_world; // vector light to corner
            let mut length = 1.0 / light_ray.length();
            light_ray *= length;
            let mut vector_scale = wwmath::fabs((corners[0].z - floor_z) / light_ray.z); // length of vector from top corner to ground.
            let mut vector_scale_max = vector_scale;
            corners[4] = corners[0] + light_ray * vector_scale;
            vector_scale_max *= length;

            light_ray = corners[1] - light_pos_world;
            length = 1.0 / light_ray.length();
            light_ray *= length;
            let mut vector_scale_temp = wwmath::fabs((corners[1].z - floor_z) / light_ray.z);
            corners[5] = corners[1] + light_ray * vector_scale_temp;
            vector_scale_temp *= length;

            if vector_scale_temp > vector_scale_max {
                vector_scale_max = vector_scale_temp; // keep track of maximum required extrusion length.
            }

            light_ray = corners[2] - light_pos_world;
            length = 1.0 / light_ray.length();
            light_ray *= length;
            vector_scale = wwmath::fabs((corners[2].z - floor_z) / light_ray.z);
            corners[6] = corners[2] + light_ray * vector_scale;
            vector_scale *= length;

            if vector_scale > vector_scale_max {
                vector_scale_max = vector_scale;
            }

            light_ray = corners[3] - light_pos_world;
            length = 1.0 / light_ray.length();
            light_ray *= length;
            vector_scale_temp = wwmath::fabs((corners[3].z - floor_z) / light_ray.z);
            corners[7] = corners[3] + light_ray * vector_scale_temp;
            vector_scale_temp *= length;

            if vector_scale_temp > vector_scale_max {
                vector_scale_max = vector_scale_temp;
            }

            // box.Init(Corners, 8): generate a new bounding box
            let mut mm = MinMaxAABoxClass::init_empty();
            for c in corners.iter() {
                mm.add_point(c);
            }
            let mut bx = aabox_from_min_max(&mm);
            // sphere.Init(box.Center, box.Extent.Length()): object space bounding sphere containing box.
            let mut sphere = SphereClass::new(bx.center, bx.extent.length());

            let mut result = env.frustum_sphere_overlap(&sphere);
            if result == OverlapType::Overlapped {
                // do a more accurate test
                result = env.frustum_box_overlap(&bx);
            }

            if result != OverlapType::Outside {
                // reset the silhouette data and build a new one from this light source perspective

                if shadow.num_silhouette_indices[mesh_index] != 0 {
                    // this silhouette was built before and is being updated. this probably means it will change again in
                    // the future. make future updates faster by pre-caching face normals.
                    self.w3d_shadow_geometry_manager.storage[shadow.geometry.unwrap()].mesh_list[mesh_index].build_polygon_normals();
                }
                shadow.reset_silhouette(mesh_index as i32);
                shadow.build_silhouette(mesh_index as i32, &light_pos_object, &mut self.w3d_shadow_geometry_manager);

                // in a multiple shadow situation we would be allocating a volume for this current shadow light, not the 0
                // index volume all the time
                if shadow.shadow_volume[light_index][mesh_index].is_none() {
                    shadow.allocate_shadow_volume(light_index as i32, mesh_index as i32);
                }
                if shadow.shadow_volume_vb[light_index][mesh_index].is_some() {
                    // Updating an existing vertex buffer shadow volume. This means we're probably dealing with an animated
                    // mesh. Update flags to reflect this fact.
                    if is_mesh_rotating || is_light_moving {
                        if is_mesh_rotating {
                            // rotating meshes will most likely need updates each frame, so stop using static vertex buffers.
                            let sv = shadow.shadow_volume[light_index][mesh_index].as_mut().unwrap();
                            sv.flags |= SHADOW_DYNAMIC;
                        }
                        // release memory used to store vertices/polygons
                        shadow.reset_shadow_volume(light_index as i32, mesh_index as i32, self.w3d_buffer_manager.as_mut().unwrap()); // free vertex buffers since not used for dynamic.
                        // Resize the shadow volume since we'll need room to store the vertices in memory instead of VB.
                        shadow.allocate_shadow_volume(light_index as i32, mesh_index as i32);
                    }
                }

                // construct the shadow volume at this light position in the passed shadow volume geometry index
                if (shadow.shadow_volume[light_index][mesh_index].as_ref().unwrap().flags & SHADOW_DYNAMIC) != 0 {
                    shadow.construct_volume(&light_pos_object, vector_scale_max, light_index as i32, mesh_index as i32, &self.w3d_shadow_geometry_manager);
                } else {
                    shadow.construct_volume_vb(
                        &light_pos_object,
                        vector_scale_max,
                        light_index as i32,
                        mesh_index as i32,
                        &self.w3d_shadow_geometry_manager,
                        self.w3d_buffer_manager.as_mut().unwrap(),
                    );
                }

                // store the current light position and orientation that we constructed shadow info at
                shadow.object_xform_history[light_index][mesh_index] = object_to_world;
                shadow.light_pos_history[light_index][mesh_index] = light_pos_world;

                bx.center -= object_center; // translate box to object space.
                let sv = shadow.shadow_volume[light_index][mesh_index].as_mut().unwrap();
                sv.bounding_box = bx;
                sphere.center -= object_center;
                sv.bounding_sphere = sphere;
                sv.visible_state = VisibleState::Visible; // this volume needs rendering.
            } else if let Some(sv) = shadow.shadow_volume[light_index][mesh_index].as_mut() {
                // outside view frustum, shadow wasn't updated.
                bx.center -= object_center; // translate box to object space.
                sv.bounding_box = bx;
                sphere.center -= object_center;
                sv.bounding_sphere = sphere;
                sv.visible_state = VisibleState::Invisible;
            }
        } else if let Some(sv) = shadow.shadow_volume[light_index][mesh_index].as_mut() {
            // not reconstructing volume, so don't know if visible or not.
            sv.visible_state = VisibleState::Unknown;
        }

        self.shadows[slot] = Some(shadow);
    }

    /// `W3DVolumetricShadow::updateVolumes` (W3DVolumetricShadow.cpp:1891): update shadow volumes belonging to all meshes of
    /// this shadow caster. Use `zoffset` to extend shadows below object's base by given amount.
    fn update_volumes(&mut self, slot: usize, zoffset: f32, light_pos_world0: &Vector3, env: &impl VolumetricShadowEnv) {
        let (robj, mesh_count, geom_id) = {
            let shadow = self.shadows[slot].as_ref().unwrap();
            (shadow.robj.clone().unwrap(), self.w3d_shadow_geometry_manager.storage[shadow.geometry.unwrap()].mesh_count, shadow.geometry.unwrap())
        };

        let parent_vis = robj.is_really_visible;

        for i in 0..MAX_SHADOW_LIGHTS {
            for j in 0..mesh_count as usize {
                let mesh_index = self.w3d_shadow_geometry_manager.storage[geom_id].mesh_list[j].mesh_robj_index;

                let mesh = match env.mesh_of_robj(robj.handle, mesh_index) {
                    Some(m) => m,
                    None => continue,
                };
                if !mesh.is_not_hidden_at_all {
                    continue;
                }

                // @todo: Getting the transform of the mesh may be forcing a full hierarchy evaluation. Extend floor of
                // model by 'zoffset' to compensate for flying units.
                self.update_mesh_volume(slot, j, i, &mesh.transform, &mesh.bounding_box, robj.position.z - zoffset, light_pos_world0, env);

                // update visibility if not set yet
                let state = self.shadows[slot].as_ref().unwrap().shadow_volume[i][j].as_ref().map(|g| g.visible_state);
                if let Some(mut state) = state {
                    if state == VisibleState::Unknown {
                        // Updating the mesh volume didn't update the visibility, must do it here. First check against
                        // bounding sphere
                        let (bsphere, bbox) = {
                            let sv = self.shadows[slot].as_ref().unwrap().shadow_volume[i][j].as_ref().unwrap();
                            (sv.bounding_sphere, sv.bounding_box)
                        };
                        let new_state;
                        if parent_vis {
                            // parent is visible so most likely all sub_objects are also visible so probably all shadows
                            // also visible. skip additional visibility tests
                            new_state = VisibleState::Visible;
                        } else {
                            let mut sphere = bsphere;
                            sphere.center += mesh.transform.get_translation();
                            let result = env.frustum_sphere_overlap(&sphere);
                            if result == OverlapType::Overlapped {
                                // do a more accurate test against bounding box.
                                let mut aa_box = bbox;
                                aa_box.center += mesh.transform.get_translation(); // translate bounding box to world space.
                                if env.frustum_box_overlap(&aa_box) != OverlapType::Outside {
                                    new_state = VisibleState::Visible;
                                } else {
                                    new_state = VisibleState::Invisible;
                                }
                            } else if result == OverlapType::Inside {
                                new_state = VisibleState::Visible;
                            } else {
                                new_state = VisibleState::Invisible;
                            }
                        }
                        self.shadows[slot].as_mut().unwrap().shadow_volume[i][j].as_mut().unwrap().visible_state = new_state;
                        state = new_state;
                    }
                    if state == VisibleState::Visible {
                        // shadow volume is visible. Add it to list of rendertasks.
                        let task = W3DVolumetricShadowRenderTask { parent_shadow: slot, mesh_index: j as u8, light_index: i as u8 };
                        let vb_slot = self.shadows[slot].as_ref().unwrap().shadow_volume_vb[i][j];
                        if let Some(vs) = vb_slot {
                            // add to static mesh volume list.
                            let vb = self.w3d_buffer_manager.as_ref().unwrap().vertex_buffer_empty_slots[vs].vb.unwrap();
                            while self.vb_render_task_lists.len() <= vb {
                                self.vb_render_task_lists.push(Vec::new());
                            }
                            self.vb_render_task_lists[vb].push(task);
                        } else {
                            self.add_dynamic_shadow_task(task);
                        }
                    }
                }
            }
        }
    }

    /// `W3DVolumetricShadow::Update` (W3DVolumetricShadow.cpp:1812): called once per frame for each object, when necessary
    /// it will reconstruct the shadow volume for this shadow from the silhouette of the geometry and any light sources.
    pub fn update_shadow(&mut self, slot: usize, light_pos_world0: &Vector3, env: &impl VolumetricShadowEnv) {
        let (geometry, robj, robj_extent, extra_padding) = {
            let s = self.shadows[slot].as_ref().unwrap();
            (s.geometry, s.robj.clone(), s.robj_extent, s.extra_extrusion_padding)
        };
        // sanity
        if geometry.is_none() {
            return;
        }
        let robj = robj.unwrap();

        // for now we will just rebuild a shadow volume every so often (the delay test is commented out upstream)
        let origin_compare_vector = Vector3::new(0.0, 0.0, 0.0);
        let pos = robj.position;
        if pos == origin_compare_vector {
            // the transform on this object was never set so we can't make any determination if it's visible or what the
            // shadow looks like.
            return;
        }
        // Check if this is a "flying" unit. Flying units are defined as anything that moves more than
        // AIRBORNE_UNIT_GROUND_DELTA world units above ground. This will allow "jumping" units like rocket buggies from
        // being flagged as "flying". We minimize the number of flying units because their shadow volumes are extruded
        // longer to reach the lowest point on the map. Regular shadows only extend far enough to reach the base of the
        // model (presumes base is already touching the ground).
        let ground_height = match env.logic_ground_height(pos.x, pos.y) {
            Some(h) => h, // logic knows about bridges so use if available.
            None => env.height_map_height(pos.x, pos.y),
        };
        if wwmath::fabs(pos.z - ground_height) >= AIRBORNE_UNIT_GROUND_DELTA {
            let extent = MAX_SHADOW_LENGTH_EXTRA_AIRBORNE_SCALE_FACTOR * robj_extent;
            if wwmath::fabs(pos.x - self.bc.x) > (self.be.x + extent)
                || wwmath::fabs(pos.y - self.bc.y) > (self.be.y + extent)
                || wwmath::fabs(pos.z - self.bc.z) > (self.be.z + extent)
            {
                return; // shadow can't be visible so no point in updating.
            }

            // this unit is above ground, extend shadow volume to reach lowest point on the terrain plus extra bit to make
            // sure shadow goes under ground. (`fabs` is the double function; `+ SHADOW_EXTRUSION_BUFFER` promotes.)
            let zoffset = ((pos.z - env.terrain_min_height()) as f64).abs() + SHADOW_EXTRUSION_BUFFER as f64;
            self.update_volumes(slot, zoffset as f32, light_pos_world0, env);
        } else {
            // normal object that is not floating above ground so we don't need to extend the shadow lower than the
            // object's base since it should be sitting directly at ground level.
            if wwmath::fabs(pos.x - self.bc.x) > (self.be.x + robj_extent)
                || wwmath::fabs(pos.y - self.bc.y) > (self.be.y + robj_extent)
                || wwmath::fabs(pos.z - self.bc.z) > (self.be.z + robj_extent)
            {
                return; // shadow can't be visible so no point in updating.
            }

            // check if this object has never had it's extrusion length updated. Will only be true for immobile objects
            // because finding an optimal extrusion length is expensive.
            let mut padding = extra_padding;
            if padding == 0.0 {
                let mut s = self.shadows[slot].take().unwrap();
                s.update_optimal_extrusion_padding(light_pos_world0, env);
                padding = s.extra_extrusion_padding;
                self.shadows[slot] = Some(s);
            }

            self.update_volumes(slot, padding, light_pos_world0, env);
        }
    }

    /// `W3DVolumetricShadowManager::renderStencilShadows` (W3DVolumetricShadow.cpp:3387): the quad covering the tactical
    /// view, drawn with the stencil test to darken the shadowed pixels. `None` when there is no device (not modelled).
    fn render_stencil_shadows(&self, env: &impl VolumetricShadowEnv, shadow_color: u32, stencil_shadow_mask: i32) -> StencilQuad {
        let (xpos, ypos, width, height) = env.tactical_view();
        // v[0] = (xpos+width, ypos+height), v[1] = (xpos+width, 0), v[2] = (xpos, ypos+height), v[3] = (xpos, 0)
        StencilQuad {
            positions: [
                ((xpos + width) as f32, (ypos + height) as f32),
                ((xpos + width) as f32, 0.0),
                (xpos as f32, (ypos + height) as f32),
                (xpos as f32, 0.0),
            ],
            color: shadow_color,
            stencil_mask: !(stencil_shadow_mask as u32),
        }
    }

    /// `W3DVolumetricShadowManager::renderShadows` (W3DVolumetricShadow.cpp:3453): updates and renders all the shadow
    /// volumes into the stencil buffer, then draws the shadow quad. `light_pos_world0`, `stencil_shadow_mask` and
    /// `shadow_color` are `TheW3DShadowManager`'s. The render state set-up is device-only.
    pub fn render_shadows(
        &mut self,
        env: &impl VolumetricShadowEnv,
        force_stencil_fill: bool,
        light_pos_world0: &Vector3,
        stencil_shadow_mask: i32,
        shadow_color: u32,
    ) -> VolumetricShadowRender {
        let mut out = VolumetricShadowRender::default();

        let bbox = env.maximum_visible_box();
        self.bc = bbox.center;
        self.be = bbox.extent;

        if self.shadow_list.is_some() && env.use_shadow_volumes() {
            self.buffers.n_shadow_indices_in_buf = 0xffff;
            self.buffers.n_shadow_verts_in_buf = 0xffff;

            // (preset material, opaque shader, textures off, ZFUNC LESSEQUAL, ZWRITE off, alpha test off, fog off, flat
            // shading, lighting off, texture stage states; colour writes masked off when the device supports it, else
            // ZERO/ONE alpha blending; stencil on, func NOTEQUAL for mask 0x80808080 else GREATEREQUAL, ref 0x80808080,
            // mask = the stencil shadow mask, INCR on pass, CULL_CW) — device-only
            out.stencil_not_equal = stencil_shadow_mask as u32 == 0x8080_8080;

            self.dynamic_shadow_volumes_to_render.clear(); // clear list of pending dynamic shadows

            let mut cur = self.shadow_list;
            while let Some(slot) = cur {
                let (enabled, next) = {
                    let s = self.shadows[slot].as_ref().unwrap();
                    (s.base.is_enabled && !s.base.is_invisible_enabled, s.next)
                };
                if enabled {
                    let tasks_start = self.dynamic_shadow_volumes_to_render.len();
                    self.update_shadow(slot, light_pos_world0, env);
                    // update() added dynamic shadows (LIFO: the newest first)
                    let mut idx = self.dynamic_shadow_volumes_to_render.len();
                    while idx > tasks_start {
                        idx -= 1;
                        let t = self.dynamic_shadow_volumes_to_render[idx];
                        if let Some(d) = self.render_volume(t.parent_shadow, t.mesh_index as usize, t.light_index as usize, env) {
                            out.dynamic_update_draws.push(d);
                        }
                        out.num_rendered_shadows += 1;
                    }
                }
                cur = next;
            } // end for

            // render the static volumes of each vertex buffer (front faces: INCR, CULL_CW)
            let lists = self.vb_render_task_lists.clone();
            for list in lists.iter() {
                for t in list.iter().rev() {
                    if let Some(d) = self.render_volume(t.parent_shadow, t.mesh_index as usize, t.light_index as usize, env) {
                        out.static_incr_draws.push(d);
                    }
                    out.num_rendered_shadows += 1;
                }
            }

            // back faces: STENCILPASS = DECRSAT, CULL_CCW
            for list in lists.iter() {
                for t in list.iter().rev() {
                    if let Some(d) = self.render_volume(t.parent_shadow, t.mesh_index as usize, t.light_index as usize, env) {
                        out.static_decr_draws.push(d);
                    }
                }
            }

            // dynamic shadow volumes don't need to wait in queue since they were rendered once already (the second pass)
            let dynamic = self.dynamic_shadow_volumes_to_render.clone();
            for t in dynamic.iter().rev() {
                if let Some(d) = self.render_volume(t.parent_shadow, t.mesh_index as usize, t.light_index as usize, env) {
                    out.dynamic_decr_draws.push(d);
                }
            }

            for list in self.vb_render_task_lists.iter_mut() {
                list.clear(); // nextVb->m_renderTaskList = NULL
            }

            // (CULL_CW, colour writes restored)

            // @todo: Put this check back in after water is fixed so it doesn't require shadow rendering to fix alpha.
            out.stencil_quad = Some(self.render_stencil_shadows(env, shadow_color, stencil_shadow_mask));
        } else if force_stencil_fill {
            // no shadows to render, but still need to fill stencil buffer
            out.stencil_quad = Some(self.render_stencil_shadows(env, shadow_color, stencil_shadow_mask));
        }
        out
    }
}

/// `W3DShadowGeometryManagerIterator::Get_Current_Geom` (W3DVolumetricShadow.cpp:4096), merged from the former
/// sweep_device/w3d_volumetric_shadow.rs (#214): the typed accessor over the hash-table iterator's current value
/// (`(W3DShadowGeometry *)Get_Current()`). The current geometry of the iteration over the manager's table; `None` once the
/// iteration is done (`Get_Current()` is then NULL). `geoms` is the table in iteration order and `index` the iterator position.
pub fn get_current_geom<T>(geoms: &[T], index: usize) -> Option<&T> {
    geoms.get(index)
}
