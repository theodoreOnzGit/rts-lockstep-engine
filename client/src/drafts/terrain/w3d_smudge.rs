//! UNTESTED DRAFT (#202, #204): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DSmudge.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DSmudge.cpp
//!   GeneralsMD/Code/GameEngine/Include/GameClient/Smudge.h, Source/GameClient/System/Smudge.cpp (`Smudge`,
//!     `SmudgeSet`, `SmudgeManager`, the base classes: not in this area and not in the port, so copied here)
//!
//! The intrusive doubly linked lists (`DLListClass`) are `Vec`s of indices into pools the manager owns; the static
//! `SmudgeSet::m_freeSmudgeList` (one list shared by every set) is the manager's `free_smudge_list`. `render` returns the
//! batches (vertices and the `Draw_Triangles` range) instead of using `DynamicVBAccessClass`. The render target texture,
//! `copyRect` (`CopyRects` and `LockRect` readback) and the hardware test's draws are the renderer's: `testHardwareSupport`
//! keeps its state machine and takes the readback as a closure.
//!
//! Notes on upstream behaviour kept as it is:
//! - `SmudgeSet::reset` does not reset `m_usedSmudgeCount`.
//! - `~SmudgeManager` removes smudges from `m_freeSmudgeSetList` (not the smudge list) in its second loop, so it would
//!   remove sets it has already deleted; it is not translated beyond `reset`, since memory is not freed by hand here.
//! - The centre vertex uv's Y uses `smudge->m_offset.X`, not `.Y`.
//! - `USE_COPY_RECTS` is not defined upstream; only the render-to-texture path is translated.

use super::w3d_common::DrawTriangles;
use rts_lockstep_engine::client::tactical_view::Matrix4x4;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;

/// `SMUDGE_DRAW_SIZE`: draw at most this many smudges per call. Make sure (SMUDGE_DRAW_SIZE * 12) < 65535 because that's
/// the max index buffer size.
pub const SMUDGE_DRAW_SIZE: i32 = 500;
/// `UNIQUE_COLOR`.
pub const UNIQUE_COLOR: u32 = 0x1234_5678;
/// `BLOCK_SIZE`.
pub const BLOCK_SIZE: i32 = 8;
/// `THE_COLOR`.
pub const THE_COLOR: u32 = 0x00ff_eedd;

/// `Smudge::smudgeVertex`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SmudgeVertex {
    /// `pos`: world-space position of vertex.
    pub pos: Vector3,
    /// `uv`: uv coordinates of vertex.
    pub uv: Vector2,
}

/// `struct Smudge`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Smudge {
    /// `m_pos`: position of smudge center.
    pub pos: Vector3,
    /// `m_offset`: difference in position between "texture" extraction and re-insertion for center vertex.
    pub offset: Vector2,
    /// `m_size`: size of smudge in world space.
    pub size: f32,
    /// `m_opacity`: alpha of center vertex, corners are assumed at 0.
    pub opacity: f32,
    /// `m_verts`: 5 vertices of this smudge (in counter-clockwise order, starting at top-left, ending in center).
    pub verts: [SmudgeVertex; 5],
}

/// `struct SmudgeSet`.
#[derive(Clone, Debug, Default)]
pub struct SmudgeSet {
    /// `m_usedSmudgeList`: list of smudges in this set (indices into the manager's smudge pool).
    pub used_smudge_list: Vec<usize>,
    /// `m_usedSmudgeCount`.
    pub used_smudge_count: i32,
}

impl SmudgeSet {
    /// `SmudgeSet::getUsedSmudgeCount`: active smudges that need rendering.
    pub fn get_used_smudge_count(&self) -> i32 {
        self.used_smudge_count
    }
}

/// `SmudgeManager::HardwareSmudgeSupport`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareSmudgeSupport {
    /// `SMUDGE_SUPPORT_UNKNOWN`.
    Unknown,
    /// `SMUDGE_SUPPORT_NO`.
    No,
    /// `SMUDGE_SUPPORT_YES`.
    Yes,
}

