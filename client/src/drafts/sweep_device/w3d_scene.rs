//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DScene.cpp
//!
//! Sweep 2 (#213). `RTS3DScene`'s render lists are W3D render objects the port does not have, so they are
//! [`SceneObjectHandle`]s (an opaque index, as `RenderObjHandle` elsewhere) and everything that asks a render object
//! something (`Cast_Ray`, `Get_Bounding_Sphere`, `Get_User_Data`) is a closure from the caller.
//!
//! Drafted: `flagOccludedObjects`, `updateFixedLightEnvironments` (the light arithmetic), the dynamic light list
//! (`addDynamicLight`, `getADynamicLight`, `removeDynamicLight`), the lights iterator, `renderSpecificDrawables`'
//! selection, `flushOccludedObjects` and `flushTranslucentObjects` as ordered render plans, `RTS3DScene::doRender` and
//! `RTS2DScene::doRender`.
//!
//! Excluded / not drafted (listed in the #213 comment):
//! - `updatePlayerColorPasses` (W3DScene.cpp:925): its body is inside `#ifdef USE_NON_STENCIL_OCCLUSION`, which no header
//!   defines, so it is an empty function in the retail build. Drafted as that empty function.
//! - `flushOccludedObjectsIntoStencil` (W3DScene.cpp:1326): the stencil player-colour pass (colour assignment per player and
//!   the per-colour draw order) is D3D stencil state around that loop; left for a follow-up.

use rts_lockstep_engine::ww::vector3::Vector3;

/// An opaque `RenderObjClass *` of the scene's lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SceneObjectHandle(pub u32);

/// `DrawableInfo::ERF_IS_OCCLUDED`.
pub const ERF_IS_OCCLUDED: i32 = 0x0001;

/// A `LightClass` as `updateFixedLightEnvironments` handles it: ambient and diffuse (transforms are copied unchanged).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SceneLight {
    pub ambient: Vector3,
    pub diffuse: Vector3,
}

/// The lights `updateFixedLightEnvironments` leaves for the three light environments.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FixedLightEnvironments {
    /// `m_defaultLightEnv`: reset to ambient `Get_Ambient_Light()`, then each global light as is.
    pub default_ambient: Vector3,
    pub default_lights: Vec<SceneLight>,
    /// `m_foggedLightEnv`: ambient `Get_Ambient_Light() * foggedLightFrac`, lights scaled by the same fraction.
    pub fogged_ambient: Vector3,
    pub fogged_lights: Vec<SceneLight>,
    /// `*m_infantryLight[i]`: the global lights scaled by the infantry light scale, absolute values capped to 1.
    pub infantry_lights: Vec<SceneLight>,
    /// `m_infantryAmbient = Get_Ambient_Light()` (not scaled).
    pub infantry_ambient: Vector3,
}

/// `Vector3::Cap_Absolute_To(cap)`: each component's magnitude limited to the cap's, sign kept.
fn cap_absolute_to(v: &mut Vector3, cap: &Vector3) {
    if v.x > cap.x {
        v.x = cap.x;
    }
    if v.y > cap.y {
        v.y = cap.y;
    }
    if v.z > cap.z {
        v.z = cap.z;
    }
    if v.x < -cap.x {
        v.x = -cap.x;
    }
    if v.y < -cap.y {
        v.y = -cap.y;
    }
    if v.z < -cap.z {
        v.z = -cap.z;
    }
}

