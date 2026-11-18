//! UNTESTED DRAFT (#202, #204, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DShroud.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DShroud.cpp
//!
//! The retail path is translated (R5G6B5 shroud, no `m_fogOfWarOn`): the `#if defined(_DEBUG) || defined(_INTERNAL)`
//! blocks (A4R4G4B4 fog-of-war debug mode) and `LOAD_DUMMY_SHROUD` are not compiled in the retail build and are left
//! out. `DO_FOG_INTERPOLATION` is defined nowhere in the upstream tree, so `interpolateFogLevels` and the two fog arrays
//! are never used: [`W3DShroud::interpolate_fog_levels`] is translated for completeness but nothing calls it.
//!
//! The D3D surfaces are `Vec<u16>`s: `m_pSrcTexture` is the system-memory copy (`src_texture`, pitch `num_cells_x * 2`
//! bytes) and `m_pDstTexture` the "video memory" copy (`dst_texture`, `dst_texture_width * dst_texture_height`), which the
//! renderer uploads as an R5G6B5 clamped texture with no mip-maps (`ReAcquireResources`). `_Copy_DX8_Rects` is a rectangle
//! copy of `u16`s. `W3DShroudMaterialPassClass` / `W3DMaskMaterialPassClass` only set shader state
//! (`W3DShaderManager::setShader(ST_SHROUD_TEXTURE ...)`), which is device-only; their flags are kept.

use super::w3d_common::validate_texture_size;
use rts_lockstep_engine::common::base_type::{real_to_int_ceil, real_to_int_floor};
use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::logic::terrain_visual::MAP_XY_FACTOR;

/// `typedef UnsignedByte W3DShroudLevel`.
pub type W3DShroudLevel = u8;

/// `DEFAULT_SHROUD_CELL_SIZE`: assume shroud at same resolution as terrain cells.
pub const DEFAULT_SHROUD_CELL_SIZE: f32 = MAP_XY_FACTOR;

/// `TextureFilterClass::FilterType` values `W3DShroud` uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterType {
    /// `FILTER_TYPE_NONE`.
    None,
    /// `FILTER_TYPE_DEFAULT`.
    Default,
}

/// The `WorldHeightMap` members `W3DShroud::init` and `render` read (`getXExtent`, `getYExtent`, `getBorderSizeInline`,
/// `getDrawWidth`, `getDrawHeight`, `getDrawOrgX`, `getDrawOrgY`).
#[derive(Clone, Copy, Debug, Default)]
pub struct ShroudMapDims {
    /// `getXExtent()`.
    pub x_extent: i32,
    /// `getYExtent()`.
    pub y_extent: i32,
    /// `getBorderSizeInline()`.
    pub border_size: i32,
    /// `getDrawWidth()`.
    pub draw_width: i32,
    /// `getDrawHeight()`.
    pub draw_height: i32,
    /// `getDrawOrgX()`.
    pub draw_org_x: i32,
    /// `getDrawOrgY()`.
    pub draw_org_y: i32,
}

/// The device's texture limits `TextureLoader::Validate_Texture_Size` reads (`D3DCAPS8`).
#[derive(Clone, Copy, Debug)]
pub struct TextureCaps {
    /// `MaxTextureWidth`.
    pub max_texture_width: u32,
    /// `MaxTextureHeight`.
    pub max_texture_height: u32,
    /// `MaxVolumeExtent`.
    pub max_volume_extent: u32,
}

/// A rectangle as `RECT` (left, top, right, bottom).
#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    /// `left`.
    pub left: i32,
    /// `top`.
    pub top: i32,
    /// `right`.
    pub right: i32,
    /// `bottom`.
    pub bottom: i32,
}