/// `_TRANS_LIT_TEX_VERTEX` (`D3DFVF_XYZRHW | D3DFVF_DIFFUSE | D3DFVF_TEX1`) of the hardware test.
#[derive(Clone, Copy, Debug, Default)]
pub struct TransLitTexVertex {
    /// `p`.
    pub p: Vector4,
    /// `color`.
    pub color: u32,
    /// `u`.
    pub u: f32,
    /// `v`.
    pub v: f32,
}

/// `VertexFormatXYZNDUV2` (dx8fvf.h): position, normal, diffuse, two UV sets. `DX8_FVF_XYZNDUV2`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexFormatXYZNDUV2 {
    /// `x`.
    pub x: f32,
    /// `y`.
    pub y: f32,
    /// `z`.
    pub z: f32,
    /// `nx`.
    pub nx: f32,
    /// `ny`.
    pub ny: f32,
    /// `nz`.
    pub nz: f32,
    /// `diffuse`.
    pub diffuse: u32,
    /// `u1`.
    pub u1: f32,
    /// `v1`.
    pub v1: f32,
    /// `u2`.
    pub u2: f32,
    /// `v2`.
    pub v2: f32,
}

/// One batch `render` draws: the dynamic vertex buffer's contents (`count*5` vertices) and
/// `Draw_Triangles(0, smudgesInRenderBatch*4, 0, smudgesInRenderBatch*5)`.
#[derive(Clone, Debug)]
pub struct SmudgeBatch {
    /// The vertices written (`smudgesInRenderBatch*5` of the `count*5` allocated).
    pub vertices: Vec<VertexFormatXYZNDUV2>,
    /// The draw.
    pub draw: DrawTriangles,
}

/// `class SmudgeManager` plus `W3DSmudgeManager` (W3DSmudge.h).
pub struct W3DSmudgeManager {
    /// `m_hardwareSupportStatus`.
    pub hardware_support_status: HardwareSmudgeSupport,
    /// All the `Smudge`s (the pool behind the lists).
    pub smudges: Vec<Smudge>,
    /// `SmudgeSet::m_freeSmudgeList`: list of unused smudges for use by SmudgeSets (static, shared by every set).
    pub free_smudge_list: Vec<usize>,
    /// All the `SmudgeSet`s.
    pub sets: Vec<SmudgeSet>,
    /// `m_usedSmudgeSetList`: used SmudgeSets (indices into `sets`).
    pub used_smudge_set_list: Vec<usize>,
    /// `m_freeSmudgeSetList`: unused SmudgeSets ready for re-use.
    pub free_smudge_set_list: Vec<usize>,
    /// `m_smudgeCountLastFrame`: number of total smudges in manager last frame.
    pub smudge_count_last_frame: i32,
    /// `m_indexBuffer`: `SMUDGE_DRAW_SIZE*4*3` indices.
    pub index_buffer: Option<Vec<u16>>,
    /// `m_backBufferWidth`.
    pub back_buffer_width: i32,
    /// `m_backBufferHeight`.
    pub back_buffer_height: i32,
}

impl Default for W3DSmudgeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DSmudgeManager {
    /// `SmudgeManager::SmudgeManager` (Smudge.cpp:41) via `W3DSmudgeManager::W3DSmudgeManager` (W3DSmudge.cpp:50).
    pub fn new() -> Self {
        W3DSmudgeManager {
            hardware_support_status: HardwareSmudgeSupport::Unknown,
            smudges: Vec::new(),
            free_smudge_list: Vec::new(),
            sets: Vec::new(),
            used_smudge_set_list: Vec::new(),
            free_smudge_set_list: Vec::new(),
            smudge_count_last_frame: 0,
            index_buffer: None,
            back_buffer_width: 0,
            back_buffer_height: 0,
        }
    }

