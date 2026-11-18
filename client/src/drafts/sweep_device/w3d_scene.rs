//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
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
//! - ~~`flushOccludedObjectsIntoStencil` (W3DScene.cpp:1326): left for a follow-up.~~ **Corrected 2026-11-18 (second pass,
//!   #213):** drafted at the bottom of this file as an ordered plan ([`OccludedStencilStep`]); the D3D stencil
//!   render states are named steps, the object bucketing, colour-index assignment and draw order are translated as written.

use rts_lockstep_engine::common::game_common::MAX_PLAYER_COUNT;
use rts_lockstep_engine::formats::texture::colorspace::{hsv_to_rgb, rgb_to_hsv};
use rts_lockstep_engine::formats::w3d::mesh::dx8_convert_color;
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

// ---------------------------------------------------------------------------------------------------------------------
// flushOccludedObjectsIntoStencil
// ---------------------------------------------------------------------------------------------------------------------

/// `MAX_VISIBLE_OCCLUDED_PLAYER_OBJECTS` (W3DScene.cpp:1325): the most occluded objects kept per player.
pub const MAX_VISIBLE_OCCLUDED_PLAYER_OBJECTS: usize = 512;
/// `NUMBER_PLAYER_COLOR_BITS` (W3DScene.cpp:1202): 4 bits encode 8 players, because the indices start at 1.
pub const NUMBER_PLAYER_COLOR_BITS: i32 = 4;
/// `DrawableInfo::ERF_IS_TRANSLUCENT`: set in a render object's drawable info flags (the verified `ERF_IS_OCCLUDED`
/// above is 0x0001; the flag values are read from the caller's [`OccludeeInfo::is_translucent`]).
///
/// `playerIndexToColorIndex` (W3DScene.cpp:1205): the 4 low bits of `player_index`, bit-reversed, so low stencil bits
/// stay free for shadows.
pub fn player_index_to_color_index(player_index: i32) -> i32 {
    let tmp = player_index;
    let mut result = 0;
    // Player index is stored in 4 bits
    for i in 0..NUMBER_PLAYER_COLOR_BITS {
        let flipped_position = NUMBER_PLAYER_COLOR_BITS - 1 - i; // correct position of bit after it's flipped left/right
        if flipped_position > i {
            // shifting left
            result |= (tmp & (1 << i)) << (flipped_position - i);
        } else {
            // shifting right
            result |= (tmp & (1 << i)) >> (i - flipped_position);
        }
    }
    result
}

/// One potentially occluded object (`m_potentialOccludees[i]`) with what the function reads from its `DrawableInfo` /
/// `Object`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OccludeeInfo {
    pub object: SceneObjectHandle,
    /// `object->getControllingPlayer()->getPlayerIndex()`.
    pub controlling_player_index: i32,
    /// `object->getControllingPlayer()->getPlayerColor()` (0xRRGGBB).
    pub player_color: i32,
    /// `drawInfo->m_flags & DrawableInfo::ERF_IS_TRANSLUCENT`.
    pub is_translucent: bool,
}

/// What `flushOccludedObjectsIntoStencil` reads from the scene and the globals.
#[derive(Clone, Debug, Default)]
pub struct OccludedStencilInputs {
    /// `m_potentialOccludees[0..m_numPotentialOccludees]`.
    pub potential_occludees: Vec<OccludeeInfo>,
    /// `m_potentialOccluders[0..m_numPotentialOccluders]`.
    pub potential_occluders: Vec<SceneObjectHandle>,
    /// `m_nonOccludersOrOccludees[0..m_numNonOccluderOrOccludee]`.
    pub non_occluders_or_occludees: Vec<SceneObjectHandle>,
    /// `ThePlayerList ? getLocalPlayer()->getPlayerIndex() : 0`, passed on to each `renderOneObject`.
    pub local_player_index: i32,
    /// `TheGlobalData->m_occludedLuminanceScale`.
    pub occluded_luminance_scale: f32,
    /// `TheGlobalData->m_useShadowVolumes`.
    pub use_shadow_volumes: bool,
}

