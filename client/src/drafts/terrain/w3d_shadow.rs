//! UNTESTED DRAFT (#202, #204): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DShadow.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Shadow/W3DShadow.cpp
//!   GeneralsMD/Code/GameEngine/Include/GameClient/Shadow.h (`ShadowType`, `Shadow`, `Shadow::ShadowTypeInfo`; the
//!     interface base of both shadow kinds, not in the port, so copied here)
//!
//! `W3DShadowManager` owns the two shadow systems (`TheW3DVolumetricShadowManager`, `TheW3DProjectedShadowManager`; the
//! globals become fields) and routes calls to them. A shadow created by either is addressed by [`ShadowHandle`].
//! `DoShadows(rinfo, stencilPass)` is [`W3DShadowManager::do_shadows`]; `shadowCameraFrustum` is the environments' frustum
//! tests. The managers' device environments are the traits in their own files.
//!
//! Notes on upstream behaviour kept as it is:
//! - `Shadow::Shadow()` sets `m_diffuse`, `m_color`, `m_opacity` and `m_localAngle` only; `m_isEnabled`,
//!   `m_isInvisibleEnabled`, `m_type`, the position and the decal sizes are uninitialised (zero here). The derived
//!   shadows set what they need.
//! - `W3DShadowManager::init` returns TRUE whatever the sub-managers answer (it only ever assigns TRUE).
//! - `LightPosWorld[0]` starts at (94.0161, 50.499, 200).

use super::w3d_projected_shadow::{ProjectedShadowEnv, W3DProjectedShadowManager};
use super::w3d_volumetric_shadow::{VolumetricShadowEnv, W3DVolumetricShadowManager};
use super::w3d_common::{AABoxClass, RenderObjHandle, SphereClass};
use rts_lockstep_engine::common::base_type::real_to_int;
use rts_lockstep_engine::common::game_type::PathfindLayer;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::common::global_data::{GlobalData, TerrainLighting};
use rts_lockstep_engine::ww::vector3::Vector3;

/// `ShadowType` bit flags (Shadow.h): `SHADOW_NONE`.
pub const SHADOW_NONE: u32 = 0x0000_0000;
/// `SHADOW_DECAL`: shadow decal applied via modulate blend.
pub const SHADOW_DECAL: u32 = 0x0000_0001;
/// `SHADOW_VOLUME`.
pub const SHADOW_VOLUME: u32 = 0x0000_0002;
/// `SHADOW_PROJECTION`.
pub const SHADOW_PROJECTION: u32 = 0x0000_0004;
/// `SHADOW_DYNAMIC_PROJECTION`: extra setting for shadows which need dynamic updates.
pub const SHADOW_DYNAMIC_PROJECTION: u32 = 0x0000_0008;
/// `SHADOW_DIRECTIONAL_PROJECTION`: extra setting for shadow decals that rotate with sun direction.
pub const SHADOW_DIRECTIONAL_PROJECTION: u32 = 0x0000_0010;
/// `SHADOW_ALPHA_DECAL`: not really for shadows but for other decal uses. Alpha blended.
pub const SHADOW_ALPHA_DECAL: u32 = 0x0000_0020;
/// `SHADOW_ADDITIVE_DECAL`: not really for shadows but for other decal uses. Additive blended.
pub const SHADOW_ADDITIVE_DECAL: u32 = 0x0000_0040;

/// `MAX_SHADOW_LIGHTS`: maximum number of shadow casting light sources in scene.
pub const MAX_SHADOW_LIGHTS: usize = 1;
/// `SUN_DISTANCE_FROM_GROUND`: distance of sun (our only light source).
pub const SUN_DISTANCE_FROM_GROUND: f32 = 10000.0;

/// `Shadow::ShadowTypeInfo`.
#[derive(Clone, Debug, Default)]
pub struct ShadowTypeInfo {
    /// `m_ShadowName[64]`: when set, overrides the default model shadow (used mostly for Decals).
    pub shadow_name: Vec<u8>,
    /// `m_type`: type of shadow.
    pub shadow_type: u32,
    /// `allowUpdates`: whether to update the shadow image when object/light moves.
    pub allow_updates: bool,
    /// `allowWorldAlign`: whether to align shadow to world geometry or draw as horizontal decal.
    pub allow_world_align: bool,
    /// `m_sizeX`: world size of decal projection.
    pub size_x: f32,
    /// `m_sizeY`: world size of decal projection.
    pub size_y: f32,
    /// `m_offsetX`: world shift along x axis.
    pub offset_x: f32,
    /// `m_offsetY`: world shift along y axis.
    pub offset_y: f32,
}