    /// `W3DSmudgeManager::~W3DSmudgeManager` (W3DSmudge.cpp:54): releases the resources (and `~SmudgeManager`'s `reset`).
    pub fn destroy(&mut self) {
        self.release_resources();
        self.reset_base();
    }

    /// `SmudgeManager::getSmudgeCountLastFrame` (Smudge.h).
    pub fn get_smudge_count_last_frame(&self) -> i32 {
        self.smudge_count_last_frame
    }

    /// `SmudgeManager::setSmudgeCountLastFrame` (Smudge.h).
    pub fn set_smudge_count_last_frame(&mut self, count: i32) {
        self.smudge_count_last_frame = count;
    }

    /// `SmudgeManager::getHardwareSupport` (Smudge.h).
    pub fn get_hardware_support(&self) -> bool {
        self.hardware_support_status != HardwareSmudgeSupport::No
    }

    /// `SmudgeManager::init` (Smudge.cpp:67): empty.
    fn init_base(&mut self) {}

    /// `SmudgeManager::reset` (Smudge.cpp:72): return all smudgeSets back to the free pool.
    fn reset_base(&mut self) {
        while !self.used_smudge_set_list.is_empty() {
            let head = self.used_smudge_set_list.remove(0);
            self.reset_set(head); // free all smudges.
            self.free_smudge_set_list.push(head);
        }
    }

    /// `SmudgeManager::addSmudgeSet` (Smudge.cpp:83).
    pub fn add_smudge_set(&mut self) -> usize {
        if !self.free_smudge_set_list.is_empty() {
            let set = self.free_smudge_set_list.remove(0); // remove from free list
            self.used_smudge_set_list.push(set); // add to used list.
            return set;
        }
        let set = self.sets.len();
        self.sets.push(SmudgeSet { used_smudge_list: Vec::new(), used_smudge_count: 0 });
        self.used_smudge_set_list.push(set); // add to used list.
        set
    }

    /// `SmudgeManager::removeSmudgeSet` (Smudge.cpp:97).
    pub fn remove_smudge_set(&mut self, set: usize) {
        self.used_smudge_set_list.retain(|&s| s != set); // remove from used list
        self.free_smudge_set_list.insert(0, set); // add to free list.
    }

    /// `SmudgeSet::reset` (Smudge.cpp:116): moves all the set's smudges to the free list (`m_usedSmudgeCount` is not
    /// reset, as upstream).
    pub fn reset_set(&mut self, set: usize) {
        while !self.sets[set].used_smudge_list.is_empty() {
            let head = self.sets[set].used_smudge_list.remove(0);
            self.free_smudge_list.insert(0, head); // add to free list
        }
    }

    /// `SmudgeSet::addSmudgeToSet` (Smudge.cpp:127).
    pub fn add_smudge_to_set(&mut self, set: usize) -> usize {
        if !self.free_smudge_list.is_empty() {
            let smudge = self.free_smudge_list.remove(0); // remove from free list
            self.sets[set].used_smudge_list.push(smudge); // add to used list.
            self.sets[set].used_smudge_count += 1;
            return smudge;
        }
        let smudge = self.smudges.len();
        self.smudges.push(Smudge::default());
        self.sets[set].used_smudge_list.push(smudge); // add to used list.
        self.sets[set].used_smudge_count += 1;
        smudge
    }

    /// `SmudgeSet::removeSmudgeFromSet` (Smudge.cpp:144).
    pub fn remove_smudge_from_set(&mut self, set: usize, smudge: usize) {
        self.sets[set].used_smudge_list.retain(|&s| s != smudge); // remove from used list.
        self.free_smudge_list.insert(0, smudge); // add to free list
        self.sets[set].used_smudge_count -= 1;
    }

    /// `W3DSmudgeManager::init` (W3DSmudge.cpp:59). `back_buffer` is `(width, height)` of the back buffer surface.
    pub fn init(&mut self, back_buffer: (i32, i32)) {
        self.init_base();
        self.re_acquire_resources(back_buffer);
    }

