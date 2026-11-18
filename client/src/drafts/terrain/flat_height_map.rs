//! UNTESTED DRAFT (#202, #204): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/FlatHeightMap.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/FlatHeightMap.cpp
//!
//! `FlatHeightMapRenderObjClass` derives from `BaseHeightMapRenderObjClass` (BaseHeightMap.cpp, not in this area and not
//! in the port). Rather than redefine the base, the class is generic over a [`BaseHeightMapRenderObj`] contract naming
//! exactly the base members and methods this file touches (`m_map`, `m_needFullUpdate`, and the base's own versions of the
//! methods this class overrides); a concrete base implements it when `BaseHeightMap.cpp` is translated. This is not
//! dynamic dispatch: the base is a type parameter.
//!
//! `Render` returns a [`FlatRenderPlan`]: the shader choice, the pass count, each pass's tile draws and the steps that
//! follow (feathered shorelines, roads, scorches, bridges, terrain tracks, waypoints, bibs) with the values they are given,
//! for the caller that owns the base's buffers to perform. The D3D state calls are comments.
//!
//! Notes on upstream behaviour kept as it is:
//! - `Render` initialises `yCoordMin` from `getXExtent()` and `xCoordMin` from `getYExtent()` (swapped).
//! - `updateCenter` logs (`DEBUG_LOG`) when the culled or texture-multiplier counts change; the three `static` previous
//!   counts persist across calls and are fields here.
//! - `FlatHeightMapRenderObjClass::updateBlock` (header, inline) returns 0.
//! - `DO_UNIT_TIMINGS` is not defined: its early returns are not compiled. `EXTENDED_STATS` is not defined.

use super::w3d_common::AABoxClass;
use super::w3d_shader_manager::ShaderTypes;
use super::w3d_terrain_background::{SharedHeightMap, TerrainBackgroundDraw, W3DTerrainBackground};
use rts_lockstep_engine::common::base_type::IRegion2D;
use rts_lockstep_engine::ww::vector3::Vector3;

/// `CELLS_PER_TILE`: in order to be efficient in texture, needs to be a power of 2. [3/24/2003]
pub const CELLS_PER_TILE: i32 = 16;

/// What the base class `BaseHeightMapRenderObjClass` provides to `FlatHeightMapRenderObjClass`: its own implementations of
/// the methods this class overrides and calls up to, and the two members it reads and writes.
pub trait BaseHeightMapRenderObj {
    /// `BaseHeightMapRenderObjClass::freeMapResources`.
    fn free_map_resources(&mut self);
    /// `BaseHeightMapRenderObjClass::adjustTerrainLOD`.
    fn adjust_terrain_lod(&mut self, adj: i32);
    /// `BaseHeightMapRenderObjClass::ReleaseResources`.
    fn release_resources(&mut self);
    /// `BaseHeightMapRenderObjClass::ReAcquireResources`.
    fn re_acquire_resources(&mut self);
    /// `BaseHeightMapRenderObjClass::reset`.
    fn reset(&mut self);
    /// `BaseHeightMapRenderObjClass::On_Frame_Update`.
    fn on_frame_update(&mut self);
    /// `BaseHeightMapRenderObjClass::initHeightData(x, y, pMap, pLightsIterator)` (`updateExtraPassTiles` defaulted).
    fn init_height_data(&mut self, x: i32, y: i32, map: &SharedHeightMap) -> i32;
    /// `BaseHeightMapRenderObjClass::staticLightingChanged`.
    fn static_lighting_changed(&mut self);
    /// `BaseHeightMapRenderObjClass::updateCenter(camera, pLightsIterator)` (the caller gives its base the camera).
    fn update_center(&mut self);
    /// `m_map`.
    fn map(&self) -> Option<SharedHeightMap>;
    /// `REF_PTR_SET(m_map, htMap)`.
    fn set_map(&mut self, map: Option<SharedHeightMap>);
    /// `m_needFullUpdate = value`.
    fn set_need_full_update(&mut self, value: bool);
}

