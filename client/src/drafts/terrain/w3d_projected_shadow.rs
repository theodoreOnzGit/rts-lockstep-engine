//! UNTESTED DRAFT (#202, #204): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DProjectedShadow.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Shadow/W3DProjectedShadow.cpp
//!   GeneralsMD/Code/GameEngine/Include/GameClient/Shadow.h (`ProjectedShadowManager`, the interface base)
//!
//! The shadows live in a slot table (`shadows`); `m_shadowList`/`m_decalList` and `m_next` are slot indices, and a shadow
//! is addressed by its slot. `W3DShadowTexture`s are in the texture manager's storage and addressed by
//! [`ShadowTextureId`] (the manager's name table is the `HashTableClass`, `stricmp` keyed; `freeAllTextures` empties the
//! table but the storage stays, as the reference counts keep the objects alive upstream). The `RenderObjClass` of a
//! caster is a [`ShadowRenderObj`] snapshot. Everything the shadows ask of the game and the device (the terrain, the
//! frustum, the partition manager, the asset manager, `TexProjectClass`) is [`ProjectedShadowEnv`]. The decal vertex and
//! index buffers (`shadowDecalVertexBufferD3D` etc.) are `Vec`s of `SHADOW_DECAL_VERTEX_SIZE`/`SHADOW_DECAL_INDEX_SIZE`;
//! `flushDecals` returns a self-contained [`DecalFlushDraw`] (the batch's vertices and indices copied), since the buffers
//! are overwritten after a flush as they would be on the device. `renderProjectedTerrainShadow` writes the shared
//! `shadowVertexBufferD3D`/`shadowIndexBufferD3D` state of the volumetric system ([`ShadowVolumeBuffers`]).
//!
//! `TexProjectClass` (the projector: `Compute_Perspective_Projection`, `Compute_Texture`, `Pre_Render_Update`,
//! `Peek_Material_Pass`) and the shadow render target are the renderer's; the calls are comments where they occur and
//! [`ProjectedShadowEnv`] has the one hook each needs.
//!
//! Notes on upstream behaviour kept as it is:
//! - `queueSimpleDecal` is never called (`renderShadows` has it commented out); translated as it is.
//! - `queueDecal` reads the uninitialised local `box` for `cx, cy, dx, dy` (never used afterwards): not translated.
//! - `removeShadow`'s counters switch on `shadow->m_type`, so a combined type bit set falls into `default:`; the
//!   decal list search only runs for `SHADOW_ALPHA_DECAL|SHADOW_ADDITIVE_DECAL`.
//! - `renderProjectedTerrainShadow` ignores `flipForBlend` of `getAlphaUVData` only in the commented-out branch; it uses
//!   it to pick the winding. `queueDecal` uses `getFlipState` instead.
//! - `fabs` (not `abs`) is used on floats in `updateBounds`: the double overload, so no int conversion.
//! - `W3DShadowTexture::init` creates a new texture the size of the render target (`getRenderTarget()`).

use super::w3d_common::{AABoxClass, RenderObjHandle, SphereClass, TextureHandle};
use super::w3d_shadow::{
    Shadow, ShadowRenderObj, ShadowTypeInfo, SHADOW_ADDITIVE_DECAL, SHADOW_ALPHA_DECAL, SHADOW_DECAL, SHADOW_DIRECTIONAL_PROJECTION, SHADOW_NONE,
    SHADOW_PROJECTION,
};
use super::w3d_terrain_background::{get_flip_state, MinMaxAABoxClass, SharedHeightMap};
use super::w3d_volumetric_shadow::ShadowVolumeBuffers;
use rts_lockstep_engine::common::base_type::{real_to_int_ceil, real_to_int_floor, Coord3D};
use rts_lockstep_engine::common::game_type::{PathfindLayer, LAYER_GROUND};
use rts_lockstep_engine::logic::terrain_visual::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::wwmath;

/// `DEFAULT_RENDER_TARGET_WIDTH`.
pub const DEFAULT_RENDER_TARGET_WIDTH: i32 = 512;
/// `DEFAULT_RENDER_TARGET_HEIGHT`.
pub const DEFAULT_RENDER_TARGET_HEIGHT: i32 = 512;
/// `SHADOW_DECAL_VERTEX_SIZE`.
pub const SHADOW_DECAL_VERTEX_SIZE: i32 = 32768;
/// `SHADOW_DECAL_INDEX_SIZE`.
pub const SHADOW_DECAL_INDEX_SIZE: i32 = 65536;
/// `BRIDGE_OFFSET_FACTOR` (this file's: 1.5).
pub const BRIDGE_OFFSET_FACTOR: f32 = 1.5;
/// `DECAL_TEXELS_PER_WORLD_UNIT`: 64 texels per 2 terrain cells (20 units).
pub const DECAL_TEXELS_PER_WORLD_UNIT: f32 = 64.0 / 20.0;

/// `CollisionMath::OverlapType` as the shadows use it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlapType {
    /// `OUTSIDE`.
    Outside,
    /// `INSIDE`.
    Inside,
    /// `OVERLAPPED`.
    Overlapped,
}

/// A shadow texture as the asset manager hands it out: the handle and its level 0 description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShadowTextureInfo {
    /// The texture.
    pub handle: TextureHandle,
    /// `Get_Level_Description` width.
    pub width: i32,
    /// `Get_Level_Description` height.
    pub height: i32,
}

/// The visible terrain (`TheTerrainRenderObject->getMap()`).
#[derive(Clone)]
pub struct ShadowTerrain {
    /// `getMap()`.
    pub map: SharedHeightMap,
    /// `getDrawOrgX()`.
    pub draw_org_x: i32,
    /// `getDrawOrgY()`.
    pub draw_org_y: i32,
    /// `getDrawWidth()`.
    pub draw_width: i32,
    /// `getDrawHeight()`.
    pub draw_height: i32,
}

/// An object in range of a projected shadow, as `renderShadows` iterates it: the model draw's render object when the
/// drawable has a `W3DModelDraw` (`getRenderObject()`), with the flags the filter reads.
#[derive(Clone, Copy, Debug)]
pub struct ShadowReceiver {
    /// `w3dDraw->getRenderObject()`.
    pub robj: RenderObjHandle,
    /// `robj->Is_Alpha()`.
    pub is_alpha: bool,
    /// `obj->isKindOf(KINDOF_IMMOBILE)`.
    pub is_immobile: bool,
    /// `robj->Is_Really_Visible()`.
    pub is_really_visible: bool,
}

/// What the projected shadows ask of the game and the device.
pub trait ProjectedShadowEnv {
    /// `TheGlobalData->m_useShadowDecals`.
    fn use_shadow_decals(&self) -> bool;
    /// `TheTerrainRenderObject` and its map; `None` for NULL.
    fn terrain(&self) -> Option<ShadowTerrain>;
    /// `WW3DAssetManager::Get_Texture(name)` with U and V clamp and no mip mapping set; `None` when not found.
    fn get_texture(&mut self, name: &[u8]) -> Option<ShadowTextureInfo>;
    /// `TheTerrainLogic->getLayerHeight(x, y, layer)`.
    fn layer_height(&self, x: f32, y: f32, layer: PathfindLayer) -> f32;
    /// `TheTerrainRenderObject->getHeightMapHeight(x, y, &normal)`.
    fn height_map_height(&self, x: f32, y: f32) -> (f32, Coord3D);
    /// `CollisionMath::Overlap_Test(*shadowCameraFrustum, sphere)`.
    fn frustum_sphere_overlap(&self, sphere: &SphereClass) -> OverlapType;
    /// `CollisionMath::Overlap_Test(*shadowCameraFrustum, aaBox)`.
    fn frustum_box_overlap(&self, b: &AABoxClass) -> OverlapType;
    /// `ThePartitionManager->iterateObjectsInRange(center, radius, FROM_CENTER_3D)` mapped to the objects' model draws.
    fn receivers_in_range(&mut self, sphere: &SphereClass) -> Vec<ShadowReceiver>;
    /// `robj->Intersect_AABox(AABoxIntersectionTestClass(aaBox, COLL_TYPE_ALL))`.
    fn robj_intersects_aabox(&self, robj: RenderObjHandle, b: &AABoxClass) -> bool;
    /// `new TextureClass(width, height, format, MIP_LEVELS_1)` for a projected shadow texture; its handle.
    fn create_shadow_texture(&mut self, width: i32, height: i32) -> TextureHandle;
    /// `DX8Wrapper::Create_Render_Target(width, height, WW3D_FORMAT_A8R8G8B8)` (`with_alpha`) or without; `None` on failure.
    fn create_render_target(&mut self, width: i32, height: i32, with_alpha: bool) -> Option<TextureHandle>;
}

/// An identifier of a `W3DShadowTexture` in the texture manager's storage.
pub type ShadowTextureId = usize;

/// `class W3DShadowTexture`.
#[derive(Clone, Debug)]
pub struct W3DShadowTexture {
    /// `m_namebuf`: name of model hierarchy (always ends with a null character within `2*W3D_NAME_LEN`).
    pub name: Vec<u8>,
    /// `m_texture`: texture holding the shadow for this renderobject, with its description.
    pub texture: Option<ShadowTextureInfo>,
    /// `m_lastLightPosition`: position of light source at time of last texture update.
    pub last_light_position: Vector3,
    /// `m_lastObjectOrientation`: orientation of shadow casting object when texture was generated (`Matrix3x3`).
    pub last_object_orientation: Matrix3D,
    /// `m_areaEffectBox`: boundary defining object-space volume affected by shadow.
    pub area_effect_box: AABoxClass,
    /// `m_areaEffectSphere`.
    pub area_effect_sphere: SphereClass,
    /// `m_shadowUV`: world-space vectors defining the u and v texture coordinate axis.
    pub shadow_uv: [Vector3; 2],
}

/// `W3D_NAME_LEN`.
pub const W3D_NAME_LEN: usize = 16;

impl Default for W3DShadowTexture {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DShadowTexture {
    /// `W3DShadowTexture::W3DShadowTexture` (W3DProjectedShadow.h): `m_lastLightPosition.Set(0,0,0);
    /// m_lastObjectOrientation.Make_Identity(); m_shadowUV[0] = (1,0,0), m_shadowUV[1] = (0,-1,0)`.
    pub fn new() -> Self {
        W3DShadowTexture {
            name: Vec::new(),
            texture: None,
            last_light_position: Vector3::new(0.0, 0.0, 0.0),
            last_object_orientation: Matrix3D::identity(),
            area_effect_box: AABoxClass::default(),
            area_effect_sphere: SphereClass::default(),
            shadow_uv: [Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, -1.0, 0.0)], // u along world x, v along world -y
        }
    }

    /// `W3DShadowTexture::Set_Name`: pads with zero so it always ends with a null character.
    pub fn set_name(&mut self, name: &[u8]) {
        let n = name.len().min(2 * W3D_NAME_LEN - 1);
        self.name = name[..n].to_vec();
    }

    /// `W3DShadowTexture::setDecalUVAxis`.
    pub fn set_decal_uv_axis(&mut self, u: Vector3, v: Vector3) {
        self.shadow_uv[0] = u;
        self.shadow_uv[1] = v;
    }

    /// `W3DShadowTexture::init` (W3DProjectedShadow.cpp:2271): creates a texture of the render target's size (`render_target`
    /// is its level 0 description `(width, height)`).
    pub fn init(&mut self, render_target: (i32, i32), env: &mut impl ProjectedShadowEnv) -> bool {
        // @todo: implement this function
        let handle = env.create_shadow_texture(render_target.0, render_target.1);
        self.texture = Some(ShadowTextureInfo { handle, width: render_target.0, height: render_target.1 });
        true
    }

