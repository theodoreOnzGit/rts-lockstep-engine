//! UNTESTED DRAFT (#202, #204): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DTreeBuffer.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DTreeBuffer.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DTreeDraw.h (`W3DTreeDrawModuleData`, the
//!     data members only: the INI parse table and `W3DTreeDraw` itself belong to the draw module)
//!
//! Design as in `w3d_bib_buffer.rs`: the DX8 vertex and index buffers are `Vec`s of upstream's sizes
//! (`MAX_TREE_VERTEX + 4`, `MAX_TREE_INDEX + 4`); textures are the CPU-side atlas `Vec<u32>` (A8R8G8B8) the renderer
//! uploads; the draw is returned as data (`TreeDraw`: the index/vertex range and the vertex shader constants); the
//! asset manager, the camera, the script engine's breeze, the game frame, the partition manager's shroud query and the
//! file system are parameters. `FXList::doFXPos` calls are queued in `pending_fx` for the caller to play.
//!
//! Notes on upstream behaviour kept as it is:
//! - The constructor's `memset(this, sizeof(W3DTreeBuffer), 0)` has its arguments swapped (it fills zero bytes with the
//!   value `sizeof`), so it clears nothing; every member the constructor does not set is uninitialised memory upstream
//!   and is zero here.
//! - `addTreeType` returns 0, never a negative number, when the model is missing, so `addTree`'s `treeType<0` check
//!   after it is dead; the tree is then given type 0 whatever that is. Kept.
//! - `swayFactor[i]` is left uninitialised upstream when `minOffset` is out of range; it is zero here.
//! - `fabs` (not `abs`) is used on the angular velocity, so it is the double overload: no int conversion.

use super::w3d_common::{AABoxClass, DrawTriangles, SphereClass, VertexFormatXYZNDUV1};
use rts_lockstep_engine::client::terrain_height_map::{
    count_tiles, read_tiles, TileData, TERRAIN_TGA_DIR_PATH, TILE_BYTES_PER_PIXEL, TILE_PIXEL_EXTENT,
};
use rts_lockstep_engine::common::base_type::{real_to_int, real_to_int_ceil, real_to_int_floor, real_to_unsigned_int, Coord2D, Coord3D, ICoord2D, Region2D, PI};
use rts_lockstep_engine::common::game_common::ObjectShroudStatus;
use rts_lockstep_engine::common::game_type::{DrawableId, ObjectId, INVALID_ID};
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::global_data::{TerrainLighting, MAX_GLOBAL_LIGHTS};
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::xfer::{Xfer, XferMode, XferResult, XferVersion};
use rts_lockstep_engine::logic::world::partition::collide::geom_collides_with_geom;
use rts_lockstep_engine::logic::world::script_engine::BreezeInfo;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::wwmath;

/// `W3D_TOPPLE_OPTIONS_NONE`.
pub const W3D_TOPPLE_OPTIONS_NONE: u32 = 0x0000_0000;
/// `W3D_TOPPLE_OPTIONS_NO_BOUNCE`: do not bounce when hit the ground.
pub const W3D_TOPPLE_OPTIONS_NO_BOUNCE: u32 = 0x0000_0001;
/// `W3D_TOPPLE_OPTIONS_NO_FX`: do not play any FX when hit the ground.
pub const W3D_TOPPLE_OPTIONS_NO_FX: u32 = 0x0000_0002;

/// `END_OF_PARTITION`.
pub const END_OF_PARTITION: i32 = -1;
/// `DELETED_TREE_TYPE`.
pub const DELETED_TREE_TYPE: i32 = -2;
/// `MAX_TREE_VERTEX`.
pub const MAX_TREE_VERTEX: i32 = 30000;
/// `MAX_TREE_INDEX`.
pub const MAX_TREE_INDEX: i32 = 60000;
/// `MAX_TREES`.
pub const MAX_TREES: usize = 4000;
/// `MAX_TYPES`.
pub const MAX_TYPES: usize = 64;
/// `MAX_TILES`.
pub const MAX_TILES: usize = 512;
/// `NUM_SWAY_ENTRIES`.
pub const NUM_SWAY_ENTRIES: usize = 100;
/// `MAX_SWAY_TYPES`.
pub const MAX_SWAY_TYPES: usize = 10;
/// `MAX_BUFFERS`.
pub const MAX_BUFFERS: usize = 1;
/// `SORT_ITERATIONS_PER_FRAME`.
pub const SORT_ITERATIONS_PER_FRAME: i32 = 10;
/// `PARTITION_WIDTH_HEIGHT`.
pub const PARTITION_WIDTH_HEIGHT: i32 = 100;
/// `TREE_RADIUS_APPROX`: value to assume for the tree radius.
pub const TREE_RADIUS_APPROX: f32 = 7.0;
/// `ANGULAR_LIMIT`: this is our "bounce" limit -- slightly less that 90 degrees, to account for slop.
pub const ANGULAR_LIMIT: f32 = PI / 2.0 - PI / 64.0;

/// `TGA_DIR_PATH` (FileSystem.h:76).
pub const TGA_DIR_PATH: &[u8] = b"Art/Textures/";

/// `enum W3DToppleState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum W3DToppleState {
    /// `TOPPLE_UPRIGHT`.
    Upright = 0,
    /// `TOPPLE_FALLING`.
    Falling = 1,
    /// `TOPPLE_FOGGED`.
    Fogged = 2,
    /// `TOPPPLE_SHROUDED` (upstream's spelling).
    Shrouded = 3,
    /// `TOPPLE_DOWN`.
    Down = 4,
}

impl W3DToppleState {
    /// The enum from the `int` `xferUser` reads; an out-of-range value is undefined upstream and reads as upright.
    fn from_i32(v: i32) -> W3DToppleState {
        match v {
            1 => W3DToppleState::Falling,
            2 => W3DToppleState::Fogged,
            3 => W3DToppleState::Shrouded,
            4 => W3DToppleState::Down,
            _ => W3DToppleState::Upright,
        }
    }
}

/// `class W3DTreeDrawModuleData` (W3DTreeDraw.h): the data members `W3DTreeBuffer` reads. The `FXList` pointers are the
/// lists' names; the caller plays them.
#[derive(Clone, Debug, Default)]
pub struct W3DTreeDrawModuleData {
    /// `m_modelName`.
    pub model_name: Vec<u8>,
    /// `m_textureName`.
    pub texture_name: Vec<u8>,
    /// `m_framesToMoveOutward`: push aside parameters.
    pub frames_to_move_outward: u32,
    /// `m_framesToMoveInward`.
    pub frames_to_move_inward: u32,
    /// `m_maxOutwardMovement`.
    pub max_outward_movement: f32,
    /// `m_darkening`.
    pub darkening: f32,
    /// `m_toppleFX`: name of the `FXList`, empty for NULL.
    pub topple_fx: Vec<u8>,
    /// `m_bounceFX`: name of the `FXList`, empty for NULL.
    pub bounce_fx: Vec<u8>,
    /// `m_stumpName`.
    pub stump_name: Vec<u8>,
    /// `m_initialVelocityPercent`.
    pub initial_velocity_percent: f32,
    /// `m_initialAccelPercent`.
    pub initial_accel_percent: f32,
    /// `m_bounceVelocityPercent`.
    pub bounce_velocity_percent: f32,
    /// `m_minimumToppleSpeed`.
    pub minimum_topple_speed: f32,
    /// `m_killWhenToppled`.
    pub kill_when_toppled: bool,
    /// `m_doTopple`.
    pub do_topple: bool,
    /// `m_sinkFrames`: how long it takes to sink after toppling.
    pub sink_frames: u32,
    /// `m_sinkDistance`: how far it sinks.
    pub sink_distance: f32,
    /// `m_doShadow`.
    pub do_shadow: bool,
}

/// `TTree`: the individual data for a tree.
#[derive(Clone, Debug)]
pub struct TTree {
    /// `location`: drawing location.
    pub location: Vector3,
    /// `scale`: scale at location.
    pub scale: f32,
    /// `sin`: sine of the rotation angle at location.
    pub sin: f32,
    /// `cos`: cosine of the rotation angle at location.
    pub cos: f32,
    /// `treeType`: type of tree (index into `m_treeTypes`, or `DELETED_TREE_TYPE`).
    pub tree_type: i32,
    /// `visible`: visible flag, updated each frame.
    pub visible: bool,
    /// `bounds`: bounding sphere for culling to set the visible flag.
    pub bounds: SphereClass,
    /// `sortKey`: essentially the distance along the look at vector.
    pub sort_key: f32,
    /// `drawableID`: drawable this tree corresponds to.
    pub drawable_id: DrawableId,
    /// `pushAside`.
    pub push_aside: f32,
    /// `pushAsideDelta`.
    pub push_aside_delta: f32,
    /// `pushAsideSin`.
    pub push_aside_sin: f32,
    /// `pushAsideCos`.
    pub push_aside_cos: f32,
    /// `pushAsideSource`: unit that is pushing us aside.
    pub push_aside_source: ObjectId,
    /// `lastFrameUpdated`: last frame push aside was updated.
    pub last_frame_updated: u32,
    /// `nextInPartition`.
    pub next_in_partition: i32,
    /// `swayType`: which sway array entry we are using.
    pub sway_type: i32,
    /// `firstIndex`: first index in the vertex buffer for this tree.
    pub first_index: i32,
    /// `bufferNdx`: which vertex buffer this is in.
    pub buffer_ndx: i32,
    /// `m_angularVelocity`.
    pub angular_velocity: f32,
    /// `m_angularAcceleration`.
    pub angular_acceleration: f32,
    /// `m_toppleDirection`: z-less direction we are toppling.
    pub topple_direction: Coord3D,
    /// `m_toppleState`.
    pub topple_state: W3DToppleState,
    /// `m_angularAccumulation`: how much have I rotated so I know when to bounce.
    pub angular_accumulation: f32,
    /// `m_options`: topple options.
    pub options: u32,
    /// `m_mtx`.
    pub mtx: Matrix3D,
    /// `m_sinkFramesLeft`: toppled trees sink into the terrain & disappear, how many frames left.
    pub sink_frames_left: u32,
}

impl Default for TTree {
    fn default() -> Self {
        TTree {
            location: Vector3::default(),
            scale: 0.0,
            sin: 0.0,
            cos: 0.0,
            tree_type: 0,
            visible: false,
            bounds: SphereClass::default(),
            sort_key: 0.0,
            drawable_id: 0,
            push_aside: 0.0,
            push_aside_delta: 0.0,
            push_aside_sin: 0.0,
            push_aside_cos: 0.0,
            push_aside_source: 0,
            last_frame_updated: 0,
            next_in_partition: 0,
            sway_type: 0,
            first_index: 0,
            buffer_ndx: 0,
            angular_velocity: 0.0,
            angular_acceleration: 0.0,
            topple_direction: Coord3D::default(),
            topple_state: W3DToppleState::Upright,
            angular_accumulation: 0.0,
            options: 0,
            mtx: Matrix3D::default(),
            sink_frames_left: 0,
        }
    }
}

