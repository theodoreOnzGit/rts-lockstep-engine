//! UNTESTED DRAFT (#202, #204): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DBibBuffer.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DBibBuffer.cpp
//!
//! The DX8 vertex and index buffers are `Vec`s of the same length (`size + 4`); `loadBibsInVertexAndIndexBuffers` fills
//! them as upstream does and `renderBibs` returns the draw ranges instead of calling `DX8Wrapper`. The textures
//! (`TBBib.tga`, `TBRedBib.tga`, clamped) and the `detailAlphaShader` are the renderer's.

use super::w3d_common::{DrawTriangles, VertexFormatXYZDUV1};
use rts_lockstep_engine::common::base_type::real_to_int;
use rts_lockstep_engine::common::game_type::{DrawableId, ObjectId, INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::ww::vector3::Vector3;

/// `INITIAL_BIB_VERTEX`.
pub const INITIAL_BIB_VERTEX: i32 = 256;
/// `INITIAL_BIB_INDEX`.
pub const INITIAL_BIB_INDEX: i32 = 384;
/// `MAX_BIBS`.
pub const MAX_BIBS: usize = 1000;

/// `TBib`: the individual data for a bib.
#[derive(Clone, Copy, Debug, Default)]
pub struct TBib {
    /// `m_corners`: drawing location.
    pub corners: [Vector3; 4],
    /// `m_highlight`: use the highlight texture.
    pub highlight: bool,
    /// `m_color`: tint perhaps.
    pub color: i32,
    /// `m_objectID`: the object id this bib corresponds to.
    pub object_id: ObjectId,
    /// `m_drawableID`: the drawable id this bib corresponds to.
    pub drawable_id: DrawableId,
    /// `m_unused`: true if this bib is currently unused.
    pub unused: bool,
}

/// `class W3DBibBuffer` (W3DBibBuffer.h): draw buffer for the bibs.
pub struct W3DBibBuffer {
    /// `m_vertexBib`: bib vertex buffer (`m_vertexBibSize + 4` vertices); `None` where upstream's is NULL.
    pub vertex_bib: Option<Vec<VertexFormatXYZDUV1>>,
    /// `m_vertexBibSize`: num vertices in bib buffer.
    pub vertex_bib_size: i32,
    /// `m_indexBib`: indices defining the triangles for the bib drawing (`m_indexBibSize + 4`).
    pub index_bib: Option<Vec<u16>>,
    /// `m_indexBibSize`: indices available in `m_indexBib`.
    pub index_bib_size: i32,
    /// `m_curNumBibVertices`: number of vertices used in `m_vertexBib`.
    pub cur_num_bib_vertices: i32,
    /// `m_curNumBibIndices`: number of indices used in `m_indexBib`.
    pub cur_num_bib_indices: i32,
    /// `m_curNumNormalBibIndices`: number of non-highlighted bib index.
    pub cur_num_normal_bib_indices: i32,
    /// `m_curNumNormalBibVertex`: number of non-highlighted bib vertex.
    pub cur_num_normal_bib_vertex: i32,
    /// `m_bibs[MAX_BIBS]`: all bibs are stored here.
    pub bibs: Vec<TBib>,
    /// `m_numBibs`: number of bibs in `m_bibs`.
    pub num_bibs: i32,
    /// `m_anythingChanged`: set to true if visibility or sorting changed.
    pub anything_changed: bool,
    /// `m_updateAllKeys`.
    pub update_all_keys: bool,
    /// `m_initialized`: true if the subsystem initialized.
    pub initialized: bool,
    /// `m_isTerrainPass`.
    pub is_terrain_pass: bool,
}

impl Default for W3DBibBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DBibBuffer {
    /// `W3DBibBuffer::W3DBibBuffer` (W3DBibBuffer.cpp:131). The textures `TBBib.tga` and `TBRedBib.tga` (U and V address
    /// modes clamped) are loaded by the renderer.
    pub fn new() -> Self {
        let mut b = W3DBibBuffer {
            vertex_bib: None,
            vertex_bib_size: 0,
            index_bib: None,
            index_bib_size: 0,
            cur_num_bib_vertices: 0,
            cur_num_bib_indices: 0,
            cur_num_normal_bib_indices: 0,
            cur_num_normal_bib_vertex: 0,
            bibs: vec![TBib::default(); MAX_BIBS],
            num_bibs: 0,
            anything_changed: false,
            update_all_keys: false,
            initialized: false,
            is_terrain_pass: false,
        };
        b.initialized = false;
        b.cur_num_bib_vertices = 0;
        b.cur_num_bib_indices = 0;
        b.clear_all_bibs();
        b.index_bib_size = INITIAL_BIB_INDEX;
        b.vertex_bib_size = INITIAL_BIB_VERTEX;
        b.allocate_bib_buffers();
        b.initialized = true;
        b
    }

    /// `W3DBibBuffer::~W3DBibBuffer` (W3DBibBuffer.cpp:117): releases the buffers (the textures are the renderer's).
    pub fn destroy(&mut self) {
        self.free_bib_buffers();
    }

    /// `W3DBibBuffer::freeBibBuffers` (W3DBibBuffer.cpp:157): frees the index and vertex buffers.
    pub fn free_bib_buffers(&mut self) {
        self.vertex_bib = None;
        self.index_bib = None;
    }

    /// `W3DBibBuffer::allocateBibBuffers` (W3DBibBuffer.cpp:169): allocates the index and vertex buffers.
    pub fn allocate_bib_buffers(&mut self) {
        // DX8VertexBufferClass(DX8_FVF_XYZDUV1, m_vertexBibSize+4, USAGE_DYNAMIC)
        self.vertex_bib = Some(vec![VertexFormatXYZDUV1::default(); (self.vertex_bib_size + 4) as usize]);
        // DX8IndexBufferClass(m_indexBibSize+4, USAGE_DYNAMIC)
        self.index_bib = Some(vec![0u16; (self.index_bib_size + 4) as usize]);
        self.cur_num_bib_vertices = 0;
        self.cur_num_bib_indices = 0;
    }

    /// `W3DBibBuffer::clearAllBibs` (W3DBibBuffer.cpp:182): removes all bibs.
    pub fn clear_all_bibs(&mut self) {
        self.num_bibs = 0;
        self.anything_changed = true;
    }

    /// `W3DBibBuffer::removeHighlighting` (W3DBibBuffer.cpp:202): clears highlighting flag.
    pub fn remove_highlighting(&mut self) {
        for bib_index in 0..self.num_bibs as usize {
            self.bibs[bib_index].highlight = false;
        }
    }

    /// `W3DBibBuffer::addBib` (W3DBibBuffer.cpp:215): adds a bib.
    pub fn add_bib(&mut self, corners: &[Vector3; 4], id: ObjectId, highlight: bool) {
        let mut bib_index = 0usize;
        while bib_index < self.num_bibs as usize {
            if !self.bibs[bib_index].unused && self.bibs[bib_index].object_id == id {
                break;
            }
            bib_index += 1;
        }
        if bib_index == self.num_bibs as usize {
            bib_index = 0;
            while bib_index < self.num_bibs as usize {
                if self.bibs[bib_index].unused {
                    break;
                }
                bib_index += 1;
            }
        }
        if bib_index == self.num_bibs as usize {
            if self.num_bibs as usize >= MAX_BIBS {
                return;
            }
            self.num_bibs += 1;
        }
        self.anything_changed = true;
        let bib = &mut self.bibs[bib_index];
        bib.corners = *corners;
        bib.highlight = highlight;
        bib.color = 0; // for now.
        bib.unused = false; // for now.
        bib.object_id = id;
        bib.drawable_id = INVALID_DRAWABLE_ID;
    }

    /// `W3DBibBuffer::addBibDrawable` (W3DBibBuffer.cpp:252): adds a bib.
    pub fn add_bib_drawable(&mut self, corners: &[Vector3; 4], id: DrawableId, highlight: bool) {
        let mut bib_index = 0usize;
        while bib_index < self.num_bibs as usize {
            if !self.bibs[bib_index].unused && self.bibs[bib_index].drawable_id == id {
                break;
            }
            bib_index += 1;
        }
        if bib_index == self.num_bibs as usize {
            bib_index = 0;
            while bib_index < self.num_bibs as usize {
                if self.bibs[bib_index].unused {
                    break;
                }
                bib_index += 1;
            }
        }
        if bib_index == self.num_bibs as usize {
            if self.num_bibs as usize >= MAX_BIBS {
                return;
            }
            self.num_bibs += 1;
        }
        self.anything_changed = true;
        let bib = &mut self.bibs[bib_index];
        bib.corners = *corners;
        bib.highlight = highlight;
        bib.color = 0; // for now.
        bib.unused = false; // for now.
        bib.object_id = INVALID_ID;
        bib.drawable_id = id;
    }

    /// `W3DBibBuffer::removeBib` (W3DBibBuffer.cpp:289): removes a bib.
    pub fn remove_bib(&mut self, id: ObjectId) {
        for bib_index in 0..self.num_bibs as usize {
            if self.bibs[bib_index].object_id == id {
                self.bibs[bib_index].unused = true;
                self.bibs[bib_index].object_id = INVALID_ID;
                self.bibs[bib_index].drawable_id = INVALID_DRAWABLE_ID;
                self.anything_changed = true;
            }
        }
    }

    /// `W3DBibBuffer::removeBibDrawable` (W3DBibBuffer.cpp:307): removes a bib.
    pub fn remove_bib_drawable(&mut self, id: DrawableId) {
        for bib_index in 0..self.num_bibs as usize {
            if self.bibs[bib_index].drawable_id == id {
                self.bibs[bib_index].unused = true;
                self.bibs[bib_index].object_id = INVALID_ID;
                self.bibs[bib_index].drawable_id = INVALID_DRAWABLE_ID;
                self.anything_changed = true;
            }
        }
    }

    /// `W3DBibBuffer::loadBibsInVertexAndIndexBuffers` (W3DBibBuffer.cpp:75): loads the bibs into the vertex buffer for
    /// drawing. `global_data` is `TheGlobalData`. (`IndexBufferExceptionFunc` and the `try`/`catch` guard the buffer lock;
    /// there is no lock here.)
    pub fn load_bibs_in_vertex_and_index_buffers(&mut self, global_data: &GlobalData) {
        if self.index_bib.is_none() || self.vertex_bib.is_none() || !self.initialized {
            return;
        }
        if !self.anything_changed {
            return;
        }

        self.cur_num_bib_vertices = 0;
        self.cur_num_bib_indices = 0;
        self.cur_num_normal_bib_indices = 0;
        self.cur_num_normal_bib_vertex = 0;

        if self.num_bibs == 0 {
            return;
        }

        let mut vb = self.vertex_bib.take().unwrap();
        let mut ib = self.index_bib.take().unwrap();
        let mut cur_ib: usize = 0;
        let mut cur_vb: usize = 0;

        // Calculate a static lighting value to use for all the bibs.
        let mut shade_r = global_data.terrain_ambient[0].red;
        let mut shade_g = global_data.terrain_ambient[0].green;
        let mut shade_b = global_data.terrain_ambient[0].blue;
        shade_r += global_data.terrain_diffuse[0].red;
        shade_g += global_data.terrain_diffuse[0].green;
        shade_b += global_data.terrain_diffuse[0].blue;
        if shade_r > 1.0 {
            shade_r = 1.0;
        }
        if shade_g > 1.0 {
            shade_g = 1.0;
        }
        if shade_b > 1.0 {
            shade_b = 1.0;
        }
        shade_r *= 255.0;
        shade_g *= 255.0;
        shade_b *= 255.0;

        let diffuse: i32 = real_to_int(shade_b) | (real_to_int(shade_g) << 8) | (real_to_int(shade_r) << 16) | (255i32 << 24);
        for do_highlight in 0..=1i32 {
            if do_highlight == 1 {
                self.cur_num_normal_bib_indices = self.cur_num_bib_indices;
                self.cur_num_normal_bib_vertex = self.cur_num_bib_vertices;
            }
            for cur_bib in 0..self.num_bibs as usize {
                if self.bibs[cur_bib].unused {
                    continue;
                }
                if (self.bibs[cur_bib].highlight as i32) != do_highlight {
                    continue;
                }
                let start_vertex = self.cur_num_bib_vertices;
                let num_vertex = 4;
                if self.cur_num_bib_vertices + num_vertex + 2 >= self.vertex_bib_size {
                    break;
                }
                let num_index = 6;
                if self.cur_num_bib_indices + num_index + 6 >= self.index_bib_size {
                    break;
                }

                for i in 0..num_vertex as usize {
                    // Update the uv values.  The W3D models each have their own texture, and
                    // we use one texture with all images in one, so we have to change the uvs to
                    // match.
                    let (u, v): (f32, f32);
                    let v_loc = self.bibs[cur_bib].corners[i];
                    match i {
                        0 => {
                            u = 0.0;
                            v = 1.0;
                        }
                        1 => {
                            u = 1.0;
                            v = 1.0;
                        }
                        2 => {
                            u = 1.0;
                            v = 0.0;
                        }
                        _ => {
                            u = 0.0;
                            v = 0.0;
                        }
                    }

                    vb[cur_vb].u1 = u;
                    vb[cur_vb].v1 = v;
                    vb[cur_vb].x = v_loc.x;
                    vb[cur_vb].y = v_loc.y;
                    vb[cur_vb].z = v_loc.z;
                    vb[cur_vb].diffuse = diffuse as u32;
                    cur_vb += 1;
                    self.cur_num_bib_vertices += 1;
                }

                let sv = start_vertex as u16;
                ib[cur_ib] = sv;
                ib[cur_ib + 1] = sv + 1;
                ib[cur_ib + 2] = sv + 2;
                ib[cur_ib + 3] = sv;
                ib[cur_ib + 4] = sv + 2;
                ib[cur_ib + 5] = sv + 3;
                cur_ib += 6;
                self.cur_num_bib_indices += 6;
            }
        }
        self.vertex_bib = Some(vb);
        self.index_bib = Some(ib);
    }

    /// `W3DBibBuffer::renderBibs` (W3DBibBuffer.cpp:337): draws the bibs. Returns the `Draw_Triangles` calls upstream
    /// makes: the normal bibs with `m_bibTexture` first, then the highlighted ones with `m_highlightBibTexture`, each
    /// `Some` only when upstream draws it. Index buffer, vertex buffer and `detailAlphaShader` set-up are device-only.
    pub fn render_bibs(&mut self, global_data: &GlobalData) -> (Option<DrawTriangles>, Option<DrawTriangles>) {
        self.load_bibs_in_vertex_and_index_buffers(global_data);

        if self.cur_num_bib_indices == 0 {
            return (None, None);
        }
        let mut normal = None;
        let mut highlighted = None;
        if self.cur_num_normal_bib_indices != 0 {
            normal = Some(DrawTriangles {
                start_index: 0,
                polygon_count: self.cur_num_normal_bib_indices / 3,
                min_vertex_index: 0,
                vertex_count: self.cur_num_normal_bib_vertex,
            });
        }
        if self.cur_num_bib_indices > self.cur_num_normal_bib_indices {
            highlighted = Some(DrawTriangles {
                start_index: self.cur_num_normal_bib_indices,
                polygon_count: (self.cur_num_bib_indices - self.cur_num_normal_bib_indices) / 3,
                min_vertex_index: self.cur_num_normal_bib_vertex,
                vertex_count: self.cur_num_bib_vertices - self.cur_num_normal_bib_vertex,
            });
        }
        (normal, highlighted)
    }
}