/// `class W3DShroud` (W3DShroud.h): terrain shroud rendering class.
pub struct W3DShroud {
    /// `m_numCellsX`: number of cells covering entire map.
    pub num_cells_x: i32,
    /// `m_numCellsY`.
    pub num_cells_y: i32,
    /// `m_numMaxVisibleCellsX`: maximum number of shroud cells that can be visible.
    pub num_max_visible_cells_x: i32,
    /// `m_numMaxVisibleCellsY`.
    pub num_max_visible_cells_y: i32,
    /// `m_cellWidth`: spacing between adjacent cells.
    pub cell_width: f32,
    /// `m_cellHeight`.
    pub cell_height: f32,
    /// `m_pSrcTexture`: sysmem copy of visible shroud, R5G6B5 words, `num_cells_x` wide and `num_cells_y + 1` tall
    /// (the extra row is the unused one `fillBorderShroudData` fills). `None` where upstream's is NULL.
    pub src_texture: Option<Vec<u16>>,
    /// `m_srcTexturePitch`: width (in bytes) of the shroud data buffer.
    pub src_texture_pitch: u32,
    /// `m_pDstTexture`: the vidmem copy; `None` where upstream's is NULL.
    pub dst_texture: Option<Vec<u16>>,
    /// `m_dstTextureWidth`.
    pub dst_texture_width: i32,
    /// `m_dstTextureHeight`.
    pub dst_texture_height: i32,
    /// `m_shroudFilter`.
    pub shroud_filter: FilterType,
    /// `m_drawOriginX`.
    pub draw_origin_x: f32,
    /// `m_drawOriginY`.
    pub draw_origin_y: f32,
    /// `m_drawFogOfWar`: switch to draw alternate fog style instead of solid black.
    pub draw_fog_of_war: bool,
    /// `m_clearDstTexture`: flag indicating we must clear the video memory destination texture.
    pub clear_dst_texture: bool,
    /// `m_boderShroudLevel`: the level used to clear the shroud border.
    pub border_shroud_level: W3DShroudLevel,
    /// `m_finalFogData`: copy of logical shroud (`DO_FOG_INTERPOLATION`, never defined).
    pub final_fog_data: Option<Vec<W3DShroudLevel>>,
    /// `m_currentFogData` (`DO_FOG_INTERPOLATION`, never defined).
    pub current_fog_data: Option<Vec<W3DShroudLevel>>,
}

/// The pixel `setShroudLevel` and `fillShroudData` write for a level (the retail R5G6B5 branch).
fn shroud_pixel(level: W3DShroudLevel, shroud_color_as_int: i32) -> u16 {
    let mut bluepixel = (level as f32 * ((shroud_color_as_int & 0xff) as f32 / 255.0)) as u32;
    let mut greenpixel = (level as f32 * (((shroud_color_as_int & 0xff00) >> 8) as f32 / 255.0)) as u32;
    let mut redpixel = (level as f32 * (((shroud_color_as_int & 0xff0000) >> 16) as f32 / 255.0)) as u32;
    if level == 255 {
        // unshrouded pixels should be fully lit
        redpixel = 255;
        greenpixel = 255;
        bluepixel = 255;
    }
    (((bluepixel & 0xf8) >> 3) | ((greenpixel & 0xfc) << 3) | ((redpixel & 0xf8) << 8)) as u16
}

impl W3DShroud {
    /// `W3DShroud::W3DShroud` (W3DShroud.cpp:62). `global_data` is `TheGlobalData`.
    pub fn new(global_data: &GlobalData) -> Self {
        W3DShroud {
            final_fog_data: None,
            current_fog_data: None,
            src_texture: None,
            dst_texture: None,
            src_texture_pitch: 0,
            dst_texture_width: 0,
            num_max_visible_cells_x: 0,
            dst_texture_height: 0,
            num_max_visible_cells_y: 0,
            border_shroud_level: global_data.shroud_alpha as W3DShroudLevel, // assume border is black
            clear_dst_texture: true, // force clearing of destination texture;
            cell_width: DEFAULT_SHROUD_CELL_SIZE,
            cell_height: DEFAULT_SHROUD_CELL_SIZE,
            num_cells_x: 0,
            num_cells_y: 0,
            shroud_filter: FilterType::Default,
            draw_origin_x: 0.0,
            draw_origin_y: 0.0,
            draw_fog_of_war: false,
        }
    }

    /// `W3DShroud::~W3DShroud` (W3DShroud.cpp:84).
    pub fn destroy(&mut self) {
        self.release_resources();
        self.src_texture = None;
        self.final_fog_data = None;
        self.current_fog_data = None;
        self.draw_fog_of_war = false;
    }

