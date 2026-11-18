//! UNTESTED DRAFT (#202, #204): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DPropBuffer.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DPropBuffer.cpp
//!
//! The render objects (`RenderObjClass`) and the asset manager are the device's: a prop keeps an opaque
//! [`RenderObjHandle`], its `Matrix3D` transform and object scale (what upstream hands `Set_Transform` and
//! `Set_ObjectScale`), ready for a Bevy entity. Where upstream asks `WW3DAssetManager::Create_Render_Obj` for a model,
//! the caller passes the model's bounding sphere and handle through a closure. `drawProps` returns the list of props to
//! draw (and with which pass) instead of calling `Render`.

use super::w3d_common::{RenderObjHandle, SphereClass};
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::ObjectShroudStatus;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::global_data::MAX_GLOBAL_LIGHTS;
use rts_lockstep_engine::common::global_data::TerrainLighting;
use rts_lockstep_engine::logic::world::partition::collide::geom_collides_with_geom;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;

/// `MAX_PROPS`.
pub const MAX_PROPS: usize = 4000;
/// `MAX_TYPES`.
pub const MAX_TYPES: usize = 64;

/// `TProp`: the individual data for a prop.
#[derive(Clone, Debug)]
pub struct TProp {
    /// `m_robj`: render object for this kind of prop.
    pub robj: Option<RenderObjHandle>,
    /// The transform upstream gives `m_robj->Set_Transform`.
    pub transform: Matrix3D,
    /// The scale upstream gives `m_robj->Set_ObjectScale`.
    pub object_scale: f32,
    /// `id`.
    pub id: i32,
    /// `location`: drawing location.
    pub location: Coord3D,
    /// `propType`: type of prop. Index into `m_propTypes`.
    pub prop_type: i32,
    /// `ss`.
    pub ss: ObjectShroudStatus,
    /// `visible`: visible flag, updated each frame.
    pub visible: bool,
    /// `bounds`: bounding sphere for culling to set the visible flag.
    pub bounds: SphereClass,
}

impl Default for TProp {
    fn default() -> Self {
        TProp {
            robj: None,
            transform: Matrix3D::default(),
            object_scale: 1.0,
            id: 0,
            location: Coord3D::default(),
            prop_type: 0,
            ss: ObjectShroudStatus::Invalid,
            visible: false,
            bounds: SphereClass::default(),
        }
    }
}

/// `TPropType`: the individual data for a prop type.
#[derive(Clone, Debug, Default)]
pub struct TPropType {
    /// `m_robj`: render object for this kind of prop.
    pub robj: Option<RenderObjHandle>,
    /// `m_robjName`: name of the render obj.
    pub robj_name: Vec<u8>,
    /// `m_bounds`: bounding sphere for the base prop models.
    pub bounds: SphereClass,
}

/// One prop `drawProps` draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropDraw {
    /// Index into the prop buffer.
    pub prop: usize,
    /// True when upstream pushes `m_propShroudMaterialPass` around the `Render`.
    pub shroud_pass: bool,
}

/// What `drawProps` sets up for the lights (`LightEnvironmentClass::Reset` and `Add_Light`), as data.
#[derive(Clone, Debug, Default)]
pub struct PropLighting {
    /// The environment's ambient: `objectLighting[0].ambient`.
    pub ambient: Vector3,
    /// One entry per global light: diffuse colour and the direction's `lightPos` (`Set_Transform`'s third axis).
    pub lights: Vec<(Vector3, Vector3)>,
}

/// `class W3DPropBuffer : Snapshot` (W3DPropBuffer.h).
pub struct W3DPropBuffer {
    /// `m_props[MAX_PROPS]`: the prop buffer. All props are stored here.
    pub props: Vec<TProp>,
    /// `m_numProps`.
    pub num_props: i32,
    /// `m_propTypes[MAX_TYPES]`.
    pub prop_types: Vec<TPropType>,
    /// `m_numPropTypes`.
    pub num_prop_types: i32,
    /// `m_initialized`.
    pub initialized: bool,
    /// `m_anythingChanged`.
    pub anything_changed: bool,
    /// `m_doCull`.
    pub do_cull: bool,
}