    /// `W3DShadowTexture::updateBounds` (W3DProjectedShadow.cpp:2284): update extent of shadow.
    pub fn update_bounds(&mut self, light_pos: &Vector3, robj: &ShadowRenderObj) {
        let mut corners = [Vector3::default(); 8];

        // calculate local bounding box of shadow projection
        let obj_pos = robj.position;
        let mut bx = robj.bounding_box;
        let floor_z = obj_pos.z - 2.0; // lower slightly so shadows go under ground.

        // project each box corner to base of object to determine rough extent of shadow
        // Get vertices of top of bounding box
        corners[0] = bx.center + bx.extent; // top right corner
        corners[1] = corners[0];
        corners[1].x -= 2.0 * bx.extent.x; // top left corner
        corners[2] = corners[1];
        corners[2].y -= 2.0 * bx.extent.y; // bottom left corner
        corners[3] = corners[2];
        corners[3].x += 2.0 * bx.extent.x; // bottom right corner

        // Project top volume corners onto ground plane
        let mut light_ray = corners[0] - *light_pos; // vector light to corner
        let mut length = 1.0 / light_ray.length();
        light_ray *= length;
        let mut vector_scale = wwmath::fabs((corners[0].z - floor_z) / light_ray.z); // length of vector from top corner to ground.
        let mut vector_scale_max = vector_scale;
        corners[4] = corners[0] + light_ray * vector_scale;
        vector_scale_max *= length;

        light_ray = corners[1] - *light_pos; // vector light to corner
        length = 1.0 / light_ray.length();
        light_ray *= length;
        let mut vector_scale_temp = wwmath::fabs((corners[1].z - floor_z) / light_ray.z); // length of vector from top corner to ground.
        corners[5] = corners[1] + light_ray * vector_scale_temp;
        vector_scale_temp *= length;

        if vector_scale_temp > vector_scale_max {
            vector_scale_max = vector_scale_temp; // keep track of maximum required extrusion length.
        }

        light_ray = corners[2] - *light_pos; // vector light to corner
        length = 1.0 / light_ray.length();
        light_ray *= length;
        vector_scale = wwmath::fabs((corners[2].z - floor_z) / light_ray.z); // length of vector from top corner to ground.
        corners[6] = corners[2] + light_ray * vector_scale;
        vector_scale *= length;

        if vector_scale > vector_scale_max {
            vector_scale_max = vector_scale; // keep track of maximum required extrusion length.
        }

        light_ray = corners[3] - *light_pos; // vector light to corner
        length = 1.0 / light_ray.length();
        light_ray *= length;
        vector_scale_temp = wwmath::fabs((corners[3].z - floor_z) / light_ray.z); // length of vector from top corner to ground.
        corners[7] = corners[3] + light_ray * vector_scale_temp;
        vector_scale_temp *= length;

        if vector_scale_temp > vector_scale_max {
            vector_scale_max = vector_scale_temp; // keep track of maximum required extrusion length.
        }
        let _ = vector_scale_max;

        // box.Init(Corners, 8): generate a new bounding box to fit the shadow projection
        let mut mm = MinMaxAABoxClass::init_empty();
        for c in corners.iter() {
            mm.add_point(c);
        }
        bx = super::w3d_terrain_background::aabox_from_min_max(&mm);
        self.area_effect_sphere = SphereClass::new(bx.center, bx.extent.length());

        self.area_effect_sphere.center -= obj_pos; // translate sphere to object space.
        bx.center -= obj_pos; // translate box to object space (Translate(-objPos))
        self.area_effect_box = bx;
    }
}

/// `class W3DShadowTextureManager`.
#[derive(Clone, Debug, Default)]
pub struct W3DShadowTextureManager {
    /// The textures (storage; ids stay valid after `freeAllTextures`, as the reference counts keep them alive upstream).
    pub storage: Vec<W3DShadowTexture>,
    /// `texturePtrTable`: the cache, `(name, id)`; keyed with `stricmp`.
    pub texture_ptr_table: Vec<(Vec<u8>, ShadowTextureId)>,
    /// `missingTextureTable`.
    pub missing_texture_table: Vec<Vec<u8>>,
}

fn eq_no_case(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

impl W3DShadowTextureManager {
    /// `W3DShadowTextureManager::W3DShadowTextureManager` (W3DProjectedShadow.cpp:2394).
    pub fn new() -> Self {
        Self::default()
    }

    /// `W3DShadowTextureManager::freeAllTextures` (W3DProjectedShadow.cpp:2410): release all loaded textures.
    pub fn free_all_textures(&mut self) {
        // Then clear the table
        self.texture_ptr_table.clear();
    }

    /// `W3DShadowTextureManager::peekTexture` (W3DProjectedShadow.cpp:2424): find texture in cache.
    pub fn peek_texture(&self, name: &[u8]) -> Option<ShadowTextureId> {
        self.texture_ptr_table.iter().find(|(n, _)| eq_no_case(n, name)).map(|(_, id)| *id)
    }

    /// `W3DShadowTextureManager::getTexture` (W3DProjectedShadow.cpp:2430): get texture from cache and increment its
    /// reference count (references are the slots' lifetime here).
    pub fn get_texture(&self, name: &[u8]) -> Option<ShadowTextureId> {
        self.peek_texture(name)
    }

    /// `W3DShadowTextureManager::addTexture` (W3DProjectedShadow.cpp:2441): add texture to cache.
    pub fn add_texture(&mut self, new_texture: W3DShadowTexture) -> ShadowTextureId {
        let id = self.storage.len();
        let name = new_texture.name.clone();
        self.storage.push(new_texture);
        self.texture_ptr_table.push((name, id));
        id
    }

    /// `W3DShadowTextureManager::invalidateCachedLightPositions` (W3DProjectedShadow.cpp:2452): step through each of our
    /// shadow textures and update previous light position.
    pub fn invalidate_cached_light_positions(&mut self) {
        let id_vec = Vector3::new(0.0, 0.0, 0.0);
        for (_, id) in self.texture_ptr_table.clone() {
            self.storage[id].last_light_position = id_vec;
        }
    }

    /// `W3DShadowTextureManager::registerMissing` (W3DProjectedShadow.cpp:2481).
    pub fn register_missing(&mut self, name: &[u8]) {
        self.missing_texture_table.push(name.to_vec());
    }

    /// `W3DShadowTextureManager::isMissing` (W3DProjectedShadow.cpp:2486).
    pub fn is_missing(&self, name: &[u8]) -> bool {
        self.missing_texture_table.iter().any(|n| eq_no_case(n, name))
    }

    /// `W3DShadowTextureManager::createTexture` (W3DProjectedShadow.cpp:2492): create shadow geometry from a reference
    /// render object. Returns 0 on success, 1 on error (as upstream's `int`).
    pub fn create_texture(&mut self, name: &[u8], render_target: (i32, i32), env: &mut impl ProjectedShadowEnv) -> i32 {
        let mut new_texture = W3DShadowTexture::new();
        new_texture.set_name(name);

        let res = new_texture.init(render_target, env);

        if !res {
            // load failed!
            return 1;
        } else if self.peek_texture(&new_texture.name).is_some() {
            // duplicate exists!
            return 1;
        } else {
            self.add_texture(new_texture);
        }

        0
    }
}

/// `SHADOW_DECAL_VERTEX`: the vertex structure passed to D3D.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShadowDecalVertex {
    /// `x`.
    pub x: f32,
    /// `y`.
    pub y: f32,
    /// `z`.
    pub z: f32,
    /// `diffuse`.
    pub diffuse: u32,
    /// `u`.
    pub u: f32,
    /// `v`.
    pub v: f32,
}

/// What one `flushDecals` draws: the texture and blend, the batch's vertices and indices (copied from the buffers at the
/// flush) and the `SetIndices(.., base)` / `DrawIndexedPrimitive(TRIANGLELIST, 0, verts, startIndex, polys)` values.
#[derive(Clone, Debug)]
pub struct DecalFlushDraw {
    /// `texture->getTexture()`.
    pub texture: Option<ShadowTextureInfo>,
    /// The decal type (`SHADOW_DECAL` multiplicative, `SHADOW_ALPHA_DECAL` alpha, `SHADOW_ADDITIVE_DECAL` additive;
    /// any other leaves the previous shader).
    pub shadow_type: u32,
    /// `SetIndices(shadowDecalIndexBufferD3D, nShadowDecalStartBatchVertex)`.
    pub base_vertex: i32,
    /// `DrawIndexedPrimitive`'s `startIndex` (`nShadowDecalStartBatchIndex`).
    pub start_index: i32,
    /// `nShadowDecalPolysInBatch`.
    pub polys: i32,
    /// `nShadowDecalVertsInBatch`.
    pub verts: i32,
    /// The batch's vertices (`verts` of them from `base_vertex`).
    pub vertices: Vec<ShadowDecalVertex>,
    /// The batch's indices (`polys*3` from `start_index`).
    pub indices: Vec<u16>,
}

/// One `renderProjectedTerrainShadow` draw (stencil-incrementing, `DESTCOLOR`/`ZERO` blend): the vertices and indices
/// written into the shared shadow buffers, and the draw values.
#[derive(Clone, Debug)]
pub struct TerrainShadowDraw {
    /// `SetIndices(shadowIndexBufferD3D, nShadowStartBatchVertex)` base vertex.
    pub base_vertex: i32,
    /// `DrawIndexedPrimitive`'s `startIndex` (`nShadowStartBatchIndex`).
    pub start_index: i32,
    /// `numPolys`.
    pub polys: i32,
    /// `numVerts`.
    pub verts: i32,
    /// The vertices written.
    pub vertices: Vec<super::w3d_volumetric_shadow::ShadowVolumeVertex>,
    /// The indices written.
    pub indices: Vec<u16>,
}

/// The result of `renderShadows`.
#[derive(Clone, Debug, Default)]
pub struct ProjectedShadowRender {
    /// The returned `projectionCount`.
    pub projection_count: i32,
    /// The decal flushes, in order.
    pub decal_flushes: Vec<DecalFlushDraw>,
    /// The terrain shadow draws, in order (`SHADOW_PROJECTION`).
    pub terrain_shadows: Vec<TerrainShadowDraw>,
    /// The receivers rendered with the shadow's material pass: `(shadow slot, receiver render object)`.
    pub receivers: Vec<(usize, RenderObjHandle)>,
}

/// `class W3DProjectedShadow`: object for maintaining and updating an object's shadow texture.
#[derive(Clone, Debug)]
pub struct W3DProjectedShadow {
    /// The `Shadow` base.
    pub base: Shadow,
    /// `m_shadowTexture[MAX_SHADOW_LIGHTS]`: cached shadow data.
    pub shadow_texture: [Option<ShadowTextureId>; 1],
    /// `m_shadowProjector` exists (`TexProjectClass`, the renderer's).
    pub shadow_projector: bool,
    /// `m_robj`: render object used to cast the shadow.
    pub robj: Option<ShadowRenderObj>,
    /// `m_lastObjPosition`: position of object at time of projection matrix update.
    pub last_obj_position: Vector3,
    /// `m_next`: for the shadow manager list (a slot).
    pub next: Option<usize>,
    /// `m_allowWorldAlign`: wrap shadow around world geometry - else align perpendicular to local z-axis.
    pub allow_world_align: bool,
    /// `m_decalOffsetU`.
    pub decal_offset_u: f32,
    /// `m_decalOffsetV`.
    pub decal_offset_v: f32,
    /// `m_flags`: custom rendering flags.
    pub flags: i32,
}