    /// `W3DShroud::init` (W3DShroud.cpp:104): called to initialize a new shroud for a new map. Should be done after the
    /// map is loaded into the terrain object. `world_cell_size_x/y` are the world-space dimensions of each shroud cell.
    /// The system will generate enough cells to cover the full map. Returns whether `ThePartitionManager->
    /// refreshShroudForLocalPlayer()` must be called afterwards (upstream calls it when `ThePartitionManager` exists).
    pub fn init(&mut self, p_map: Option<&ShroudMapDims>, world_cell_size_x: f32, world_cell_size_y: f32, caps: &TextureCaps, global_data: &GlobalData) {
        debug_assert!(self.src_texture.is_none(), "ReAcquire of existing shroud textures");
        debug_assert!(p_map.is_some(), "Shroud init with NULL WorldHeightMap");

        let mut dst_texture_width: i32 = 0;
        let mut dst_texture_height: i32 = 0;
        self.cell_width = world_cell_size_x;
        self.cell_height = world_cell_size_y;

        // Precompute a bounding box for entire shroud layer
        if let Some(map) = p_map {
            self.num_cells_x = real_to_int_ceil((map.x_extent - 1 - map.border_size * 2) as f32 * MAP_XY_FACTOR / self.cell_width);
            self.num_cells_y = real_to_int_ceil((map.y_extent - 1 - map.border_size * 2) as f32 * MAP_XY_FACTOR / self.cell_height);

            // Maximum visible cells will depend on maximum drawable terrain size plus 1 for partial cells (since
            // shroud cells are larger than terrain cells).
            self.num_max_visible_cells_x = real_to_int_floor((map.draw_width - 1) as f32 * MAP_XY_FACTOR / self.cell_width) + 1;
            dst_texture_width = self.num_max_visible_cells_x;
            self.num_max_visible_cells_y = real_to_int_floor((map.draw_height - 1) as f32 * MAP_XY_FACTOR / self.cell_height) + 1;
            dst_texture_height = self.num_max_visible_cells_y;

            dst_texture_width = self.num_cells_x;
            dst_texture_height = self.num_cells_y;

            dst_texture_width += 2; // enlarge by 2 pixels so we can have a border color all the way around.
            let depth: u32 = 1;
            dst_texture_height += 2; // enlarge by 2 pixels so we can have border color all the way around.
            let (w, h, _d) = validate_texture_size(dst_texture_width as u32, dst_texture_height as u32, depth, caps.max_texture_width, caps.max_texture_height, caps.max_volume_extent);
            dst_texture_width = w as i32;
            dst_texture_height = h as i32;
        }

        let src_width = self.num_cells_x;
        // vertical size is larger by 1 pixel so that we have some unused pixels to use in clearing the video texture.
        // To clear the video texture, I will copy pixels from this unused area.  There is no other way to clear a video
        // memory texture to a known value because you can't lock it - only copy into it.
        let mut src_height = self.num_cells_y;
        src_height += 1;

        // upstream: DX8Wrapper::_Create_DX8_Surface(srcWidth, srcHeight, WW3D_FORMAT_R5G6B5) — a system-memory surface
        self.src_texture_pitch = (src_width * 2) as u32;
        // clear entire texture to black
        self.src_texture = Some(vec![0u16; (src_width * src_height) as usize]);

        if dst_texture_width != self.dst_texture_width || dst_texture_height != self.dst_texture_height {
            // @todo: Check if size has changed - probably never
            self.release_resources(); // need a new sized shroud
        }

        if self.dst_texture.is_none() {
            self.dst_texture_width = dst_texture_width;
            self.dst_texture_height = dst_texture_height;
            self.re_acquire_resources(); // allocate video memory surface
        }

        // Force a refresh of shroud data since we just created a new source texture.
        // upstream: if (ThePartitionManager) ThePartitionManager->refreshShroudForLocalPlayer() — the caller's
        let _ = global_data;
    }