impl Default for W3DPropBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DPropBuffer {
    /// `W3DPropBuffer::W3DPropBuffer` (W3DPropBuffer.cpp:97). The `memset` (whose arguments upstream has swapped, so it
    /// clears nothing) is a fresh struct here; `m_light` (a `DIRECTIONAL` `LightClass`) and `m_propShroudMaterialPass` are
    /// device objects, not translated.
    pub fn new() -> Self {
        let mut b = W3DPropBuffer {
            props: vec![TProp::default(); MAX_PROPS],
            num_props: 0,
            prop_types: vec![TPropType::default(); MAX_TYPES],
            num_prop_types: 0,
            initialized: false,
            anything_changed: false,
            do_cull: false,
        };
        b.initialized = false;
        b.clear_all_props();
        // upstream: m_light = NEW_REF(LightClass, (LightClass::DIRECTIONAL)) — device-only, not translated
        // upstream: m_propShroudMaterialPass = NEW_REF(W3DShroudMaterialPassClass, ()) — device-only, not translated
        b.initialized = true;
        b
    }

    /// `W3DPropBuffer::cull` (W3DPropBuffer.cpp:60): culls the props, marking the visible flag. `cull_sphere` is
    /// `CameraClass::Cull_Sphere`, true when the sphere is outside the view.
    pub fn cull(&mut self, cull_sphere: &impl Fn(&SphereClass) -> bool) {
        for cur_prop in 0..self.num_props as usize {
            let visible = !cull_sphere(&self.props[cur_prop].bounds);
            self.props[cur_prop].visible = visible;
        }
    }

    /// `W3DPropBuffer::clearAllProps` (W3DPropBuffer.cpp:127): removes all props.
    pub fn clear_all_props(&mut self) {
        self.num_props = 0;
        for i in 0..MAX_TYPES {
            self.prop_types[i].robj = None; // upstream: REF_PTR_RELEASE(m_propTypes[i].m_robj) — device-only, not translated
            self.prop_types[i].robj_name.clear();
        }
        self.num_prop_types = 0;
    }

    /// `W3DPropBuffer::addPropType` (W3DPropBuffer.cpp:144): adds a type of prop (model and texture). `create_render_obj`
    /// is `WW3DAssetManager::Create_Render_Obj` followed by `Get_Bounding_Sphere`: `None` when the model is not found.
    pub fn add_prop_type(&mut self, model_name: &[u8], create_render_obj: &mut impl FnMut(&[u8]) -> Option<(RenderObjHandle, SphereClass)>) -> i32 {
        if self.num_prop_types as usize >= MAX_TYPES {
            // upstream: DEBUG_CRASH("Too many kinds of props in map. ...")
            return 0;
        }

        let n = self.num_prop_types as usize;
        let created = create_render_obj(model_name);
        self.prop_types[n].robj = created.map(|c| c.0);
        if created.is_none() {
            // upstream: DEBUG_CRASH("Unable to find model for prop %s")
            return -1;
        }
        self.prop_types[n].robj_name = model_name.to_vec();

        let bounds = created.map(|c| c.1).unwrap_or_default();
        self.prop_types[n].bounds = bounds;
        self.num_prop_types += 1;
        self.num_prop_types - 1
    }

    /// `W3DPropBuffer::addProp` (W3DPropBuffer.cpp:173): adds a prop. Name is the W3D model name. `clone_render_obj` is
    /// `m_propTypes[propType].m_robj->Clone()`.
    pub fn add_prop(
        &mut self,
        id: i32,
        location: Coord3D,
        angle: f32,
        scale: f32,
        model_name: &[u8],
        create_render_obj: &mut impl FnMut(&[u8]) -> Option<(RenderObjHandle, SphereClass)>,
        clone_render_obj: &mut impl FnMut(RenderObjHandle) -> RenderObjHandle,
    ) {
        if self.num_props as usize >= MAX_PROPS {
            return;
        }
        if !self.initialized {
            return;
        }
        let mut prop_type: i32 = -1;
        for i in 0..self.num_prop_types as usize {
            // AsciiString::compareNoCase
            if self.prop_types[i].robj_name.eq_ignore_ascii_case(model_name) {
                prop_type = i as i32;
                break;
            }
        }
        if prop_type < 0 {
            prop_type = self.add_prop_type(model_name, create_render_obj);
            if prop_type < 0 {
                return;
            }
        }

        let mtx = Self::prop_matrix(&location, angle, scale);

        let n = self.num_props as usize;
        let type_robj = self.prop_types[prop_type as usize].robj;
        let type_bounds = self.prop_types[prop_type as usize].bounds;
        let p = &mut self.props[n];
        p.location = location;
        p.id = id;
        p.ss = ObjectShroudStatus::Invalid;
        p.robj = type_robj.map(|r| clone_render_obj(r));
        p.transform = mtx; // upstream: m_robj->Set_Transform(mtx)
        p.object_scale = scale; // upstream: m_robj->Set_ObjectScale(scale)
        p.prop_type = prop_type;
        // Translate the bounding sphere of the model.
        p.bounds = type_bounds;
        p.bounds.center += Vector3::new(location.x, location.y, location.z);
        // Initially set it invisible.  cull will update it's visiblity flag.
        p.visible = false;

        self.num_props += 1;
    }