impl Default for W3DProjectedShadow {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DProjectedShadow {
    /// `W3DProjectedShadow::W3DProjectedShadow` (W3DProjectedShadow.cpp:2188).
    pub fn new() -> Self {
        let mut base = Shadow::new();
        base.diffuse = 0xffff_ffff;
        base.shadow_type = SHADOW_NONE; // type of projection
        base.is_enabled = true;
        base.is_invisible_enabled = false;
        W3DProjectedShadow {
            base,
            shadow_texture: [None],
            shadow_projector: false,
            robj: None,
            last_obj_position: Vector3::new(0.0, 0.0, 0.0),
            next: None,
            allow_world_align: false, // wrap shadow around world geometry - else align perpendicular to local z-axis.
            decal_offset_u: 0.0,
            decal_offset_v: 0.0,
            flags: 0,
        }
    }

    /// `W3DProjectedShadow::setRenderObject` (W3DProjectedShadow.h).
    pub fn set_render_object(&mut self, robj: Option<ShadowRenderObj>) {
        self.robj = robj;
    }

    /// `W3DProjectedShadow::setObjPosHistory` (W3DProjectedShadow.h): position of object when projection matrix was
    /// updated.
    pub fn set_obj_pos_history(&mut self, pos: Vector3) {
        self.last_obj_position = pos;
    }

    /// `W3DProjectedShadow::setTexture` (W3DProjectedShadow.h): texture with light's shadow.
    pub fn set_texture(&mut self, light_index: usize, texture: Option<ShadowTextureId>) {
        self.shadow_texture[light_index] = texture;
    }

    /// `W3DProjectedShadow::getTexture` (W3DProjectedShadow.h).
    pub fn get_texture(&self, light_index: usize) -> Option<ShadowTextureId> {
        self.shadow_texture[light_index]
    }

    /// `W3DProjectedShadow::getRenderCost` (W3DProjectedShadow.cpp:2178, `_DEBUG`/`_INTERNAL` only): whether the shadow
    /// adds a draw call to the render cost.
    pub fn get_render_cost_draw_calls(&self, use_shadow_decals: bool) -> i32 {
        if use_shadow_decals && self.base.is_enabled && !self.base.is_invisible_enabled {
            1
        } else {
            0
        }
    }

    /// `W3DProjectedShadow::init` (W3DProjectedShadow.cpp:2207): allocates local member variables used for projection.
    /// The `TexProjectClass` (`Set_Intensity(0.4f, true)`, `Set_Texture(m_shadowTexture[0]->getTexture())`) is the
    /// renderer's.
    pub fn init(&mut self) {
        // DEBUG_ASSERTCRASH(m_shadowProjector == NULL, ("Init of existing shadow projector"));

        if self.base.shadow_type == SHADOW_PROJECTION {
            self.shadow_projector = true;
        }
    }

    /// `W3DProjectedShadow::updateTexture` (W3DProjectedShadow.cpp:2223): updates the shadow texture image using the render
    /// object and the given light position. `textures` is the manager's texture storage.
    pub fn update_texture(&mut self, light_pos: &Vector3, light_pos_world0: &Vector3, textures: &mut W3DShadowTextureManager) {
        // default uv coordinates before rotation starting at top/left going clockwise (unused: uvData)

        // force light always 2000 units from object - for some reason projection fails if light is too far.
        // @todo: See why infinite light sources don't project shadows correctly.

        let tex_id = self.shadow_texture[0].unwrap();
        let robj = self.robj.clone().unwrap();
        if self.base.shadow_type == SHADOW_PROJECTION {
            // projected shadows use custom runtime generated textures based on object geometry
            let obj_pos = robj.position;
            if obj_pos == Vector3::new(0.0, 0.0, 0.0) {
                return; // render object does not have a valid position (never rendered).
            }
            let mut obj_to_light = *light_pos - obj_pos;
            obj_to_light.normalize();
            let _obj_to_light = obj_pos + obj_to_light * 2000.0;

            // upstream: m_shadowProjector->Compute_Perspective_Projection(m_robj, objToLight), Set_Render_Target(
            // getRenderTarget()), context->light_environment->Reset(position, (0,0,0)) (ambient 0 for a black shadow),
            // Compute_Texture(m_robj, context), then Copy the render target surface into the permanent texture
            // (512x512) — device-only, not translated
            textures.storage[tex_id].update_bounds(light_pos_world0, &robj); // update local shadow bounds
        } else if self.base.shadow_type == SHADOW_DECAL {
            // decal shadows use artist supplied textures.  We just need to tweak the uv coordinates to match the light
            // direction.
            let obj_pos = robj.position;
            let mut object_to_light: Vector3;
            if (self.flags as u32 & SHADOW_DIRECTIONAL_PROJECTION) != 0 {
                object_to_light = *light_pos - obj_pos;
                // we're ignoring sun's distance from horizon, so drop vertical component
                object_to_light.z = 0.0;
                object_to_light.normalize();
            } else {
                object_to_light = Vector3::new(1.0, 0.0, 0.0);
            }

            let desc = textures.storage[tex_id].texture.unwrap();
            // default shadow texture points along world -x axis (west).  Rotate uv coordinates to fit actual light direction
            let u_vec = object_to_light * DECAL_TEXELS_PER_WORLD_UNIT / desc.width as f32;
            object_to_light.rotate_z_sc(-1.0, 0.0); // rotate u vector by -90 degress to get v vector.
            let v_vec = object_to_light * DECAL_TEXELS_PER_WORLD_UNIT / desc.height as f32;

            textures.storage[tex_id].set_decal_uv_axis(u_vec, v_vec);

            // @todo: tweak decal bounding volumes to something sensible
            // shadow uses same bounding box as object
            textures.storage[tex_id].area_effect_sphere = robj.obj_space_bounding_sphere;
            textures.storage[tex_id].area_effect_box = robj.obj_space_bounding_box;
        }

        textures.storage[tex_id].last_light_position = *light_pos; // store position of light at time of texture update.
    }

    /// `W3DProjectedShadow::updateProjectionParameters` (W3DProjectedShadow.cpp:2273): recompute projection matrix -
    /// needed when light or object moves (`m_shadowProjector->Pre_Render_Update(cameraXform)`, the renderer's).
    pub fn update_projection_parameters(&mut self, _camera_xform: &Matrix3D) {}

    /// `W3DProjectedShadow::update` (W3DProjectedShadow.cpp:2278): updates the texture and/or projection parameters when
    /// the object or light moves. `light_pos_world0` is `TheW3DShadowManager->getLightPosWorld(0)`.
    pub fn update(&mut self, light_pos_world0: &Vector3, textures: &mut W3DShadowTextureManager) {
        let tex_id = self.shadow_texture[0].unwrap();
        if textures.storage[tex_id].last_light_position != *light_pos_world0 {
            // light has moved since last time this shadow was calculated. Need update
            self.update_texture(light_pos_world0, light_pos_world0, textures);
        }
        let robj_pos = self.robj.as_ref().unwrap().position;
        if self.last_obj_position != robj_pos {
            // object has moved.  Texture stays the same but projection matrix needs updating.
            // force light always 2000 units from object - for some reason projection fails if light is too far.
            // @todo: See why infinite light sources don't project shadows correctly.
            if self.base.shadow_type == SHADOW_PROJECTION {
                let mut obj_to_light = *light_pos_world0 - robj_pos;
                obj_to_light.normalize();
                let _obj_to_light = robj_pos + obj_to_light * 2000.0;

                // m_shadowProjector->Compute_Perspective_Projection(m_robj, objToLight) — device-only
            }
            self.set_obj_pos_history(robj_pos);
        }
    }
}

/// `class W3DProjectedShadowManager`.
pub struct W3DProjectedShadowManager {
    /// The shadow slot table (`m_shadowList` and `m_decalList` link slots through `m_next`).
    pub shadows: Vec<Option<W3DProjectedShadow>>,
    /// `m_shadowList`.
    pub shadow_list: Option<usize>,
    /// `m_decalList`.
    pub decal_list: Option<usize>,
    /// `m_dynamicRenderTarget`: offscreen video memory texture used to render all shadow textures.
    pub dynamic_render_target: Option<TextureHandle>,
    /// `m_renderTargetHasAlpha`: does render target have destination alpha support?
    pub render_target_has_alpha: bool,
    /// `m_shadowCamera` and `m_shadowContext` exist (created by `init`).
    pub shadow_context_present: bool,
    /// `m_W3DShadowTextureManager`.
    pub w3d_shadow_texture_manager: Option<W3DShadowTextureManager>,
    /// `m_numDecalShadows`: number of decal shadows in the system.
    pub num_decal_shadows: i32,
    /// `m_numProjectionShadows`: number of projected shadows in the system.
    pub num_projection_shadows: i32,
    /// `shadowDecalVertexBufferD3D` (`SHADOW_DECAL_VERTEX_SIZE` vertices); `None` while released.
    pub decal_vertex_buffer: Option<Vec<ShadowDecalVertex>>,
    /// `shadowDecalIndexBufferD3D` (`SHADOW_DECAL_INDEX_SIZE` indices); `None` while released.
    pub decal_index_buffer: Option<Vec<u16>>,
    /// `nShadowDecalVertsInBuf`.
    pub n_shadow_decal_verts_in_buf: i32,
    /// `nShadowDecalStartBatchVertex`.
    pub n_shadow_decal_start_batch_vertex: i32,
    /// `nShadowDecalIndicesInBuf`.
    pub n_shadow_decal_indices_in_buf: i32,
    /// `nShadowDecalStartBatchIndex`.
    pub n_shadow_decal_start_batch_index: i32,
    /// `nShadowDecalPolysInBatch`.
    pub n_shadow_decal_polys_in_batch: i32,
    /// `nShadowDecalVertsInBatch`.
    pub n_shadow_decal_verts_in_batch: i32,
    /// `drawEdgeX` (a file-static).
    pub draw_edge_x: i32,
    /// `drawEdgeY`.
    pub draw_edge_y: i32,
    /// `drawStartX`.
    pub draw_start_x: i32,
    /// `drawStartY`.
    pub draw_start_y: i32,
    /// The decal flushes made outside `renderShadows` (by `queueDecal`'s buffer-full path), taken by the caller.
    pub pending_flushes: Vec<DecalFlushDraw>,
    /// The shared shadow volume buffers (`shadowVertexBufferD3D` etc., owned by the volumetric system upstream).
    pub volume_buffers: ShadowVolumeBuffers,
}

impl Default for W3DProjectedShadowManager {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DProjectedShadowManager {
    /// `W3DProjectedShadowManager::W3DProjectedShadowManager` (W3DProjectedShadow.cpp:147).
    pub fn new() -> Self {
        W3DProjectedShadowManager {
            shadows: Vec::new(),
            shadow_list: None,
            decal_list: None,
            dynamic_render_target: None,
            render_target_has_alpha: false,
            shadow_context_present: false,
            w3d_shadow_texture_manager: None,
            num_decal_shadows: 0,
            num_projection_shadows: 0,
            decal_vertex_buffer: None,
            decal_index_buffer: None,
            n_shadow_decal_verts_in_buf: 0,
            n_shadow_decal_start_batch_vertex: 0,
            n_shadow_decal_indices_in_buf: 0,
            n_shadow_decal_start_batch_index: 0,
            n_shadow_decal_polys_in_batch: 0,
            n_shadow_decal_verts_in_batch: 0,
            draw_edge_x: 0,
            draw_edge_y: 0,
            draw_start_x: 0,
            draw_start_y: 0,
            pending_flushes: Vec::new(),
            volume_buffers: ShadowVolumeBuffers::new(),
        }
    }

