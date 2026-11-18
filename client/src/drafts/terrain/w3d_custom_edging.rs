//! UNTESTED DRAFT (#202, #204, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DCustomEdging.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DCustomEdging.cpp
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/WorldHeightMap.cpp (`getUVForBlend`, which the
//!     port's `TerrainHeightMap` does not have: [`get_uv_for_blend`] below)
//!
//! `addEdging` is declared in the header and defined nowhere upstream, so it is not translated. `FLIP_TRIANGLES` is
//! defined (HeightMap.h:48). The DX8 buffers are `Vec`s of the sizes upstream allocates; `drawEdging` returns the passes
//! (render state and draw range) it would issue rather than calling `DX8Wrapper`.

use super::w3d_common::{DrawTriangles, VertexFormatXYZDUV2};
use rts_lockstep_engine::client::terrain_height_map::{TerrainHeightMap, TEXTURE_WIDTH, TILE_PIXEL_EXTENT};
use rts_lockstep_engine::logic::terrain_visual::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};

/// `MAX_BLENDS`.
pub const MAX_BLENDS: usize = 2000;
/// `MAX_EDGE_VERTEX`.
pub const MAX_EDGE_VERTEX: i32 = 4 * MAX_BLENDS as i32;
/// `MAX_EDGE_INDEX`.
pub const MAX_EDGE_INDEX: i32 = 6 * MAX_BLENDS as i32;

/// `WorldHeightMap::getUVForBlend` (WorldHeightMap.cpp:1662): the texture coordinate range `(lo.x, lo.y, hi.x, hi.y)` of
/// an edge class's tiles in the edge texture.
pub fn get_uv_for_blend(map: &TerrainHeightMap, edge_class: i32) -> (f32, f32, f32, f32) {
    let class = &map.edge_texture_classes[edge_class as usize];
    let pos = class.position_in_texture;
    let width = class.width;
    let lo_x = pos.x as f32 / TEXTURE_WIDTH as f32;
    let lo_y = pos.y as f32 / map.alpha_edge_height as f32;
    let hi_x = (pos.x as f32 + (width * TILE_PIXEL_EXTENT) as f32) / TEXTURE_WIDTH as f32;
    let hi_y = (pos.y as f32 + (width * TILE_PIXEL_EXTENT) as f32) / map.alpha_edge_height as f32;
    (lo_x, lo_y, hi_x, hi_y)
}

/// One pass of `drawEdging`: the textures and alpha test it sets, and the draw range (the same for every pass).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgingPass {
    /// `detailAlphaTestShader`, texture 0 the terrain, texture 1 the edge texture, `D3DRS_ALPHAREF = 0x7B`,
    /// `D3DCMP_LESSEQUAL`.
    TerrainThroughEdgeMask,
    /// Texture 0 the edge texture, no texture 1, `D3DRS_ALPHAREF = 0x84`, `D3DCMP_GREATEREQUAL`: the custom edge itself.
    CustomEdge,
    /// `detailOpaqueShader` with the cloud texture on stage 0 and the edge texture on stage 1 (`D3DTSS_*` stage states as
    /// upstream), `D3DRS_ALPHAREF = 0x80`, `D3DCMP_NOTEQUAL`, blend `DESTCOLOR` / `ZERO`.
    Cloud,
    /// The noise texture on stage 0, the edge texture on stage 1, `D3DRS_ALPHAREF = 0x80`, `D3DCMP_NOTEQUAL`, blend
    /// `DESTCOLOR` / `ZERO`.
    Noise,
}

/// `class W3DCustomEdging` (W3DCustomEdging.h): draw buffer for the custom tile edges.
pub struct W3DCustomEdging {
    /// `m_vertexEdging`: edging vertex buffer (`MAX_EDGE_VERTEX + 4`); `None` where upstream's is NULL.
    pub vertex_edging: Option<Vec<VertexFormatXYZDUV2>>,
    /// `m_indexEdging`: indices defining the triangles (`2 * MAX_EDGE_INDEX + 4`).
    pub index_edging: Option<Vec<u16>>,
    /// `m_curNumEdgingVertices`.
    pub cur_num_edging_vertices: i32,
    /// `m_curNumEdgingIndices`.
    pub cur_num_edging_indices: i32,
    /// `m_curEdgingIndexOffset`: first index to draw at. Upstream's constructor never sets it; 0 here.
    pub cur_edging_index_offset: i32,
    /// `m_anythingChanged`.
    pub anything_changed: bool,
    /// `m_initialized`.
    pub initialized: bool,
}

impl Default for W3DCustomEdging {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DCustomEdging {
    /// `W3DCustomEdging::W3DCustomEdging` (W3DCustomEdging.cpp:201).
    pub fn new() -> Self {
        let mut e = W3DCustomEdging {
            vertex_edging: None,
            index_edging: None,
            cur_num_edging_vertices: 0,
            cur_num_edging_indices: 0,
            cur_edging_index_offset: 0,
            anything_changed: false,
            initialized: false,
        };
        e.initialized = false;
        e.clear_all_edging();
        e.allocate_edging_buffers();
        e.initialized = true;
        e
    }