    /// `W3DShroud::reset` (W3DShroud.cpp:195): called on map reset.
    pub fn reset(&mut self) {
        // Free old shroud data since it may no longer fit new map.
        self.src_texture = None;
        self.final_fog_data = None;
        self.current_fog_data = None;
        self.clear_dst_texture = true; // always refill the destination texture after a reset
    }

    /// `W3DShroud::ReleaseResources` (W3DShroud.cpp:212): release any resources that can't survive a D3D device reset.
    pub fn release_resources(&mut self) {
        self.dst_texture = None; // REF_PTR_RELEASE(m_pDstTexture)
    }

    /// `W3DShroud::ReAcquireResources` (W3DShroud.cpp:219): restore resources that are lost on D3D device reset.
    pub fn re_acquire_resources(&mut self) -> bool {
        if self.dst_texture_width == 0 {
            return true; // nothing to reaquire since shroud was never initialized with valid data
        }

        debug_assert!(self.dst_texture.is_none(), "ReAcquire of existing shroud texture");

        // Create destination texture (stored in video memory).
        // Since we control the video memory copy, we can do partial updates more efficiently. Or do shift blits.
        // upstream: new TextureClass(w, h, WW3D_FORMAT_R5G6B5, MIP_LEVELS_1, POOL_DEFAULT); U/V address modes CLAMP,
        // mip-mapping FILTER_TYPE_NONE — the renderer's
        self.dst_texture = Some(vec![0u16; (self.dst_texture_width * self.dst_texture_height) as usize]);
        self.clear_dst_texture = true; // force clearing of destination texture first time it's used.

        true
    }

    /// `W3DShroud::getShroudLevel` (W3DShroud.cpp:253).
    pub fn get_shroud_level(&self, x: i32, y: i32) -> W3DShroudLevel {
        debug_assert!(self.src_texture.is_some(), "Reading empty shroud");

        if x < self.num_cells_x && y < self.num_cells_y {
            let src = self.src_texture.as_ref().expect("Reading empty shroud");
            let pixel = src[(x + y * self.num_cells_x) as usize];
            // in this mode, green has the best precision at 6 bits.
            return (((pixel >> 5) & 0x3f) as f32 / 63.0 * 255.0) as W3DShroudLevel;
        }
        0
    }

    /// `W3DShroud::setShroudLevel` (W3DShroud.cpp:277). `texture_only` only matters under `DO_FOG_INTERPOLATION`.
    pub fn set_shroud_level(&mut self, x: i32, y: i32, mut level: W3DShroudLevel, _texture_only: bool, global_data: &GlobalData) {
        debug_assert!(self.src_texture.is_some(), "Writing empty shroud.  Usually means that map failed to load.");

        if self.src_texture.is_none() {
            return;
        }

        if x < self.num_cells_x && y < self.num_cells_y {
            if level < global_data.shroud_alpha {
                level = global_data.shroud_alpha;
            }

            let pixel = shroud_pixel(level, global_data.shroud_color.get_as_int());
            let idx = (x + y * self.num_cells_x) as usize;
            if let Some(src) = self.src_texture.as_mut() {
                src[idx] = pixel;
            }
        }
    }

    /// `W3DShroud::fillShroudData` (W3DShroud.cpp:368): quickly sets the shroud level of entire map to a single value.
    pub fn fill_shroud_data(&mut self, mut level: W3DShroudLevel, global_data: &GlobalData) {
        if level < global_data.shroud_alpha {
            level = global_data.shroud_alpha;
        }

        let pixel = shroud_pixel(level, global_data.shroud_color.get_as_int());

        let (nx, ny) = (self.num_cells_x, self.num_cells_y);
        if let Some(src) = self.src_texture.as_mut() {
            for y in 0..ny {
                for x in 0..nx {
                    src[(x + y * nx) as usize] = pixel;
                }
            }
        }
        // upstream: DO_FOG_INTERPOLATION block (never defined) sets m_finalFogData to level
    }