    /// `W3DProjectedShadowManager::~W3DProjectedShadowManager` (W3DProjectedShadow.cpp:159).
    pub fn destroy(&mut self) {
        self.release_resources();
        self.dynamic_render_target = None;
        self.render_target_has_alpha = false;
        self.shadow_context_present = false;
        self.w3d_shadow_texture_manager = None;

        // all shadows should be freed up at this point but check anyway
        // DEBUG_ASSERTCRASH(m_shadowList == NULL, ("Destroy of non-empty projected shadow list"));
        // DEBUG_ASSERTCRASH(m_decalList == NULL, ("Destroy of non-empty projected decal list"));
    }

    fn textures(&mut self) -> &mut W3DShadowTextureManager {
        self.w3d_shadow_texture_manager.as_mut().unwrap()
    }

    /// `W3DProjectedShadowManager::getRenderTarget` (W3DProjectedShadow.h).
    pub fn get_render_target(&self) -> Option<TextureHandle> {
        self.dynamic_render_target
    }

    /// `W3DProjectedShadowManager::reset` (W3DProjectedShadow.cpp:172): free all existing shadows - ready for next map.
    pub fn reset(&mut self) {
        // DEBUG_ASSERTCRASH(m_shadowList == NULL, ("Reset of non-empty projected shadow list"));
        // DEBUG_ASSERTCRASH(m_decalList == NULL, ("Reset of non-empty projected decal list"));

        self.textures().free_all_textures();
    }

    /// `W3DProjectedShadowManager::init` (W3DProjectedShadow.cpp:182): allocate one-time shadow assets for length of
    /// entire game (the `CameraClass` and `SpecialRenderInfoClass` are the renderer's).
    pub fn init(&mut self) -> bool {
        self.w3d_shadow_texture_manager = Some(W3DShadowTextureManager::new());
        self.shadow_context_present = true;

        true
    }

    /// `W3DProjectedShadowManager::ReAcquireResources` (W3DProjectedShadow.cpp:193): grab assets which don't survive a
    /// device reset and need to be present for duration of game. (The render target is made by `env` in
    /// [`W3DProjectedShadowManager::re_acquire_render_target`]; the buffers here.)
    pub fn re_acquire_resources(&mut self) -> bool {
        // @todo: We should allocate our render target pool here.
        // DEBUG_ASSERTCRASH(shadowDecalIndexBufferD3D == NULL && ...)

        // CreateIndexBuffer(SHADOW_DECAL_INDEX_SIZE*sizeof(WORD), WRITEONLY|DYNAMIC, INDEX16, POOL_DEFAULT)
        self.decal_index_buffer = Some(vec![0u16; SHADOW_DECAL_INDEX_SIZE as usize]);

        if self.decal_vertex_buffer.is_none() {
            // Create vertex buffer
            self.decal_vertex_buffer = Some(vec![ShadowDecalVertex::default(); SHADOW_DECAL_VERTEX_SIZE as usize]);
        }

        true
    }

    /// The render target part of `ReAcquireResources` (W3DProjectedShadow.cpp:197): `DX8Wrapper::Create_Render_Target` with
    /// destination alpha, else without.
    pub fn re_acquire_render_target(&mut self, env: &mut impl ProjectedShadowEnv) {
        // DEBUG_ASSERTCRASH(m_dynamicRenderTarget == NULL, ("Acquire of existing shadow render target"));
        self.render_target_has_alpha = true;
        self.dynamic_render_target = env.create_render_target(DEFAULT_RENDER_TARGET_WIDTH, DEFAULT_RENDER_TARGET_HEIGHT, true);
        if self.dynamic_render_target.is_none() {
            self.render_target_has_alpha = false;

            // failed to get a render target with alpha. try again without.
            self.dynamic_render_target = env.create_render_target(DEFAULT_RENDER_TARGET_WIDTH, DEFAULT_RENDER_TARGET_HEIGHT, false);
        }
    }

    /// `W3DProjectedShadowManager::ReleaseResources` (W3DProjectedShadow.cpp:243): release device dependent D3D resources.
    pub fn release_resources(&mut self) {
        self.invalidate_cached_light_positions(); // textures need to be updated
        self.dynamic_render_target = None; // need to create a new render target
        self.decal_index_buffer = None;
        self.decal_vertex_buffer = None;
    }

    /// `W3DProjectedShadowManager::invalidateCachedLightPositions` (W3DProjectedShadow.cpp:255): forces shadows to update
    /// regardless of last lightposition.
    pub fn invalidate_cached_light_positions(&mut self) {
        if let Some(t) = self.w3d_shadow_texture_manager.as_mut() {
            t.invalidate_cached_light_positions();
        }
    }

    /// `W3DProjectedShadowManager::updateRenderTargetTextures` (W3DProjectedShadow.cpp:260): render into any textures that
    /// need updating.
    pub fn update_render_target_textures(&mut self, use_shadow_decals: bool, light_pos_world0: &Vector3) {
        // @todo: Don't update texture for shadows that can't be seen!!

        if self.shadow_list.is_none() {
            return; // there are no shadows to render.
        }

        if !use_shadow_decals {
            return;
        }

        if self.num_projection_shadows != 0 {
            let mut cur = self.shadow_list;
            while let Some(slot) = cur {
                // decals don't need any updates on a per-frame basis since the image never changes.
                let mut shadow = self.shadows[slot].take().unwrap();
                if shadow.base.shadow_type != SHADOW_DECAL {
                    shadow.update(light_pos_world0, self.w3d_shadow_texture_manager.as_mut().unwrap());
                }
                cur = shadow.next;
                self.shadows[slot] = Some(shadow);
            }
        }
    }

    /// `W3DProjectedShadowManager::renderProjectedTerrainShadow` (W3DProjectedShadow.cpp:285): renders shadow on part of
    /// terrain covered by world-space bounding box `bx`. Returns the draw when upstream returns 1.
    pub fn render_projected_terrain_shadow(&mut self, terrain: &Option<ShadowTerrain>, bx: &AABoxClass) -> Option<TerrainShadowDraw> {
        // static mWorld(identity) is the world transform — device-only
        let terrain = terrain.as_ref()?;
        let hmap_arc = terrain.map.clone();
        let hmap = hmap_arc.read().unwrap();

        // Find size of heightmap sub-rectangle affected by shadow
        let cx = bx.center.x;
        let cy = bx.center.y;
        let dx = bx.extent.x;
        let dy = bx.extent.y;
        let map_scale_inv = 1.0 / MAP_XY_FACTOR;

        // Get terrain cell index for area with shadow
        let mut start_x = real_to_int_floor((cx - dx) * map_scale_inv);
        let mut end_x = real_to_int_ceil((cx + dx) * map_scale_inv);
        let mut start_y = real_to_int_floor((cy - dy) * map_scale_inv);
        let mut end_y = real_to_int_ceil((cy + dy) * map_scale_inv);

        // clip bounds to extents of heightmap
        start_x = start_x.max(0);
        end_x = end_x.min(hmap.width - 1);
        start_y = start_y.max(0);
        end_y = end_y.min(hmap.height - 1);

        let verts_per_row = end_x - start_x + 1; // number of cells +1
        let verts_per_column = end_y - start_y + 1; // number of cells +1

        if verts_per_row == 1 || verts_per_column == 1 {
            return None; // nothing to render
        }

        let num_verts = verts_per_row * verts_per_column; // number of terrain vertices

        let vb = &mut self.volume_buffers;
        let vstart: i32;
        if vb.n_shadow_verts_in_buf > (vb.shadow_vertex_size - num_verts) {
            // check if room for model verts: flush the buffer by drawing the contents and re-locking again (DISCARD)
            vb.n_shadow_verts_in_buf = 0;
            vb.n_shadow_start_batch_vertex = 0;
            vstart = 0;
        } else {
            vstart = vb.n_shadow_verts_in_buf; // NOOVERWRITE
        }

        let mut vertices = Vec::with_capacity(num_verts as usize);
        // insert each cell's bottom/left edge vertex
        for j in start_y..=end_y {
            let ycoord = j as f32 * MAP_XY_FACTOR;

            for i in start_x..=end_x {
                vertices.push(super::w3d_volumetric_shadow::ShadowVolumeVertex {
                    x: i as f32 * MAP_XY_FACTOR,
                    y: ycoord,
                    z: hmap.get_height(i, j) as f32 * MAP_HEIGHT_SCALE,
                });
            }
        }
        for (k, v) in vertices.iter().enumerate() {
            vb.vertex_buffer[vstart as usize + k] = *v;
        }

        let num_index = (end_x - start_x) * (end_y - start_y) * 6; // 6 indices per terrain cell (2 triangles).

        let istart: i32;
        if vb.n_shadow_indices_in_buf > (vb.shadow_index_size - num_index) {
            vb.n_shadow_indices_in_buf = 0;
            vb.n_shadow_start_batch_index = 0;
            istart = 0;
        } else {
            istart = vb.n_shadow_indices_in_buf;
        }

        // fill each cell's vertex indices
        let mut indices: Vec<u16> = Vec::with_capacity(num_index as usize);
        let mut row_start = 0;
        for j in start_y..end_y {
            let mut i = row_start;
            for k in start_x..end_x {
                // @todo: Fix this to deal with flipped triangles. hmap->getAlphaUVData(k, j, ..., &flipForBlend, false)
                let flip_for_blend = hmap.get_alpha_uv_data(k, j, false).3;
                // @todo: fix the winding order in heightmap to be in strip order like above!
                if flip_for_blend {
                    indices.push((i + 1) as u16);
                    indices.push((i + verts_per_row) as u16);
                    indices.push(i as u16);
                    indices.push((i + 1) as u16);
                    indices.push((i + 1 + verts_per_row) as u16);
                    indices.push((i + verts_per_row) as u16);
                } else {
                    indices.push(i as u16);
                    indices.push((i + 1 + verts_per_row) as u16);
                    indices.push((i + verts_per_row) as u16);
                    indices.push(i as u16);
                    indices.push((i + 1) as u16);
                    indices.push((i + 1 + verts_per_row) as u16);
                }
                i += 1;
            }
            row_start += verts_per_row;
        }
        for (k, v) in indices.iter().enumerate() {
            vb.index_buffer[istart as usize + k] = *v;
        }

        // SetIndices(shadowIndexBufferD3D, nShadowStartBatchVertex), SetTransform(WORLD, identity), SetStreamSource,
        // SetVertexShader(D3DFVF_XYZ); alpha test on; stencil ALWAYS, ref 1, INCR on pass; lighting off;
        // SRCBLEND DESTCOLOR, DESTBLEND ZERO — device-only, not translated
        let num_polys = (end_x - start_x) * (end_y - start_y) * 2; // 2 triangles per cell
        let draw = TerrainShadowDraw {
            base_vertex: vb.n_shadow_start_batch_vertex,
            start_index: vb.n_shadow_start_batch_index,
            polys: num_polys,
            verts: num_verts,
            vertices,
            indices,
        };
        // (render states restored: alpha test off, stencil off, lighting on)

        vb.n_shadow_verts_in_buf += num_verts;
        vb.n_shadow_start_batch_vertex = vb.n_shadow_verts_in_buf;

        vb.n_shadow_indices_in_buf += num_index;
        vb.n_shadow_start_batch_index = vb.n_shadow_indices_in_buf;
        Some(draw)
    }