/// `FlatHeightMapRenderObjClass::m_updateState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlatUpdateState {
    /// `STATE_IDLE`: sleeping.
    Idle,
    /// `STATE_MOVING`: camera moving, updating visibility.
    Moving,
    /// `STATE_MOVING2`: second moving state.
    Moving2,
    /// `STATE_UPDATE_TEXTURES`: camera stopped, updating textures.
    UpdateTextures,
}

/// The inputs of `Render` that upstream reads from globals and base members.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlatRenderInput {
    /// `TheGlobalData->m_useCloudMap`.
    pub use_cloud_map: bool,
    /// `TheGlobalData->m_timeOfDay == TIME_OF_DAY_NIGHT`.
    pub night: bool,
    /// `TheGlobalData->m_useLightMap`.
    pub use_light_map: bool,
    /// `m_disableTextures`.
    pub disable_textures: bool,
    /// `m_shroud`.
    pub shroud: bool,
    /// `rinfo.Additional_Pass_Count() != 0`.
    pub additional_pass_count: bool,
    /// `DX8Wrapper::getBackBufferFormat() == WW3D_FORMAT_A8R8G8B8` (then colour writes mask out destination alpha).
    pub back_buffer_a8r8g8b8: bool,
    /// `ShaderClass::Is_Backface_Culling_Inverted()` (roads and scorches are skipped).
    pub backface_culling_inverted: bool,
    /// `m_map->getXExtent()`.
    pub map_x_extent: i32,
    /// `m_map->getYExtent()`.
    pub map_y_extent: i32,
    /// `m_map->getBorderSizeInline()`.
    pub border_size: i32,
}

/// One tile drawn in a pass of `Render`.
#[derive(Clone, Copy, Debug)]
pub struct FlatTileDraw {
    /// The pass (`pass`).
    pub pass: i32,
    /// The tile's column (`i`).
    pub i: i32,
    /// The tile's row (`j`).
    pub j: i32,
    /// `disableTex` as given to `drawVisiblePolys`.
    pub disable_tex: bool,
    /// The tile's draw (`None` when it returned without drawing).
    pub draw: Option<TerrainBackgroundDraw>,
}

/// The values `drawRoads` is given (`DO_ROADS` is defined upstream).
#[derive(Clone, Copy, Debug)]
pub struct FlatRoadsStep {
    /// `doCloud ? m_stageTwoTexture : NULL`.
    pub use_cloud_texture: bool,
    /// `TheGlobalData->m_useLightMap ? m_stageThreeTexture : NULL`.
    pub use_noise_texture: bool,
    /// `m_disableTextures`.
    pub disable_textures: bool,
    /// `xCoordMin - m_map->getBorderSizeInline()`.
    pub x_min: i32,
    /// `xCoordMax - m_map->getBorderSizeInline()`.
    pub x_max: i32,
    /// `yCoordMin - m_map->getBorderSizeInline()`.
    pub y_min: i32,
    /// `yCoordMax - m_map->getBorderSizeInline()`.
    pub y_max: i32,
}

/// The ordered work `Render` does besides the terrain tiles, for the caller to perform (all on the base class's members).
#[derive(Clone, Debug)]
pub struct FlatRenderPlan {
    /// `m_treeBuffer->setIsTerrain()` is done when the base has a tree buffer (the caller checks).
    pub tree_buffer_set_is_terrain: bool,
    /// The shader chosen (`st`).
    pub shader: ShaderTypes,
    /// The number of device passes (`devicePasses`, 1 when textures are disabled).
    pub device_passes: i32,
    /// Texture 0 is the shroud texture instead of `m_stageZeroTexture`
    /// (`m_shroud && rinfo.Additional_Pass_Count() && !m_disableTextures`).
    pub stage0_is_shroud: bool,
    /// Destination alpha writes are masked (`D3DRS_COLORWRITEENABLE` RGB).
    pub mask_destination_alpha: bool,
    /// The tile draws, in order.
    pub tiles: Vec<FlatTileDraw>,
    /// `if (pass) W3DShaderManager::resetShader(st)`: the shader was applied at least once.
    pub reset_shader: bool,
    /// `renderShoreLines(&rinfo.Camera)`.
    pub render_shore_lines: bool,
    /// `m_roadBuffer->drawRoads(...)` (`None` when culling is inverted or there is no scene).
    pub roads: Option<FlatRoadsStep>,
    /// `drawScorches()` (`DO_SCORCH` is not defined upstream, so never).
    pub draw_scorches: bool,
    /// `m_bridgeBuffer->drawBridges(&rinfo.Camera, m_disableTextures, m_stageTwoTexture)`.
    pub draw_bridges: bool,
    /// `TheTerrainTracksRenderObjClassSystem->flush()` (when the system exists).
    pub flush_terrain_tracks: bool,
    /// `m_waypointBuffer->drawWaypoints(rinfo)`.
    pub draw_waypoints: bool,
    /// `m_bibBuffer->renderBibs()`.
    pub render_bibs: bool,
}

