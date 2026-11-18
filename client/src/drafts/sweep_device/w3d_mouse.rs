//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DMouse.cpp
//!
//! Sweep 2 (#213): the cursor asset management of `W3DMouse` for its three redraw modes (polygon, DX8 texture, W3D model).
//! Images, textures, render objects and animations are asset handles/names the renderer resolves (`ImageId`, `TextureId`
//! below are opaque ids), so each function returns what it would load and updates the bookkeeping. The `mutex`
//! (`CriticalSectionClass::LockClass`) guarding the init functions protects the cursor thread (`isThread`), which the client
//! does not have: kept as the `is_thread` early return only.

/// `NUM_MOUSE_CURSORS` (Mouse.h), the count of `MouseCursor` values.
pub const NUM_MOUSE_CURSORS: usize = 27;
/// `MAX_2D_CURSOR_ANIM_FRAMES`.
pub const MAX_2D_CURSOR_ANIM_FRAMES: usize = 21;
/// `MouseCursor::NONE`.
pub const MOUSE_CURSOR_NONE: usize = 0;

/// `m_currentRedrawMode` values the init functions test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedrawMode {
    /// `RM_WINDOWS`.
    Windows,
    /// `RM_W3D`.
    W3D,
    /// `RM_POLYGON`.
    Polygon,
    /// `RM_DX8`.
    Dx8,
}

/// `CursorInfo` (Mouse.h) as W3DMouse reads it.
#[derive(Clone, Debug, Default)]
pub struct CursorInfo {
    pub image_name: Vec<u8>,
    pub texture_name: Vec<u8>,
    pub num_frames: i32,
    pub w3d_model_name: Vec<u8>,
    pub w3d_anim_name: Vec<u8>,
    pub w3d_scale: f32,
    pub looping: bool,
}

/// Opaque asset ids.
pub type AssetId = u32;

/// The asset arrays of `W3DMouse`.
#[derive(Clone, Debug)]
pub struct W3DMouseAssets {
    pub is_thread: bool,
    pub current_redraw_mode: RedrawMode,
    pub cursor_info: Vec<CursorInfo>,
    pub current_cursor: usize,
    pub current_polygon_cursor: usize,
    /// `cursorImages[NUM_MOUSE_CURSORS]`.
    pub cursor_images: Vec<Option<AssetId>>,
    /// `cursorTextures[NUM_MOUSE_CURSORS][MAX_2D_CURSOR_ANIM_FRAMES]` and `m_currentD3DSurface[]`.
    pub cursor_textures: Vec<Vec<Option<AssetId>>>,
    pub current_d3d_surface: Vec<Option<AssetId>>,
    pub current_frames: i32,
    /// `cursorModels[]` / `cursorAnims[]`.
    pub cursor_models: Vec<Option<AssetId>>,
    pub cursor_anims: Vec<Option<AssetId>>,
    pub has_camera: bool,
    pub ortho_camera: bool,
    pub ortho_zoom: f32,
}

/// What an init function asks the renderer to load.
#[derive(Clone, Debug, PartialEq)]
pub enum MouseAssetRequest {
    /// `TheMappedImageCollection->findImageByName(name)` for cursor `i`.
    FindImage { cursor: usize, name: Vec<u8> },
    /// `am->Get_Texture(name)` for cursor `i`, frame `frame`.
    GetTexture { cursor: usize, frame: usize, name: Vec<u8> },
    /// `Create_Render_Obj(model, scale, 0)` for cursor `i`, positioned at (0, 0, -1).
    CreateRenderObj { cursor: usize, name: Vec<u8>, scale: f32 },
    /// `Get_HAnim(name)` and, with a model, `Set_Animation(anim, 0, loop ? ANIM_MODE_LOOP : ANIM_MODE_ONCE)`.
    GetHAnim { cursor: usize, name: Vec<u8>, looping: bool },
    /// The interface camera: position (0, 1, 1), view plane (-1,-1)..(1,1), clip planes 0.995..20, orthographic if asked.
    CreateCamera { ortho: bool },
}