    /// `W3DSmudgeManager::reset` (W3DSmudge.cpp:65).
    pub fn reset(&mut self) {
        self.reset_base(); // base
    }

    /// `W3DSmudgeManager::ReleaseResources` (W3DSmudge.cpp:70).
    pub fn release_resources(&mut self) {
        self.index_buffer = None;
    }

    /// `W3DSmudgeManager::ReAcquireResources` (W3DSmudge.cpp:81). `back_buffer` is the back buffer surface's description
    /// (`DX8Wrapper::_Get_DX8_Back_Buffer()->Get_Description`): `(width, height)`.
    pub fn re_acquire_resources(&mut self, back_buffer: (i32, i32)) {
        self.release_resources();

        self.back_buffer_width = back_buffer.0;
        self.back_buffer_height = back_buffer.1;

        // allocate 4 triangles per smudge, each with 3 indices.
        let mut ib = vec![0u16; (SMUDGE_DRAW_SIZE * 4 * 3) as usize];

        // Fill up the IB with static vertex indices that will be used for all smudges.
        // quad of 4 triangles:
        //	0-----3
        //  |\   /|
        //  |  4  |
        //	|/   \|
        //  1-----2
        let mut vb_count: i32 = 0;
        let mut p = 0usize;
        for _i in 0..SMUDGE_DRAW_SIZE {
            // Top
            ib[p] = vb_count as u16;
            ib[p + 1] = (vb_count + 4) as u16;
            ib[p + 2] = (vb_count + 3) as u16;
            // Right
            ib[p + 3] = (vb_count + 3) as u16;
            ib[p + 4] = (vb_count + 4) as u16;
            ib[p + 5] = (vb_count + 2) as u16;
            // Bottom
            ib[p + 6] = (vb_count + 2) as u16;
            ib[p + 7] = (vb_count + 4) as u16;
            ib[p + 8] = (vb_count + 1) as u16;
            // Left
            ib[p + 9] = (vb_count + 1) as u16;
            ib[p + 10] = (vb_count + 4) as u16;
            ib[p + 11] = (vb_count + 0) as u16;

            vb_count += 5;
            p += 12;
        }
        self.index_buffer = Some(ib);
    }