    /// `W3DProjectedShadowManager::flushDecals` (W3DProjectedShadow.cpp:630): empty the queue by rendering all decals with
    /// the given texture. `None` when there is nothing to render.
    pub fn flush_decals(&mut self, texture: Option<ShadowTextureId>, shadow_type: u32) -> Option<DecalFlushDraw> {
        if self.n_shadow_decal_verts_in_batch == 0 && self.n_shadow_decal_polys_in_batch == 0 {
            // nothing to render
            return None;
        }

        // upstream: Set_Material(PRELIT_DIFFUSE), Set_Texture(0, texture->getTexture()), the shader by type, Apply_Render_State_
        // Changes, SetIndices/SetTransform/SetStreamSource/SetVertexShader, DrawIndexedPrimitive — device-only
        let vb = self.decal_vertex_buffer.as_ref().unwrap();
        let ib = self.decal_index_buffer.as_ref().unwrap();
        let v0 = self.n_shadow_decal_start_batch_vertex as usize;
        let i0 = self.n_shadow_decal_start_batch_index as usize;
        let tex = texture.and_then(|t| self.w3d_shadow_texture_manager.as_ref().and_then(|m| m.storage[t].texture));
        let draw = DecalFlushDraw {
            texture: tex,
            shadow_type,
            base_vertex: self.n_shadow_decal_start_batch_vertex,
            start_index: self.n_shadow_decal_start_batch_index,
            polys: self.n_shadow_decal_polys_in_batch,
            verts: self.n_shadow_decal_verts_in_batch,
            vertices: vb[v0..v0 + self.n_shadow_decal_verts_in_batch as usize].to_vec(),
            indices: ib[i0..i0 + (self.n_shadow_decal_polys_in_batch * 3) as usize].to_vec(),
        };

        self.n_shadow_decal_start_batch_vertex = self.n_shadow_decal_verts_in_buf;
        self.n_shadow_decal_start_batch_index = self.n_shadow_decal_indices_in_buf;
        self.n_shadow_decal_polys_in_batch = 0; // reset number of polys in texture batch
        self.n_shadow_decal_verts_in_batch = 0;
        Some(draw)
    }

    /// The buffer-full part both queue functions share: flush the batch, restart the vertex or index buffer at 0.
    fn make_room_for_decal_vertices(&mut self, shadow_slot: usize, num_verts: i32) -> usize {
        if self.n_shadow_decal_verts_in_buf > (SHADOW_DECAL_VERTEX_SIZE - num_verts) {
            // check if room for model verts: flush the buffer by drawing the contents and re-locking again (DISCARD)
            let (tex, ty) = {
                let s = self.shadows[shadow_slot].as_ref().unwrap();
                (s.shadow_texture[0], s.base.shadow_type)
            };
            if let Some(d) = self.flush_decals(tex, ty) {
                self.pending_flushes.push(d);
            }

            self.n_shadow_decal_start_batch_vertex = 0;
            self.n_shadow_decal_polys_in_batch = 0; // reset number of polys in texture batch
            self.n_shadow_decal_verts_in_batch = 0;
            self.n_shadow_decal_verts_in_buf = 0;
            0
        } else {
            self.n_shadow_decal_verts_in_buf as usize // NOOVERWRITE
        }
    }

    fn make_room_for_decal_indices(&mut self, shadow_slot: usize, num_index: i32) -> usize {
        if self.n_shadow_decal_indices_in_buf > (SHADOW_DECAL_INDEX_SIZE - num_index) {
            let (tex, ty) = {
                let s = self.shadows[shadow_slot].as_ref().unwrap();
                (s.shadow_texture[0], s.base.shadow_type)
            };
            if let Some(d) = self.flush_decals(tex, ty) {
                self.pending_flushes.push(d);
            }

            self.n_shadow_decal_start_batch_index = 0;
            self.n_shadow_decal_polys_in_batch = 0; // reset number of polys in texture batch
            self.n_shadow_decal_verts_in_batch = 0;
            self.n_shadow_decal_indices_in_buf = 0;
            0
        } else {
            self.n_shadow_decal_indices_in_buf as usize
        }
    }

    /// `W3DProjectedShadowManager::queueDecal` (W3DProjectedShadow.cpp:795): decals have a low poly count so it's better
    /// to render large numbers at once. This system will queue them up until the buffers fill up. It will then flush the
    /// buffer (draw decals) and be ready for new decals. This is an optimized system that only uses the render objects
    /// bounding box to determine shadow visibility.
    pub fn queue_decal(&mut self, shadow_slot: usize, env: &impl ProjectedShadowEnv) {
        let terrain = match env.terrain() {
            Some(t) => t,
            None => return,
        };
        let hmap_arc = terrain.map.clone();
        let hmap = hmap_arc.read().unwrap();
        let shadow = self.shadows[shadow_slot].clone().unwrap();

        let map_scale_inv = 1.0 / MAP_XY_FACTOR;
        let mut layer_height: f32 = 0.0;
        let border_size = hmap.border_size;
        let obj_pos: Vector3;
        let mut obj_xform = Matrix3D::identity();
        if let Some(robj) = shadow.robj.as_ref() {
            obj_pos = robj.position;
            obj_xform = robj.transform;
            if let Some(layer) = robj.owner_layer {
                if layer != LAYER_GROUND {
                    // check if object that this decal belongs to is not on the ground (bridge?)
                    layer_height = BRIDGE_OFFSET_FACTOR + env.layer_height(obj_pos.x, obj_pos.y, layer);
                }
            }
        } else {
            // no render object so use shadow's local position and default orientation
            obj_pos = Vector3::new(shadow.base.x, shadow.base.y, shadow.base.z);
            obj_xform.rotate_z(shadow.base.local_angle);
        }
        let mut obj_pos = obj_pos;

        // Find size of heightmap sub-rectangle affected by shadow
        // If user supplied size values, ignore bounding box

        obj_pos.z = 0.0; // we don't care about object height since shadows project top-down

        let mut u_vector = obj_xform.get_x_vector();
        let mut v_vector: Vector3;

        u_vector.z = 0.0;
        let mut vec_length = u_vector.length();
        if vec_length != 0.0 {
            // prevent divide by zero
            u_vector *= 1.0 / vec_length;
            v_vector = u_vector;
            v_vector.rotate_z_sc(-1.0, 0.0); // rotate u vector by -90 degress to get v vector.
        } else {
            v_vector = obj_xform.get_y_vector();
            v_vector.z = 0.0;
            vec_length = v_vector.length();
            if vec_length != 0.0 {
                // prevent divide by zero
                v_vector *= 1.0 / vec_length;
            } else {
                v_vector.set(0.0, -1.0, 0.0); // Point uvector in default direction
            }

            u_vector = v_vector;
            u_vector.rotate_z_sc(1.0, 0.0); // rotate v vector by 90 degress to get u vector.
        }

        // Compute bounding box of projection
        let mut box_corners = [Vector3::default(); 4]; // top-left, top-right, bottom-right, bottom-left
        let dx = shadow.base.decal_size_x;
        let dy = shadow.base.decal_size_y;
        let left_x = (-dx) * (u_vector * (0.5 + shadow.decal_offset_u));
        let right_x = dx * (u_vector * (0.5 - shadow.decal_offset_u));
        let top_y = (-dy) * (v_vector * (0.5 + shadow.decal_offset_v));
        let bottom_y = dy * (v_vector * (0.5 - shadow.decal_offset_v));
        // @todo: Optimize this bounding box calculation to use transformed extents
        // Also skip bounding box calculation if object has not moved.
        box_corners[0] = left_x + top_y;
        box_corners[1] = right_x + top_y;
        box_corners[2] = right_x + bottom_y;
        box_corners[3] = left_x + bottom_y;

        let mut max_x = box_corners[0].x;
        let mut min_x = max_x;
        let mut max_y = box_corners[0].y;
        let mut min_y = max_y;

        for bi in 1..4 {
            max_x = max_x.max(box_corners[bi].x);
            min_x = min_x.min(box_corners[bi].x);
            max_y = max_y.max(box_corners[bi].y);
            min_y = min_y.min(box_corners[bi].y);
        }

        u_vector *= shadow.base.oow_decal_size_x;
        v_vector *= shadow.base.oow_decal_size_y;
        let u_offset = shadow.decal_offset_u + 0.5;
        let v_offset = shadow.decal_offset_v + 0.5;

        // (upstream then copies the never-initialised local `box` into cx, cy, dx, dy: not used, not translated)

        // Get terrain cell index for area with shadow
        let mut start_x = real_to_int_floor((obj_pos.x + min_x) * map_scale_inv) + border_size;
        let mut end_x = real_to_int_ceil((obj_pos.x + max_x) * map_scale_inv) + border_size;
        let mut start_y = real_to_int_floor((obj_pos.y + min_y) * map_scale_inv) + border_size;
        let mut end_y = real_to_int_ceil((obj_pos.y + max_y) * map_scale_inv) + border_size;

        start_x = start_x.max(self.draw_start_x);
        start_x = start_x.min(self.draw_edge_x);
        start_y = start_y.max(self.draw_start_y);
        start_y = start_y.min(self.draw_edge_y);

        end_x = end_x.max(self.draw_start_x);
        end_x = end_x.min(self.draw_edge_x);
        end_y = end_y.max(self.draw_start_y);
        end_y = end_y.min(self.draw_edge_y);

        // Check if decal too large to fit inside 65536 index buffer. try clipping each direction to < 104 since that's
        // more than enough to cover typical map.
        let num_extra_x = (end_x - start_x + 1) - 104;
        if num_extra_x > 0 {
            // figure out how much to clip out at each edge of decal
            let num_start_extra_x = real_to_int_floor(num_extra_x as f32 / 2.0);
            let num_edge_extra_x = num_extra_x - num_start_extra_x;
            start_x += num_start_extra_x;
            end_x -= num_edge_extra_x;
        }
        let num_extra_y = (end_y - start_y + 1) - 104;
        if num_extra_y > 0 {
            let num_start_extra_y = real_to_int_floor(num_extra_y as f32 / 2.0);
            let num_edge_extra_y = num_extra_y - num_start_extra_y;

            start_y += num_start_extra_y;
            end_y -= num_edge_extra_y;
        }

        let verts_per_row = end_x - start_x + 1; // number of cells +1
        let verts_per_column = end_y - start_y + 1; // number of cells +1

        if verts_per_row <= 1 || verts_per_column <= 1 {
            return; // nothing to render
        }

        let num_verts = verts_per_row * verts_per_column; // number of terrain vertices
        let num_index = (end_x - start_x) * (end_y - start_y) * 6; // 6 indices per terrain cell (2 triangles).

        let vstart = self.make_room_for_decal_vertices(shadow_slot, num_verts);

        // code to deal with rotated shadows based on sun direction, fix this later.  For now shadow rotates with object
        // rotation.
        // DEBUG_ASSERTCRASH(numVerts == ((endY-startY+1)*(endX-startX+1)), ("queueDecal VB size mismatch"));

        let mut k_out = vstart;
        {
            let vb = self.decal_vertex_buffer.as_mut().unwrap();
            let diffuse = shadow.base.diffuse;
            if layer_height != 0.0 {
                for j in start_y..=end_y {
                    let mut hmap_vertex = Vector3::default();
                    hmap_vertex.y = (j - border_size) as f32 * MAP_XY_FACTOR;

                    for i in start_x..=end_x {
                        hmap_vertex.x = (i - border_size) as f32 * MAP_XY_FACTOR;
                        hmap_vertex.z = (hmap.get_height(i, j) as f32 * MAP_HEIGHT_SCALE).max(layer_height);
                        vb[k_out] = ShadowDecalVertex {
                            x: hmap_vertex.x,
                            y: hmap_vertex.y,
                            z: hmap_vertex.z,
                            diffuse,
                            u: Vector3::dot_product(&u_vector, &(hmap_vertex - obj_pos)) + u_offset,
                            v: Vector3::dot_product(&v_vector, &(hmap_vertex - obj_pos)) + v_offset,
                        };
                        k_out += 1;
                    }
                }
            } else {
                // insert each cell's bottom/left edge vertex
                for j in start_y..=end_y {
                    let mut hmap_vertex = Vector3::default();
                    hmap_vertex.y = (j - border_size) as f32 * MAP_XY_FACTOR;

                    for i in start_x..=end_x {
                        hmap_vertex.x = (i - border_size) as f32 * MAP_XY_FACTOR;
                        hmap_vertex.z = hmap.get_height(i, j) as f32 * MAP_HEIGHT_SCALE + 0.01 * MAP_XY_FACTOR;
                        vb[k_out] = ShadowDecalVertex {
                            x: hmap_vertex.x,
                            y: hmap_vertex.y,
                            z: hmap_vertex.z,
                            diffuse,
                            u: Vector3::dot_product(&u_vector, &(hmap_vertex - obj_pos)) + u_offset,
                            v: Vector3::dot_product(&v_vector, &(hmap_vertex - obj_pos)) + v_offset,
                        };
                        k_out += 1;
                    }
                }
            }
        }

        let istart = self.make_room_for_decal_indices(shadow_slot, num_index);

        {
            // fill each cell's vertex indices
            let batch = self.n_shadow_decal_verts_in_batch;
            let ib = self.decal_index_buffer.as_mut().unwrap();
            let mut p = istart;
            let mut row_start = 0;
            for j in start_y..end_y {
                let mut i = row_start;
                for k in start_x..end_x {
                    // @todo: fix the winding order in heightmap to be in strip order like above!
                    if get_flip_state(&hmap, k, j) {
                        ib[p] = (i + 1 + batch) as u16;
                        ib[p + 1] = (i + verts_per_row + batch) as u16;
                        ib[p + 2] = (i + batch) as u16;
                        ib[p + 3] = (i + 1 + batch) as u16;
                        ib[p + 4] = (i + 1 + verts_per_row + batch) as u16;
                        ib[p + 5] = (i + verts_per_row + batch) as u16;
                    } else {
                        ib[p] = (i + batch) as u16;
                        ib[p + 1] = (i + 1 + verts_per_row + batch) as u16;
                        ib[p + 2] = (i + verts_per_row + batch) as u16;
                        ib[p + 3] = (i + batch) as u16;
                        ib[p + 4] = (i + 1 + batch) as u16;
                        ib[p + 5] = (i + 1 + verts_per_row + batch) as u16;
                    }
                    p += 6;
                    i += 1;
                }
                row_start += verts_per_row;
            }
        }

        let num_polys = (end_x - start_x) * (end_y - start_y) * 2; // 2 triangles per cell
        self.n_shadow_decal_polys_in_batch += num_polys;

        self.n_shadow_decal_verts_in_buf += num_verts;
        self.n_shadow_decal_verts_in_batch += num_verts;

        self.n_shadow_decal_indices_in_buf += num_index;
    }