impl W3DMouseAssets {
    pub fn new(mode: RedrawMode) -> Self {
        W3DMouseAssets {
            is_thread: false,
            current_redraw_mode: mode,
            cursor_info: vec![CursorInfo::default(); NUM_MOUSE_CURSORS],
            current_cursor: 0,
            current_polygon_cursor: 0,
            cursor_images: vec![None; NUM_MOUSE_CURSORS],
            cursor_textures: vec![vec![None; MAX_2D_CURSOR_ANIM_FRAMES]; NUM_MOUSE_CURSORS],
            current_d3d_surface: vec![None; MAX_2D_CURSOR_ANIM_FRAMES],
            current_frames: 0,
            cursor_models: vec![None; NUM_MOUSE_CURSORS],
            cursor_anims: vec![None; NUM_MOUSE_CURSORS],
            has_camera: false,
            ortho_camera: false,
            ortho_zoom: 1.0,
        }
    }

    /// `W3DMouse::initPolygonAssets` (W3DMouse.cpp:131): in polygon mode, looks up each cursor's image by name (once:
    /// when `cursorImages[1]` is empty); `m_currentPolygonCursor = m_currentCursor`.
    pub fn init_polygon_assets(&mut self) -> Vec<MouseAssetRequest> {
        let mut req = Vec::new();
        if self.is_thread {
            return req;
        }
        if self.current_redraw_mode == RedrawMode::Polygon && self.cursor_images[1].is_none() {
            for i in 0..NUM_MOUSE_CURSORS {
                self.current_polygon_cursor = self.current_cursor;
                if !self.cursor_info[i].image_name.is_empty() {
                    req.push(MouseAssetRequest::FindImage { cursor: i, name: self.cursor_info[i].image_name.clone() });
                }
            }
        }
        req
    }

    /// `W3DMouse::freePolygonAssets` (W3DMouse.cpp:152).
    pub fn free_polygon_assets(&mut self) {
        for i in 0..NUM_MOUSE_CURSORS {
            self.cursor_images[i] = None;
        }
    }

    /// `W3DMouse::releaseD3DCursorTextures` (W3DMouse.cpp:162): always true.
    pub fn release_d3d_cursor_textures(&mut self, cursor: usize) -> bool {
        if cursor == MOUSE_CURSOR_NONE || self.cursor_textures[cursor][0].is_none() {
            return true; // no texture for this cursor or texture never loaded
        }
        for i in 0..MAX_2D_CURSOR_ANIM_FRAMES {
            self.current_d3d_surface[i] = None;
            self.cursor_textures[cursor][i] = None;
        }
        true
    }

    /// `W3DMouse::loadD3DCursorTextures` (W3DMouse.cpp:177): the texture requests for a cursor. One frame loads
    /// `<base>.tga`; several load `<base>NNNN.tga` (4 digits) up to `MAX_2D_CURSOR_ANIM_FRAMES`. `Err(())` is FALSE (no
    /// animation frames defined); `Ok(requests)` is TRUE (also when already loaded, with no requests). The caller fills
    /// `cursor_textures`/`current_d3d_surface` from what it loads and sets `current_frames` via [`Self::note_frame_loaded`].
    pub fn load_d3d_cursor_textures(&mut self, cursor: usize) -> Result<Vec<MouseAssetRequest>, ()> {
        if cursor == MOUSE_CURSOR_NONE || self.cursor_textures[cursor][0].is_some() {
            return Ok(Vec::new()); // no texture for this cursor or texture already loaded
        }
        let mut anim_frames = self.cursor_info[cursor].num_frames;
        if anim_frames == 0 {
            return Err(()); // no animation frames defined.
        }
        let base_name = String::from_utf8_lossy(&self.cursor_info[cursor].texture_name).into_owned();
        if anim_frames as usize > MAX_2D_CURSOR_ANIM_FRAMES {
            anim_frames = MAX_2D_CURSOR_ANIM_FRAMES as i32;
        }
        self.current_frames = 0;
        let mut req = Vec::new();
        if anim_frames == 1 {
            // single animation frame without trailing numbers
            req.push(MouseAssetRequest::GetTexture { cursor, frame: 0, name: format!("{}.tga", base_name).into_bytes() });
        } else {
            for i in 0..anim_frames as usize {
                req.push(MouseAssetRequest::GetTexture { cursor, frame: i, name: format!("{}{:04}.tga", base_name, i).into_bytes() });
            }
        }
        Ok(req)
    }