/// `RTS3DScene::updateFixedLightEnvironments` (W3DScene.cpp:876). `fog_alpha` / `clear_alpha` are
/// `TheGlobalData->m_fogAlpha` / `m_clearAlpha`; `script_override_infantry_light_scale` is
/// `m_scriptOverrideInfantryLightScale` (-1 = none); `infantry_light_scale_for_tod` is
/// `m_infantryLightScale[m_timeOfDay]`. `Pre_Render_Update(camera transform)` on each environment is the caller's.
pub fn update_fixed_light_environments(
    ambient_light: &Vector3,
    global_lights: &[SceneLight],
    fog_alpha: f32,
    clear_alpha: f32,
    script_override_infantry_light_scale: f32,
    infantry_light_scale_for_tod: f32,
) -> FixedLightEnvironments {
    let fogged_light_frac = fog_alpha / clear_alpha;
    let infantry_light_scale = if script_override_infantry_light_scale != -1.0 { script_override_infantry_light_scale } else { infantry_light_scale_for_tod };
    let mut out = FixedLightEnvironments {
        default_ambient: *ambient_light,
        fogged_ambient: Vector3 { x: ambient_light.x * fogged_light_frac, y: ambient_light.y * fogged_light_frac, z: ambient_light.z * fogged_light_frac },
        infantry_ambient: *ambient_light, // for now don't adjust ambient so that we don't lose directional lighting.
        ..Default::default()
    };
    let id = Vector3 { x: 1.0, y: 1.0, z: 1.0 };
    for light in global_lights {
        out.default_lights.push(*light);
        let mut old_diffuse = light.diffuse;
        let mut old_ambient = light.ambient;
        old_diffuse = Vector3 { x: old_diffuse.x * infantry_light_scale, y: old_diffuse.y * infantry_light_scale, z: old_diffuse.z * infantry_light_scale };
        old_ambient = Vector3 { x: old_ambient.x * infantry_light_scale, y: old_ambient.y * infantry_light_scale, z: old_ambient.z * infantry_light_scale };
        cap_absolute_to(&mut old_diffuse, &id);
        cap_absolute_to(&mut old_ambient, &id);
        out.infantry_lights.push(SceneLight { ambient: old_ambient, diffuse: old_diffuse }); // CLAMPED
        out.fogged_lights.push(SceneLight {
            ambient: Vector3 { x: light.ambient.x * fogged_light_frac, y: light.ambient.y * fogged_light_frac, z: light.ambient.z * fogged_light_frac },
            diffuse: Vector3 { x: light.diffuse.x * fogged_light_frac, y: light.diffuse.y * fogged_light_frac, z: light.diffuse.z * fogged_light_frac },
        });
    }
    out
}

/// `RTS3DScene::updatePlayerColorPasses` (W3DScene.cpp:925): empty (`USE_NON_STENCIL_OCCLUSION` undefined).
pub fn update_player_color_passes() {}

/// A potential occluder: its bounding sphere, and the ray test `robj->Cast_Ray(raytest)`.
#[derive(Clone, Copy, Debug)]
pub struct Occluder {
    pub handle: SceneObjectHandle,
    pub sphere_center: Vector3,
    pub sphere_radius: f32,
}

/// `RTS3DScene::flagOccludedObjects` (W3DScene.cpp:250): for every potential occludee, casts a ray from the camera to the
/// occludee's position against the potential occluders (a bounding sphere pre-test first: skip when the ray misses the
/// sphere, i.e. `Beta < 0`); the first occluder whose `Cast_Ray` hits flags the occludee. Returns the occluded
/// occludees, which upstream also packs to the front of `m_potentialOccludees` (so the count is `m_occludedObjectsCount`).
/// `occludee_position(h)` is `Get_Position()`; `cast_ray(occluder, p0, p1)` is `robj->Cast_Ray(raytest)` for the segment.
pub fn flag_occluded_objects(
    camera_position: &Vector3,
    potential_occludees: &[SceneObjectHandle],
    potential_occluders: &[Occluder],
    occludee_position: &impl Fn(SceneObjectHandle) -> Vector3,
    cast_ray: &impl Fn(SceneObjectHandle, &Vector3, &Vector3) -> bool,
) -> Vec<SceneObjectHandle> {
    let mut occluded = Vec::new();
    for occludee in potential_occludees {
        let p1 = occludee_position(*occludee);
        // raytest.Ray.Set(camPosition, position): P0 and the unit direction to P1
        let mut dir = Vector3 { x: p1.x - camera_position.x, y: p1.y - camera_position.y, z: p1.z - camera_position.z };
        dir.normalize();
        let mut hit = false;
        for occluder in potential_occluders {
            let sphere_vector = Vector3 {
                x: occluder.sphere_center.x - camera_position.x,
                y: occluder.sphere_center.y - camera_position.y,
                z: occluder.sphere_center.z - camera_position.z,
            };
            let alpha = Vector3::dot_product(&sphere_vector, &dir);
            let beta = occluder.sphere_radius * occluder.sphere_radius - (Vector3::dot_product(&sphere_vector, &sphere_vector) - alpha * alpha);
            if beta < 0.0 {
                continue; // no intersection
            }
            if cast_ray(occluder.handle, camera_position, &p1) {
                hit = true;
                break;
            }
        }
        if hit {
            // occludee was blocked by something so flag it for custom rendering (drawInfo->m_flags |= ERF_IS_OCCLUDED)
            occluded.push(*occludee);
        }
    }
    occluded
}