/// `class Shadow`: the interface to all shadow objects (data members and the inline setters).
#[derive(Clone, Copy, Debug)]
pub struct Shadow {
    /// `m_isEnabled`: toggle to turn rendering of this shadow on/off.
    pub is_enabled: bool,
    /// `m_isInvisibleEnabled`: if set, overrides and causes no rendering.
    pub is_invisible_enabled: bool,
    /// `m_opacity`: value between 0 (transparent) and 255 (opaque).
    pub opacity: u32,
    /// `m_color`: color in ARGB format. (Alpha is ignored).
    pub color: u32,
    /// `m_type`: type of projection.
    pub shadow_type: u32,
    /// `m_diffuse`: diffuse color used to tint/fade shadow (the bits of upstream's `Int`).
    pub diffuse: u32,
    /// `m_x`: world position of shadow center when not bound to robj/drawable.
    pub x: f32,
    /// `m_y`.
    pub y: f32,
    /// `m_z`.
    pub z: f32,
    /// `m_oowDecalSizeX`.
    pub oow_decal_size_x: f32,
    /// `m_oowDecalSizeY`.
    pub oow_decal_size_y: f32,
    /// `m_decalSizeX`.
    pub decal_size_x: f32,
    /// `m_decalSizeY`.
    pub decal_size_y: f32,
    /// `m_localAngle`: yaw or rotation around z-axis of shadow image when not bound to robj/drawable.
    pub local_angle: f32,
}

impl Default for Shadow {
    fn default() -> Self {
        Self::new()
    }
}

impl Shadow {
    /// `Shadow::Shadow` (Shadow.h): `m_diffuse(0xffffffff), m_color(0xffffffff), m_opacity(0x000000ff),
    /// m_localAngle(0.0f)`.
    pub fn new() -> Self {
        Shadow {
            is_enabled: false,
            is_invisible_enabled: false,
            opacity: 0x0000_00ff,
            color: 0xffff_ffff,
            shadow_type: SHADOW_NONE,
            diffuse: 0xffff_ffff,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            oow_decal_size_x: 0.0,
            oow_decal_size_y: 0.0,
            decal_size_x: 0.0,
            decal_size_y: 0.0,
            local_angle: 0.0,
        }
    }

    /// `Shadow::enableShadowInvisible`: if this is set, then no render will occur, even if `enableShadowRender()` is
    /// enabled. Used by Shroud.
    pub fn enable_shadow_invisible(&mut self, is_enabled: bool) {
        self.is_invisible_enabled = is_enabled;
    }

    /// `Shadow::enableShadowRender`.
    pub fn enable_shadow_render(&mut self, is_enabled: bool) {
        self.is_enabled = is_enabled;
    }

    /// `Shadow::isRenderEnabled`.
    pub fn is_render_enabled(&self) -> bool {
        self.is_enabled
    }

    /// `Shadow::isInvisibleEnabled`.
    pub fn is_invisible_enabled(&self) -> bool {
        self.is_invisible_enabled
    }

    /// The additive decal tint: `REAL_TO_INT(c&0xff * f) | REAL_TO_INT((c>>8)&0xff * f) | REAL_TO_INT((c>>16)&0xff * f)`.
    fn additive_diffuse(&self) -> u32 {
        let fvalue = self.opacity as f32 / 255.0;
        (real_to_int((self.color & 0xff) as f32 * fvalue)
            | real_to_int(((self.color >> 8) & 0xff) as f32 * fvalue)
            | real_to_int(((self.color >> 16) & 0xff) as f32 * fvalue)) as u32
    }

    /// `Shadow::setOpacity`: adjust opacity of decal/shadow.
    pub fn set_opacity(&mut self, value: i32) {
        self.opacity = value as u32;

        if (self.shadow_type & SHADOW_ALPHA_DECAL) != 0 {
            self.diffuse = (self.color & 0x00ff_ffff).wrapping_add((value as u32) << 24);
        } else if (self.shadow_type & SHADOW_ADDITIVE_DECAL) != 0 {
            self.diffuse = self.additive_diffuse();
        }
    }