/// `class FlatHeightMapRenderObjClass`: custom W3D render object that draws the terrain as flat tiles at lower
/// resolution.
pub struct FlatHeightMapRenderObjClass<B: BaseHeightMapRenderObj> {
    /// The base class part.
    pub base: B,
    /// `m_tiles`.
    pub tiles: Vec<W3DTerrainBackground>,
    /// `m_numTiles`.
    pub num_tiles: i32,
    /// `m_tilesWidth`.
    pub tiles_width: i32,
    /// `m_tilesHeight`.
    pub tiles_height: i32,
    /// `m_updateState`.
    pub update_state: FlatUpdateState,
    /// The function-local `static Int prevCulled` of `updateCenter`.
    pub prev_culled: i32,
    /// The function-local `static Int prevT2X` of `updateCenter`.
    pub prev_t2x: i32,
    /// The function-local `static Int prevT4X` of `updateCenter`.
    pub prev_t4x: i32,
}

impl<B: BaseHeightMapRenderObj> FlatHeightMapRenderObjClass<B> {
    /// `FlatHeightMapRenderObjClass::FlatHeightMapRenderObjClass` (FlatHeightMap.cpp:121): mostly nulls out the member
    /// variables (`TheFlatHeightMap = this`: the owner keeps the single instance).
    pub fn new(base: B) -> Self {
        FlatHeightMapRenderObjClass {
            base,
            tiles: Vec::new(),
            tiles_width: 0,
            tiles_height: 0,
            num_tiles: 0,
            update_state: FlatUpdateState::Idle,
            prev_culled: 0,
            prev_t2x: 0,
            prev_t4x: 0,
        }
    }

    /// `FlatHeightMapRenderObjClass::~FlatHeightMapRenderObjClass` (FlatHeightMap.cpp:110): releases tiles.
    pub fn destroy(&mut self) {
        self.release_tiles();
    }

    /// `FlatHeightMapRenderObjClass::freeMapResources` (FlatHeightMap.cpp:96): frees the w3d resources used to draw the
    /// terrain.
    pub fn free_map_resources(&mut self) -> i32 {
        self.base.free_map_resources();

        0
    }

    /// `FlatHeightMapRenderObjClass::adjustTerrainLOD` (FlatHeightMap.cpp:136): adjust the terrain Level Of Detail.
    pub fn adjust_terrain_lod(&mut self, adj: i32) {
        self.base.adjust_terrain_lod(adj);
    }

    /// `FlatHeightMapRenderObjClass::ReleaseResources` (FlatHeightMap.cpp:146): releases all w3d assets, to prepare for
    /// Reset device call.
    pub fn release_resources(&mut self) {
        self.base.release_resources();
    }