/// A step of `flushOccludedObjectsIntoStencil`, in upstream's order. The D3D stencil states are named after the block
/// they belong to; the values are in the variant docs.
#[derive(Clone, Debug, PartialEq)]
pub enum OccludedStencilStep {
    /// `TheW3DShadowManager->setStencilShadowMask(mask)`.
    SetStencilShadowMask(u32),
    /// Occludee pass set-up: STENCILENABLE, ZENABLE true; STENCILMASK / WRITEMASK 0xffffffff; FUNC ALWAYS; ZFAIL and
    /// FAIL KEEP; PASS REPLACE (always store the player index unless occluded by another player's occludees).
    StencilStoreColorIndexSetup,
    /// `D3DRS_STENCILREF = colour index << 3`.
    StencilRef(i32),
    /// `TheDX8MeshRenderer.Flush()` (and `SHD_FLUSH` where upstream calls it).
    Flush,
    /// Translucent object: STENCILFUNC NEVER (no frame buffer writes), STENCILFAIL REPLACE (always replace the stencil).
    StencilNeverReplace,
    /// After a translucent object: STENCILFAIL KEEP, STENCILFUNC ALWAYS.
    StencilRestoreKeepAlways,
    /// `renderOneObject(rinfo, robj, localPlayerIndex)`.
    RenderOneObject { object: SceneObjectHandle, local_player_index: i32 },
    /// `D3DRS_STENCILENABLE = FALSE`.
    StencilDisable,
    /// Occluder pass set-up: STENCILENABLE, ZENABLE true; REF 0xffffffff; MASK 0xffffffff; WRITEMASK 0x80 (only the
    /// MSB); FUNC ALWAYS; ZFAIL / FAIL KEEP; PASS REPLACE.
    StencilOccluderSetup,
    /// `renderStenciledPlayerColor(color, stencilRef, clear)` (W3DScene.cpp:1229): a full-screen quad at the tactical
    /// view's rectangle, drawn where the stencil matches. `color` is `DX8Wrapper::Convert_Color(rgb, 0.5)`.
    RenderStenciledPlayerColor { color: u32, stencil_ref: u32, clear: bool },
    /// `D3DRS_AMBIENT = Convert_Color(Get_Ambient_Light(), 0)`.
    RestoreAmbient,
}