    /// `Shadow::setColor`: adjust ARGB color of decal/shadow.
    pub fn set_color(&mut self, value: u32) {
        self.color = value & 0x00ff_ffff; // filter out alpha

        if (self.shadow_type & SHADOW_ALPHA_DECAL) != 0 {
            self.diffuse = self.color | (self.opacity << 24);
        } else if (self.shadow_type & SHADOW_ADDITIVE_DECAL) != 0 {
            self.diffuse = self.additive_diffuse();
        }
    }

    /// `Shadow::setAngle`: adjust orientation around z-axis.
    pub fn set_angle(&mut self, angle: f32) {
        self.local_angle = angle;
    }

    /// `Shadow::setPosition`.
    pub fn set_position(&mut self, x: f32, y: f32, z: f32) {
        self.x = x;
        self.y = y;
        self.z = z;
    }

    /// `Shadow::setSize`.
    pub fn set_size(&mut self, size_x: f32, size_y: f32) {
        self.decal_size_x = size_x;
        self.decal_size_y = size_y;

        if size_x == 0.0 {
            self.oow_decal_size_x = 0.0;
        } else {
            self.oow_decal_size_x = 1.0 / size_x;
        }

        if size_y == 0.0 {
            self.oow_decal_size_y = 0.0;
        } else {
            self.oow_decal_size_y = 1.0 / size_y;
        }
    }
}

/// The `RenderObjClass *` of a shadow caster as the shadow systems read it: a snapshot the caller refreshes each frame
/// (`Get_Name`, `Get_Position`, `Get_Transform`, `Get_Bounding_Box`, `Get_Obj_Space_Bounding_Box`,
/// `Get_Obj_Space_Bounding_Sphere`, `Is_Really_Visible`, `Is_Alpha`, and the layer of the drawable's object found through
/// `Get_User_Data()`, `None` when there is no user data or no object).
#[derive(Clone, Debug)]
pub struct ShadowRenderObj {
    /// The render object's handle.
    pub handle: RenderObjHandle,
    /// `Get_Name()`.
    pub name: Vec<u8>,
    /// `Get_Position()`.
    pub position: Vector3,
    /// `Get_Transform()`.
    pub transform: Matrix3D,
    /// `Get_Bounding_Box()`: world space.
    pub bounding_box: AABoxClass,
    /// `Get_Obj_Space_Bounding_Box()`.
    pub obj_space_bounding_box: AABoxClass,
    /// `Get_Obj_Space_Bounding_Sphere()`.
    pub obj_space_bounding_sphere: SphereClass,
    /// `Is_Really_Visible()`.
    pub is_really_visible: bool,
    /// `Is_Alpha()`.
    pub is_alpha: bool,
    /// `((DrawableInfo *)robj->Get_User_Data())->m_drawable->getObject()->getLayer()`.
    pub owner_layer: Option<PathfindLayer>,
}

/// A shadow made by one of the two shadow systems (`Shadow *`): the owning system and the slot in its table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadowHandle {
    /// A `W3DVolumetricShadow` (`SHADOW_VOLUME`).
    Volumetric(usize),
    /// A `W3DProjectedShadow` (`SHADOW_PROJECTION`, `SHADOW_DECAL`).
    Projected(usize),
}

/// `W3DShadowManager`.
pub struct W3DShadowManager {
    /// `TheW3DVolumetricShadowManager`.
    pub volumetric: Option<W3DVolumetricShadowManager>,
    /// `TheW3DProjectedShadowManager` (also `TheProjectedShadowManager`).
    pub projected: Option<W3DProjectedShadowManager>,
    /// `m_isShadowScene`: flag if current scene needs shadows. No shadows on pre-pass and 2D.
    pub is_shadow_scene: bool,
    /// `m_shadowColor`: color and alpha for all shadows in scene.
    pub shadow_color: u32,
    /// `m_stencilShadowMask`.
    pub stencil_shadow_mask: i32,
    /// `LightPosWorld[MAX_SHADOW_LIGHTS]` (a file-static global upstream).
    pub light_pos_world: [Vector3; MAX_SHADOW_LIGHTS],
}