    /// `W3DProjectedShadowManager::queueSimpleDecal` (W3DProjectedShadow.cpp:1123): simpler/faster decal system that always
    /// uses 2 triangles that are roughly oriented to terrain. Since they are not projected onto terrain, there may be
    /// clipping artifacts in certain situations. (Never called.)
    pub fn queue_simple_decal(&mut self, shadow_slot: usize, env: &impl ProjectedShadowEnv) {
        if env.terrain().is_none() {
            return;
        }
        let shadow = self.shadows[shadow_slot].clone().unwrap();
        let robj = shadow.robj.clone().unwrap();

        let mut obj_pos = robj.position;
        let obj_xform = robj.transform;
        let (ground_height, normal) = env.height_map_height(obj_pos.x, obj_pos.y);
        let ground_normal = Vector3::new(normal.x, normal.y, normal.z);

        // Find new tu vector parallel to terrain by projecting existing x_vector onto terrain normal and subtracting the
        // result.
        let mut u_vector = obj_xform.get_x_vector();
        let u_vector_along_normal = Vector3::dot_product(&u_vector, &ground_normal);
        u_vector -= u_vector_along_normal * ground_normal;
        u_vector.normalize();
        // Find new tv vector parallel to terrain by crossing new tu vector with terrain normal.
        let v_vector = Vector3::cross_product(&u_vector, &ground_normal);

        let num_verts = 4; // number of decal vertices
        let num_index = 6; // (2 triangles).

        let vstart = self.make_room_for_decal_vertices(shadow_slot, num_verts);

        obj_pos.z = ground_height; // force decal to ground level
        obj_pos += ground_normal * 1.0; // offset decal slightly above terrain to reduce z-fighting.
        {
            let vb = self.decal_vertex_buffer.as_mut().unwrap();
            // Top-left
            let mut vertex = obj_pos + v_vector * shadow.base.decal_size_y * -0.5 - u_vector * shadow.base.decal_size_x * 0.5;
            vb[vstart] = ShadowDecalVertex { x: vertex.x, y: vertex.y, z: vertex.z, diffuse: vb[vstart].diffuse, u: 0.0, v: 0.0 };

            // Bottom-left
            vertex += v_vector * shadow.base.decal_size_y;
            vb[vstart + 1] = ShadowDecalVertex { x: vertex.x, y: vertex.y, z: vertex.z, diffuse: vb[vstart + 1].diffuse, u: 0.0, v: 1.0 };

            // Bottom-right
            vertex += u_vector * shadow.base.decal_size_x;
            vb[vstart + 2] = ShadowDecalVertex { x: vertex.x, y: vertex.y, z: vertex.z, diffuse: vb[vstart + 2].diffuse, u: 1.0, v: 1.0 };

            // Top-right
            vertex -= v_vector * shadow.base.decal_size_y;
            vb[vstart + 3] = ShadowDecalVertex { x: vertex.x, y: vertex.y, z: vertex.z, diffuse: vb[vstart + 3].diffuse, u: 1.0, v: 0.0 };
        }

        let istart = self.make_room_for_decal_indices(shadow_slot, num_index);
        {
            let b = self.n_shadow_decal_verts_in_batch;
            let ib = self.decal_index_buffer.as_mut().unwrap();
            ib[istart] = b as u16;
            ib[istart + 1] = (b + 1) as u16;
            ib[istart + 2] = (b + 2) as u16;
            ib[istart + 3] = b as u16;
            ib[istart + 4] = (b + 2) as u16;
            ib[istart + 5] = (b + 3) as u16;
        }

        let num_polys = 2; // 2 triangles per decal
        self.n_shadow_decal_polys_in_batch += num_polys;

        self.n_shadow_decal_verts_in_buf += num_verts;
        self.n_shadow_decal_verts_in_batch += num_verts;

        self.n_shadow_decal_indices_in_buf += num_index;
    }

    /// `W3DProjectedShadowManager::renderShadows` (W3DProjectedShadow.cpp:1291): iterate over each object and render its
    /// shadow onto affected objects. `light_pos_world0` is unused here (the shadows' updates are
    /// `updateRenderTargetTextures`).
    pub fn render_shadows(&mut self, env: &mut impl ProjectedShadowEnv, _light_pos_world0: &Vector3) -> ProjectedShadowRender {
        let mut out = ProjectedShadowRender::default();
        let mut projection_count = 0;

        if self.shadow_list.is_none() && self.decal_list.is_none() {
            return out; // there are no shadows to render.
        }

        // Find extents of visible terrain
        let terrain = env.terrain();
        match terrain.as_ref() {
            Some(t) => {
                let hmap = t.map.read().unwrap();
                self.draw_edge_y = t.draw_org_y + t.draw_height - 1;
                self.draw_edge_x = t.draw_org_x + t.draw_width - 1;
                if self.draw_edge_x > (hmap.width - 1) {
                    self.draw_edge_x = hmap.width - 1;
                }
                if self.draw_edge_y > (hmap.height - 1) {
                    self.draw_edge_y = hmap.height - 1;
                }
                self.draw_start_x = t.draw_org_x;
                self.draw_start_y = t.draw_org_y;
            }
            None => return out,
        }

        // According to Nvidia there's a D3D bug that happens if you don't start with a new dynamic VB each frame - so we
        // force a DISCARD by overflowing the counter.
        self.n_shadow_decal_verts_in_buf = 0xffff;
        self.n_shadow_decal_indices_in_buf = 0xffff;

        if env.use_shadow_decals() {
            // Render the object: TheDX8MeshRenderer.Set_Camera(&rinfo.Camera)

            // keep track of active decal texture so we can render all decals at once.
            let mut last_shadow_decal_texture: Option<ShadowTextureId> = None;
            let mut last_shadow_type: u32 = SHADOW_NONE;

            let mut cur = self.shadow_list;
            while let Some(slot) = cur {
                let shadow = self.shadows[slot].clone().unwrap();
                cur = shadow.next;
                if shadow.base.is_enabled && !shadow.base.is_invisible_enabled {
                    if (shadow.base.shadow_type & SHADOW_DECAL) != 0 {
                        let head = self.shadows[self.shadow_list.unwrap()].as_ref().unwrap();
                        if last_shadow_decal_texture.is_none() {
                            last_shadow_decal_texture = head.shadow_texture[0];
                        }
                        if last_shadow_type == SHADOW_NONE {
                            last_shadow_type = head.base.shadow_type;
                        }

                        if shadow.shadow_texture[0] != last_shadow_decal_texture || shadow.base.shadow_type != last_shadow_type {
                            // switched to a new texture, need to render polys using last texture.
                            if let Some(d) = self.flush_decals(last_shadow_decal_texture, last_shadow_type) {
                                out.decal_flushes.push(d);
                            }
                            last_shadow_decal_texture = shadow.shadow_texture[0];
                            last_shadow_type = shadow.base.shadow_type;
                        }
                        // @todo: may need to fix this if shadows are large enough to be seen while object is not visible
                        if shadow.robj.as_ref().unwrap().is_really_visible {
                            // queueSimpleDecal(shadow);
                            self.queue_decal(slot, &*env); // only draw shadow if casting object is visible
                            out.decal_flushes.append(&mut self.pending_flushes);
                            projection_count += 1;
                        }
                        continue;
                    }

                    // First test if shadow is visible on screen
                    let tex = self.w3d_shadow_texture_manager.as_ref().unwrap().storage[shadow.shadow_texture[0].unwrap()].clone();
                    let robj = shadow.robj.clone().unwrap();
                    let mut sphere = tex.area_effect_sphere;
                    sphere.center += robj.position;

                    let result = env.frustum_sphere_overlap(&sphere);
                    let mut aa_box = AABoxClass::default();
                    if result == OverlapType::Overlapped {
                        // do a more accurate test against bounding box.
                        aa_box = tex.area_effect_box;
                        aa_box.center += robj.position; // translate bounding box to world space.
                        if env.frustum_box_overlap(&aa_box) == OverlapType::Outside {
                            continue;
                        }
                    } else if result == OverlapType::Outside {
                        continue;
                    }

                    // Shadow is visible on screen.  Figure out which visible objects it may affect.

                    // Check if bounding sphere was inside so bounding box never initialized
                    if result == OverlapType::Inside {
                        aa_box = tex.area_effect_box;
                        aa_box.center += robj.position; // translate bounding box to world space.
                    }

                    if shadow.base.shadow_type == SHADOW_PROJECTION {
                        // build inverse camera/view transforms needed for projection:
                        // shadow->updateProjectionParameters(rinfo.Camera.Get_Transform())

                        // terrain is always visible and affected by all shadows so must render
                        // (Install_Materials, Apply_Render_State_Changes, UnInstall_Materials: device-only)
                        if let Some(d) = self.render_projected_terrain_shadow(&terrain, &aa_box) {
                            out.terrain_shadows.push(d);
                            projection_count += 1;
                        }

                        for r in env.receivers_in_range(&sphere) {
                            // @todo: don't apply shadows to translcuent objects unless they are MOBILE - hack to get tanks to work.
                            if (!r.is_alpha || !r.is_immobile) && r.robj != robj.handle && r.is_really_visible {
                                // do a more accurate test against W3D render bounding boxes.
                                if env.robj_intersects_aabox(r.robj, &aa_box) {
                                    // Shadow reached a visible object so it needs to be rendered with shadow applied
                                    // (Push_Material_Pass(projector's), RINFO_OVERRIDE_ADDITIONAL_PASSES_ONLY, Render).
                                    out.receivers.push((slot, r.robj));
                                    projection_count += 1; // keep track of number of shadow projections
                                }
                            }
                        }
                    }
                } // shadow is enabled
            }

            // make sure there are not any unrendered decals left over.
            if let Some(d) = self.flush_decals(last_shadow_decal_texture, last_shadow_type) {
                out.decal_flushes.push(d);
            }
            // TheDX8MeshRenderer.Flush(): draw all the shadow receiving objects — device-only
        } // rendering shadows
        if self.decal_list.is_some() {
            // keep track of active decal texture so we can render all decals at once.
            let mut last_shadow_decal_texture: Option<ShadowTextureId> = None;
            let mut last_shadow_type: u32 = SHADOW_NONE;

            let mut cur = self.decal_list;
            while let Some(slot) = cur {
                let shadow = self.shadows[slot].clone().unwrap();
                cur = shadow.next;
                if shadow.base.is_enabled && !shadow.base.is_invisible_enabled {
                    let head = self.shadows[self.decal_list.unwrap()].as_ref().unwrap();
                    if last_shadow_decal_texture.is_none() {
                        last_shadow_decal_texture = head.shadow_texture[0];
                    }
                    if last_shadow_type == SHADOW_NONE {
                        last_shadow_type = head.base.shadow_type;
                    }

                    if shadow.shadow_texture[0] != last_shadow_decal_texture || shadow.base.shadow_type != last_shadow_type {
                        // switched to a new texture, need to render polys using last texture.
                        if let Some(d) = self.flush_decals(last_shadow_decal_texture, last_shadow_type) {
                            out.decal_flushes.push(d);
                        }
                        last_shadow_decal_texture = shadow.shadow_texture[0];
                        last_shadow_type = shadow.base.shadow_type;
                    }
                    // @todo: may need to fix this if shadows are large enough to be seen while object is not visible
                    if !(shadow.robj.is_some() && !shadow.robj.as_ref().unwrap().is_really_visible) {
                        // queueSimpleDecal(shadow);
                        self.queue_decal(slot, &*env); // only draw shadow if casting object is visible
                        out.decal_flushes.append(&mut self.pending_flushes);
                        projection_count += 1;
                    }
                } // shadow is enabled
            }

            // make sure there are not any unrendered decals left over.
            if let Some(d) = self.flush_decals(last_shadow_decal_texture, last_shadow_type) {
                out.decal_flushes.push(d);
            }
        }
        out.projection_count = projection_count;
        out
    }