    /// `W3DCustomEdging::~W3DCustomEdging` (W3DCustomEdging.cpp:190).
    pub fn destroy(&mut self) {
        self.free_edging_buffers();
    }

    /// `W3DCustomEdging::freeEdgingBuffers` (W3DCustomEdging.cpp:219).
    pub fn free_edging_buffers(&mut self) {
        self.vertex_edging = None;
        self.index_edging = None;
    }

    /// `W3DCustomEdging::allocateEdgingBuffers` (W3DCustomEdging.cpp:231).
    pub fn allocate_edging_buffers(&mut self) {
        // DX8VertexBufferClass(DX8_FVF_XYZDUV2, MAX_EDGE_VERTEX+4, USAGE_DYNAMIC)
        self.vertex_edging = Some(vec![VertexFormatXYZDUV2::default(); (MAX_EDGE_VERTEX + 4) as usize]);
        // DX8IndexBufferClass(2*MAX_EDGE_INDEX+4, USAGE_DYNAMIC)
        self.index_edging = Some(vec![0u16; (2 * MAX_EDGE_INDEX + 4) as usize]);
        self.cur_num_edging_vertices = 0;
        self.cur_num_edging_indices = 0;
    }

    /// `W3DCustomEdging::clearAllEdging` (W3DCustomEdging.cpp:246).
    pub fn clear_all_edging(&mut self) {
        self.cur_num_edging_vertices = 0;
        self.cur_num_edging_indices = 0;
        self.anything_changed = true;
    }

    /// `W3DCustomEdging::doFullUpdate` (W3DCustomEdging.h).
    pub fn do_full_update(&mut self) {
        self.clear_all_edging();
    }

    /// `W3DCustomEdging::loadEdgingsInVertexAndIndexBuffers` (W3DCustomEdging.cpp:97): loads the edgings into the vertex
    /// buffer for drawing. `static_diffuse` is `TheTerrainRenderObject->getStaticDiffuse(x, y)`.
    pub fn load_edgings_in_vertex_and_index_buffers(
        &mut self,
        p_map: &TerrainHeightMap,
        min_x: i32,
        mut max_x: i32,
        mut min_y: i32,
        mut max_y: i32,
        static_diffuse: &impl Fn(i32, i32) -> i32,
    ) {
        let mut min_x = min_x;
        if self.index_edging.is_none() || self.vertex_edging.is_none() || !self.initialized {
            return;
        }
        if !self.anything_changed {
            return;
        }
        self.anything_changed = false;
        self.cur_num_edging_vertices = 0;
        self.cur_num_edging_indices = 0;
        let mut vb = self.vertex_edging.take().unwrap();
        let mut ib = self.index_edging.take().unwrap();
        let mut cur_ib: usize = 0;
        let mut cur_vb: usize = 0;

        if min_x < 0 {
            min_x = 0;
        }
        if min_y < 0 {
            min_y = 0;
        }
        if max_x >= p_map.width {
            max_x = p_map.width - 1;
        }
        if max_y >= p_map.height {
            max_y = p_map.height - 1;
        }
        for row in min_y..(max_y - 1) {
            for column in min_x..(max_x - 1) {
                let mut cell_ndx = column + row * p_map.width;
                let blend = p_map.blend_tile_ndxes[cell_ndx as usize] as i32;
                if blend == 0 {
                    continue; // no blend.
                }

                let bt = p_map.blended_tiles[blend as usize];
                if bt.custom_blend_edge_class < 0 {
                    continue; // alpha blend.
                }
                let mut u_offset: f32;
                let mut v_offset: f32;

                if bt.horiz != 0 {
                    u_offset = 0.0;
                    v_offset = 0.25 * (1 + (row & 1)) as f32;
                    if bt.inverted != 0 {
                        u_offset = 0.75;
                    }
                } else if bt.vert != 0 {
                    v_offset = 0.75;
                    u_offset = 0.25 * (1 + (column & 1)) as f32;
                    if bt.inverted != 0 {
                        v_offset = 0.0;
                    }
                } else if bt.right_diagonal != 0 {
                    if bt.long_diagonal != 0 {
                        v_offset = 0.25;
                        u_offset = 0.5;
                        if bt.inverted != 0 {
                            u_offset = 0.5;
                            v_offset = 0.5;
                        }
                    } else {
                        v_offset = 0.75;
                        u_offset = 0.0;
                        if bt.inverted != 0 {
                            u_offset = 0.0;
                            v_offset = 0.0;
                        }
                    }
                } else if bt.left_diagonal != 0 {
                    if bt.long_diagonal != 0 {
                        u_offset = 0.25;
                        v_offset = 0.25;
                        if bt.inverted != 0 {
                            u_offset = 0.25;
                            v_offset = 0.5;
                        }
                    } else {
                        v_offset = 0.75;
                        u_offset = 0.75;
                        if bt.inverted != 0 {
                            u_offset = 0.75;
                            v_offset = 0.0;
                        }
                    }
                } else {
                    continue;
                }
                let (range_lo_x, range_lo_y, range_hi_x, range_hi_y) = get_uv_for_blend(p_map, bt.custom_blend_edge_class);
                let range_width = range_hi_x - range_lo_x;
                let range_height = range_hi_y - range_lo_y;

                u_offset = range_lo_x + range_width * u_offset;
                v_offset = range_lo_y + range_height * v_offset;

                let (ua, va, _alpha, flip_for_blend) = p_map.get_alpha_uv_data(column - p_map.draw_origin_x, row - p_map.draw_origin_y, false);

                let start_vertex = self.cur_num_edging_vertices;
                for j in 0..2i32 {
                    for i in 0..2i32 {
                        if self.cur_num_edging_vertices >= MAX_EDGE_VERTEX {
                            self.vertex_edging = Some(vb);
                            self.index_edging = Some(ib);
                            return;
                        }
                        cell_ndx = column + i + (row + j) * p_map.width;

                        let diffuse = static_diffuse(column + i, row + j);
                        vb[cur_vb].diffuse = 0x8000_0000u32.wrapping_add((diffuse & 0x00FF_FFFF) as u32); // set alpha to 5b.
                        let the_z = p_map.data[cell_ndx as usize] as f32 * MAP_HEIGHT_SCALE;
                        let x = (column + i) as f32 * MAP_XY_FACTOR;
                        let y = (row + j) as f32 * MAP_XY_FACTOR;
                        vb[cur_vb].u2 = u_offset + i as f32 * 0.25 * range_width;
                        vb[cur_vb].v2 = v_offset + (1 - j) as f32 * 0.25 * range_height;
                        let ndx: usize = if j == 0 { i as usize } else { (3 - i) as usize };
                        vb[cur_vb].u1 = ua[ndx];
                        vb[cur_vb].v1 = va[ndx];
                        vb[cur_vb].x = x;
                        vb[cur_vb].y = y;
                        vb[cur_vb].z = the_z;
                        cur_vb += 1;
                        self.cur_num_edging_vertices += 1;
                    }
                }
                let y_offset: i32 = 2;
                if self.cur_num_edging_indices + 6 > MAX_EDGE_INDEX {
                    self.vertex_edging = Some(vb);
                    self.index_edging = Some(ib);
                    return;
                }
                // FLIP_TRIANGLES: jba - reduces "diamonding" in some cases, not others.  Better cliffs, though.
                let sv = start_vertex;
                if flip_for_blend {
                    ib[cur_ib] = (sv + 1) as u16;
                    ib[cur_ib + 1] = (sv + y_offset) as u16;
                    ib[cur_ib + 2] = sv as u16;
                    ib[cur_ib + 3] = (sv + 1) as u16;
                    ib[cur_ib + 4] = (sv + 1 + y_offset) as u16;
                    ib[cur_ib + 5] = (sv + y_offset) as u16;
                } else {
                    ib[cur_ib] = sv as u16;
                    ib[cur_ib + 1] = (sv + 1 + y_offset) as u16;
                    ib[cur_ib + 2] = (sv + y_offset) as u16;
                    ib[cur_ib + 3] = sv as u16;
                    ib[cur_ib + 4] = (sv + 1) as u16;
                    ib[cur_ib + 5] = (sv + 1 + y_offset) as u16;
                }
                cur_ib += 6;
                self.cur_num_edging_indices += 6;
            }
        }
        self.vertex_edging = Some(vb);
        self.index_edging = Some(ib);
        self.anything_changed = false;
    }