    /// The tile (re)allocation `ReAcquireResources` and `initHeightData` share: a fresh `new W3DTerrainBackground[numTiles]`
    /// whose tiles are given their buffers and flip bits.
    fn allocate_tiles(&mut self, map: &SharedHeightMap, width: i32, height: i32) {
        let num_tiles = width * height;
        self.tiles = (0..num_tiles).map(|_| W3DTerrainBackground::new()).collect();
        self.num_tiles = num_tiles;
        self.tiles_width = width;
        self.tiles_height = height;
        for i in 0..self.tiles_width {
            for j in 0..self.tiles_height {
                let tile = &mut self.tiles[(j * self.tiles_width + i) as usize];
                tile.allocate_terrain_buffers(Some(map), i * CELLS_PER_TILE, j * CELLS_PER_TILE, CELLS_PER_TILE);
                tile.set_flip(Some(map));
            }
        }
    }

    /// `FlatHeightMapRenderObjClass::ReAcquireResources` (FlatHeightMap.cpp:156): reallocates all W3D assets after a
    /// reset.
    pub fn re_acquire_resources(
        &mut self,
        static_diffuse: &impl Fn(i32, i32) -> u32,
        get_flat_texture: &mut impl FnMut(i32, i32, i32, i32) -> Option<super::w3d_common::TextureHandle>,
    ) {
        if let Some(map) = self.base.map() {
            let (x_extent, y_extent) = {
                let m = map.read().unwrap();
                (m.width, m.height)
            };
            let width = (x_extent + CELLS_PER_TILE - 2) / CELLS_PER_TILE;
            let height = (y_extent + CELLS_PER_TILE - 2) / CELLS_PER_TILE;
            // (upstream does not delete the old m_tiles here: they leak)
            self.allocate_tiles(&map, width, height);
            let range = IRegion2D {
                lo: rts_lockstep_engine::common::base_type::ICoord2D { x: 0, y: 0 },
                hi: rts_lockstep_engine::common::base_type::ICoord2D { x: x_extent, y: y_extent },
            };
            for i in 0..self.tiles_width {
                for j in 0..self.tiles_height {
                    let tile = &mut self.tiles[(j * self.tiles_width + i) as usize];
                    tile.do_partial_update(&range, None, true, static_diffuse, get_flat_texture);
                }
            }
        }
        self.base.re_acquire_resources();
    }

    /// `FlatHeightMapRenderObjClass::reset` (FlatHeightMap.cpp:200): updates the macro noise/lightmap texture (pass 3).
    pub fn reset(&mut self) {
        self.base.reset();
    }

    /// `FlatHeightMapRenderObjClass::oversizeTerrain` (FlatHeightMap.cpp:209): sets the terrain oversize amount. Not
    /// needed with the flat version. [3/20/2003]
    pub fn oversize_terrain(&mut self, _tiles_to_oversize: i32) {}

    /// `FlatHeightMapRenderObjClass::updateBlock` (FlatHeightMap.h:76, inline).
    pub fn update_block(&mut self, _x0: i32, _y0: i32, _x1: i32, _y1: i32) -> i32 {
        0
    }

    /// `FlatHeightMapRenderObjClass::doPartialUpdate` (FlatHeightMap.cpp:226): updates a partial block of vertices from
    /// [x0,y0 to x1,y1]. The coordinates in partialRange are map cell coordinates, relative to the entire map. The vertex
    /// coordinates and texture coordinates, as well as static lighting are updated.
    pub fn do_partial_update(
        &mut self,
        partial_range: &IRegion2D,
        ht_map: Option<&SharedHeightMap>,
        static_diffuse: &impl Fn(i32, i32) -> u32,
        get_flat_texture: &mut impl FnMut(i32, i32, i32, i32) -> Option<super::w3d_common::TextureHandle>,
    ) {
        if let Some(m) = ht_map {
            self.base.set_map(Some(std::sync::Arc::clone(m)));
        }
        for i in 0..self.tiles_width {
            for j in 0..self.tiles_height {
                let tile = &mut self.tiles[(j * self.tiles_width + i) as usize];
                tile.do_partial_update(partial_range, ht_map, true, static_diffuse, get_flat_texture);
            }
        }
    }