    /// Bookkeeping for a frame `loadD3DCursorTextures` obtained: stored at the next free surface slot (`m_currentFrames++`);
    /// upstream skips a texture that failed to load, so the slot is not consumed.
    pub fn note_frame_loaded(&mut self, cursor: usize, frame: usize, texture: AssetId, surface: AssetId) {
        self.cursor_textures[cursor][frame] = Some(texture);
        self.current_d3d_surface[self.current_frames as usize] = Some(surface);
        self.current_frames += 1;
    }

    /// `W3DMouse::initD3DAssets` (W3DMouse.cpp:216): in DX8 mode, once (when `cursorTextures[1]` — upstream indexes the
    /// 2-D array with one subscript, i.e. the row pointer, which is never NULL; kept as "first time through" here) and
    /// with an asset manager, clears every cursor texture and surface slot (the loads are on demand by
    /// `loadD3DCursorTextures`). Upstream's `m_currentD3DSurface[i]` uses the cursor index `i` on the frame array.
    pub fn init_d3d_assets(&mut self, have_asset_manager: bool) {
        if self.is_thread {
            return;
        }
        if self.current_redraw_mode == RedrawMode::Dx8 && self.cursor_textures[1][0].is_none() && have_asset_manager {
            for i in 0..NUM_MOUSE_CURSORS {
                for j in 0..MAX_2D_CURSOR_ANIM_FRAMES {
                    self.cursor_textures[i][j] = None;
                    if i < self.current_d3d_surface.len() {
                        self.current_d3d_surface[i] = None;
                    }
                    let _ = j;
                }
            }
        }
    }

    /// `W3DMouse::freeD3DAssets` (W3DMouse.cpp:244).
    pub fn free_d3d_assets(&mut self) {
        for i in 0..MAX_2D_CURSOR_ANIM_FRAMES {
            self.current_d3d_surface[i] = None;
        }
        for i in 0..NUM_MOUSE_CURSORS {
            for j in 0..MAX_2D_CURSOR_ANIM_FRAMES {
                self.cursor_textures[i][j] = None;
            }
        }
    }

    /// `W3DMouse::initW3DAssets` (W3DMouse.cpp:259): the render objects (scaled by the ortho zoom for an orthographic
    /// camera), their animations, and the interface camera.
    pub fn init_w3d_assets(&mut self, have_asset_manager: bool) -> Vec<MouseAssetRequest> {
        let mut req = Vec::new();
        if self.is_thread {
            return req;
        }
        if self.cursor_models[1].is_none() && have_asset_manager {
            for i in 1..NUM_MOUSE_CURSORS {
                if !self.cursor_info[i].w3d_model_name.is_empty() {
                    let scale = if self.ortho_camera { self.cursor_info[i].w3d_scale * self.ortho_zoom } else { self.cursor_info[i].w3d_scale };
                    req.push(MouseAssetRequest::CreateRenderObj { cursor: i, name: self.cursor_info[i].w3d_model_name.clone(), scale });
                }
            }
        }
        if self.cursor_anims[1].is_none() && have_asset_manager {
            for i in 1..NUM_MOUSE_CURSORS {
                if !self.cursor_info[i].w3d_anim_name.is_empty() {
                    req.push(MouseAssetRequest::GetHAnim { cursor: i, name: self.cursor_info[i].w3d_anim_name.clone(), looping: self.cursor_info[i].looping });
                }
            }
        }
        self.has_camera = true;
        req.push(MouseAssetRequest::CreateCamera { ortho: self.ortho_camera });
        req
    }

    /// `W3DMouse::freeW3DAssets` (W3DMouse.cpp:314): returns the cursors whose model must be removed from the interface
    /// scene (`m_3DInterfaceScene->Remove_Render_Object`) before everything is released.
    pub fn free_w3d_assets(&mut self, have_interface_scene: bool) -> Vec<usize> {
        let mut remove = Vec::new();
        for i in 0..NUM_MOUSE_CURSORS {
            if have_interface_scene && self.cursor_models[i].is_some() {
                remove.push(i);
            }
            self.cursor_models[i] = None;
            self.cursor_anims[i] = None;
        }
        self.has_camera = false;
        remove
    }
}