/// `RTS3DScene::flushOccludedObjectsIntoStencil` (W3DScene.cpp:1326): the stencil version of the occlusion pass.
///
/// 1. The stencil shadow mask is cleared.
/// 2. If there are both potential occludees and potential occluders: the occludees are bucketed by controlling player
///    (a bucket takes at most 512 objects, extra ones are dropped with an assert), each player with objects gets the next
///    colour index (`playerIndexToColorIndex(usedPlayerColorIndex++)`, starting at 1) and a colour made from his
///    player colour with HSV value scaled by `m_occludedLuminanceScale`, converted with alpha 0.5. Each player's objects
///    are drawn with `STENCILREF = colourIndex << 3`; a translucent object is drawn alone between two flushes with the
///    stencil function NEVER/REPLACE (colour writes off, since translucent objects draw at the end of the frame). Then the
///    objects that are neither occluders nor occludees, then the occluders (writing only the stencil MSB), then one
///    full-screen quad per visible player colour, and the shadow mask becomes the OR of the stencil refs used. With 8 or
///    more visible colours and shadow volumes, the stencil is cleared to leave only the MSBs and the mask is
///    `0x80808080`.
/// 3. Else, if anything is queued: occludees, occluders and the rest are drawn normally.
/// 4. The scene ambient is restored.
pub fn flush_occluded_objects_into_stencil(inp: &OccludedStencilInputs) -> Vec<OccludedStencilStep> {
    use OccludedStencilStep as S;
    let mut steps: Vec<S> = Vec::new();

    let max_players = MAX_PLAYER_COUNT as usize;
    let mut player_objects: Vec<Vec<OccludeeInfo>> = vec![Vec::new(); max_players];
    let mut player_color_index: Vec<i32> = vec![-1; max_players];
    let mut visible_player_colors: Vec<u32> = Vec::with_capacity(max_players);

    let mut used_player_color_index: i32 = 1;

    // Assume no player colors are visible and all stencil bits are free for use by shadows.
    steps.push(S::SetStencilShadowMask(0));

    let local = inp.local_player_index;

    if !inp.potential_occludees.is_empty() && !inp.potential_occluders.is_empty() {
        // bucket sort all possibly occluded objects by player index/color.
        for o in &inp.potential_occludees {
            let index = o.controlling_player_index as usize;
            if player_objects[index].len() >= MAX_VISIBLE_OCCLUDED_PLAYER_OBJECTS {
                // DEBUG_ASSERTCRASH(FALSE, ("Exceeded Maximum Number of potentially occluded models"));
                continue;
            }
            player_objects[index].push(*o);
        }

        steps.push(S::StencilStoreColorIndexSetup);

        // Find out which player indices are actually used and remap them to a color index. Render all objects using the
        // same color index at once. Potential occludees are rendered first so they z-sort correctly behind an occluder.
        let mut rgb = [0.0f32; 3];
        for i in 0..max_players {
            let num_objects = player_objects[i].len();
            if num_objects != 0 {
                // this player has some objects so draw them using his color index.
                if player_color_index[i] == -1 {
                    // color index not assigned yet: assign a new color index to this player
                    player_color_index[i] = player_index_to_color_index(used_player_color_index);
                    used_player_color_index += 1;
                    // assign a color to this index by copying it from the controlling player of all objects in this list.
                    let color = player_objects[i][0].player_color;
                    let mut hsv = rgb_to_hsv([
                        ((color >> 16) & 0xff) as f32 / 255.0,
                        ((color >> 8) & 0xff) as f32 / 255.0,
                        (color & 0xff) as f32 / 255.0,
                    ]);
                    hsv[2] *= inp.occluded_luminance_scale;
                    let _ = hsv_to_rgb(&mut rgb, hsv); // on Err the previous rgb stays, as upstream's `rgb`
                    visible_player_colors.push(dx8_convert_color([rgb[0], rgb[1], rgb[2], 0.5]));
                }

                let this_player_color_index = player_color_index[i];

                // Store this object's color index into bits 3-6 of stencil buffer
                steps.push(S::StencilRef(this_player_color_index << 3));

                // Render all of this player's objects for which we care when they are occluded.
                for o in &player_objects[i] {
                    if o.is_translucent {
                        steps.push(S::Flush); // render all the submitted meshes using current stencil function (+ SHD_FLUSH)
                        // Disable writing to color buffer since translucent objects are rendered at end of frame.
                        steps.push(S::StencilNeverReplace);
                        steps.push(S::RenderOneObject { object: o.object, local_player_index: local });
                        steps.push(S::Flush);
                        steps.push(S::StencilRestoreKeepAlways);
                    } else {
                        steps.push(S::RenderOneObject { object: o.object, local_player_index: local });
                    }
                }

                steps.push(S::Flush);
            }
        }
        // Stencil buffer is now filled with color indices of potentially occluded objects. Draw non-occluder or
        // occludee objects such as small rocks, shrubs, etc. here so they don't interfere with building occlusion.
        steps.push(S::StencilDisable); // these objects are not stored in stencil
        for object in &inp.non_occluders_or_occludees {
            steps.push(S::RenderOneObject { object: *object, local_player_index: local });
        }
        steps.push(S::Flush);

        // Draw occluder objects so they cover up and modify stencil MSB wherever they are in front of other objects.
        steps.push(S::StencilOccluderSetup);

        // Render all potential occluders on top of already rendered potential occludees.
        for object in &inp.potential_occluders {
            steps.push(S::RenderOneObject { object: *object, local_player_index: local });
        }
        steps.push(S::Flush);

        // We now have a stencil buffer where pixels that are occluded have a bit pattern of 1INDX000. INDX contains the
        // occluded player's color index. Walk through all the player colors and draw them wherever the stencil matches.
        let mut used_player_color_bits: u32 = 0;
        let num_visible_player_colors = visible_player_colors.len() as i32;
        for i in 0..num_visible_player_colors {
            let color = visible_player_colors[i as usize];
            let stencil_ref = ((player_index_to_color_index(i + 1) << 3) | 0x80) as u32;
            steps.push(S::RenderStenciledPlayerColor { color, stencil_ref, clear: false });
            used_player_color_bits |= stencil_ref; // keep track of all bits used for occlusion/player colors.
        }

        steps.push(S::SetStencilShadowMask(used_player_color_bits));
        if num_visible_player_colors >= 8 && inp.use_shadow_volumes {
            // With 8 or more visible players only 3 bits are left for stencil shadows (7 overlapping shadows): clear the
            // stencil, leaving only the MSB set on occluded player pixels so the shadow code does not overwrite them.
            steps.push(S::RenderStenciledPlayerColor { color: 0, stencil_ref: 0, clear: true });
            // msb indicates occluded player pixels so ignore it when filling screen with shadow
            steps.push(S::SetStencilShadowMask(0x8080_8080));
        }

        steps.push(S::StencilDisable);
    } else if !inp.non_occluders_or_occludees.is_empty()
        || !inp.potential_occluders.is_empty()
        || !inp.potential_occludees.is_empty()
    {
        // no occluded objects so don't need to render anything special. Just draw the queued up objects like normal
        // because they were skipped in the main scene traversal.
        for o in &inp.potential_occludees {
            steps.push(S::RenderOneObject { object: o.object, local_player_index: local });
        }
        for object in &inp.potential_occluders {
            steps.push(S::RenderOneObject { object: *object, local_player_index: local });
        }
        for object in &inp.non_occluders_or_occludees {
            steps.push(S::RenderOneObject { object: *object, local_player_index: local });
        }
        steps.push(S::Flush);
    }

    // Reset scene ambient because we sometimes mess around with it to make objects glow, etc. when processing drawables.
    steps.push(S::RestoreAmbient);
    steps
}