    /// The matrix `addProp` and `updatePropPosition` build: `Matrix3D mtx(true); Rotate_Z(angle); Scale(scale);
    /// Set_Translation(location)`.
    fn prop_matrix(location: &Coord3D, angle: f32, scale: f32) -> Matrix3D {
        let mut mtx = Matrix3D::default();
        mtx.make_identity(); // Matrix3D mtx(true)
        mtx.rotate_z(angle);
        mtx.scale(scale);
        mtx.set_translation(&Vector3::new(location.x, location.y, location.z));
        mtx
    }

    /// `W3DPropBuffer::updatePropPosition` (W3DPropBuffer.cpp:233): updates a prop's position.
    pub fn update_prop_position(&mut self, id: i32, location: &Coord3D, angle: f32, scale: f32) -> bool {
        for i in 0..self.num_props as usize {
            if self.props[i].id == id {
                let mtx = Self::prop_matrix(location, angle, scale);
                self.props[i].location = *location;
                self.props[i].transform = mtx; // upstream: m_robj->Set_Transform(mtx)
                self.props[i].object_scale = scale; // upstream: m_robj->Set_ObjectScale(scale)
                // Translate the bounding sphere of the model.
                let ty = self.props[i].prop_type as usize;
                self.props[i].bounds = self.prop_types[ty].bounds;
                self.props[i].bounds.center += Vector3::new(location.x, location.y, location.z);
                self.anything_changed = true;
                return true;
            }
        }
        false
    }

    /// `W3DPropBuffer::removeProp` (W3DPropBuffer.cpp:258): removes a prop. Upstream does not stop at the first match.
    pub fn remove_prop(&mut self, id: i32) {
        for i in 0..self.num_props as usize {
            if self.props[i].id == id {
                self.props[i].location = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
                self.props[i].prop_type = -1;
                self.props[i].robj = None; // upstream: REF_PTR_RELEASE(m_props[i].m_robj) — device-only, not translated
                // Translate the bounding sphere of the model.
                self.props[i].bounds.center = Vector3::new(0.0, 0.0, 0.0);
                self.props[i].bounds.radius = 1.0;
                self.anything_changed = true;
            }
        }
    }

    /// `W3DPropBuffer::removePropsForConstruction` (W3DPropBuffer.cpp:281): removes any props that would be under a
    /// building.
    pub fn remove_props_for_construction(&mut self, pos: &Coord3D, geom: &GeometryInfo, angle: f32) {
        // Just iterate all trees, as even non-collidable ones get removed. jba. [7/11/2003]
        for i in 0..self.num_props as usize {
            if self.props[i].robj.is_none() {
                continue; // already deleted.
            }
            let radius = self.props[i].bounds.radius;
            let info = GeometryInfo::new(GeometryType::Cylinder, false, 5.0 * radius, 2.0 * radius, 2.0 * radius);
            if geom_collides_with_geom(pos, geom, angle, &self.props[i].location, &info, 0.0) {
                // remove it [7/11/2003]
                self.props[i].location = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
                self.props[i].prop_type = -1;
                self.props[i].robj = None; // upstream: REF_PTR_RELEASE(m_props[i].m_robj) — device-only, not translated
                // Translate the bounding sphere of the model.
                self.props[i].bounds.center = Vector3::new(0.0, 0.0, 0.0);
                self.props[i].bounds.radius = 1.0;
                self.anything_changed = true;
            }
        }
    }

    /// `W3DPropBuffer::notifyShroudChanged` (W3DPropBuffer.cpp:315): sets the shroud to status, so it is recomputed.
    /// `have_partition_manager` is `ThePartitionManager != NULL`.
    pub fn notify_shroud_changed(&mut self, have_partition_manager: bool) {
        for i in 0..self.num_props as usize {
            self.props[i].ss = if have_partition_manager { ObjectShroudStatus::Invalid } else { ObjectShroudStatus::Clear };
        }
    }

    /// `W3DPropBuffer::doFullUpdate` (W3DPropBuffer.h): called when the view changes.
    pub fn do_full_update(&mut self) {
        self.do_cull = true;
    }