    /// The common tail of the three decal creators: link the new shadow next to others using the same texture in the
    /// given list, and count it. `decal_list` selects `m_decalList` over `m_shadowList`.
    fn link_shadow(&mut self, shadow: W3DProjectedShadow, st: ShadowTextureId, decal_list: bool) -> usize {
        let slot = self.shadows.len();
        self.shadows.push(Some(shadow));

        // add to our shadow list through the shadow next links, insert next to other shadows using same texture
        let head = if decal_list { self.decal_list } else { self.shadow_list };
        let mut prev_shadow: Option<usize> = None;
        let mut next_shadow = head;
        let mut found = false;
        while let Some(ns) = next_shadow {
            if self.shadows[ns].as_ref().unwrap().shadow_texture[0] == Some(st) {
                // found start of other shadows using same texture, insert new shadow here.
                self.shadows[slot].as_mut().unwrap().next = Some(ns);
                if let Some(ps) = prev_shadow {
                    self.shadows[ps].as_mut().unwrap().next = Some(slot);
                } else if decal_list {
                    self.decal_list = Some(slot);
                } else {
                    self.shadow_list = Some(slot);
                }
                found = true;
                break;
            }
            prev_shadow = Some(ns);
            next_shadow = self.shadows[ns].as_ref().unwrap().next;
        }

        if !found {
            // shadow with new texture. Add to top of list.
            self.shadows[slot].as_mut().unwrap().next = head;
            if decal_list {
                self.decal_list = Some(slot);
            } else {
                self.shadow_list = Some(slot);
            }
        }
        slot
    }

    /// Finds or makes the cached decal texture named `<ShadowName>.tga` (`addDecal` and `addShadow`'s decal branch): the
    /// texture is clamped with no mip mapping by the asset manager (`env.get_texture`). `None` when it does not load.
    fn decal_texture(&mut self, texture_name: &[u8], env: &mut impl ProjectedShadowEnv) -> Option<ShadowTextureId> {
        let mgr = self.w3d_shadow_texture_manager.as_mut().unwrap();
        let mut st = mgr.get_texture(texture_name);
        if st.is_none() {
            // Adding a new decal texture
            let w3d_texture = env.get_texture(texture_name)?;

            let mut t = W3DShadowTexture::new();
            t.set_name(texture_name);
            t.texture = Some(w3d_texture);
            st = Some(mgr.add_texture(t));
        }
        st
    }

    /// `W3DProjectedShadowManager::addDecal(shadowInfo)` (W3DProjectedShadow.cpp:1470): generic function which can be used
    /// to create arbitrary decals that don't have to be used for shadows. Some examples: Scorch marks, blood, stains,
    /// selection/status indicators, etc.
    pub fn add_decal_info(&mut self, shadow_info: Option<&ShadowTypeInfo>, env: &mut impl ProjectedShadowEnv) -> Option<usize> {
        let shadow_info = shadow_info?; // right now we require hardware render-to-texture support

        // simple decal using the premade texture specified. can be always perpendicular to model's z-axis or projected
        // onto world geometry.
        let mut texture_name = shadow_info.shadow_name.clone();
        texture_name.extend_from_slice(b".tga"); // append texture extension

        // Check if we previously added a decal using this texture
        let st = self.decal_texture(&texture_name, env)?;

        let shadow_type = shadow_info.shadow_type;
        let allow_world_align = shadow_info.allow_world_align;
        let allow_sun_direction = shadow_info.shadow_type & SHADOW_DIRECTIONAL_PROJECTION;
        let decal_size_x = shadow_info.size_x;
        let decal_size_y = shadow_info.size_y;

        let mut shadow = W3DProjectedShadow::new();

        shadow.set_render_object(None);
        shadow.set_texture(0, Some(st)); // @todo: Fix projected shadows to allow multiple lights
        shadow.base.shadow_type = shadow_type; // type of projection
        shadow.allow_world_align = allow_world_align;

        shadow.base.oow_decal_size_x = 1.0 / decal_size_x; // one over width
        shadow.base.oow_decal_size_y = 1.0 / decal_size_y; // one over height

        // Prestore some values used during projection to optimize out division.
        shadow.base.decal_size_x = decal_size_x; // width
        shadow.base.decal_size_y = decal_size_y; // height

        shadow.decal_offset_u = 0.0;
        shadow.decal_offset_v = 0.0;

        shadow.flags = allow_sun_direction as i32;

        shadow.init();

        let slot = self.link_shadow(shadow, st, true);

        match self.shadows[slot].as_ref().unwrap().base.shadow_type {
            SHADOW_DECAL | SHADOW_ALPHA_DECAL | SHADOW_ADDITIVE_DECAL => {
                self.num_decal_shadows += 1;
            }
            SHADOW_PROJECTION => {
                self.num_projection_shadows += 1;
            }
            _ => {}
        }

        Some(slot)
    }

    /// `W3DProjectedShadowManager::addDecal(robj, shadowInfo)` (W3DProjectedShadow.cpp:1608): decals that follow the render
    /// object.
    pub fn add_decal(&mut self, robj: Option<ShadowRenderObj>, shadow_info: Option<&ShadowTypeInfo>, env: &mut impl ProjectedShadowEnv) -> Option<usize> {
        let (robj, shadow_info) = match (robj, shadow_info) {
            (Some(r), Some(i)) => (r, i), // right now we require hardware render-to-texture support
            _ => return None,
        };

        // simple decal using the premade texture specified.
        let mut texture_name = shadow_info.shadow_name.clone();
        texture_name.extend_from_slice(b".tga"); // append texture extension

        // Check if we previously added a decal using this texture
        let st = self.decal_texture(&texture_name, env)?;

        let shadow_type = shadow_info.shadow_type;
        let allow_world_align = shadow_info.allow_world_align;
        let allow_sun_direction = shadow_info.shadow_type & SHADOW_DIRECTIONAL_PROJECTION;
        let mut decal_size_x = shadow_info.size_x;
        let mut decal_size_y = shadow_info.size_y;
        let mut decal_offset_x = shadow_info.offset_x;
        let mut decal_offset_y = shadow_info.offset_y;

        let mut shadow = W3DProjectedShadow::new();

        shadow.set_render_object(Some(robj.clone()));
        shadow.set_texture(0, Some(st)); // @todo: Fix projected shadows to allow multiple lights
        shadow.base.shadow_type = shadow_type; // type of projection
        shadow.allow_world_align = allow_world_align;

        let bx = robj.obj_space_bounding_box;

        // Check if app is overriding any of the default texture stretch factors.
        if decal_size_x == 0.0 {
            decal_size_x = bx.extent.x * 2.0; // use bounding box to determine size
        }
        shadow.base.oow_decal_size_x = 1.0 / decal_size_x; // one over width

        if decal_size_y == 0.0 {
            decal_size_y = bx.extent.y * 2.0; // world space distance to stretch full texture
        }
        shadow.base.oow_decal_size_y = 1.0 / decal_size_y; // one over height

        if decal_offset_x != 0.0 {
            decal_offset_x *= shadow.base.oow_decal_size_x;
        }

        if decal_offset_y != 0.0 {
            decal_offset_y *= shadow.base.oow_decal_size_y;
        }

        // Prestore some values used during projection to optimize out division.
        shadow.base.decal_size_x = decal_size_x; // width
        shadow.base.decal_size_y = decal_size_y; // height

        shadow.decal_offset_u = decal_offset_x;
        shadow.decal_offset_v = decal_offset_y;

        shadow.flags = allow_sun_direction as i32;

        shadow.init();

        let slot = self.link_shadow(shadow, st, true);

        match self.shadows[slot].as_ref().unwrap().base.shadow_type {
            SHADOW_DECAL | SHADOW_ALPHA_DECAL | SHADOW_ADDITIVE_DECAL => {
                self.num_decal_shadows += 1;
            }
            SHADOW_PROJECTION => {
                self.num_projection_shadows += 1;
            }
            _ => {}
        }

        Some(slot)
    }