impl W3DShadowManager {
    /// `W3DShadowManager::W3DShadowManager` (W3DShadow.cpp:78): creates both shadow systems. `global_data` is
    /// `TheGlobalData`, from which the first terrain light direction gives the sun.
    pub fn new(global_data: &GlobalData) -> Self {
        // DEBUG_ASSERTCRASH(TheW3DVolumetricShadowManager == NULL && TheW3DProjectedShadowManager == NULL, ...)
        let mut light_ray = Vector3::new(-global_data.terrain_light_pos[0].x, -global_data.terrain_light_pos[0].y, -global_data.terrain_light_pos[0].z);
        light_ray.normalize();

        let mut light_pos_world = [Vector3::new(94.0161, 50.499, 200.0); MAX_SHADOW_LIGHTS];
        light_pos_world[0] = light_ray * SUN_DISTANCE_FROM_GROUND;

        W3DShadowManager {
            volumetric: Some(W3DVolumetricShadowManager::new()),
            projected: Some(W3DProjectedShadowManager::new()),
            is_shadow_scene: false,
            shadow_color: 0x7fa0_a0a0,
            stencil_shadow_mask: 0, // all bits can be used for storing shadows.
            light_pos_world,
        }
    }

    /// `W3DShadowManager::~W3DShadowManager` (W3DShadow.cpp:100): deletes both shadow systems.
    pub fn destroy(&mut self) {
        self.volumetric = None;
        self.projected = None;
    }

    /// `W3DShadowManager::queueShadows` (W3DShadow.h): flags system to process shadows on next render call.
    pub fn queue_shadows(&mut self, state: bool) {
        self.is_shadow_scene = state;
    }

    /// `W3DShadowManager::isShadowScene`.
    pub fn is_shadow_scene(&self) -> bool {
        self.is_shadow_scene
    }

    /// `W3DShadowManager::setShadowColor`: sets the shadow color and alpha, value in ARGB format.
    pub fn set_shadow_color(&mut self, color: u32) {
        self.shadow_color = color;
    }

    /// `W3DShadowManager::getShadowColor`.
    pub fn get_shadow_color(&self) -> u32 {
        self.shadow_color
    }

    /// `W3DShadowManager::setStencilShadowMask`: mask used to mask out stencil bits used for storing
    /// occlusion/playerColor.
    pub fn set_stencil_shadow_mask(&mut self, mask: i32) {
        self.stencil_shadow_mask = mask;
    }

    /// `W3DShadowManager::getStencilShadowMask`.
    pub fn get_stencil_shadow_mask(&self) -> i32 {
        self.stencil_shadow_mask
    }

    /// `W3DShadowManager::init` (W3DShadow.cpp:117): do one-time initialization of shadow systems that need to be active
    /// for the full duration of the game.
    pub fn init(&mut self) -> bool {
        let result = true;

        if let Some(v) = self.volumetric.as_mut() {
            if v.init() {
                v.re_acquire_resources();
            }
        }
        if let Some(p) = self.projected.as_mut() {
            if p.init() {
                p.re_acquire_resources();
            }
        }

        result
    }

    /// `W3DShadowManager::Reset` (W3DShadow.cpp:134): do per-map reset. This frees up shadows from all objects since they
    /// may not exist on the next map.
    pub fn reset(&mut self) {
        if let Some(v) = self.volumetric.as_mut() {
            v.reset();
        }
        if let Some(p) = self.projected.as_mut() {
            p.reset();
        }
    }

    /// `W3DShadowManager::ReAcquireResources` (W3DShadow.cpp:143).
    pub fn re_acquire_resources(&mut self) -> bool {
        let mut result = true;

        if let Some(v) = self.volumetric.as_mut() {
            if !v.re_acquire_resources() {
                result = false;
            }
        }
        if let Some(p) = self.projected.as_mut() {
            if !p.re_acquire_resources() {
                result = false;
            }
        }

        result
    }

    /// `W3DShadowManager::ReleaseResources` (W3DShadow.cpp:155).
    pub fn release_resources(&mut self) {
        if let Some(v) = self.volumetric.as_mut() {
            v.release_resources();
        }
        if let Some(p) = self.projected.as_mut() {
            p.release_resources();
        }
    }