    /// `FlatHeightMapRenderObjClass::releaseTiles` (FlatHeightMap.cpp:244): releases tiles.
    pub fn release_tiles(&mut self) {
        if !self.tiles.is_empty() {
            for t in self.tiles.iter_mut() {
                t.destroy();
            }
            self.tiles = Vec::new();
        }
        self.tiles_width = 0;
        self.tiles_height = 0;
        self.num_tiles = 0;
    }

    /// `FlatHeightMapRenderObjClass::initHeightData` (FlatHeightMap.cpp:263): allocate a heightmap of x by y vertices and
    /// fill with initial height values. Also allocates all rendering resources such as vertex buffers, index buffers,
    /// shaders, and materials.
    pub fn init_height_data(
        &mut self,
        x: i32,
        y: i32,
        p_map: &SharedHeightMap,
        static_diffuse: &impl Fn(i32, i32) -> u32,
        get_flat_texture: &mut impl FnMut(i32, i32, i32, i32) -> Option<super::w3d_common::TextureHandle>,
    ) -> i32 {
        self.base.init_height_data(x, y, p_map);

        let (x_extent, y_extent) = {
            let m = p_map.read().unwrap();
            (m.width, m.height)
        };
        let width = (x_extent + CELLS_PER_TILE - 2) / CELLS_PER_TILE;
        let height = (y_extent + CELLS_PER_TILE - 2) / CELLS_PER_TILE;

        p_map.write().map(|mut m| super::w3d_terrain_background::clear_flip_states(&mut m)).unwrap();
        if !self.tiles.is_empty() && self.tiles_width == width && self.tiles_height == height {
            // current allocation matches. Just redo vertex & index buffers. [3/21/2003]
            for i in 0..self.tiles_width {
                for j in 0..self.tiles_height {
                    let tile = &mut self.tiles[(j * self.tiles_width + i) as usize];
                    tile.set_flip(Some(p_map));
                }
            }
        } else {
            self.release_tiles();
            self.allocate_tiles(p_map, width, height);
        }
        let range = IRegion2D {
            lo: rts_lockstep_engine::common::base_type::ICoord2D { x: 0, y: 0 },
            hi: rts_lockstep_engine::common::base_type::ICoord2D { x: x_extent, y: y_extent },
        };
        for i in 0..self.tiles_width {
            for j in 0..self.tiles_height {
                let tile = &mut self.tiles[(j * self.tiles_width + i) as usize];
                tile.do_partial_update(&range, Some(p_map), true, static_diffuse, get_flat_texture);
            }
        }
        0
    }

    /// `FlatHeightMapRenderObjClass::On_Frame_Update` (FlatHeightMap.cpp:338): updates the diffuse color values in the
    /// vertices as affected by the dynamic lights.
    pub fn on_frame_update(&mut self, get_flat_texture: &mut impl FnMut(i32, i32, i32, i32) -> Option<super::w3d_common::TextureHandle>) {
        self.base.on_frame_update();

        match self.update_state {
            FlatUpdateState::Idle => {}
            FlatUpdateState::Moving => {
                self.update_state = FlatUpdateState::Moving2;
            }
            FlatUpdateState::Moving2 => {
                for i in 0..self.tiles_width {
                    for j in 0..self.tiles_height {
                        let tile = &mut self.tiles[(j * self.tiles_width + i) as usize];
                        tile.update_texture(get_flat_texture);
                    }
                }
                self.update_state = FlatUpdateState::Idle;
            }
            // (no case for STATE_UPDATE_TEXTURES: the switch falls out)
            FlatUpdateState::UpdateTextures => {}
        }
    }