/// `RTS3DScene`'s dynamic light list (`m_dynamicLightList`): lights are reused when disabled.
#[derive(Clone, Debug, Default)]
pub struct DynamicLightList {
    /// `isEnabled()` per light, in list order.
    pub enabled: Vec<bool>,
}

impl DynamicLightList {
    /// `RTS3DScene::addDynamicLight` (W3DScene.cpp:1665): `m_dynamicLightList.Add(obj); UpdateList.Add(obj)`. Returns the
    /// new light's index.
    pub fn add_dynamic_light(&mut self, enabled: bool) -> usize {
        self.enabled.push(enabled);
        self.enabled.len() - 1
    }

    /// `RTS3DScene::getADynamicLight` (W3DScene.cpp:1676): the first disabled light, now enabled, else a new enabled one.
    pub fn get_a_dynamic_light(&mut self) -> usize {
        for (i, e) in self.enabled.iter_mut().enumerate() {
            if !*e {
                *e = true;
                return i;
            }
        }
        let i = self.add_dynamic_light(false);
        self.enabled[i] = true;
        i
    }

    /// `RTS3DScene::removeDynamicLight` (W3DScene.cpp:1700).
    pub fn remove_dynamic_light(&mut self, index: usize) {
        if index < self.enabled.len() {
            self.enabled.remove(index);
        }
    }

    /// `RTS3DScene::createLightsIterator` (W3DScene.cpp:1642): an iterator over `LightList`; here the list's length.
    pub fn create_lights_iterator(light_list_len: usize) -> std::ops::Range<usize> {
        0..light_list_len
    }

    /// `RTS3DScene::destroyLightsIterator` (W3DScene.cpp:1654): `delete it`; nothing to free.
    pub fn destroy_lights_iterator(_it: std::ops::Range<usize>) {}
}

/// `RTS3DScene::renderSpecificDrawables` (W3DScene.cpp:538): of the scene's objects (render list order), those whose
/// drawable is in `the_drawables` (by identity), each to be `renderOneObject`-ed. `drawable_of(h)` is
/// `((DrawableInfo*)robj->Get_User_Data())->m_drawable`, `None` when there is no user data or no drawable.
pub fn render_specific_drawables<D: PartialEq + Copy>(
    scene_objects: &[SceneObjectHandle],
    the_drawables: &[D],
    drawable_of: &impl Fn(SceneObjectHandle) -> Option<D>,
) -> Vec<SceneObjectHandle> {
    let mut out = Vec::new();
    for robj in scene_objects {
        let draw = match drawable_of(*robj) {
            Some(d) => d,
            None => continue,
        };
        if the_drawables.iter().any(|d| *d == draw) {
            out.push(*robj);
        }
    }
    out
}