    /// `W3DCustomEdging::drawEdging` (W3DCustomEdging.cpp:263): draws the edgings. Returns the draw range and the passes
    /// upstream issues, in order (`cloud_texture` / `noise_texture` say whether those textures were given). Empty when
    /// nothing is to be drawn. The local `static Bool foo = false` that would return early is never set, so is not kept.
    pub fn draw_edging(
        &mut self,
        p_map: &TerrainHeightMap,
        min_x: i32,
        max_x: i32,
        min_y: i32,
        max_y: i32,
        cloud_texture: bool,
        noise_texture: bool,
        static_diffuse: &impl Fn(i32, i32) -> i32,
    ) -> (DrawTriangles, Vec<EdgingPass>) {
        // m_anythingChanged = true;  (commented out upstream)
        self.load_edgings_in_vertex_and_index_buffers(p_map, min_x, max_x, min_y, max_y, static_diffuse);

        let draw = DrawTriangles {
            start_index: self.cur_edging_index_offset,
            polygon_count: self.cur_num_edging_indices / 3,
            min_vertex_index: 0,
            vertex_count: self.cur_num_edging_vertices,
        };
        let mut passes = Vec::new();
        if self.cur_num_edging_indices == 0 {
            return (draw, passes);
        }
        // upstream: Set_Index_Buffer / Set_Vertex_Buffer / Set_Shader(detailAlphaTestShader) / Set_Texture — device-only
        passes.push(EdgingPass::TerrainThroughEdgeMask);
        passes.push(EdgingPass::CustomEdge);
        if cloud_texture {
            passes.push(EdgingPass::Cloud);
        }
        if noise_texture {
            passes.push(EdgingPass::Noise);
        }
        (draw, passes)
    }
}