    /// `FlatHeightMapRenderObjClass::staticLightingChanged` (FlatHeightMap.cpp:367): notification that all lighting needs
    /// to be recalculated.
    pub fn static_lighting_changed(
        &mut self,
        static_diffuse: &impl Fn(i32, i32) -> u32,
        get_flat_texture: &mut impl FnMut(i32, i32, i32, i32) -> Option<super::w3d_common::TextureHandle>,
    ) {
        self.base.static_lighting_changed();
        let map = match self.base.map() {
            Some(m) => m,
            None => return,
        };
        let bounds = IRegion2D {
            lo: rts_lockstep_engine::common::base_type::ICoord2D { x: 0, y: 0 },
            hi: rts_lockstep_engine::common::base_type::ICoord2D { x: self.tiles_width * CELLS_PER_TILE, y: self.tiles_height * CELLS_PER_TILE },
        };
        for i in 0..self.tiles_width {
            for j in 0..self.tiles_height {
                let tile = &mut self.tiles[(j * self.tiles_width + i) as usize];
                tile.do_partial_update(&bounds, Some(&map), true, static_diffuse, get_flat_texture);
            }
        }
    }

    /// `FlatHeightMapRenderObjClass::updateCenter` (FlatHeightMap.cpp:392): updates the positioning of the drawn portion of
    /// the height map in the heightmap. As the view slides around, this determines what is the actually rendered portion
    /// of the terrain. `cull_box` is `camera->Cull_Box`, `camera_pos` is `camera->Get_Position()`.
    pub fn update_center(&mut self, cull_box: &impl Fn(&AABoxClass) -> bool, camera_pos: &Vector3) {
        self.base.update_center();
        self.base.set_need_full_update(false);
        let mut culled = 0;
        let mut t2x = 0;
        let mut t4x = 0;
        for i in 0..self.tiles_width {
            for j in 0..self.tiles_height {
                let tile = &mut self.tiles[(j * self.tiles_width + i) as usize];
                tile.update_center(cull_box, camera_pos);
                if tile.is_culled() {
                    culled += 1;
                }
                let tx = tile.get_tex_multiplier();
                if tx == 4 {
                    t4x += 1;
                } else if tx == 2 {
                    t2x += 1;
                }
            }
        }
        if culled != self.prev_culled || t4x != self.prev_t4x || t2x != self.prev_t2x {
            // DEBUG_LOG(("%d of %d culled, %d 4X, %d 2X.\n", culled, m_numTiles, t4X, t2X));
            self.prev_culled = culled;
            self.prev_t2x = t2x;
            self.prev_t4x = t4x;
        }
        self.update_state = FlatUpdateState::Moving;
    }