/// One step of a flush pass.
#[derive(Clone, Debug, PartialEq)]
pub enum FlushStep {
    /// `TheW3DShadowManager->setStencilShadowMask(mask)`.
    SetStencilShadowMask(u32),
    /// The stencil render-state block (enable, ref 128, REPLACE on pass) when the device has a stencil buffer.
    StencilEnableWriteRef128,
    /// `rinfo.Push_Override_Flags(RINFO_OVERRIDE_ADDITIONAL_PASSES_ONLY)` (disable textures) / `Pop_Override_Flags()`.
    PushOverrideAdditionalPassesOnly,
    PopOverrideFlags,
    /// `rinfo.Push_Material_Pass(m_occludedMaterialPass[player]); robj->Render(rinfo); Pop_Material_Pass()`.
    RenderWithOccludedMaterialPass { object: SceneObjectHandle, player_index: i32 },
    /// `TheDX8MeshRenderer.Flush()`.
    FlushMeshRenderer,
    /// `D3DRS_STENCILREF = 0`.
    StencilRef0,
    /// `renderOneObject(rinfo, robj, localPlayerIndex)`, with `rinfo.alphaOverride` as given.
    RenderOneObject { object: SceneObjectHandle, alpha_override: Option<f32> },
    /// `D3DRS_STENCILENABLE = FALSE`.
    StencilDisable,
    /// `WW3D::Render_And_Clear_Static_Sort_Lists(rinfo)` (draws things like water), then `rinfo.alphaOverride = 1`.
    RenderAndClearStaticSortLists,
    /// `D3DRS_AMBIENT = Convert_Color(Get_Ambient_Light(), 0)`.
    RestoreAmbient,
}

/// `RTS3DScene::flushOccludedObjects` (W3DScene.cpp:1534): the occluded objects are drawn with their player colour in the
/// stencil, then drawn normally over it. `occluded` pairs each object with its controlling player's index;
/// `has_stencil` is `DX8Wrapper::Has_Stencil()`.
pub fn flush_occluded_objects(occluded: &[(SceneObjectHandle, i32)], has_stencil: bool) -> Vec<FlushStep> {
    let mut steps = vec![FlushStep::SetStencilShadowMask(0)];
    if !occluded.is_empty() {
        if has_stencil {
            // just in case we have shadows, disable them over occluded pixels.
            steps.push(FlushStep::StencilEnableWriteRef128);
        }
        steps.push(FlushStep::PushOverrideAdditionalPassesOnly); // disable textures
        for (object, player_index) in occluded {
            steps.push(FlushStep::RenderWithOccludedMaterialPass { object: *object, player_index: *player_index });
        }
        steps.push(FlushStep::PopOverrideFlags);
        steps.push(FlushStep::FlushMeshRenderer);
        if has_stencil {
            steps.push(FlushStep::StencilRef0);
        }
        for (object, _) in occluded {
            steps.push(FlushStep::RenderOneObject { object: *object, alpha_override: None });
        }
        steps.push(FlushStep::FlushMeshRenderer);
        steps.push(FlushStep::StencilDisable);
        // upper MSB always contains flag indicating occluded player color.
        steps.push(FlushStep::SetStencilShadowMask(0x8080_8080));
    }
    steps.push(FlushStep::RestoreAmbient);
    steps
}

/// `RTS3DScene::flushTranslucentObjects` (W3DScene.cpp:1605): each translucent object is drawn with the drawable's
/// effective opacity as the alpha override, then the static sort lists are drawn.
pub fn flush_translucent_objects(translucent: &[(SceneObjectHandle, f32)]) -> Vec<FlushStep> {
    let mut steps = Vec::new();
    if !translucent.is_empty() {
        for (object, opacity) in translucent {
            steps.push(FlushStep::RenderOneObject { object: *object, alpha_override: Some(*opacity) });
        }
        steps.push(FlushStep::FlushMeshRenderer);
        steps.push(FlushStep::RenderAndClearStaticSortLists);
    }
    steps.push(FlushStep::RestoreAmbient);
    steps
}

/// `RTS3DScene::doRender` / `RTS2DScene::doRender` (W3DScene.cpp:1710, 1781): `m_camera = cam; DRAW(); m_camera = NULL`.
/// `DRAW()` is the scene draw, `WW3D::Render(this, m_camera)` (device-only, not translated), which runs with the camera
/// set; the scene is `RTS3DScene::draw`'s error case (`DEBUG_CRASH("Null m_camera")`) when the camera is NULL.
pub fn do_render<C: Copy>(camera: &mut Option<C>, cam: C, draw: &mut impl FnMut(C)) {
    *camera = Some(cam);
    draw(cam);
    *camera = None;
}