    /// `W3DShroud::fillBorderShroudData` (W3DShroud.cpp:442): fills the unused row of the source with the border colour
    /// and copies it across every row of the destination (`pDestSurface` is `dst_texture`, `_Copy_DX8_Rects` is a copy of
    /// `u16` rows).
    pub fn fill_border_shroud_data(&mut self, mut level: W3DShroudLevel, global_data: &GlobalData) {
        if level < global_data.shroud_alpha {
            level = global_data.shroud_alpha;
        }

        let pixel = shroud_pixel(level, global_data.shroud_color.get_as_int());

        // Skip to unused texels within the shroud data
        let unused_row = (self.num_cells_y * self.num_cells_x) as usize;
        // Fill unused texels with border color
        if let Some(src) = self.src_texture.as_mut() {
            for x in 0..self.num_cells_x as usize {
                src[unused_row + x] = pixel;
            }
        }

        // Fill destination texture with border color

        // create a rectangle enclosing bottom row of unused pixels long enough to cover destination width.
        let mut src_rect = Rect { left: 0, top: self.num_cells_y, right: self.num_cells_x, bottom: self.num_cells_y + 1 };

        let num_full_copies = self.dst_texture_width / src_rect.right;
        let num_extra_pixels = self.dst_texture_width % src_rect.right;

        for y in 0..self.dst_texture_height {
            let mut dst_x;

            for x in 0..num_full_copies {
                dst_x = x * src_rect.right; // advance to next set of pixel in row.
                self.copy_rect_to_dst(&src_rect, dst_x, y);
            }
            if num_extra_pixels != 0 {
                let old_val = src_rect.right;
                dst_x = num_full_copies * old_val;
                src_rect.right = num_extra_pixels;
                self.copy_rect_to_dst(&src_rect, dst_x, y);
                src_rect.right = old_val;
            }
        }
    }

    /// `DX8Wrapper::_Copy_DX8_Rects(m_pSrcTexture, &rect, 1, dstSurface, &point)` for one rectangle, on the `u16`
    /// vectors. Rows or columns outside the destination are not copied (D3D rejects the copy).
    fn copy_rect_to_dst(&mut self, rect: &Rect, dst_x: i32, dst_y: i32) {
        let (Some(src), Some(dst)) = (self.src_texture.as_ref(), self.dst_texture.as_mut()) else {
            return;
        };
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        if w <= 0 || h <= 0 || dst_x < 0 || dst_y < 0 || dst_x + w > self.dst_texture_width || dst_y + h > self.dst_texture_height {
            return;
        }
        for row in 0..h {
            for col in 0..w {
                let s = ((rect.top + row) * self.num_cells_x + rect.left + col) as usize;
                let d = ((dst_y + row) * self.dst_texture_width + dst_x + col) as usize;
                dst[d] = src[s];
            }
        }
    }

    /// `W3DShroud::setBorderShroudLevel` (W3DShroud.cpp:538): set the shroud color within the border area of the map.
    pub fn set_border_shroud_level(&mut self, level: W3DShroudLevel) {
        self.border_shroud_level = level;
        self.clear_dst_texture = true;
    }