/// What `MeshClass` hands the tree buffer: the model (`Peek_Model()`), its vertex colours (`Get_Color_Array(0, false)`,
/// `None` for NULL) and the first vertex material's emissive colour (`MaterialInfoClass::Peek_Vertex_Material(0)` then
/// `VertexMaterialClass::Get_Emissive`; zero when there is no material). `model.normals` empty is upstream's NULL
/// `Get_Vertex_Normal_Array`.
#[derive(Clone, Debug, Default)]
pub struct TreeMesh {
    /// `Peek_Model()`.
    pub model: super::w3d_common::MeshModelData,
    /// `Get_Color_Array(0, false)`.
    pub colors: Option<Vec<u32>>,
    /// `Get_Emissive`.
    pub emissive: Vector3,
}

/// `TTreeType`: the individual data for a tree type.
#[derive(Clone, Debug, Default)]
pub struct TTreeType {
    /// `m_mesh`: mesh for this kind of tree.
    pub mesh: Option<TreeMesh>,
    /// `m_bounds`: bounding sphere for the base tree models.
    pub bounds: SphereClass,
    /// `m_data`.
    pub data: Option<W3DTreeDrawModuleData>,
    /// `m_textureOrigin`: texture origin in the mega texture.
    pub texture_origin: ICoord2D,
    /// `m_numTiles`: number of tex tiles.
    pub num_tiles: i32,
    /// `m_firstTile`: first texture tile.
    pub first_tile: i32,
    /// `m_tileWidth`: width in tiles of texture.
    pub tile_width: i32,
    /// `m_halfTile`: tiles are 64x64 pixels, half tile supports a 32x32 bit texture. Have to adjust the uv values.
    pub half_tile: bool,
    /// `m_offset`.
    pub offset: Vector3,
    /// `m_shadowSize`: shadow radius.
    pub shadow_size: f32,
    /// `m_doShadow`: draw shadow.
    pub do_shadow: bool,
}

/// The result of `AssetManager::Create_Render_Obj(modelName)` as `addTreeType` reads it: the object's object-space
/// bounding box, the translation of its first sub object's bone 0 when it is an HLOD (`offset`, zero otherwise), and the
/// mesh when it is (or its sub object 0 is) a `CLASSID_MESH`, `None` when it is not a simple mesh.
#[derive(Clone, Debug, Default)]
pub struct TreeModelLoad {
    /// `Get_Obj_Space_Bounding_Box`.
    pub obj_box: AABoxClass,
    /// The HLOD's `Get_Bone_Transform(0).Get_Translation`.
    pub offset: Vector3,
    /// The mesh, `None` for a model that is not a simple mesh.
    pub mesh: Option<TreeMesh>,
}

/// The tree texture (`m_treeTexture`).
#[derive(Clone, Debug)]
pub enum TreeTexture {
    /// `new TextureClass("missing.tga")`: the fallback when there are too many trees in a scene.
    Missing,
    /// `W3DTreeTextureClass`: `width * height` A8R8G8B8 words, row 0 first. D3DX's box-filtered mip chain
    /// (`D3DXFilterTexture`) and `SetLOD` are the renderer's.
    Atlas {
        /// Width in pixels.
        width: i32,
        /// Height in pixels.
        height: i32,
        /// Level 0.
        pixels: Vec<u32>,
        /// `setLOD` (`m_textureReductionFactor` at creation, or `setTextureLOD`), `None` while not set.
        lod: Option<i32>,
    },
}

/// One `FXList::doFXPos(fx, &pos)` the buffer asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeFxRequest {
    /// The `FXList`'s name.
    pub fx_name: Vec<u8>,
    /// The position.
    pub pos: Coord3D,
}

/// One `TheW3DProjectedShadowManager->queueDecal(m_shadow)` with the `setSize` and `setPosition` before it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeShadowDecal {
    /// `setSize(width, height)`: width.
    pub size_x: f32,
    /// `setSize(width, height)`: height (negative).
    pub size_y: f32,
    /// `setPosition`: x.
    pub x: f32,
    /// `setPosition`: y.
    pub y: f32,
    /// `setPosition`: z.
    pub z: f32,
}

/// What `drawTrees` reads from `TheTerrainRenderObject->getShroud()`.
#[derive(Clone, Copy, Debug, Default)]
pub struct TreeShroudInfo {
    /// `getCellWidth`.
    pub cell_width: f32,
    /// `getCellHeight`.
    pub cell_height: f32,
    /// `getDrawOriginX`.
    pub draw_origin_x: f32,
    /// `getDrawOriginY`.
    pub draw_origin_y: f32,
    /// `getTextureWidth`.
    pub texture_width: f32,
    /// `getTextureHeight`.
    pub texture_height: f32,
}

/// The per-frame inputs of `drawTrees` that upstream reads from singletons.
#[derive(Clone, Debug, Default)]
pub struct TreeFrameInfo {
    /// `TheScriptEngine->getBreezeInfo()`.
    pub breeze: BreezeInfo,
    /// `TheScriptEngine->isTimeFrozenScript() || isTimeFrozenDebug()`.
    pub time_frozen: bool,
    /// `TheGameLogic && TheGameLogic->isGamePaused()`.
    pub game_paused: bool,
    /// `TheGameLogic->getFrame()` (used by `pushAsideTree`).
    pub game_frame: u32,
    /// `TheGlobalData->m_useShadowDecals`.
    pub use_shadow_decals: bool,
    /// `TheW3DProjectedShadowManager != NULL`.
    pub shadow_manager_present: bool,
    /// `ThePlayerList ? ThePlayerList->getLocalPlayer()->getPlayerIndex() : 0`.
    pub local_player_index: i32,
    /// `TheGlobalData->m_terrainObjectsLighting[TheGlobalData->m_timeOfDay]`.
    pub object_lighting: [TerrainLighting; MAX_GLOBAL_LIGHTS],
    /// `TheGlobalData->m_textureReductionFactor`.
    pub texture_reduction_factor: i32,
    /// `TheTerrainRenderObject->getShroud()`, `None` for NULL.
    pub shroud: Option<TreeShroudInfo>,
}

/// What `drawTrees` would set and draw after all the CPU work: the vertex shader constants and the
/// `Draw_Triangles(0, indices/3, 0, vertices)` (`None` when `m_curNumTreeIndices[0] == 0`). The world-view-projection
/// matrix (c4) is the renderer's.
#[derive(Clone, Debug, Default)]
pub struct TreeDraw {
    /// `Draw_Triangles` of buffer 0.
    pub draw: Option<DrawTriangles>,
    /// c8: `noSway`.
    pub no_sway: [f32; 4],
    /// c9 .. c9+MAX_SWAY_TYPES: the sway amounts.
    pub sway: [[f32; 4]; MAX_SWAY_TYPES],
    /// c32: the shroud offset.
    pub shroud_offset: [f32; 4],
    /// c33: the shroud scale.
    pub shroud_scale: [f32; 4],
    /// The tree shadow decals queued this frame (then `flushDecals`).
    pub shadow_decals: Vec<TreeShadowDecal>,
}

/// `class W3DTreeBuffer`: draw buffer for the trees.
pub struct W3DTreeBuffer {
    /// `m_vertexTree[MAX_BUFFERS]`: `MAX_TREE_VERTEX + 4` vertices each.
    pub vertex_tree: Vec<Option<Vec<VertexFormatXYZNDUV1>>>,
    /// `m_indexTree[MAX_BUFFERS]`: `MAX_TREE_INDEX + 4` indices each.
    pub index_tree: Vec<Option<Vec<u16>>>,
    /// `m_dwTreePixelShader`, `m_dwTreeVertexShader`: whether `Trees.vso`/`Trees.pso` were created (the renderer's).
    pub tree_vertex_shader_loaded: bool,
    /// `m_areaPartition[PARTITION_WIDTH_HEIGHT*PARTITION_WIDTH_HEIGHT]`.
    pub area_partition: Vec<i16>,
    /// `m_bounds`.
    pub bounds: Region2D,
    /// `m_treeTexture`.
    pub tree_texture: Option<TreeTexture>,
    /// `m_textureWidth`.
    pub texture_width: i32,
    /// `m_textureHeight`.
    pub texture_height: i32,
    /// `m_curNumTreeVertices[MAX_BUFFERS]`.
    pub cur_num_tree_vertices: [i32; MAX_BUFFERS],
    /// `m_curNumTreeIndices[MAX_BUFFERS]`.
    pub cur_num_tree_indices: [i32; MAX_BUFFERS],
    /// `m_trees[MAX_TREES]`.
    pub trees: Vec<TTree>,
    /// `m_numTrees`.
    pub num_trees: i32,
    /// `m_anythingChanged`.
    pub anything_changed: bool,
    /// `m_anyPushChanged`.
    pub any_push_changed: bool,
    /// `m_updateAllKeys`.
    pub update_all_keys: bool,
    /// `m_initialized`.
    pub initialized: bool,
    /// `m_isTerrainPass`.
    pub is_terrain_pass: bool,
    /// `m_needToUpdateTexture`.
    pub need_to_update_texture: bool,
    /// `m_treeTypes[MAX_TYPES]`.
    pub tree_types: Vec<TTreeType>,
    /// `m_numTreeTypes`.
    pub num_tree_types: i32,
    /// `m_numTiles`.
    pub num_tiles: i32,
    /// `m_sourceTiles[MAX_TILES]`.
    pub source_tiles: Vec<Option<TileData>>,
    /// `m_cameraLookAtVector`.
    pub camera_look_at_vector: Vector3,
    /// `m_swayOffsets[NUM_SWAY_ENTRIES]`.
    pub sway_offsets: Vec<Vector3>,
    /// `m_curSwayVersion`.
    pub cur_sway_version: i32,
    /// `m_curSwayOffset[MAX_SWAY_TYPES]`.
    pub cur_sway_offset: [f32; MAX_SWAY_TYPES],
    /// `m_curSwayStep[MAX_SWAY_TYPES]`.
    pub cur_sway_step: [f32; MAX_SWAY_TYPES],
    /// `m_curSwayFactor[MAX_SWAY_TYPES]`.
    pub cur_sway_factor: [f32; MAX_SWAY_TYPES],
    /// `m_shadow`: the decal shadow `createDecalShadow` made (the shadow manager's handle), `None` for NULL.
    pub shadow: Option<u32>,
    /// The `FXList::doFXPos` calls made since the caller last drained this.
    pub pending_fx: Vec<TreeFxRequest>,
}