    /// `W3DShadowManager::removeShadow` (W3DShadow.cpp:197): `shadow->release()` removes it from its manager.
    pub fn remove_shadow(&mut self, shadow: ShadowHandle) {
        match shadow {
            ShadowHandle::Volumetric(s) => {
                if let Some(v) = self.volumetric.as_mut() {
                    v.remove_shadow(s);
                }
            }
            ShadowHandle::Projected(s) => {
                if let Some(p) = self.projected.as_mut() {
                    p.remove_shadow(s);
                }
            }
        }
    }

    /// `W3DShadowManager::removeAllShadows` (W3DShadow.cpp:202).
    pub fn remove_all_shadows(&mut self) {
        if let Some(v) = self.volumetric.as_mut() {
            v.remove_all_shadows();
        }
        if let Some(p) = self.projected.as_mut() {
            p.remove_all_shadows();
        }
    }

    /// `W3DShadowManager::invalidateCachedLightPositions` (W3DShadow.cpp:211): force update of all shadows even when light
    /// source and object have not moved.
    pub fn invalidate_cached_light_positions(&mut self) {
        if let Some(v) = self.volumetric.as_mut() {
            v.invalidate_cached_light_positions();
        }
        if let Some(p) = self.projected.as_mut() {
            p.invalidate_cached_light_positions();
        }
    }

    /// `W3DShadowManager::getLightPosWorld` (W3DShadow.cpp:219).
    pub fn get_light_pos_world(&self, light_index: usize) -> Vector3 {
        self.light_pos_world[light_index]
    }

    /// `W3DShadowManager::setLightPosition` (W3DShadow.cpp:224).
    pub fn set_light_position(&mut self, light_index: i32, x: f32, y: f32, z: f32) {
        if light_index != 0 {
            return; // @todo: Add support for multiple lights
        }

        self.light_pos_world[light_index as usize] = Vector3::new(x, y, z);
    }

    /// `W3DShadowManager::setTimeOfDay` (W3DShadow.cpp:232): `ol` is
    /// `&TheGlobalData->m_terrainObjectsLighting[tod][0]`.
    pub fn set_time_of_day(&mut self, ol: &TerrainLighting) {
        // Ray to light source
        let mut light_ray = Vector3::new(-ol.light_pos.x, -ol.light_pos.y, -ol.light_pos.z);

        light_ray.normalize();
        light_ray *= SUN_DISTANCE_FROM_GROUND;

        self.set_light_position(0, light_ray.x, light_ray.y, light_ray.z);
    }

    /// `DoShadows(rinfo, stencilPass)` (W3DShadow.cpp:55): `shadowCameraFrustum = &rinfo.Camera.Get_Frustum()` is the
    /// environments' frustum tests. Projected shadows render first because they may fill the stencil buffer which will be
    /// used by the shadow volumes. Returns the projected shadows' render result (when it ran).
    pub fn do_shadows<P: ProjectedShadowEnv, V: VolumetricShadowEnv>(
        &mut self,
        stencil_pass: bool,
        projected_env: &mut P,
        volumetric_env: &mut V,
    ) -> Option<super::w3d_projected_shadow::ProjectedShadowRender> {
        let mut projection_count = 0;
        let mut result = None;

        if !stencil_pass && self.projected.is_some() && self.is_shadow_scene {
            let light = self.light_pos_world[0];
            let r = self.projected.as_mut().unwrap().render_shadows(projected_env, &light);
            projection_count = r.projection_count;
            result = Some(r);
        }

        if stencil_pass && self.volumetric.is_some() {
            // TheW3DShadowManager->loadTerrainShadows() is commented out upstream.

            // This function gets called many times by the W3D renderer so we use this flag to make sure shadows rendered
            // only once per frame.
            if self.is_shadow_scene {
                let (light, mask) = (self.light_pos_world[0], self.stencil_shadow_mask);
                self.volumetric.as_mut().unwrap().render_shadows(volumetric_env, projection_count != 0, &light, mask, self.shadow_color);
            }
        }
        if stencil_pass {
            // reset so no more shadow processing this frame.
            self.queue_shadows(false);
        }
        result
    }
}