    /// `W3DShroud::render` (W3DShroud.cpp:552): updates the video memory surface with the currently visible shroud data.
    /// `hm` is `TheTerrainRenderObject->getMap()`. `device_ready` is `TestCooperativeLevel() == D3D_OK` (true without a
    /// device). The camera is unused upstream.
    pub fn render(&mut self, hm: &ShroudMapDims, device_ready: bool, global_data: &GlobalData) {
        if self.src_texture.is_none() {
            return; // nothing to update from.  Must be in reset state.
        }

        if !device_ready {
            return; // device not ready to render anything
        }

        debug_assert!(self.src_texture.is_some(), "Updating unallocated shroud texture");

        let mut vis_start_x = real_to_int_floor((hm.draw_org_x - hm.border_size) as f32 * MAP_XY_FACTOR / self.cell_width); // start of rendered heightmap rectangle
        if vis_start_x < 0 {
            vis_start_x = 0; // no shroud is applied in border area so it always starts at > 0
        }
        let mut vis_start_y = real_to_int_floor((hm.draw_org_y - hm.border_size) as f32 * MAP_XY_FACTOR / self.cell_height);
        if vis_start_y < 0 {
            vis_start_y = 0; // no shroud is applied in border area so it always starts at > 0
        }

        // Do it all [3/11/2003]
        vis_start_x = 0;
        vis_start_y = 0;

        let mut vis_end_x = vis_start_x + real_to_int_floor((hm.draw_width - 1) as f32 * MAP_XY_FACTOR / self.cell_width) + 1; // size of rendered heightmap rectangle
        let mut vis_end_y = vis_start_y + real_to_int_floor((hm.draw_height - 1) as f32 * MAP_XY_FACTOR / self.cell_height) + 1;

        // Do it all [3/11/2003]
        vis_end_x = self.num_cells_x;
        vis_end_y = self.num_cells_y;

        if vis_end_x > self.num_cells_x {
            vis_start_x -= vis_end_x - self.num_cells_x; // shift visible rectangle to fall within terrain bounds
            if vis_start_x < 0 {
                vis_start_x = 0;
            }
            vis_end_x = self.num_cells_x;
        }

        if vis_end_y > self.num_cells_y {
            vis_start_y -= vis_end_y - self.num_cells_y; // shift visible rectangle to fall within terrain bounds
            if vis_start_y < 0 {
                vis_start_y = 0;
            }
            vis_end_y = self.num_cells_y;
        }

        self.draw_origin_x = vis_start_x as f32 * self.cell_width;
        self.draw_origin_y = vis_start_y as f32 * self.cell_height;

        // upstream: if (m_pDstTexture->Get_Filter().Get_Mag_Filter() != m_shroudFilter) set the mag and min filters to
        // m_shroudFilter — device-only, not translated (the renderer reads `shroud_filter`)

        // Update video memory texture with sysmem copy
        let src_rect = Rect { left: vis_start_x, top: vis_start_y, right: vis_end_x, bottom: vis_end_y };
        // POINT dstPoint = {1,1}: first row/column is reserved for border.

        // upstream: DO_FOG_INTERPOLATION -> interpolateFogLevels(&srcRect) — never defined

        if self.clear_dst_texture {
            // we need to clear unused parts of the destination texture to a known
            // color in order to keep map border in the state we want.
            self.clear_dst_texture = false;

            self.fill_border_shroud_data(self.border_shroud_level, global_data);
        }

        self.copy_rect_to_dst(&src_rect, 1, 1);
    }

    /// `W3DShroud::interpolateFogLevels` (W3DShroud.cpp:690): fade current fog levels to actual logic side levels.
    /// `DO_FOG_INTERPOLATION` is never defined, so nothing calls this. `time_diff` is `timeGetTime() - prevTime`
    /// (milliseconds); the caller keeps `prevTime`, which upstream holds in a function-local static.
    pub fn interpolate_fog_levels(&mut self, time_diff: u32, global_data: &GlobalData) {
        const FOG_INTERPOLATION_RATE: f32 = 255.0 / 1000.0; // take one second to go from black to fully lit.
        if time_diff == 0 {
            return; // no time has elapsed
        }

        let mut max_fog_change = (FOG_INTERPOLATION_RATE * time_diff as f32) as i32; // maximum amount of fog change allowed in frame.
        if max_fog_change > 255 {
            max_fog_change = 255;
        }
        let level_delta = max_fog_change as W3DShroudLevel;

        let (Some(mut current), Some(final_data)) = (self.current_fog_data.take(), self.final_fog_data.take()) else {
            return;
        };
        let mut idx = 0usize;
        for j in 0..self.num_cells_y {
            for i in 0..self.num_cells_x {
                let (start, fin) = (current[idx], final_data[idx]);
                if start != fin {
                    // fog needs fading.
                    if fin < start {
                        if (start - fin) < level_delta {
                            current[idx] = fin; // change too large so clamp to final value.
                        } else {
                            current[idx] = start.wrapping_sub(level_delta);
                        }
                    } else if fin > start {
                        if (fin - start) < level_delta {
                            current[idx] = fin; // change too large so clamp to final value.
                        } else {
                            current[idx] = start.wrapping_add(level_delta);
                        }
                    }
                    let v = current[idx];
                    self.set_shroud_level(i, j, v, true, global_data);
                }
                idx += 1;
            }
        }
        self.current_fog_data = Some(current);
        self.final_fog_data = Some(final_data);
    }