    /// `W3DSmudgeManager::testHardwareSupport` (W3DSmudge.cpp:195): test if the video card supports the effect.
    /// `render_texture_exists` is `W3DShaderManager::getRenderTexture() != NULL`, `rendering_to_texture` is
    /// `W3DShaderManager::isRenderingToTexture()`, `display_width`/`display_height` are `TheDisplay`'s. `probe(quad,
    /// with_back_texture)` does what upstream does with the quad around it: sets the preset material and an opaque shader
    /// with `PASS_ALWAYS` and no depth write, no texture (or `backTexture` as stage 0 when `with_back_texture`),
    /// `SetVertexShader(D3DFVF_XYZRHW | D3DFVF_DIFFUSE | D3DFVF_TEX1)`, `DrawPrimitiveUP(D3DPT_TRIANGLESTRIP, 2, v, ..)`,
    /// then `copyRect` of the top left `BLOCK_SIZE x BLOCK_SIZE` block, returning the pixels or `None` when `copyRect`
    /// returned 0.
    pub fn test_hardware_support(
        &mut self,
        render_texture_exists: bool,
        rendering_to_texture: bool,
        display_width: i32,
        display_height: i32,
        probe: &mut impl FnMut(&[TransLitTexVertex; 4], bool) -> Option<Vec<u32>>,
    ) -> bool {
        if self.hardware_support_status == HardwareSmudgeSupport::Unknown {
            // we have not done the test yet.

            if !render_texture_exists {
                // do trivial test first to see if render target exists.
                self.hardware_support_status = HardwareSmudgeSupport::No;
                return false;
            }

            if !rendering_to_texture {
                return false; // can't do the test unless we're rendering to texture.
            }

            let mut v = [TransLitTexVertex::default(); 4];
            let bs = BLOCK_SIZE as f32;

            // bottom right
            v[0].p = Vector4::new(bs - 0.5, bs - 0.5, 0.0, 1.0);
            v[0].u = bs / display_width as f32;
            v[0].v = bs / display_height as f32;
            // top right
            v[1].p = Vector4::new(bs - 0.5, 0.0 - 0.5, 0.0, 1.0);
            v[1].u = bs / display_width as f32;
            v[1].v = 0.0;
            // bottom left
            v[2].p = Vector4::new(0.0 - 0.5, bs - 0.5, 0.0, 1.0);
            v[2].u = 0.0;
            v[2].v = bs / display_height as f32;
            // top left
            v[3].p = Vector4::new(0.0 - 0.5, 0.0 - 0.5, 0.0, 1.0);
            v[3].u = 0.0;
            v[3].v = 0.0;

            v[0].color = UNIQUE_COLOR;
            v[1].color = UNIQUE_COLOR;
            v[2].color = UNIQUE_COLOR;
            v[3].color = UNIQUE_COLOR;

            // draw polygons like this is very inefficient but for only 2 triangles, it's not worth bothering with
            // index/vertex buffers.
            // copy area we just rendered using solid color
            let ref_data = match probe(&v, false) {
                Some(d) => d,
                None => {
                    self.hardware_support_status = HardwareSmudgeSupport::No;
                    return false;
                }
            };

            // Set_DX8_Texture(0, backTexture)
            v[0].color = 0xffff_ffff;
            v[1].color = 0xffff_ffff;
            v[2].color = 0xffff_ffff;
            v[3].color = 0xffff_ffff;

            let test_data = match probe(&v, true) {
                Some(d) => d,
                None => {
                    self.hardware_support_status = HardwareSmudgeSupport::No;
                    return false;
                }
            };

            // compare the 2 buffers to see if they match.
            if test_data == ref_data {
                self.hardware_support_status = HardwareSmudgeSupport::Yes;
                return true;
            }
            self.hardware_support_status = HardwareSmudgeSupport::No;
        }

        HardwareSmudgeSupport::Yes == self.hardware_support_status
    }