    /// `FlatHeightMapRenderObjClass::Render` (FlatHeightMap.cpp:455): renders (draws) the terrain. `shader_passes` is
    /// `W3DShaderManager::getShaderPasses(st)`. `has_tree_buffer`, `has_scene` and `has_terrain_tracks` say whether the
    /// base's tree buffer, the render object's scene and `TheTerrainTracksRenderObjClassSystem` exist.
    pub fn render(
        &mut self,
        input: &FlatRenderInput,
        has_tree_buffer: bool,
        has_scene: bool,
        has_terrain_tracks: bool,
        shader_passes: &impl Fn(ShaderTypes) -> i32,
    ) -> FlatRenderPlan {
        let mut do_cloud = input.use_cloud_map;

        // If there are trees, tell them to draw at the transparent time to draw.
        // (m_treeBuffer->setIsTerrain())

        // upstream: Set_Light_Environment, m_stageTwoTexture->restore(), Set_Texture(0/1, NULL), ShaderClass::Invalidate,
        // Set_Transform(WORLD, tm), Set_Material(m_vertexMaterialClass), Set_Shader(m_shaderClass) — device-only

        if input.night {
            do_cloud = false;
        }

        let mut st = ShaderTypes::FlatTerrainBase; // set default shader

        // set correct shader based on current settings
        if input.use_light_map && do_cloud {
            st = ShaderTypes::FlatTerrainBaseNoise12;
        } else if input.use_light_map {
            // lightmap only
            st = ShaderTypes::FlatTerrainBaseNoise2;
        } else if do_cloud {
            // cloudmap only
            st = ShaderTypes::FlatTerrainBaseNoise1;
        }

        // Find number of passes required to render current shader
        let mut device_passes = shader_passes(st);

        if input.disable_textures {
            device_passes = 1; // force to 1 lighting-only pass
        }

        // Specify all textures that this shader may need:
        // setTexture(0, m_stageZeroTexture) (or the shroud texture), setTexture(1, NULL) set by the tile later,
        // setTexture(2, m_stageTwoTexture) cloud, setTexture(3, m_stageThreeTexture) noise
        let stage0_is_shroud = input.shroud && input.additional_pass_count && !input.disable_textures;
        // Disable writes to destination alpha channel (if there is one)

        let mut tiles = Vec::new();
        let mut pass: i32 = 0;
        let mut y_coord_max: i32 = 0;
        let mut y_coord_min: i32 = input.map_x_extent;
        let mut x_coord_max: i32 = 0;
        let mut x_coord_min: i32 = input.map_y_extent;
        while pass < device_passes {
            let mut disable_tex = input.disable_textures;
            // if m_disableTextures: Set_Shader(_PresetOpaque2DShader), Set_Texture(0, NULL); else setShader(st, pass)

            for i in 0..self.tiles_width {
                for j in 0..self.tiles_height {
                    let tile = &self.tiles[(j * self.tiles_width + i) as usize];
                    if pass > 0 {
                        disable_tex = true; // doing cloud/noise
                    }
                    if !tile.is_culled() {
                        let draw = tile.draw_visible_polys(disable_tex);
                        tiles.push(FlatTileDraw { pass, i, j, disable_tex, draw });
                        if i * CELLS_PER_TILE < x_coord_min {
                            x_coord_min = i * CELLS_PER_TILE;
                        }
                        if j * CELLS_PER_TILE < y_coord_min {
                            y_coord_min = j * CELLS_PER_TILE;
                        }
                        if (i + 1) * CELLS_PER_TILE > x_coord_max {
                            x_coord_max = (i + 1) * CELLS_PER_TILE;
                        }
                        if (j + 1) * CELLS_PER_TILE > y_coord_max {
                            y_coord_max = (j + 1) * CELLS_PER_TILE;
                        }
                    }
                }
            }
            pass += 1;
        }

        let reset_shader = pass != 0; // shader was applied at least once?

        // Draw feathered shorelines: renderShoreLines(&rinfo.Camera)

        // DO_ROADS: set textures NULL, m_stageTwoTexture->restore(), ShaderClass::Invalidate()
        let mut roads = None;
        if !input.backface_culling_inverted {
            // Set_Material(m_vertexMaterialClass); if (Scene) drawRoads(...)
            if has_scene {
                roads = Some(FlatRoadsStep {
                    use_cloud_texture: do_cloud,
                    use_noise_texture: input.use_light_map,
                    disable_textures: input.disable_textures,
                    x_min: x_coord_min - input.border_size,
                    x_max: x_coord_max - input.border_size,
                    y_min: y_coord_min - input.border_size,
                    y_max: y_coord_max - input.border_size,
                });
            }
        }

        // DO_SCORCH is not defined upstream: drawScorches() is not compiled.

        // Set_Texture(0/1, NULL), m_stageTwoTexture->restore(), ShaderClass::Invalidate(), Apply_Render_State_Changes —
        // device-only

        // m_bridgeBuffer->drawBridges, terrain tracks flush, ShaderClass::Invalidate, Apply_Render_State_Changes,
        // m_waypointBuffer->drawWaypoints(rinfo), m_bibBuffer->renderBibs()

        // We do some custom blending, so tell the shader class to reset everything: Set_Texture(0/1, NULL),
        // m_stageTwoTexture->restore(), ShaderClass::Invalidate(), Set_Material(NULL) — device-only

        FlatRenderPlan {
            tree_buffer_set_is_terrain: has_tree_buffer,
            shader: st,
            device_passes,
            stage0_is_shroud,
            mask_destination_alpha: input.back_buffer_a8r8g8b8,
            tiles,
            reset_shader,
            render_shore_lines: true,
            roads,
            draw_scorches: false,
            draw_bridges: true,
            flush_terrain_tracks: has_terrain_tracks,
            draw_waypoints: true,
            render_bibs: true,
        }
    }
}