    /// `W3DPropBuffer::drawProps` (W3DPropBuffer.cpp:338): draws the props. Uses the camera to cull.
    ///
    /// - `cull_sphere`: `CameraClass::Cull_Sphere` of `rinfo.Camera`.
    /// - `object_lighting`: `TheGlobalData->m_terrainObjectsLighting[TheGlobalData->m_timeOfDay]`.
    /// - `have_player_list` / `have_partition_manager`: `ThePlayerList` / `ThePartitionManager` non-NULL (both are NULL in
    ///   the world builder).
    /// - `local_player_index`: `ThePlayerList->getLocalPlayer()->getPlayerIndex()`.
    /// - `prop_shroud_status`: `ThePartitionManager->getPropShroudStatusForPlayer(playerIndex, loc)`.
    /// - `terrain_has_shroud`: `TheTerrainRenderObject->getShroud() != NULL`.
    ///
    /// Returns the lighting upstream puts in `rinfo.light_environment` and the props it renders, in order.
    pub fn draw_props(
        &mut self,
        cull_sphere: &impl Fn(&SphereClass) -> bool,
        object_lighting: &[TerrainLighting; 3],
        have_player_list: bool,
        have_partition_manager: bool,
        local_player_index: i32,
        prop_shroud_status: &impl Fn(i32, &Coord3D) -> ObjectShroudStatus,
        terrain_has_shroud: bool,
    ) -> (PropLighting, Vec<PropDraw>) {
        if self.do_cull {
            self.cull(cull_sphere);
        }

        // LightEnvironmentClass lightEnv; lightEnv.Reset(center, ambient)
        let ambient = Vector3::new(object_lighting[0].ambient.red, object_lighting[0].ambient.green, object_lighting[0].ambient.blue);
        let mut lighting = PropLighting { ambient, lights: Vec::new() };
        for i in 0..MAX_GLOBAL_LIGHTS {
            // m_light->Set_Ambient(zero); Set_Diffuse(diffuse); Set_Specular(zero); Set_Transform(mtx.Set(x, y, lightPos, zero));
            // lightEnv.Add_Light(*m_light)
            let l = &object_lighting[i];
            lighting.lights.push((
                Vector3::new(l.diffuse.red, l.diffuse.green, l.diffuse.blue),
                Vector3::new(l.light_pos.x, l.light_pos.y, l.light_pos.z),
            ));
        }

        let mut draws = Vec::new();
        for i in 0..self.num_props as usize {
            if !self.props[i].visible {
                continue;
            }
            if self.props[i].robj.is_none() {
                continue;
            }
            if !have_player_list || !have_partition_manager {
                //  in Worldbuilder. No shroud either. jba. [6/9/2003]
                self.props[i].ss = ObjectShroudStatus::Clear;
            }
            if self.props[i].ss == ObjectShroudStatus::Invalid {
                let local_player_index = if have_player_list { local_player_index } else { 0 };
                self.props[i].ss = prop_shroud_status(local_player_index, &self.props[i].location);
            }
            if self.props[i].ss >= ObjectShroudStatus::Shrouded {
                continue;
            }
            if self.props[i].ss <= ObjectShroudStatus::Invalid {
                continue;
            }
            // upstream compares `ss` with CELLSHROUD_CLEAR (0), an enum of the other type: ss is never 0 here
            // (OBJECTSHROUD_INVALID was skipped above), so the shroud pass is used whenever the terrain has a shroud.
            if terrain_has_shroud && (self.props[i].ss as i32) != 0 {
                // upstream: rinfo.Push_Material_Pass(m_propShroudMaterialPass); Render; Pop_Material_Pass
                draws.push(PropDraw { prop: i, shroud_pass: true });
            } else {
                // upstream: m_props[i].m_robj->Render(rinfo) — device-only, not translated
                draws.push(PropDraw { prop: i, shroud_pass: false });
            }
        }
        // upstream: rinfo.light_environment = NULL
        (lighting, draws)
    }

    /// `W3DPropBuffer::crc` (W3DPropBuffer.cpp:431): empty upstream.
    pub fn crc(&self) {}

    /// `W3DPropBuffer::xfer` (W3DPropBuffer.cpp:441): version 1, nothing else.
    pub fn xfer_version(&self) -> u8 {
        1
    }

    /// `W3DPropBuffer::loadPostProcess` (W3DPropBuffer.cpp:455): empty upstream.
    pub fn load_post_process(&self) {}
}