    /// `W3DSmudgeManager::render` (W3DSmudge.cpp:325): draws the smudges. `view` is `camera.Get_View_Matrix`, `proj` is
    /// `camera.Get_Projection_Matrix`, `view_width`/`view_height` are `TheTacticalView`'s, `render_texture_size` is the
    /// level 0 description of `W3DShaderManager::getRenderTexture()` as `(Width, Height)` (`None` when there is no render
    /// texture or it is not being rendered to: "this card doesn't support render targets"). `hardware_supported` is the
    /// result of [`W3DSmudgeManager::test_hardware_support`]. `SortingRendererClass::Flush()` (draw sorted translucent
    /// polys like particles before reading the background) is the caller's, done when `smudge_sets_present` is true.
    /// The transforms, shader, texture states and material set-up are device-only.
    pub fn render(
        &mut self,
        hardware_supported: bool,
        view: &Matrix3D,
        proj: &Matrix4x4,
        view_width: i32,
        view_height: i32,
        render_texture_size: Option<(i32, i32)>,
    ) -> Vec<SmudgeBatch> {
        let mut batches: Vec<SmudgeBatch> = Vec::new();

        // Verify that the card supports the effect.
        if !hardware_supported {
            return batches;
        }

        let vertex_offsets: [Vector3; 4] = [
            Vector3::new(-0.5, 0.5, 0.0),
            Vector3::new(-0.5, -0.5, 0.0),
            Vector3::new(0.5, -0.5, 0.0),
            Vector3::new(0.5, 0.5, 0.0),
        ];

        let mut vertex_diffuse: [u32; 5] = [THE_COLOR, THE_COLOR, THE_COLOR, THE_COLOR, THE_COLOR];

        let (surface_width, surface_height) = match render_texture_size {
            Some(s) => s,
            None => return batches, // this card doesn't support render targets.
        };

        let tex_clamp_x: f32 = view_width as f32 / surface_width as f32;
        let tex_clamp_y: f32 = view_height as f32 / surface_height as f32;

        let tex_scale_x: f32 = tex_clamp_x * 0.5;
        let tex_scale_y: f32 = tex_clamp_y * 0.5;

        // Do a first pass over the smudges to determine how many are visible and to fill in their world-space positions
        // and screen uv coordinates.
        // TODO: Optimize out this extra pass!
        // TODO: Find size of screen rectangle that actually needs copying.

        let mut count: i32 = 0;

        // there are possibly some smudges to render, so make sure background particles have finished drawing
        // (SortingRendererClass::Flush() — device-only, not translated)

        for set_pos in 0..self.used_smudge_set_list.len() {
            let set = self.used_smudge_set_list[set_pos];
            for smudge_pos in 0..self.sets[set].used_smudge_list.len() {
                let si = self.sets[set].used_smudge_list[smudge_pos];
                let smudge = &mut self.smudges[si];

                // Get view-space center
                let vs_vert = Matrix3D::transform_vector(view, &smudge.pos);

                // Get 5 view-space vertices
                // Do center vertex outside 'for' loop since it's different.
                smudge.verts[4].pos = vs_vert;

                for i in 0..4 {
                    smudge.verts[i].pos = vs_vert + vertex_offsets[i] * smudge.size;
                    // Get uv coordinates for each vertex: ssVert = proj * verts[i].pos
                    let p = smudge.verts[i].pos;
                    let r = &proj.row;
                    let mut ss_x = r[0][0] * p.x + r[0][1] * p.y + r[0][2] * p.z + r[0][3] * 1.0;
                    let mut ss_y = r[1][0] * p.x + r[1][1] * p.y + r[1][2] * p.z + r[1][3] * 1.0;
                    let mut ss_z = r[2][0] * p.x + r[2][1] * p.y + r[2][2] * p.z + r[2][3] * 1.0;
                    let mut ss_w = r[3][0] * p.x + r[3][1] * p.y + r[3][2] * p.z + r[3][3] * 1.0;
                    let oow = 1.0 / ss_w;
                    ss_x *= oow; // returned in camera space which is -1,-1 (bottom-left) to 1,1 (top-right)
                    ss_y *= oow;
                    ss_z *= oow;
                    ss_w *= oow;
                    let _ = (ss_z, ss_w);
                    // convert camera space to uv space: 0,0 (top-left), 1,1 (bottom-right)
                    smudge.verts[i].uv.set((ss_x + 1.0) * tex_scale_x, (1.0 - ss_y) * tex_scale_y);

                    let this_uv = smudge.verts[i].uv;

                    // Clamp coordinates so we're not referencing texels outside the view.
                    if this_uv.x > tex_clamp_x {
                        smudge.offset.x = 0.0;
                    } else if this_uv.x < 0.0 {
                        smudge.offset.x = 0.0;
                    }

                    if this_uv.y > tex_clamp_y {
                        smudge.offset.y = 0.0;
                    } else if this_uv.y < 0.0 {
                        smudge.offset.y = 0.0;
                    }
                }

                // Finish center vertex
                // Get uv coordinates by interpolating corner uv coordinates and applying desired offset.
                let uv_span_x = smudge.verts[3].uv.x - smudge.verts[0].uv.x;
                let uv_span_y = smudge.verts[1].uv.y - smudge.verts[0].uv.y;
                smudge.verts[4].uv.x = smudge.verts[0].uv.x + uv_span_x * (0.5 + smudge.offset.x);
                smudge.verts[4].uv.y = smudge.verts[0].uv.y + uv_span_y * (0.5 + smudge.offset.x);

                count += 1; // increment visible smudge count.
            }
        }

        if count == 0 {
            return batches; // nothing to render.
        }

        // upstream: Set_Transform(WORLD/VIEW, identity), Set_Index_Buffer(m_indexBuffer, 0), Set_Shader
        // (_PresetAlphaShader), Set_DX8_Texture(0, backTexture) with clamp/linear states, Set_Material(PRELIT_DIFFUSE),
        // Apply_Render_State_Changes, ALPHAOP SELECTARG2 (disable reading texture alpha since it's undefined) —
        // device-only, not translated

        let mut smudges_remaining = count;
        let mut set_pos: Option<usize> = Some(0); // first smudge set that needs rendering.
        // first smudge that needs rendering: position in the set's list (None for NULL)
        let mut remaining_smudge_start: Option<usize> = {
            let set = self.used_smudge_set_list[0];
            if self.sets[set].used_smudge_list.is_empty() { None } else { Some(0) }
        };

        while smudges_remaining != 0 {
            // keep drawing smudges until we run out.
            // Now that we know how many smudges need rendering, allocate vertex buffer space and copy verts.
            let mut batch_count = smudges_remaining;

            if batch_count > SMUDGE_DRAW_SIZE {
                batch_count = SMUDGE_DRAW_SIZE;
            }

            let mut smudges_in_render_batch: i32 = 0;

            // allocate 5 verts per smudge (DynamicVBAccessClass).
            let mut verts: Vec<VertexFormatXYZNDUV2> = Vec::with_capacity((batch_count * 5) as usize);
            {
                'sets: while let Some(sp) = set_pos {
                    let set = self.used_smudge_set_list[sp];
                    let mut smudge_pos = remaining_smudge_start;

                    while let Some(smp) = smudge_pos {
                        if smp >= self.sets[set].used_smudge_list.len() {
                            break;
                        }
                        let si = self.sets[set].used_smudge_list[smp];
                        let sm_verts = self.smudges[si].verts;
                        let opacity = self.smudges[si].opacity;

                        // Check if we exceeded maximum number of smudges allowed per draw call.
                        if smudges_in_render_batch >= batch_count {
                            remaining_smudge_start = Some(smp);
                            break 'sets; // goto flushSmudges
                        }

                        // Set center vertex opacity.
                        vertex_diffuse[4] = (((opacity * 255.0) as i32) << 24) as u32 | THE_COLOR;

                        for i in 0..5 {
                            verts.push(VertexFormatXYZNDUV2 {
                                x: sm_verts[i].pos.x,
                                y: sm_verts[i].pos.y,
                                z: sm_verts[i].pos.z,
                                nx: 0.0, // keep AGP write-combining active
                                ny: 0.0,
                                nz: 0.0,
                                diffuse: vertex_diffuse[i], // set to transparent
                                u1: sm_verts[i].uv.x,
                                v1: sm_verts[i].uv.y,
                                u2: 0.0, // keep AGP write-combining active
                                v2: 0.0,
                            });
                        }

                        smudges_in_render_batch += 1;
                        smudge_pos = Some(smp + 1);
                    }

                    // advance to next node.
                    let next = sp + 1;
                    if next < self.used_smudge_set_list.len() {
                        set_pos = Some(next);
                        // start next batch at beginning of set.
                        let nset = self.used_smudge_set_list[next];
                        remaining_smudge_start = if self.sets[nset].used_smudge_list.is_empty() { None } else { Some(0) };
                    } else {
                        set_pos = None;
                    }
                }
                // flushSmudges: Set_Vertex_Buffer(vb_access)
            }

            // Draw_Triangles(0, smudgesInRenderBatch*4, 0, smudgesInRenderBatch*5)
            batches.push(SmudgeBatch {
                vertices: verts,
                draw: DrawTriangles {
                    start_index: 0,
                    polygon_count: smudges_in_render_batch * 4,
                    min_vertex_index: 0,
                    vertex_count: smudges_in_render_batch * 5,
                },
            });

            smudges_remaining -= smudges_in_render_batch;
        }

        // upstream: ALPHAOP/COLOROP back to MODULATE — device-only, not translated
        batches
    }
}