    /// `W3DShroud::setShroudFilter` (W3DShroud.cpp:737): turns on bilinear filtering of shroud cells.
    pub fn set_shroud_filter(&mut self, enable: bool) {
        self.shroud_filter = if enable { FilterType::Default } else { FilterType::None };
    }

    /// `W3DShroud::getNumShroudCellsY` (W3DShroud.h).
    pub fn get_num_shroud_cells_y(&self) -> i32 {
        self.num_cells_y
    }

    /// `W3DShroud::getNumShroudCellsX` (W3DShroud.h).
    pub fn get_num_shroud_cells_x(&self) -> i32 {
        self.num_cells_x
    }

    /// `W3DShroud::getCellWidth` (W3DShroud.h): world-space width (x) of each shroud cell.
    pub fn get_cell_width(&self) -> f32 {
        self.cell_width
    }

    /// `W3DShroud::getCellHeight` (W3DShroud.h): world-space height (y) of each shroud cell.
    pub fn get_cell_height(&self) -> f32 {
        self.cell_height
    }

    /// `W3DShroud::getTextureWidth` (W3DShroud.h): internal use by the shader system.
    pub fn get_texture_width(&self) -> i32 {
        self.dst_texture_width
    }

    /// `W3DShroud::getTextureHeight` (W3DShroud.h).
    pub fn get_texture_height(&self) -> i32 {
        self.dst_texture_height
    }

    /// `W3DShroud::getDrawOriginX` (W3DShroud.h): world-space origin of the first pixel in the shroud texture.
    pub fn get_draw_origin_x(&self) -> f32 {
        self.draw_origin_x
    }

    /// `W3DShroud::getDrawOriginY` (W3DShroud.h).
    pub fn get_draw_origin_y(&self) -> f32 {
        self.draw_origin_y
    }
}

/// `class W3DShroudMaterialPassClass : public MaterialPassClass` (W3DShroud.h): a material pass modified to apply a
/// shroud texture projection. `Install_Materials` / `UnInstall_Materials` set `ST_SHROUD_TEXTURE` shader state
/// (`W3DShaderManager::setTexture(0, shroud texture); setShader(ST_SHROUD_TEXTURE, 0)` / `resetShader`), which is
/// device-only and not translated.
#[derive(Clone, Copy, Debug, Default)]
pub struct W3DShroudMaterialPassClass {
    /// `m_isTransparentObjectPass`: customized version to deal with transparent (alpha-tested) polys.
    pub is_transparent_object_pass: bool,
}

impl W3DShroudMaterialPassClass {
    /// `W3DShroudMaterialPassClass::enableTransparentObjectPass` (W3DShroud.h).
    pub fn enable_transparent_object_pass(&mut self, enable: bool) {
        self.is_transparent_object_pass = enable;
    }
}

/// `class W3DMaskMaterialPassClass : public MaterialPassClass` (W3DShroud.h): a material pass modified to apply a
/// texture projection (`ST_MASK_TEXTURE`). The shader state calls are device-only and not translated.
#[derive(Clone, Copy, Debug)]
pub struct W3DMaskMaterialPassClass {
    /// `m_allowUninstall`: flag which allows uninstalling this material.
    pub allow_uninstall: bool,
}

impl Default for W3DMaskMaterialPassClass {
    fn default() -> Self {
        W3DMaskMaterialPassClass { allow_uninstall: true }
    }
}

impl W3DMaskMaterialPassClass {
    /// `W3DMaskMaterialPassClass::setAllowUninstall` (W3DShroud.h).
    pub fn set_allow_uninstall(&mut self, state: bool) {
        self.allow_uninstall = state;
    }

    /// Whether `UnInstall_Materials` calls `W3DShaderManager::resetShader(ST_MASK_TEXTURE)` (W3DShroud.cpp:781).
    pub fn uninstall_resets_shader(&self) -> bool {
        self.allow_uninstall
    }
}