    /// `W3DProjectedShadowManager::addShadow` (W3DProjectedShadow.cpp:1755): add a new shadow with the texture of the given
    /// name or that of robj. `use_shadow_decals` is `TheGlobalData->m_useShadowDecals`.
    pub fn add_shadow(&mut self, robj: Option<ShadowRenderObj>, shadow_info: Option<&ShadowTypeInfo>, env: &mut impl ProjectedShadowEnv) -> Option<usize> {
        const DEFAULT_DECAL_NAME: &[u8] = b"shadow.tga";
        let robj = match robj {
            Some(r) if self.dynamic_render_target.is_some() && env.use_shadow_decals() => r,
            _ => return None, // right now we require hardware render-to-texture support
        };

        let st: ShadowTextureId;
        let shadow_type: u32;
        let allow_world_align = false;
        let mut decal_size_x = 0.0f32;
        let mut decal_size_y = 0.0f32;
        let mut decal_offset_x = 0.0f32;
        let mut decal_offset_y = 0.0f32;
        let mut allow_sun_direction: u32 = 0;
        let render_target = (DEFAULT_RENDER_TARGET_WIDTH, DEFAULT_RENDER_TARGET_HEIGHT);

        if let Some(info) = shadow_info {
            // determine what kind of shadow is needed
            if info.shadow_type == SHADOW_DECAL {
                // simple decal using the premade texture specified. can be always perpendicular to model's z-axis or
                // projected onto world geometry.
                let mut texture_name: Vec<u8>;
                if info.shadow_name.len() <= 1 {
                    // no texture name given, use same as object
                    texture_name = DEFAULT_DECAL_NAME.to_vec();
                } else {
                    texture_name = info.shadow_name.clone();
                    texture_name.extend_from_slice(b".tga"); // append texture extension
                }

                st = self.decal_texture(&texture_name, env)?;
                shadow_type = SHADOW_DECAL;
                allow_sun_direction = info.shadow_type & SHADOW_DIRECTIONAL_PROJECTION;
                decal_size_x = info.size_x;
                decal_size_y = info.size_y;
                decal_offset_x = info.offset_x;
                decal_offset_y = info.offset_y;
            } else if info.shadow_type == SHADOW_PROJECTION {
                // projection of object geometry into a texture. can be applied on a plane horizontal to model's z-axis or
                // projected onto world geometry.
                let texture_name: Vec<u8> = if !info.shadow_name.is_empty() && info.shadow_name[0] != 0 {
                    // the shadow will be based on another render object to allow multiple models to share same shadow -
                    // for example, all trees could use same shadow even if slightly different color, etc.
                    info.shadow_name.clone()
                } else {
                    robj.name.clone() // not texture name give, assume model name.
                };

                let mut s = self.textures().get_texture(&texture_name);
                if s.is_none() {
                    // texture doesn't exist, use current render object to create it
                    self.w3d_shadow_texture_manager.as_mut().unwrap().create_texture(&texture_name, render_target, env);
                    // try loading again
                    s = self.textures().get_texture(&texture_name);

                    s?; // could not create the shadow texture
                }
                st = s.unwrap();
                shadow_type = SHADOW_PROJECTION;
            } else {
                // upstream leaves `st` NULL and `shadowType` NONE here and goes on to dereference `st`: a crash
                return None;
            }
        } else {
            // no shadow info, assume user wants a projected shadow
            let texture_name = robj.name.clone();

            let mut s = self.textures().get_texture(&texture_name);

            if s.is_none() {
                // did not find a cached copy of the shadow geometry, create a new one
                self.w3d_shadow_texture_manager.as_mut().unwrap().create_texture(&texture_name, render_target, env);
                // try loading again
                s = self.textures().get_texture(&texture_name);
                s?; // could not create the shadow texture
            }
            st = s.unwrap();
            shadow_type = SHADOW_PROJECTION;
        }

        let mut shadow = W3DProjectedShadow::new();

        shadow.set_render_object(Some(robj.clone()));
        shadow.set_texture(0, Some(st)); // @todo: Fix projected shadows to allow multiple lights
        shadow.base.shadow_type = shadow_type; // type of projection
        shadow.allow_world_align = allow_world_align;

        let bx = robj.obj_space_bounding_box;

        // Check if app is overriding any of the default texture stretch factors.
        if decal_size_x != 0.0 {
            decal_size_x = 1.0 / decal_size_x; // world space distance to stretch full texture scale
        } else {
            decal_size_x = 1.0 / (bx.extent.x * 2.0); // use bounding box to determine size
        }

        if decal_size_y != 0.0 {
            decal_size_y = -1.0 / decal_size_y;
        } else {
            decal_size_y = -1.0 / (bx.extent.y * 2.0); // world space distance to stretch full texture
        }

        if decal_offset_x != 0.0 {
            decal_offset_x = -decal_offset_x * decal_size_x;
        } else {
            decal_offset_x = 0.0; // -box.Center.X*decalSizeX;
        }

        if decal_offset_y != 0.0 {
            decal_offset_y = -decal_offset_y * decal_size_y;
        } else {
            decal_offset_y = 0.0; // -box.Center.Y*decalSizeY;
        }

        // Prestore some values used during projection to optimize out division.
        shadow.base.oow_decal_size_x = decal_size_x; // one over width
        shadow.base.oow_decal_size_y = decal_size_y; // one over height
        shadow.base.decal_size_x = 1.0 / decal_size_x; // width
        shadow.base.decal_size_y = 1.0 / decal_size_y; // height

        shadow.decal_offset_u = decal_offset_x;
        shadow.decal_offset_v = decal_offset_y;

        shadow.flags = allow_sun_direction as i32;

        shadow.init();

        let slot = self.link_shadow(shadow, st, false);

        match self.shadows[slot].as_ref().unwrap().base.shadow_type {
            SHADOW_DECAL => {
                self.num_decal_shadows += 1;
            }
            SHADOW_PROJECTION => {
                self.num_projection_shadows += 1;
            }
            _ => {}
        }

        Some(slot)
    }

    /// `W3DProjectedShadowManager::createDecalShadow` (W3DProjectedShadow.cpp:1953): add a new shadow with the texture of
    /// the given name. The shadow is not put in either list (the caller owns it: `W3DTreeBuffer`).
    pub fn create_decal_shadow(&mut self, shadow_info: &ShadowTypeInfo, env: &mut impl ProjectedShadowEnv) -> Option<usize> {
        const DEFAULT_DECAL_NAME: &[u8] = b"shadow.tga";
        let allow_world_align = false;
        const DEFAULT_WIDTH: f32 = 10.0;

        // simple decal using the premade texture specified. can be always perpendicular to model's z-axis or projected
        // onto world geometry.
        let texture_name: Vec<u8> = if shadow_info.shadow_name.len() <= 1 {
            // no texture name given, use same as object
            DEFAULT_DECAL_NAME.to_vec()
        } else {
            let mut n = shadow_info.shadow_name.clone();
            n.extend_from_slice(b".tga"); // append texture extension
            n
        };

        let st = self.decal_texture(&texture_name, env)?;
        let shadow_type = SHADOW_DECAL;
        let mut decal_size_x = shadow_info.size_x;
        let mut decal_size_y = shadow_info.size_y;
        let mut decal_offset_x = shadow_info.offset_x;
        let mut decal_offset_y = shadow_info.offset_y;

        let mut shadow = W3DProjectedShadow::new();

        shadow.set_texture(0, Some(st));
        shadow.base.shadow_type = shadow_type; // type of projection
        shadow.allow_world_align = allow_world_align;

        // Check if app is overriding any of the default texture stretch factors.
        if decal_size_x != 0.0 {
            decal_size_x = 1.0 / decal_size_x; // world space distance to stretch full texture scale
        } else {
            decal_size_x = 1.0 / (DEFAULT_WIDTH * 2.0); // use bounding box to determine size
        }

        if decal_size_y != 0.0 {
            decal_size_y = -1.0 / decal_size_y;
        } else {
            decal_size_y = -1.0 / (DEFAULT_WIDTH * 2.0); // world space distance to stretch full texture
        }

        if decal_offset_x != 0.0 {
            decal_offset_x = -decal_offset_x * decal_size_x;
        } else {
            decal_offset_x = 0.0;
        }

        if decal_offset_y != 0.0 {
            decal_offset_y = -decal_offset_y * decal_size_y;
        } else {
            decal_offset_y = 0.0;
        }

        // Prestore some values used during projection to optimize out division.
        shadow.base.oow_decal_size_x = decal_size_x; // one over width
        shadow.base.oow_decal_size_y = decal_size_y; // one over height
        shadow.base.decal_size_x = 1.0 / decal_size_x; // width
        shadow.base.decal_size_y = 1.0 / decal_size_y; // height

        shadow.decal_offset_u = decal_offset_x;
        shadow.decal_offset_v = decal_offset_y;

        shadow.flags = 0;

        shadow.init();

        let slot = self.shadows.len();
        self.shadows.push(Some(shadow));
        Some(slot)
    }

    /// `W3DProjectedShadowManager::queueDecal`/`flushDecals` for the shadows the caller owns: a shadow made by
    /// `createDecalShadow` is queued like any other (the tree buffer calls `queueDecal(m_shadow)`).
    pub fn shadow_mut(&mut self, slot: usize) -> Option<&mut W3DProjectedShadow> {
        self.shadows.get_mut(slot).and_then(|s| s.as_mut())
    }

    /// `W3DProjectedShadowManager::removeShadow` (W3DProjectedShadow.cpp:2108).
    pub fn remove_shadow(&mut self, slot: usize) {
        let shadow_type = match self.shadows.get(slot).and_then(|s| s.as_ref()) {
            Some(s) => s.base.shadow_type,
            None => return,
        };

        if (shadow_type & (SHADOW_ALPHA_DECAL | SHADOW_ADDITIVE_DECAL)) != 0 {
            let mut prev_shadow: Option<usize> = None;
            let mut next_shadow = self.decal_list;
            while let Some(ns) = next_shadow {
                if ns == slot {
                    let next = self.shadows[slot].as_ref().unwrap().next;
                    if let Some(ps) = prev_shadow {
                        self.shadows[ps].as_mut().unwrap().next = next;
                    } else {
                        self.decal_list = next;
                    }
                    match shadow_type {
                        SHADOW_DECAL => self.num_decal_shadows -= 1,
                        SHADOW_PROJECTION => self.num_projection_shadows -= 1,
                        _ => {}
                    }
                    self.shadows[slot] = None; // delete shadow
                    return;
                }
                prev_shadow = Some(ns);
                next_shadow = self.shadows[ns].as_ref().unwrap().next;
            } // end for
        }

        // search for this shadow
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
                match shadow_type {
                    SHADOW_DECAL => self.num_decal_shadows -= 1,
                    SHADOW_PROJECTION => self.num_projection_shadows -= 1,
                    _ => {}
                }
                self.shadows[slot] = None; // delete shadow
                return;
            }
            prev_shadow = Some(ns);
            next_shadow = self.shadows[ns].as_ref().unwrap().next;
        } // end for
    }

    /// `W3DProjectedShadowManager::removeAllShadows` (W3DProjectedShadow.cpp:2166): remove all shadows.
    pub fn remove_all_shadows(&mut self) {
        let mut next_shadow = self.shadow_list;
        self.shadow_list = None;
        self.num_decal_shadows = 0;
        self.num_projection_shadows = 0;

        while let Some(cur) = next_shadow {
            next_shadow = self.shadows[cur].as_ref().and_then(|s| s.next);
            self.shadows[cur] = None;
        }

        let mut next_shadow = self.decal_list;
        self.decal_list = None;
        while let Some(cur) = next_shadow {
            next_shadow = self.shadows[cur].as_ref().and_then(|s| s.next);
            self.shadows[cur] = None;
        }
    }
}

/// `Vector2` appears in upstream's `updateTexture` (`uvData`, unused); kept so the include list reads as upstream's.
#[allow(dead_code)]
type UnusedUvData = Vector2;