/// `stricmp(a, b) == 0` (`AsciiString::compareNoCase`) on byte strings.
fn eq_no_case(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

impl Default for W3DTreeBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DTreeBuffer {
    /// `W3DTreeBuffer::W3DTreeBuffer` (W3DTreeBuffer.cpp:1190). `Trees.vso` loading
    /// (`W3DShaderManager::LoadAndCreateD3DShader`) is the renderer's.
    pub fn new() -> Self {
        let mut b = W3DTreeBuffer {
            vertex_tree: vec![None; MAX_BUFFERS],
            index_tree: vec![None; MAX_BUFFERS],
            tree_vertex_shader_loaded: false,
            area_partition: vec![END_OF_PARTITION as i16; (PARTITION_WIDTH_HEIGHT * PARTITION_WIDTH_HEIGHT) as usize],
            bounds: Region2D::default(),
            tree_texture: None,
            texture_width: 0,
            texture_height: 0,
            cur_num_tree_vertices: [0; MAX_BUFFERS],
            cur_num_tree_indices: [0; MAX_BUFFERS],
            trees: vec![TTree::default(); MAX_TREES],
            num_trees: 0,
            anything_changed: false,
            any_push_changed: false,
            update_all_keys: false,
            initialized: false,
            is_terrain_pass: false,
            need_to_update_texture: false,
            tree_types: vec![TTreeType::default(); MAX_TYPES],
            num_tree_types: 0,
            num_tiles: 0,
            source_tiles: vec![None; MAX_TILES],
            camera_look_at_vector: Vector3::default(),
            sway_offsets: vec![Vector3::default(); NUM_SWAY_ENTRIES],
            cur_sway_version: 0,
            cur_sway_offset: [0.0; MAX_SWAY_TYPES],
            cur_sway_step: [0.0; MAX_SWAY_TYPES],
            cur_sway_factor: [0.0; MAX_SWAY_TYPES],
            shadow: None,
            pending_fx: Vec::new(),
        };
        // memset(this, sizeof(W3DTreeBuffer), 0) clears nothing (arguments swapped): see the module notes.
        b.initialized = false;
        for i in 0..MAX_BUFFERS {
            b.vertex_tree[i] = None;
            b.index_tree[i] = None;
            b.cur_num_tree_vertices[i] = 0;
            b.cur_num_tree_indices[i] = 0;
        }
        b.tree_texture = None;
        b.tree_vertex_shader_loaded = false;
        b.clear_all_trees();
        b.allocate_tree_buffers();
        b.initialized = true;
        b.cur_sway_version = -1;
        b.shadow = None;
        b
    }

    /// `W3DTreeBuffer::~W3DTreeBuffer` (W3DTreeBuffer.cpp:1167): releases the buffers, the texture and the meshes, and
    /// deletes the shadow (the shadow manager's `delete`: the caller destroys the handle `shadow` held).
    pub fn destroy(&mut self) {
        self.free_tree_buffers();
        self.tree_texture = None;
        for i in 0..MAX_TYPES {
            self.tree_types[i].mesh = None;
        }
        self.shadow = None;
    }

    /// `W3DTreeBuffer::setBounds` (W3DTreeBuffer.h).
    pub fn set_bounds(&mut self, bounds: &Region2D) {
        self.bounds = *bounds;
    }

    /// `W3DTreeBuffer::doFullUpdate` (W3DTreeBuffer.h): called when the view changes, and sort key needs to be
    /// recalculated.
    pub fn do_full_update(&mut self) {
        self.update_all_keys = true;
    }

    /// `W3DTreeBuffer::setIsTerrain` (W3DTreeBuffer.h): terrain calls this to tell trees to draw.
    pub fn set_is_terrain(&mut self) {
        self.is_terrain_pass = true;
    }

    /// `W3DTreeBuffer::needToDraw` (W3DTreeBuffer.h).
    pub fn need_to_draw(&self) -> bool {
        self.is_terrain_pass
    }

    /// `W3DTreeBuffer::getNumTiles` (W3DTreeBuffer.h).
    pub fn get_num_tiles(&self) -> i32 {
        self.num_tiles
    }

    /// `W3DTreeBuffer::getSourceTile` (W3DTreeBuffer.h).
    pub fn get_source_tile(&self, ndx: i32) -> Option<&TileData> {
        self.source_tiles[ndx as usize].as_ref()
    }

    /// `W3DTreeBuffer::W3DTreeTextureClass::update` (W3DTreeBuffer.cpp:96): puts the tiles into an A8R8G8B8 atlas of
    /// `width x width` pixels, the tiles placed at their `m_tileLocationInTexture`, rows inverted. Returns the texture
    /// and its height (`surface_desc.Height`). The surface lock, `D3DXFilterTexture` (box-filtered mips) and `SetLOD` of
    /// `texture_reduction_factor` are the renderer's; the U and V address modes are clamp.
    pub fn tree_texture_update(&self, width: i32, texture_reduction_factor: i32) -> (TreeTexture, i32) {
        let height = width;
        let mut pixels = vec![0u32; (width * height) as usize];
        let tile_pixel_extent = TILE_PIXEL_EXTENT;
        // surface_desc.Format == D3DFMT_A8R8G8B8 (always: the texture is created WW3D_FORMAT_A8R8G8B8)
        for tile_ndx in 0..self.get_num_tiles() {
            let p_tile = match self.get_source_tile(tile_ndx) {
                Some(t) => t,
                None => continue,
            };
            let position = p_tile.tile_location_in_texture;
            if position.x < 0 {
                continue;
            }
            for j in 0..tile_pixel_extent {
                // getRGBDataForWidth(tilePixelExtent) is the full resolution data
                let mut bgr = ((tile_pixel_extent - (1 + j)) as usize) * TILE_BYTES_PER_PIXEL * tile_pixel_extent as usize; // invert to match.
                let row = position.y + j;
                let mut bgra = (row as usize) * (width as usize) + position.x as usize;
                for _i in 0..tile_pixel_extent {
                    let p = &p_tile.tile_data[bgr..bgr + 4];
                    pixels[bgra] = ((p[3] as u32) << 24)
                        .wrapping_add((p[2] as u32) << 16)
                        .wrapping_add((p[1] as u32) << 8)
                        .wrapping_add(p[0] as u32);
                    bgra += 1;
                    bgr += TILE_BYTES_PER_PIXEL;
                }
            }
        }
        let lod = if texture_reduction_factor != 0 { Some(texture_reduction_factor) } else { None };
        (TreeTexture::Atlas { width, height, pixels, lod }, height)
    }

    /// `W3DTreeBuffer::W3DTreeTextureClass::setLOD` / `W3DTreeBuffer::setTextureLOD` (W3DTreeBuffer.cpp:226, 462): adjust
    /// the maximum mip level sent to hardware. The system memory version always remains at full resolution.
    pub fn set_texture_lod(&mut self, lod: i32) {
        if let Some(TreeTexture::Atlas { lod: l, .. }) = self.tree_texture.as_mut() {
            *l = Some(lod);
        }
    }

    /// `W3DTreeBuffer::cull` (W3DTreeBuffer.cpp:292): culls the trees, marking the visible flag. If a tree becomes
    /// visible, it sets its sortKey. `camera_transform` is `camera->Get_Transform()`, `cull_sphere` is
    /// `camera->Cull_Sphere`.
    pub fn cull(&mut self, camera_transform: &Matrix3D, cull_sphere: &impl Fn(&SphereClass) -> bool) {
        // Calulate the vector direction that the camera is looking at.
        let camera_matrix = *camera_transform;
        let zmod: f32 = -1.0;
        let x = zmod * camera_matrix[0][2];
        let y = zmod * camera_matrix[1][2];
        let z = zmod * camera_matrix[2][2];
        self.camera_look_at_vector.set(x, y, z);

        for cur_tree in 0..self.num_trees as usize {
            let mut do_key = false; // We calculate the key when a tree becomes visible.
            let visible = !cull_sphere(&self.trees[cur_tree].bounds);
            if visible != self.trees[cur_tree].visible {
                self.trees[cur_tree].visible = visible;
                self.anything_changed = true;
                if visible {
                    do_key = true;
                }
            }
            // Also calculate sort key if a tree is visible, and the view changed setting m_updateAllKeys to true.
            if do_key || (visible && self.update_all_keys) {
                // The sort key is essentially the distance of location in the direction of the camera look at.
                self.trees[cur_tree].sort_key = Vector3::dot_product(&self.trees[cur_tree].location, &self.camera_look_at_vector);
            }
        }
        self.update_all_keys = false;
    }

    /// `W3DTreeBuffer::getPartitionBucket` (W3DTreeBuffer.cpp:329): the bucket index into `m_areaPartition` for a given
    /// location.
    pub fn get_partition_bucket(&self, pos: &Coord3D) -> i32 {
        let mut x = pos.x;
        let mut y = pos.y;
        if x < self.bounds.lo.x {
            x = self.bounds.lo.x;
        }
        if y < self.bounds.lo.y {
            y = self.bounds.lo.y;
        }
        if x > self.bounds.hi.x {
            x = self.bounds.hi.x;
        }
        if y > self.bounds.hi.y {
            y = self.bounds.hi.y;
        }
        let x_index = real_to_int_floor((x / (self.bounds.hi.x - self.bounds.lo.x)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1));
        let y_index = real_to_int_floor((y / (self.bounds.hi.y - self.bounds.lo.y)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1));
        // DEBUG_ASSERTCRASH(xIndex>=0 && yIndex>=0 && xIndex<PARTITION_WIDTH_HEIGHT && yIndex<PARTITION_WIDTH_HEIGHT)
        y_index * PARTITION_WIDTH_HEIGHT + x_index
    }

    /// `W3DTreeBuffer::updateSway` (W3DTreeBuffer.cpp:350): updates the sway offsets.
    pub fn update_sway(&mut self, info: &BreezeInfo, rng: &mut RandomValues) {
        for i in 0..NUM_SWAY_ENTRIES {
            let factor = wwmath::cos(i as f32 * 2.0 * PI / (NUM_SWAY_ENTRIES as f32 + 1.0));
            let angle = info.lean + (info.intensity * factor);
            let s = wwmath::sin(angle);
            let c = wwmath::cos(angle);
            self.sway_offsets[i].x = info.direction_vec.x * s;
            self.sway_offsets[i].y = info.direction_vec.y * s;
            self.sway_offsets[i].z = c - 1.0;
        }

        let delta = info.randomness * 0.5;
        for i in 0..self.num_trees as usize {
            self.trees[i].sway_type = 1 + rng.game_client_random_value(0, MAX_SWAY_TYPES as i32 - 1);
        }
        for i in 0..MAX_SWAY_TYPES {
            self.cur_sway_step[i] = NUM_SWAY_ENTRIES as f32 / info.breeze_period as f32;
            self.cur_sway_step[i] *= rng.game_client_random_value_real(1.0 - delta, 1.0 + delta);
            if self.cur_sway_step[i] < 0.0 {
                self.cur_sway_step[i] = 0.0;
            }
            self.cur_sway_offset[i] = 0.0;
            self.cur_sway_factor[i] = rng.game_client_random_value_real(1.0 - delta, 1.0 + delta);
        }
        self.cur_sway_version = info.breeze_version as i32;
    }

    /// `W3DTreeBuffer::updateTexture` (W3DTreeBuffer.cpp:394): creates a new texture. `open_file(path)` is
    /// `TheFileSystem->openFile(path, READ|BINARY)` returning the file's bytes. (`GDIFileStream2` is the byte slice;
    /// `theFile->seek(0, START)` is re-reading it from the start.)
    pub fn update_texture(&mut self, texture_reduction_factor: i32, open_file: &mut impl FnMut(&[u8]) -> Option<Vec<u8>>) {
        const MAX_TEX_WIDTH: i32 = 2048;
        let max_tiles_per_row = (MAX_TEX_WIDTH / TILE_PIXEL_EXTENT) as usize;
        let mut max_height: i32 = 0;

        self.tree_texture = None;

        let mut available_grid = vec![vec![true; max_tiles_per_row]; max_tiles_per_row];

        for i in 0..self.num_tiles as usize {
            self.source_tiles[i] = None;
        }
        self.num_tiles = 0;
        for i in 0..self.num_tree_types as usize {
            self.tree_types[i].num_tiles = 0;
            let texture_name = self.tree_types[i].data.as_ref().unwrap().texture_name.clone();
            let mut texture_path = TERRAIN_TGA_DIR_PATH.to_vec();
            texture_path.extend_from_slice(&texture_name);
            let mut the_file = open_file(&texture_path);
            if the_file.is_none() {
                let mut texture_path = TGA_DIR_PATH.to_vec();
                texture_path.extend_from_slice(&texture_name);
                the_file = open_file(&texture_path);
            }
            if let Some(file) = the_file {
                let (mut num_tiles, half_tile) = count_tiles(&file, true);
                let mut width: i32 = 10;
                while width >= 1 {
                    if num_tiles >= width * width {
                        num_tiles = width * width;
                        break;
                    }
                    width -= 1;
                }
                // upstream's `width` is 0 after the loop when no width fits (numTiles 0)
                let mut tex_found = false;
                for j in 0..i {
                    if eq_no_case(&self.tree_types[j].data.as_ref().unwrap().texture_name, &texture_name) {
                        self.tree_types[i].first_tile = 0;
                        self.tree_types[i].tile_width = width;
                        self.tree_types[i].num_tiles = 0;
                        tex_found = true;
                        break;
                    }
                }
                if tex_found {
                    continue;
                }
                if self.num_tiles + num_tiles <= MAX_TILES as i32 {
                    self.tree_types[i].first_tile = self.num_tiles;
                    self.tree_types[i].tile_width = width;
                    self.tree_types[i].num_tiles = num_tiles;
                    self.tree_types[i].half_tile = half_tile;
                    let first = self.tree_types[i].first_tile as usize;
                    read_tiles(&file, &mut self.source_tiles[first..], width);
                    self.num_tiles += num_tiles;
                } else {
                    self.tree_types[i].first_tile = 0;
                    self.tree_types[i].tile_width = 0;
                    self.tree_types[i].num_tiles = 0;
                }
            } else {
                // DEBUG_CRASH(("Could not find texture %s\n", ...))
                self.tree_types[i].first_tile = 0;
                self.tree_types[i].tile_width = 0;
                self.tree_types[i].num_tiles = 0;
            }
        }

        let mut tmp_width: i32 = 8;
        while tmp_width * tmp_width < self.num_tiles {
            tmp_width *= 2;
        }
        let tiles_per_row = tmp_width;
        self.texture_width = tmp_width * TILE_PIXEL_EXTENT;

        if self.texture_width > MAX_TEX_WIDTH {
            self.texture_width = 64;
            self.texture_height = 64;
            if self.tree_texture.is_none() {
                self.tree_texture = Some(TreeTexture::Missing);
            }
            // DEBUG_CRASH(("Too many trees in a scene."))
            return;
        }

        for i in 0..self.num_tiles as usize {
            if let Some(t) = self.source_tiles[i].as_mut() {
                t.tile_location_in_texture.x = -1;
                t.tile_location_in_texture.y = -1;
            }
        }

        // put the tree tiles into the texture
        let mut tile_width = tiles_per_row;
        while tile_width > 0 {
            for tex_class in 0..self.num_tree_types as usize {
                let width = self.tree_types[tex_class].tile_width;
                if width != tile_width {
                    continue;
                }
                let class_texture_name = self.tree_types[tex_class].data.as_ref().unwrap().texture_name.clone();
                let mut tex_found = false;
                for i in 0..tex_class {
                    if eq_no_case(&self.tree_types[i].data.as_ref().unwrap().texture_name, &class_texture_name) {
                        self.tree_types[tex_class].texture_origin.x = self.tree_types[i].texture_origin.x;
                        self.tree_types[tex_class].texture_origin.y = self.tree_types[i].texture_origin.y;
                        tex_found = true;
                        break;
                    }
                }
                if tex_found {
                    continue;
                }

                // Find an available block of space.
                let mut found = false;
                let mut row: i32 = 0;
                let mut column: i32 = 0;
                while row < (tiles_per_row - width) + 1 && !found {
                    column = 0;
                    while column < (tiles_per_row - width) + 1 && !found {
                        if available_grid[row as usize][column as usize] {
                            let mut open = true;
                            let mut i = 0;
                            while i < width && open {
                                let mut j = 0;
                                while j < width && open {
                                    if !available_grid[(row + j) as usize][(column + i) as usize] {
                                        open = false;
                                    }
                                    j += 1;
                                }
                                i += 1;
                            }
                            if open {
                                found = true;
                            }
                            break;
                        }
                        column += 1;
                    }
                    if found {
                        break;
                    }
                    row += 1;
                }
                if !found {
                    self.tree_types[tex_class].texture_origin.x = 0;
                    self.tree_types[tex_class].texture_origin.y = 0;
                    continue;
                }

                let x_origin = column * TILE_PIXEL_EXTENT;
                let y_origin = row * TILE_PIXEL_EXTENT;
                self.tree_types[tex_class].texture_origin.x = x_origin;
                self.tree_types[tex_class].texture_origin.y = y_origin;
                let class_height = y_origin + width * TILE_PIXEL_EXTENT;
                if max_height < class_height {
                    max_height = class_height;
                }

                for i in 0..width {
                    for j in 0..width {
                        available_grid[(row + j) as usize][(column + i) as usize] = false;
                        let base_ndx = self.tree_types[tex_class].first_tile + i + j * width;
                        let x = x_origin + i * TILE_PIXEL_EXTENT;
                        let y = y_origin + ((width - j) - 1) * TILE_PIXEL_EXTENT;
                        let t = self.source_tiles[base_ndx as usize].as_mut().unwrap();
                        t.tile_location_in_texture.x = x;
                        t.tile_location_in_texture.y = y;
                    }
                }
            }
            tile_width -= 1;
        }
        // DEBUG_ASSERTCRASH(maxHeight<=m_textureWidth, ("Bad max height."))
        let _ = max_height;
        let (tex, h) = self.tree_texture_update(self.texture_width, texture_reduction_factor);
        self.texture_height = h;

        self.tree_texture = Some(tex);

        for i in 0..self.num_tiles as usize {
            self.source_tiles[i] = None;
        }
    }

    /// `W3DTreeBuffer::doLighting` (W3DTreeBuffer.cpp:469): calculates the diffuse lighting as affected by dynamic
    /// lighting.
    pub fn do_lighting(&self, normal: &Vector3, object_lighting: &[TerrainLighting; MAX_GLOBAL_LIGHTS], emissive: &Vector3, vert_diffuse: u32, scale: f32) -> u32 {
        // only the first light contributes to ambient
        let mut shade_r = object_lighting[0].ambient.red + emissive.x;
        let mut shade_g = object_lighting[0].ambient.green + emissive.y;
        let mut shade_b = object_lighting[0].ambient.blue + emissive.z;

        for i in 0..MAX_GLOBAL_LIGHTS {
            let mut light_direction = Vector3::new(object_lighting[i].light_pos.x, object_lighting[i].light_pos.y, object_lighting[i].light_pos.z);
            light_direction.normalize();
            let light_ray = Vector3::new(-light_direction.x, -light_direction.y, -light_direction.z);
            let mut shade = Vector3::dot_product(&light_ray, normal);

            if shade > 1.0 {
                shade = 1.0;
            }
            if shade < 0.0 {
                shade = 0.0;
            }
            shade_r += shade * object_lighting[i].diffuse.red;
            shade_g += shade * object_lighting[i].diffuse.green;
            shade_b += shade * object_lighting[i].diffuse.blue;
        }

        shade_r *= scale;
        shade_g *= scale;
        shade_b *= scale;

        if shade_r > 1.0 {
            shade_r = 1.0;
        }
        if shade_r < 0.0 {
            shade_r = 0.0;
        }
        if shade_g > 1.0 {
            shade_g = 1.0;
        }
        if shade_g < 0.0 {
            shade_g = 0.0;
        }
        if shade_b > 1.0 {
            shade_b = 1.0;
        }
        if shade_b < 0.0 {
            shade_b = 0.0;
        }

        if vert_diffuse != 0xFFFF_FFFF {
            let mut shade = (vert_diffuse & 0xff) as f32; // blue
            shade_b *= shade / 255.0;
            shade = ((vert_diffuse >> 8) & 0xFF) as f32; // green
            shade_g *= shade / 255.0;
            shade = ((vert_diffuse >> 16) & 0xFF) as f32; // red
            shade_r *= shade / 255.0;
        }

        shade_r *= 255.0;
        shade_g *= 255.0;
        shade_b *= 255.0;
        let alpha: f32 = 255.0;
        real_to_unsigned_int(shade_b) | ((real_to_int(shade_g) << 8) as u32) | ((real_to_int(shade_r) << 16) as u32) | (((alpha as i32) << 24) as u32)
    }

    /// `W3DTreeBuffer::loadTreesInVertexAndIndexBuffers` (W3DTreeBuffer.cpp:698): loads the trees into the vertex buffer
    /// for drawing. `create_decal_shadow` is `TheW3DProjectedShadowManager->createDecalShadow(&shadowInfo)` (decal,
    /// 20 x 20, no updates, world align), called when there is no shadow yet and the manager exists
    /// (`shadow_manager_present`). The dynamic lights loop is `#if 0` upstream.
    pub fn load_trees_in_vertex_and_index_buffers(
        &mut self,
        object_lighting: &[TerrainLighting; MAX_GLOBAL_LIGHTS],
        shadow_manager_present: bool,
        create_decal_shadow: &mut impl FnMut() -> Option<u32>,
    ) {
        if self.index_tree[0].is_none() || self.vertex_tree[0].is_none() || !self.initialized {
            return;
        }
        if !self.anything_changed {
            return;
        }

        if self.shadow.is_none() && shadow_manager_present {
            self.shadow = create_decal_shadow();
        }

        self.anything_changed = false;
        let mut cur_tree: usize = 0;
        for b_ndx in 0..MAX_BUFFERS {
            self.cur_num_tree_vertices[b_ndx] = 0;
            self.cur_num_tree_indices[b_ndx] = 0;
            if cur_tree as i32 >= self.num_trees {
                break;
            }
            // Lock the buffers.
            let mut vb = self.vertex_tree[b_ndx].take().unwrap();
            let mut ib = self.index_tree[b_ndx].take().unwrap();
            // Add to the index buffer & vertex buffer.
            let mut look_at_vector = Vector2::new(self.camera_look_at_vector.x, self.camera_look_at_vector.y);
            look_at_vector.normalize();
            // We draw from back to front, so we put the indexes in the buffer from back to front.
            let mut cur_ib: usize = 0;
            let mut cur_vb: usize = 0;

            while (cur_tree as i32) < self.num_trees {
                let ty = self.trees[cur_tree].tree_type;
                if ty < 0 {
                    cur_tree += 1;
                    continue; // Deleted tree. [6/9/2003]
                }
                if !self.trees[cur_tree].visible {
                    cur_tree += 1;
                    continue;
                }
                let scale = self.trees[cur_tree].scale;
                let loc = self.trees[cur_tree].location;
                let the_sin = self.trees[cur_tree].sin;
                let the_cos = self.trees[cur_tree].cos;
                if self.tree_types[ty as usize].mesh.is_none() {
                    cur_tree += 1;
                    continue;
                }

                let mut do_vertex_lighting = true;

                // no dynamic lighting (#if 0 upstream).
                let emissive = self.tree_types[ty as usize].mesh.as_ref().unwrap().emissive;

                let start_vertex = self.cur_num_tree_vertices[b_ndx];
                self.trees[cur_tree].first_index = start_vertex;
                self.trees[cur_tree].buffer_ndx = b_ndx as i32;
                let mesh = self.tree_types[ty as usize].mesh.clone().unwrap();
                let num_vertex = mesh.model.vertices.len() as i32;
                let p_vert = &mesh.model.vertices;

                // If we happen to have too many trees, stop.
                if self.cur_num_tree_vertices[b_ndx] + num_vertex + 2 >= MAX_TREE_VERTEX {
                    break;
                }
                let num_index = mesh.model.polygons.len() as i32;
                let p_poly = &mesh.model.polygons;
                if self.cur_num_tree_indices[b_ndx] + 3 * num_index + 6 >= MAX_TREE_INDEX {
                    break;
                }

                let uvs = &mesh.model.uvs;
                let normals = &mesh.model.normals;
                let have_normals = !normals.is_empty();
                let vec_diffuse = mesh.colors.as_ref();

                let mut diffuse: u32 = 0;
                if !have_normals {
                    do_vertex_lighting = false;
                    let normal = Vector3::new(0.0, 0.0, 1.0);
                    diffuse = self.do_lighting(&normal, object_lighting, &emissive, 0xFFFF_FFFF, 1.0);
                }
                // (the commented-out `doPanel` block for half height maps is dead upstream and not translated)
                let tt = self.tree_types[ty as usize].clone();
                let data = tt.data.as_ref().unwrap();
                let mut u_scale = tt.tile_width as f32 * TILE_PIXEL_EXTENT as f32 / self.texture_width as f32;
                let mut v_scale = tt.tile_width as f32 * TILE_PIXEL_EXTENT as f32 / self.texture_height as f32;
                let u_offset = tt.texture_origin.x as f32 / self.texture_width as f32;
                let mut v_offset = tt.texture_origin.y as f32 / self.texture_height as f32;
                if tt.half_tile {
                    u_scale *= 0.5;
                    v_scale *= 0.5;
                    v_offset += (TILE_PIXEL_EXTENT / 2) as f32 / self.texture_height as f32;
                }
                for i in 0..num_vertex as usize {
                    if self.cur_num_tree_vertices[b_ndx] >= MAX_TREE_VERTEX {
                        break;
                    }

                    // Update the uv values.  The W3D models each have their own texture, and we use one texture with
                    // all images in one, so we have to change the uvs to match.
                    let mut u = uvs[i].0;
                    let mut v = uvs[i].1;

                    if u > 1.0 {
                        u = 1.0;
                    }
                    if u < 0.0 {
                        u = 0.0;
                    }
                    if v > 1.0 {
                        v = 1.0;
                    }
                    if v < 0.0 {
                        v = 0.0;
                    }

                    vb[cur_vb].u1 = u * u_scale + u_offset;
                    vb[cur_vb].v1 = v * v_scale + v_offset;
                    let mut x = p_vert[i].x;
                    let mut y = p_vert[i].y;

                    let mut v_loc = Vector3::default();
                    x += tt.offset.x;
                    y += tt.offset.y;
                    v_loc.x = x * scale * the_cos - y * scale * the_sin;
                    v_loc.y = y * scale * the_cos + x * scale * the_sin;
                    v_loc.z = p_vert[i].z * scale;
                    v_loc.z += tt.offset.z;

                    if self.trees[cur_tree].topple_state != W3DToppleState::Upright {
                        v_loc = Matrix3D::transform_vector(&self.trees[cur_tree].mtx, &v_loc);
                    } else {
                        if self.trees[cur_tree].push_aside > 0.0 {
                            v_loc.x += p_vert[i].z * self.trees[cur_tree].push_aside * self.trees[cur_tree].push_aside_cos * data.max_outward_movement;
                            v_loc.y += p_vert[i].z * self.trees[cur_tree].push_aside * self.trees[cur_tree].push_aside_sin * data.max_outward_movement;
                        }
                        v_loc.x += loc.x;
                        v_loc.y += loc.y;
                        v_loc.z += loc.z;
                    }

                    vb[cur_vb].x = v_loc.x;
                    vb[cur_vb].y = v_loc.y;
                    vb[cur_vb].z = v_loc.z;
                    vb[cur_vb].nx = self.trees[cur_tree].sway_type as f32;
                    vb[cur_vb].ny = 1.0 - data.darkening * self.trees[cur_tree].push_aside;
                    vb[cur_vb].nz = loc.z;
                    if do_vertex_lighting {
                        let mut normal = Vector3::new(0.0, 0.0, 1.0);
                        if have_normals {
                            normal.x = normals[i].x * the_cos - normals[i].y * the_sin;
                            normal.y = normals[i].y * the_cos + normals[i].x * the_sin;
                            normal.z = normals[i].z;
                        }
                        let vertex_diffuse: u32 = match vec_diffuse {
                            Some(c) => c[i],
                            None => 0xffff_ffff,
                        };
                        vb[cur_vb].diffuse = self.do_lighting(&normal, object_lighting, &emissive, vertex_diffuse, 1.0);
                    } else {
                        vb[cur_vb].diffuse = diffuse;
                    }
                    cur_vb += 1;
                    self.cur_num_tree_vertices[b_ndx] += 1;
                }

                // (the try/catch around the index loop guards the buffer lock; there is no lock here)
                for i in 0..num_index as usize {
                    if self.cur_num_tree_indices[b_ndx] + 4 > MAX_TREE_INDEX {
                        break;
                    }
                    ib[cur_ib] = (start_vertex as u32 + p_poly[i].0) as u16;
                    ib[cur_ib + 1] = (start_vertex as u32 + p_poly[i].1) as u16;
                    ib[cur_ib + 2] = (start_vertex as u32 + p_poly[i].2) as u16;
                    cur_ib += 3;
                    self.cur_num_tree_indices[b_ndx] += 3;
                }
                cur_tree += 1;
            }
            self.vertex_tree[b_ndx] = Some(vb);
            self.index_tree[b_ndx] = Some(ib);
        }
    }

    /// `W3DTreeBuffer::updateVertexBuffer` (W3DTreeBuffer.cpp:944): updates the push aside offset in the vertex buffer.
    pub fn update_vertex_buffer(&mut self) {
        if self.index_tree[0].is_none() || self.vertex_tree[0].is_none() || !self.initialized {
            return;
        }
        for b_ndx in 0..MAX_BUFFERS {
            if self.cur_num_tree_indices[b_ndx] == 0 {
                break;
            }
            let mut vb = self.vertex_tree[b_ndx].take().unwrap();

            for cur_tree in 0..self.num_trees as usize {
                if self.trees[cur_tree].buffer_ndx != b_ndx as i32 {
                    continue;
                }
                let mut ty = self.trees[cur_tree].tree_type;
                if ty < 0 {
                    continue; // Deleted tree. [6/9/2003]
                }
                if self.trees[cur_tree].push_aside_delta == 0.0 && self.trees[cur_tree].topple_state == W3DToppleState::Upright {
                    continue; // not toppling or pushed, no need to update. jba [7/11/2003]
                }
                self.any_push_changed = true;
                if !self.trees[cur_tree].visible {
                    continue;
                }
                let scale = self.trees[cur_tree].scale;
                let loc = self.trees[cur_tree].location;
                let the_sin = self.trees[cur_tree].sin;
                let the_cos = self.trees[cur_tree].cos;
                if ty < 0 || self.tree_types[ty as usize].mesh.is_none() {
                    ty = 0;
                }

                let start_vertex = self.trees[cur_tree].first_index;
                let mut cur_vb = start_vertex as usize;
                let tt = &self.tree_types[ty as usize];
                let mesh = tt.mesh.as_ref().unwrap();
                let num_vertex = mesh.model.vertices.len();
                let p_vert = &mesh.model.vertices;
                let data = tt.data.as_ref().unwrap();

                for i in 0..num_vertex {
                    let mut x = p_vert[i].x;
                    let mut y = p_vert[i].y;

                    let mut v_loc = Vector3::default();
                    x += tt.offset.x;
                    y += tt.offset.y;
                    v_loc.x = x * scale * the_cos - y * scale * the_sin;
                    v_loc.y = y * scale * the_cos + x * scale * the_sin;
                    v_loc.z = p_vert[i].z * scale;
                    v_loc.z += tt.offset.z;

                    if self.trees[cur_tree].topple_state != W3DToppleState::Upright {
                        // m_mtx.Transform_Vector(m_mtx, vLoc, &vLoc): the static form on the tree's own matrix
                        v_loc = Matrix3D::transform_vector(&self.trees[cur_tree].mtx, &v_loc);
                    } else {
                        if self.trees[cur_tree].push_aside > 0.0 {
                            v_loc.x += p_vert[i].z * self.trees[cur_tree].push_aside * self.trees[cur_tree].push_aside_cos * data.max_outward_movement;
                            v_loc.y += p_vert[i].z * self.trees[cur_tree].push_aside * self.trees[cur_tree].push_aside_sin * data.max_outward_movement;
                        }
                        v_loc.x += loc.x;
                        v_loc.y += loc.y;
                        v_loc.z += loc.z;
                    }

                    vb[cur_vb].x = v_loc.x;
                    vb[cur_vb].y = v_loc.y;
                    vb[cur_vb].z = v_loc.z;
                    vb[cur_vb].ny = 1.0 - data.darkening * self.trees[cur_tree].push_aside;
                    cur_vb += 1;
                }
            }
            self.vertex_tree[b_ndx] = Some(vb);
        }
    }

    /// `W3DTreeBuffer::freeTreeBuffers` (W3DTreeBuffer.cpp:1252): frees the index and vertex buffers (and the shaders,
    /// which are the renderer's).
    pub fn free_tree_buffers(&mut self) {
        for i in 0..MAX_BUFFERS {
            self.vertex_tree[i] = None;
            self.index_tree[i] = None;
        }
        // upstream: DeletePixelShader / DeleteVertexShader — device-only, not translated
        self.tree_vertex_shader_loaded = false;
    }

    /// `W3DTreeBuffer::unitMoved` (W3DTreeBuffer.cpp:1275): check to see if a unit collided with a tree/grass/bush.
    /// The unit is its `isKindOf(KINDOF_IMMOBILE)`, `getGeometryInfo()`, `getPosition()`, `getCrusherLevel()`,
    /// `getUnitDirectionVector2D()` and `getID()`.
    #[allow(clippy::too_many_arguments)]
    pub fn unit_moved(&mut self, is_immobile: bool, geom: &GeometryInfo, pos: &Coord3D, crusher_level: u32, unit_direction_vector_2d: &Coord2D, unit_id: ObjectId, game_frame: u32) {
        if is_immobile {
            // This is the initial positioning of the object, and we don't care. jba. [6/5/2003]
            return;
        }
        let mut radius = geom.get_major_radius();
        if geom.get_geom_type() == GeometryType::Box && radius > geom.get_minor_radius() {
            radius = geom.get_minor_radius();
        }
        // Value to assume for the tree radius: TREE_RADIUS_APPROX
        radius += TREE_RADIUS_APPROX;

        let pos = *pos;
        let mut x = pos.x - radius;
        let mut y = pos.y - radius;
        if x < self.bounds.lo.x {
            x = self.bounds.lo.x;
        }
        if y < self.bounds.lo.y {
            y = self.bounds.lo.y;
        }
        if x > self.bounds.hi.x {
            x = self.bounds.hi.x;
        }
        if y > self.bounds.hi.y {
            y = self.bounds.hi.y;
        }
        let x_index = real_to_int_floor((x / (self.bounds.hi.x - self.bounds.lo.x)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1));
        let y_index = real_to_int_floor((y / (self.bounds.hi.y - self.bounds.lo.y)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1));

        x = pos.x + radius;
        y = pos.y + radius;
        if x < self.bounds.lo.x {
            x = self.bounds.lo.x;
        }
        if y < self.bounds.lo.y {
            y = self.bounds.lo.y;
        }
        if x > self.bounds.hi.x {
            x = self.bounds.hi.x;
        }
        if y > self.bounds.hi.y {
            y = self.bounds.hi.y;
        }
        let x_max = real_to_int_ceil((x / (self.bounds.hi.x - self.bounds.lo.x)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1));
        let y_max = real_to_int_ceil((y / (self.bounds.hi.y - self.bounds.lo.y)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1));
        for i in x_index..x_max {
            for j in y_index..y_max {
                let mut tree_ndx: i32 = self.area_partition[(i + PARTITION_WIDTH_HEIGHT * j) as usize] as i32;
                while tree_ndx != END_OF_PARTITION {
                    // paranoia [7/7/2003]
                    if tree_ndx < 0 || tree_ndx >= self.num_trees {
                        // DEBUG_CRASH(("Invalid index."))
                        break;
                    }
                    let tn = tree_ndx as usize;
                    if self.trees[tn].tree_type < 0 {
                        tree_ndx = self.trees[tn].next_in_partition;
                        continue; // Tree is deleted. [7/11/2003]
                    }
                    let mut delta = Coord3D { x: self.trees[tn].location.x, y: self.trees[tn].location.y, z: self.trees[tn].location.z };
                    delta.sub(&pos);
                    if radius * radius > delta.length_sqr() {
                        let can_topple = crusher_level > 1;
                        let data = self.tree_types[self.trees[tn].tree_type as usize].data.clone().unwrap();
                        if can_topple && data.do_topple {
                            // Give a vector with direction to thing.
                            let mut topple_vector = Coord3D { x: self.trees[tn].location.x, y: self.trees[tn].location.y, z: 0.0 };
                            topple_vector.x -= pos.x;
                            topple_vector.y -= pos.y;
                            self.apply_toppling_force(tn, &topple_vector, 0.0, W3D_TOPPLE_OPTIONS_NONE);
                        } else if data.frames_to_move_outward > 1 {
                            let id = self.trees[tn].drawable_id;
                            let dir = Coord3D { x: unit_direction_vector_2d.x, y: unit_direction_vector_2d.y, z: 0.0 };
                            self.push_aside_tree(id, &pos, &dir, unit_id, game_frame);
                        }
                    }
                    tree_ndx = self.trees[tn].next_in_partition;
                }
            }
        }
    }

    /// `W3DTreeBuffer::allocateTreeBuffers` (W3DTreeBuffer.cpp:1345): allocates the index and vertex buffers. Creating
    /// the `Trees.vso`/`Trees.pso` shaders (`W3DShaderManager::LoadAndCreateD3DShader`) is the renderer's.
    pub fn allocate_tree_buffers(&mut self) {
        for i in 0..MAX_BUFFERS {
            // DX8VertexBufferClass(DX8_FVF_XYZNDUV1, MAX_TREE_VERTEX+4, USAGE_DEFAULT) (USE_STATIC)
            self.vertex_tree[i] = Some(vec![VertexFormatXYZNDUV1::default(); (MAX_TREE_VERTEX + 4) as usize]);
            // DX8IndexBufferClass(MAX_TREE_INDEX+4, USAGE_DEFAULT)
            self.index_tree[i] = Some(vec![0u16; (MAX_TREE_INDEX + 4) as usize]);
            self.cur_num_tree_vertices[i] = 0;
            self.cur_num_tree_indices[i] = 0;
        }
        // upstream: LoadAndCreateD3DShader("shaders\\Trees.vso" / "Trees.pso") — device-only, not translated
    }

    /// `W3DTreeBuffer::clearAllTrees` (W3DTreeBuffer.cpp:1385): removes all trees.
    pub fn clear_all_trees(&mut self) {
        self.num_trees = 0;
        self.bounds.lo.x = 0.0;
        self.bounds.lo.y = 0.0;
        self.bounds.hi.x = 1.0;
        self.bounds.hi.y = 1.0;
        self.tree_texture = None;
        self.cur_num_tree_indices[0] = 0;
        self.anything_changed = true;
        for i in 0..MAX_TYPES {
            self.tree_types[i].mesh = None;
        }
        for i in 0..(PARTITION_WIDTH_HEIGHT * PARTITION_WIDTH_HEIGHT) as usize {
            self.area_partition[i] = END_OF_PARTITION as i16;
        }
        self.num_tree_types = 0;
    }

    /// `W3DTreeBuffer::removeTree` (W3DTreeBuffer.cpp:1406): removes a tree.
    pub fn remove_tree(&mut self, id: DrawableId) {
        for i in 0..self.num_trees as usize {
            if self.trees[i].drawable_id == id {
                self.trees[i].location = Vector3::new(0.0, 0.0, 0.0);
                self.trees[i].tree_type = DELETED_TREE_TYPE;
                // Translate the bounding sphere of the model.
                self.trees[i].bounds.center = Vector3::new(0.0, 0.0, 0.0);
                self.trees[i].bounds.radius = 1.0;
                self.anything_changed = true;
            }
        }
    }

    /// `W3DTreeBuffer::removeTreesForConstruction` (W3DTreeBuffer.cpp:1424): removes any trees that would be under a
    /// building.
    pub fn remove_trees_for_construction(&mut self, pos: &Coord3D, geom: &GeometryInfo, angle: f32) {
        // Just iterate all trees, as even non-collidable ones get removed. jba. [7/11/2003]
        for i in 0..self.num_trees as usize {
            if self.trees[i].tree_type < 0 {
                continue; // already deleted. jba [7/11/2003]
            }
            let info = GeometryInfo::new(GeometryType::Cylinder, false, 5.0 * TREE_RADIUS_APPROX, 2.0 * TREE_RADIUS_APPROX, 2.0 * TREE_RADIUS_APPROX);
            let tree_pos = Coord3D { x: self.trees[i].location.x, y: self.trees[i].location.y, z: self.trees[i].location.z };
            if geom_collides_with_geom(pos, geom, angle, &tree_pos, &info, 0.0) {
                // remove it [7/11/2003]
                self.trees[i].tree_type = DELETED_TREE_TYPE;
                self.anything_changed = true;
            }
        }
    }

    /// `W3DTreeBuffer::addTreeType` (W3DTreeBuffer.cpp:1451): adds a type of tree (model & texture).
    /// `create_render_obj(modelName)` is `WW3DAssetManager::Create_Render_Obj` plus the HLOD unwrapping (see
    /// [`TreeModelLoad`]), `None` when the model is not found.
    pub fn add_tree_type(&mut self, data: &W3DTreeDrawModuleData, create_render_obj: &mut impl FnMut(&[u8]) -> Option<TreeModelLoad>) -> i32 {
        if self.num_tree_types >= MAX_TYPES as i32 {
            // DEBUG_CRASH(("Too many kinds of trees in map.  Reduce kinds of trees, or raise tree limit. jba."))
            return 0;
        }
        self.need_to_update_texture = true;

        let n = self.num_tree_types as usize;
        self.tree_types[n].mesh = None;

        let load = match create_render_obj(&data.model_name) {
            Some(l) => l,
            None => {
                // DEBUG_CRASH(("Unable to find model for tree %s\n"))
                return 0;
            }
        };
        let obj_box = load.obj_box;
        let offset = load.offset;

        if load.mesh.is_some() {
            self.tree_types[n].mesh = load.mesh;
        }

        if self.tree_types[n].mesh.is_none() {
            // DEBUG_CRASH(("Tree %s is not simple mesh. Tell artist to re-export. Don't Ignore!!!\n"))
            return 0;
        }

        let p_vert = self.tree_types[n].mesh.as_ref().unwrap().model.vertices.clone();
        let mut bounds = SphereClass::from_points(&p_vert);
        bounds.center += offset;
        self.tree_types[n].bounds = bounds;
        self.tree_types[n].texture_origin.x = 0;
        self.tree_types[n].texture_origin.y = 0;
        self.tree_types[n].data = Some(data.clone());
        self.tree_types[n].offset = offset;
        self.tree_types[n].shadow_size = obj_box.extent.x + obj_box.extent.y; // Average extent * 2. jba.
        self.tree_types[n].do_shadow = data.do_shadow;
        self.num_tree_types += 1;
        self.num_tree_types - 1
    }

    /// `W3DTreeBuffer::addTree` (W3DTreeBuffer.cpp:1521): adds a tree. Name is the W3D model name.
    #[allow(clippy::too_many_arguments)]
    pub fn add_tree(
        &mut self,
        id: DrawableId,
        location: Coord3D,
        scale: f32,
        angle: f32,
        random_scale_amount: f32,
        data: &W3DTreeDrawModuleData,
        rng: &mut RandomValues,
        create_render_obj: &mut impl FnMut(&[u8]) -> Option<TreeModelLoad>,
    ) {
        if self.num_trees >= MAX_TREES as i32 {
            return;
        }
        if !self.initialized {
            return;
        }
        let mut tree_type = DELETED_TREE_TYPE;
        for i in 0..self.num_tree_types as usize {
            let d = self.tree_types[i].data.as_ref().unwrap();
            if eq_no_case(&d.model_name, &data.model_name) && eq_no_case(&d.texture_name, &data.texture_name) {
                tree_type = i as i32;
                break;
            }
        }
        if tree_type < 0 {
            tree_type = self.add_tree_type(data, create_render_obj);
            if tree_type < 0 {
                return;
            }
            self.need_to_update_texture = true;
        }
        let nt = self.num_trees as usize;
        if data.frames_to_move_outward > 2 || data.do_topple {
            // Trees/grass that topples or gets pushed aside (outward) gets put in the area partition. jba [7/7/2003]
            let bucket: i16 = self.get_partition_bucket(&location) as i16;
            self.trees[nt].next_in_partition = self.area_partition[bucket as usize] as i32;
            self.area_partition[bucket as usize] = self.num_trees as i16;
        } else {
            self.trees[nt].next_in_partition = END_OF_PARTITION;
        }

        let random_scale = rng.game_client_random_value_real(1.0 - random_scale_amount, 1.0 + random_scale_amount);
        self.trees[nt].sin = wwmath::sin(angle);
        self.trees[nt].cos = wwmath::cos(angle);
        if random_scale_amount > 0.0 {
            // Randomizes the scale and orientation of trees.
            self.trees[nt].scale = scale * random_scale;
        } else {
            // Don't randomly scale & orient
            self.trees[nt].scale = scale;
        }
        self.trees[nt].location = Vector3::new(location.x, location.y, location.z);
        self.trees[nt].tree_type = tree_type;
        // Translate the bounding sphere of the model.
        self.trees[nt].bounds = self.tree_types[tree_type as usize].bounds;
        self.trees[nt].bounds.center *= self.trees[nt].scale;
        self.trees[nt].bounds.radius *= self.trees[nt].scale;
        self.trees[nt].bounds.center += self.trees[nt].location;
        // Initially set it invisible.  cull will update it's visiblity flag.
        self.trees[nt].visible = false;
        self.trees[nt].drawable_id = id;

        self.trees[nt].first_index = 0;
        self.trees[nt].buffer_ndx = -1;

        self.trees[nt].sway_type = rng.game_client_random_value(0, MAX_SWAY_TYPES as i32 - 1);
        self.trees[nt].push_aside = 0.0;
        self.trees[nt].last_frame_updated = 0;
        self.trees[nt].push_aside_source = INVALID_ID;
        self.trees[nt].push_aside_delta = 0.0;
        self.trees[nt].push_aside_cos = 1.0;
        self.trees[nt].push_aside_sin = 1.0;
        self.trees[nt].topple_state = W3DToppleState::Upright;
        self.num_trees += 1;
    }

    /// `W3DTreeBuffer::updateTreePosition` (W3DTreeBuffer.cpp:1597): updates a tree's position.
    pub fn update_tree_position(&mut self, id: DrawableId, location: Coord3D, angle: f32) -> bool {
        for i in 0..self.num_trees as usize {
            if self.trees[i].drawable_id == id {
                self.trees[i].location = Vector3::new(location.x, location.y, location.z);
                self.trees[i].sin = wwmath::sin(angle);
                self.trees[i].cos = wwmath::cos(angle);
                // Translate the bounding sphere of the model.
                self.trees[i].bounds = self.tree_types[self.trees[i].tree_type as usize].bounds;
                self.trees[i].bounds.center *= self.trees[i].scale;
                self.trees[i].bounds.radius *= self.trees[i].scale;
                self.trees[i].bounds.center += self.trees[i].location;
                self.anything_changed = true;
                return true;
            }
        }
        false
    }

    /// `W3DTreeBuffer::pushAsideTree` (W3DTreeBuffer.cpp:1620): push sideways tree or grass. `game_frame` is
    /// `TheGameLogic->getFrame()`.
    pub fn push_aside_tree(&mut self, id: DrawableId, pusher_pos: &Coord3D, pusher_direction: &Coord3D, pusher_id: ObjectId, game_frame: u32) {
        for i in 0..self.num_trees as usize {
            if self.trees[i].drawable_id == id {
                let last_frame = self.trees[i].last_frame_updated;
                self.trees[i].last_frame_updated = game_frame;
                if self.trees[i].push_aside_source == pusher_id && self.trees[i].last_frame_updated.wrapping_sub(last_frame) < 3 {
                    return; // already pushing. [5/28/2003]
                }

                if self.trees[i].push_aside != 0.0 {
                    return; // already pushing. [5/28/2003]
                }
                self.trees[i].push_aside_source = pusher_id;
                let mut delta = Coord3D { x: self.trees[i].location.x, y: self.trees[i].location.y, z: self.trees[i].location.z };
                delta.sub(pusher_pos);

                if pusher_direction.x * delta.y - pusher_direction.y * delta.x > 0.0 {
                    self.trees[i].push_aside_cos = -pusher_direction.y;
                    self.trees[i].push_aside_sin = pusher_direction.x;
                } else {
                    self.trees[i].push_aside_cos = pusher_direction.y;
                    self.trees[i].push_aside_sin = -pusher_direction.x;
                }
                self.any_push_changed = true;
                self.trees[i].push_aside_delta = 1.0 / self.tree_types[self.trees[i].tree_type as usize].data.as_ref().unwrap().frames_to_move_outward as f32;
            }
        }
    }

    /// `W3DTreeBuffer::drawTrees` (W3DTreeBuffer.cpp:1666): draws the trees. Uses the camera to cull
    /// (`camera_transform`, `cull_sphere`). Everything D3D (`Set_Shader(detailAlphaShader)`, textures, stage states, the
    /// shroud texture, `Trees.vso`, the buffers) is the renderer's: this updates the buffers and returns the
    /// [`TreeDraw`]. `prop_shroud_status` is `ThePartitionManager->getPropShroudStatusForPlayer`. The `DEBUG_TEXTURE`
    /// block is not compiled.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_trees(
        &mut self,
        camera_transform: &Matrix3D,
        cull_sphere: &impl Fn(&SphereClass) -> bool,
        frame: &TreeFrameInfo,
        rng: &mut RandomValues,
        open_file: &mut impl FnMut(&[u8]) -> Option<Vec<u8>>,
        prop_shroud_status: &mut impl FnMut(i32, &Coord3D) -> ObjectShroudStatus,
        create_decal_shadow: &mut impl FnMut() -> Option<u32>,
    ) -> TreeDraw {
        let mut out = TreeDraw::default();
        if !self.is_terrain_pass {
            return out;
        }

        // if breeze changes, always process the full update, even if not visible, so that things offscreen won't 'pop'
        // when first viewed
        let info = frame.breeze;
        let mut pause = frame.time_frozen;
        if frame.game_paused {
            pause = true;
        }
        if !pause && info.breeze_version as i32 != self.cur_sway_version {
            self.update_sway(&info, rng);
        }
        let mut sway_factor = [Vector3::default(); MAX_SWAY_TYPES];
        for i in 0..MAX_SWAY_TYPES {
            if !pause {
                self.cur_sway_offset[i] += self.cur_sway_step[i];
                if self.cur_sway_offset[i] > (NUM_SWAY_ENTRIES - 1) as f32 {
                    self.cur_sway_offset[i] -= (NUM_SWAY_ENTRIES - 1) as f32;
                }
            }
            let min_offset = real_to_int_floor(self.cur_sway_offset[i]);
            if min_offset >= 0 && ((min_offset + 1) as usize) < NUM_SWAY_ENTRIES {
                let f2 = self.cur_sway_offset[i] - min_offset as f32;
                let f1 = 1.0 - f2;
                sway_factor[i] = f1 * self.sway_offsets[min_offset as usize] + f2 * self.sway_offsets[(min_offset + 1) as usize];
                sway_factor[i] *= self.cur_sway_factor[i];
            }
        }

        self.is_terrain_pass = false;

        if self.need_to_update_texture {
            self.need_to_update_texture = false;
            self.update_texture(frame.texture_reduction_factor, open_file);
        }
        if self.tree_texture.is_none() {
            return out;
        }
        if self.update_all_keys {
            self.cull(camera_transform, cull_sphere);
        }

        // Draw tree shadows.
        if self.shadow.is_some() && frame.shadow_manager_present && frame.use_shadow_decals {
            for cur_tree in 0..self.num_trees as usize {
                let ty = self.trees[cur_tree].tree_type;
                if ty < 0 {
                    // deleted.
                    continue;
                }
                if !self.trees[cur_tree].visible || !self.tree_types[ty as usize].do_shadow {
                    continue;
                }
                let factor: f32 = 1.0;
                if self.trees[cur_tree].topple_state == W3DToppleState::Falling || self.trees[cur_tree].topple_state == W3DToppleState::Down {
                    continue;
                }
                out.shadow_decals.push(TreeShadowDecal {
                    size_x: self.tree_types[ty as usize].shadow_size,
                    size_y: -self.tree_types[ty as usize].shadow_size * factor,
                    x: self.trees[cur_tree].location.x,
                    y: self.trees[cur_tree].location.y,
                    z: self.trees[cur_tree].location.z,
                });
            }
            // upstream: TheW3DProjectedShadowManager->flushDecals(m_shadow->getTexture(0), SHADOW_DECAL) — the caller flushes
        }

        // Update pushed aside and toppling trees.
        for cur_tree in 0..self.num_trees as usize {
            if pause {
                break;
            }
            let ty = self.trees[cur_tree].tree_type;
            if ty < 0 {
                // deleted.
                continue;
            }
            if self.trees[cur_tree].topple_state == W3DToppleState::Falling || self.trees[cur_tree].topple_state == W3DToppleState::Fogged {
                self.update_toppling_tree(cur_tree, frame.local_player_index, prop_shroud_status);
            } else if self.trees[cur_tree].topple_state == W3DToppleState::Down {
                let data = self.tree_types[ty as usize].data.clone().unwrap();
                if data.kill_when_toppled {
                    if self.trees[cur_tree].sink_frames_left == 0 {
                        self.trees[cur_tree].tree_type = DELETED_TREE_TYPE; // delete it. [7/11/2003]
                        self.anything_changed = true; // need to regenerate trees. [7/11/2003]
                    }
                    self.trees[cur_tree].sink_frames_left = self.trees[cur_tree].sink_frames_left.wrapping_sub(1);
                    self.trees[cur_tree].location.z -= data.sink_distance / data.sink_frames as f32;
                    let loc = self.trees[cur_tree].location;
                    self.trees[cur_tree].mtx.set_translation(&loc);
                }
            } else if self.trees[cur_tree].push_aside_delta != 0.0 {
                self.trees[cur_tree].push_aside += self.trees[cur_tree].push_aside_delta;
                if self.trees[cur_tree].push_aside >= 1.0 {
                    let data = self.tree_types[ty as usize].data.as_ref().unwrap();
                    self.trees[cur_tree].push_aside_delta = (-1.0f64 / data.frames_to_move_inward as f64) as f32;
                } else if self.trees[cur_tree].push_aside <= 0.0 {
                    self.trees[cur_tree].push_aside_delta = 0.0;
                    self.trees[cur_tree].push_aside = 0.0;
                }
            }
        }

        if self.anything_changed {
            self.load_trees_in_vertex_and_index_buffers(&frame.object_lighting, frame.shadow_manager_present, create_decal_shadow);
            self.anything_changed = false;
        } else if self.any_push_changed {
            self.any_push_changed = false;
            self.update_vertex_buffer();
        }

        if self.cur_num_tree_indices[0] == 0 {
            return out;
        }
        // upstream: Set_Shader(detailAlphaShader), Set_Texture(0, m_treeTexture), Set_Texture(1, NULL), stage states,
        // W3DShaderManager::setShroudTex(1) — device-only, not translated

        if self.tree_vertex_shader_loaded {
            // c4: world * view * projection, transposed — device-only, not translated
            out.no_sway = [0.0, 0.0, 0.0, 0.0]; // c8
            // c8 - c8+MAX_SWAY_TYPES - the sway amount.
            for i in 0..MAX_SWAY_TYPES {
                out.sway[i] = [sway_factor[i].x, sway_factor[i].y, sway_factor[i].z, 0.0]; // c9+i
            }

            if let Some(shroud) = frame.shroud {
                // Setup shroud texture info [6/6/2003]
                let mut width = shroud.cell_width;
                let mut height = shroud.cell_height;

                let xoffset = -shroud.draw_origin_x + width;
                let yoffset = -shroud.draw_origin_y + height;
                out.shroud_offset = [xoffset, yoffset, 0.0, 0.0]; // c32
                width = 1.0 / (width * shroud.texture_width);
                height = 1.0 / (height * shroud.texture_height);
                out.shroud_scale = [width, height, 1.0, 1.0]; // c33
            } else {
                out.shroud_offset = [0.0; 4];
                out.shroud_scale = [0.0; 4];
            }
            // upstream: Set_Vertex_Shader(m_dwTreeVertexShader); the `#if 0` pixel shader block is not compiled
        }

        for b_ndx in 0..MAX_BUFFERS {
            if self.cur_num_tree_indices[b_ndx] == 0 {
                break;
            }
            // upstream: Set_Index_Buffer / Set_Vertex_Buffer / Apply_Render_State_Changes / SetVertexShader / stage states
            // — device-only, not translated
            // Render the waving grass
            if b_ndx == 0 {
                out.draw = Some(DrawTriangles {
                    start_index: 0,
                    polygon_count: self.cur_num_tree_indices[b_ndx] / 3,
                    min_vertex_index: 0,
                    vertex_count: self.cur_num_tree_vertices[b_ndx],
                });
            }
        }
        // upstream: Set_Vertex_Shader(DX8_FVF_XYZNDUV1), Set_Pixel_Shader(NULL), Invalidate_Cached_Render_States — device-only
        out
    }

    /// `W3DTreeBuffer::applyTopplingForce` (W3DTreeBuffer.cpp:1928): start the toppling process by giving a force
    /// vector. `tree` is the index into `m_trees`.
    pub fn apply_toppling_force(&mut self, tree: usize, topple_direction: &Coord3D, topple_speed: f32, options: u32) {
        if self.trees[tree].topple_state != W3DToppleState::Upright {
            return;
        }
        let d = self.tree_types[self.trees[tree].tree_type as usize].data.clone().unwrap();
        // Having a low toppleSpeed is BAD. In particular, if the toppleSpeed is exactly 0, the tree will stay upright
        // forever, frozen in place (because the sway update is dead) but never dying
        let mut topple_speed = topple_speed;
        if topple_speed < d.minimum_topple_speed {
            topple_speed = d.minimum_topple_speed;
        }

        let t = &mut self.trees[tree];
        t.topple_direction = *topple_direction;
        t.topple_direction.normalize();
        t.angular_accumulation = 0.0;

        t.angular_velocity = topple_speed * d.initial_velocity_percent;
        t.angular_acceleration = topple_speed * d.initial_accel_percent;
        t.topple_state = W3DToppleState::Falling;
        t.options = options;
        let pos = Coord3D { x: t.location.x, y: t.location.y, z: t.location.z };
        self.pending_fx.push(TreeFxRequest { fx_name: d.topple_fx.clone(), pos });
        self.any_push_changed = true;
        let t = &mut self.trees[tree];
        t.mtx.make_identity();
        let loc = t.location;
        t.mtx.set_translation(&loc);
    }

    /// `W3DTreeBuffer::updateTopplingTree` (W3DTreeBuffer.cpp:1967): keep track of rotational fall distance, bounce and/or
    /// stop when needed. `tree` is the index into `m_trees`.
    pub fn update_toppling_tree(&mut self, tree: usize, local_player_index: i32, prop_shroud_status: &mut impl FnMut(i32, &Coord3D) -> ObjectShroudStatus) {
        // DEBUG_ASSERTCRASH(tree->m_toppleState != TOPPLE_UPRIGHT, ("hmm, we should be sleeping here"));
        if self.trees[tree].topple_state == W3DToppleState::Upright || self.trees[tree].topple_state == W3DToppleState::Down {
            return;
        }

        let d = self.tree_types[self.trees[tree].tree_type as usize].data.clone().unwrap();
        let pos = Coord3D { x: self.trees[tree].location.x, y: self.trees[tree].location.y, z: self.trees[tree].location.z };
        let ss = prop_shroud_status(local_player_index, &pos);
        if ss == ObjectShroudStatus::Fogged {
            // Don't update fogged trees. [8/11/2003]
            self.trees[tree].topple_state = W3DToppleState::Fogged;
            return;
        } else if self.trees[tree].topple_state == W3DToppleState::Fogged {
            // was fogged, now isn't.
            let t = &mut self.trees[tree];
            t.angular_velocity = 0.0;
            t.topple_state = W3DToppleState::Down;
            t.mtx.in_place_pre_rotate_x(-ANGULAR_LIMIT * t.topple_direction.y);
            t.mtx.in_place_pre_rotate_y(ANGULAR_LIMIT * t.topple_direction.x);
            if d.kill_when_toppled {
                // If got killed in the fog, just remove. jba [8/11/2003]
                t.sink_frames_left = 0;
            }
            return;
        }
        const VELOCITY_BOUNCE_LIMIT: f32 = 0.01; // if the velocity after a bounce will be this or lower, just stop at zero
        const VELOCITY_BOUNCE_SOUND_LIMIT: f32 = 0.03; // and if this low, then skip the bounce sound

        let mut cur_vel_to_use = self.trees[tree].angular_velocity;
        if self.trees[tree].angular_accumulation + cur_vel_to_use > ANGULAR_LIMIT {
            cur_vel_to_use = ANGULAR_LIMIT - self.trees[tree].angular_accumulation;
        }

        {
            let t = &mut self.trees[tree];
            t.mtx.in_place_pre_rotate_x(-cur_vel_to_use * t.topple_direction.y);
            t.mtx.in_place_pre_rotate_y(cur_vel_to_use * t.topple_direction.x);
            t.angular_accumulation += cur_vel_to_use;
        }
        if self.trees[tree].angular_accumulation >= ANGULAR_LIMIT && self.trees[tree].angular_velocity > 0.0 {
            // Hit so either bounce or stop if too little remaining velocity.
            self.trees[tree].angular_velocity *= -d.bounce_velocity_percent;

            // fabs(Real) is the double overload (<math.h>): the value is not truncated to int.
            let abs_v = (self.trees[tree].angular_velocity as f64).abs();
            if (self.trees[tree].options & W3D_TOPPLE_OPTIONS_NO_BOUNCE) != 0 || abs_v < VELOCITY_BOUNCE_LIMIT as f64 {
                // too slow, just stop
                self.trees[tree].angular_velocity = 0.0;
                self.trees[tree].topple_state = W3DToppleState::Down;
                if d.kill_when_toppled {
                    self.trees[tree].sink_frames_left = d.sink_frames;
                }
            } else if abs_v >= VELOCITY_BOUNCE_SOUND_LIMIT as f64 {
                // fast enough bounce to warrant the bounce fx
                if (self.trees[tree].options & W3D_TOPPLE_OPTIONS_NO_FX) == 0 {
                    let loc = Vector3::new(0.0, 0.0, 3.0 * TREE_RADIUS_APPROX); // Kinda towards the top of the tree. jba. [7/11/2003]
                    let xloc = Matrix3D::transform_vector(&self.trees[tree].mtx, &loc);
                    let pos = Coord3D { x: xloc.x, y: xloc.y, z: xloc.z };
                    self.pending_fx.push(TreeFxRequest { fx_name: d.bounce_fx.clone(), pos });
                }
            }
        } else {
            self.trees[tree].angular_velocity += self.trees[tree].angular_acceleration;
        }
    }

    /// `W3DTreeBuffer::crc` (W3DTreeBuffer.cpp:2015): empty. jba [8/11/2003]
    pub fn crc(&self) {}

    /// `W3DTreeBuffer::xfer` (W3DTreeBuffer.cpp:2026). Version 1: initial version. On load, the trees are re-added
    /// through `addTree` (so `rng` and `create_render_obj` are `addTree`'s).
    pub fn xfer<X: Xfer>(
        &mut self,
        xfer: &mut X,
        rng: &mut RandomValues,
        create_render_obj: &mut impl FnMut(&[u8]) -> Option<TreeModelLoad>,
    ) -> XferResult<()> {
        // version
        let current_version: XferVersion = 1;
        let mut version: XferVersion = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        let mut num_trees = self.num_trees;
        xfer.xfer_int(&mut num_trees)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.num_trees = 0;
            for i in 0..(PARTITION_WIDTH_HEIGHT * PARTITION_WIDTH_HEIGHT) as usize {
                self.area_partition[i] = END_OF_PARTITION as i16;
            }
        }

        // Save trees. [8/11/2003]
        for i in 0..num_trees as usize {
            let mut tree = TTree::default(); // memset(&tree, 0, sizeof(tree))
            let mut model_name: Vec<u8> = Vec::new();
            let mut model_texture: Vec<u8> = Vec::new();
            let mut tree_type = DELETED_TREE_TYPE;
            if xfer.get_xfer_mode() != XferMode::Load {
                tree = self.trees[i].clone();
                tree_type = self.trees[i].tree_type;
                if tree_type != DELETED_TREE_TYPE {
                    let d = self.tree_types[tree_type as usize].data.as_ref().unwrap();
                    model_name = d.model_name.clone();
                    model_texture = d.texture_name.clone();
                }
            }
            xfer.xfer_ascii_string(&mut model_name)?;
            xfer.xfer_ascii_string(&mut model_texture)?;
            if xfer.get_xfer_mode() == XferMode::Load {
                for j in 0..self.num_tree_types as usize {
                    let d = self.tree_types[j].data.as_ref().unwrap();
                    if eq_no_case(&d.model_name, &model_name) && eq_no_case(&d.texture_name, &model_texture) {
                        tree_type = j as i32;
                        break;
                    }
                }
            }

            xfer.xfer_real(&mut tree.location.x)?;
            xfer.xfer_real(&mut tree.location.y)?;
            xfer.xfer_real(&mut tree.location.z)?;

            xfer.xfer_real(&mut tree.scale)?; // Scale at location.
            xfer.xfer_real(&mut tree.sin)?; // Sine of the rotation angle at location.
            xfer.xfer_real(&mut tree.cos)?; // Cosine of the rotation angle at location.

            xfer.xfer_drawable_id(&mut tree.drawable_id)?; // Drawable this tree corresponds to.

            // Topple parameters. [7/7/2003]
            xfer.xfer_real(&mut tree.angular_velocity)?;
            xfer.xfer_real(&mut tree.angular_acceleration)?;
            xfer.xfer_coord3d(&mut tree.topple_direction)?;
            let mut state_bytes = (tree.topple_state as i32).to_le_bytes();
            xfer.xfer_user(&mut state_bytes)?; // sizeof(W3DToppleState)
            tree.topple_state = W3DToppleState::from_i32(i32::from_le_bytes(state_bytes));
            xfer.xfer_real(&mut tree.angular_accumulation)?;
            xfer.xfer_unsigned_int(&mut tree.options)?;
            xfer.xfer_matrix3d(&mut tree.mtx)?;
            xfer.xfer_unsigned_int(&mut tree.sink_frames_left)?;

            if xfer.get_xfer_mode() == XferMode::Load && tree_type != DELETED_TREE_TYPE && tree_type < self.num_tree_types {
                let pos = Coord3D { x: tree.location.x, y: tree.location.y, z: tree.location.z };
                let angle: f32 = 0.0;
                let data = self.tree_types[tree_type as usize].data.clone().unwrap();
                self.add_tree(tree.drawable_id, pos, tree.scale, angle, 0.0, &data, rng, create_render_obj);
                if self.num_trees != 0 {
                    let cur = (self.num_trees - 1) as usize;
                    let c = &mut self.trees[cur];
                    c.angular_acceleration = tree.angular_acceleration;
                    c.angular_velocity = tree.angular_velocity;
                    c.topple_direction = tree.topple_direction;
                    c.topple_state = tree.topple_state;
                    c.options = tree.options;
                    c.mtx = tree.mtx;
                    c.sink_frames_left = tree.sink_frames_left;
                }
            }
        }
        Ok(())
    }

    /// `W3DTreeBuffer::loadPostProcess` (W3DTreeBuffer.cpp:2043): empty. jba [8/11/2003]
    pub fn load_post_process(&self) {}
}
