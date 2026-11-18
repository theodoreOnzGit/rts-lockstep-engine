//! UNTESTED DRAFT (#202, #204, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DBridgeBuffer.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DBridgeBuffer.cpp
//!
//! Render objects, meshes and textures are the device's: the asset manager is reached through closures
//! (`create_render_obj`, `get_texture`) that return [`BridgeRenderObj`]s (the sub-objects' names and transforms, and the
//! mesh data [`MeshModelData`] where the object is a mesh) and [`TextureHandle`]s. `doTheLight` (the terrain's vertex
//! lighting, not in the port) is the closure `do_the_light`. `NO_USE_BRIDGE_NORMALS` is defined, so the
//! `USE_BRIDGE_NORMALS` branches are left out.
//!
//! Not translated: `createTower`, `updateTowerPos` and `W3DBridgeBuffer::worldBuilderUpdateBridgeTowers` (World Builder
//! only: they add tower render objects to a `SimpleSceneClass` and store them in `MapObject`).

use super::w3d_common::{DrawTriangles, MeshModelData, SphereClass, TextureHandle, VertexFormatXYZNDUV1};
use rts_lockstep_engine::common::base_type::{real_to_int_floor, Coord3D};
use rts_lockstep_engine::common::global_data::{GlobalData, MAX_GLOBAL_LIGHTS};
use rts_lockstep_engine::common::terrain_roads::TerrainRoadCollection;
use rts_lockstep_engine::logic::module::body_module::{BodyDamageType, BODY_DAMAGED, BODY_PRISTINE, BODY_REALLYDAMAGED, BODY_RUBBLE};
use rts_lockstep_engine::logic::terrain_logic::BridgeInfo;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;

/// `BRIDGE_FLOAT_AMT`.
pub const BRIDGE_FLOAT_AMT: f32 = 0.25;
/// `MAX_BRIDGE_VERTEX`: make sure it stays under 65535.
pub const MAX_BRIDGE_VERTEX: i32 = 12000;
/// `MAX_BRIDGE_INDEX`: make sure it stays under 65535.
pub const MAX_BRIDGE_INDEX: i32 = 2 * MAX_BRIDGE_VERTEX;
/// `MAX_BRIDGES`.
pub const MAX_BRIDGES: usize = 200;

/// `TBridgeType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TBridgeType {
    /// `FIXED_BRIDGE`.
    FixedBridge = 0,
    /// `SECTIONAL_BRIDGE`.
    SectionalBridge = 1,
}

/// What `WW3DAssetManager::Create_Render_Obj(name)` gives the bridge loader: the sub-objects of an aggregate model
/// (`Get_Num_Sub_Objects`, `Get_Sub_Object(i)->Get_Name()` and `Get_Transform()`), and the mesh data when it is a mesh.
#[derive(Clone, Debug, Default)]
pub struct BridgeRenderObj {
    /// The sub-objects: name and transform.
    pub sub_objects: Vec<(Vec<u8>, Matrix3D)>,
    /// `Peek_Model()` data when the object is a `MeshClass`.
    pub mesh: Option<MeshModelData>,
}

/// One bridge `drawBridges` renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BridgeDraw {
    /// Index into the buffer's bridges.
    pub bridge: usize,
    /// The texture set before the draw (`DX8Wrapper::Set_Texture(0, m_bridgeTexture)`); `None` when upstream does not set
    /// one (the shroud pass renders with `wireframe = TRUE`).
    pub texture: Option<TextureHandle>,
    /// `Draw_Triangles(m_firstIndex, m_numPolygons, m_firstVertex, m_numVertex)`.
    pub draw: DrawTriangles,
}

/// What `drawBridges` issues: the main pass (with the cloud texture projected into stage 1 when `cloud_pass`) and the
/// shroud pass over all the bridges (`ST_SHROUD_TEXTURE`), each in order.
#[derive(Clone, Debug, Default)]
pub struct BridgeDrawPlan {
    /// The cloud texture is projected into stage 1 for the main pass (`ST_CLOUD_TEXTURE`).
    pub cloud_pass: bool,
    /// The main pass.
    pub main: Vec<BridgeDraw>,
    /// The shroud pass.
    pub shroud: Vec<BridgeDraw>,
}

/// `strnicmp(a, b, strlen(a)) == 0`.
fn prefix_matches_no_case(a: &[u8], b: &[u8]) -> bool {
    let n = a.len();
    for i in 0..n {
        let ca = a[i].to_ascii_lowercase();
        let cb = if i < b.len() { b[i].to_ascii_lowercase() } else { 0 };
        if ca != cb {
            return false;
        }
        if ca == 0 {
            return true;
        }
    }
    true
}

/// `class W3DBridge` (W3DBridgeBuffer.h): the individual data for a bridge.
#[derive(Clone, Debug)]
pub struct W3DBridge {
    /// `m_start`: drawing location.
    pub start: Vector3,
    /// `m_end`: drawing location.
    pub end: Vector3,
    /// `m_scale`: width scale.
    pub scale: f32,
    /// `m_length`.
    pub length: f32,
    /// `m_bridgeType`.
    pub bridge_type: TBridgeType,
    /// `m_bounds`: bounding sphere for culling to set the visible flag.
    pub bounds: SphereClass,
    /// `m_bridgeTexture`.
    pub bridge_texture: Option<TextureHandle>,
    /// `m_leftMesh`: the bridge's meshes (`None` where upstream's is NULL).
    pub left_mesh: Option<MeshModelData>,
    /// `m_leftMtx`: transform for the left mesh.
    pub left_mtx: Matrix3D,
    /// `m_minY`: min y vertex.
    pub min_y: f32,
    /// `m_maxY`: max y vertex.
    pub max_y: f32,
    /// `m_leftMinX`.
    pub left_min_x: f32,
    /// `m_leftMaxX`.
    pub left_max_x: f32,
    /// `m_sectionMesh`.
    pub section_mesh: Option<MeshModelData>,
    /// `m_sectionMtx`.
    pub section_mtx: Matrix3D,
    /// `m_sectionMinX`.
    pub section_min_x: f32,
    /// `m_sectionMaxX`.
    pub section_max_x: f32,
    /// `m_rightMesh`.
    pub right_mesh: Option<MeshModelData>,
    /// `m_rightMtx`.
    pub right_mtx: Matrix3D,
    /// `m_rightMinX`.
    pub right_min_x: f32,
    /// `m_rightMaxX`.
    pub right_max_x: f32,
    /// `m_firstIndex`: starting index buffer.
    pub first_index: i32,
    /// `m_numVertex`: number of vertex used.
    pub num_vertex: i32,
    /// `m_firstVertex`.
    pub first_vertex: i32,
    /// `m_numPolygons`: number of polygons to draw.
    pub num_polygons: i32,
    /// `m_visible`.
    pub visible: bool,
    /// `m_templateName`: name of the bridge type.
    pub template_name: Vec<u8>,
    /// `m_curDamageState`.
    pub cur_damage_state: BodyDamageType,
    /// `m_enabled`.
    pub enabled: bool,
}

impl Default for W3DBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DBridge {
    /// `W3DBridge::W3DBridge` (W3DBridgeBuffer.cpp:88): initializes pointers and values.
    pub fn new() -> Self {
        W3DBridge {
            start: Vector3::default(),
            end: Vector3::default(),
            scale: 1.0,
            length: 0.0,
            bridge_type: TBridgeType::FixedBridge,
            bounds: SphereClass::default(),
            bridge_texture: None,
            left_mesh: None,
            left_mtx: Matrix3D::default(),
            min_y: 0.0,
            max_y: 0.0,
            left_min_x: 0.0,
            left_max_x: 0.0,
            section_mesh: None,
            section_mtx: Matrix3D::default(),
            section_min_x: 0.0,
            section_max_x: 0.0,
            right_mesh: None,
            right_mtx: Matrix3D::default(),
            right_min_x: 0.0,
            right_max_x: 0.0,
            first_index: 0,
            num_vertex: 0,
            first_vertex: 0,
            num_polygons: 0,
            visible: false,
            template_name: Vec::new(),
            cur_damage_state: BODY_PRISTINE,
            enabled: false,
        }
    }

    /// `W3DBridge::renderBridge` (W3DBridgeBuffer.cpp:114): renders the bridge. It is assumed that the shared vertex and
    /// index buffers are already set. Returns the draw upstream issues, or `None`.
    pub fn render_bridge(&self, wireframe: bool) -> Option<(Option<TextureHandle>, DrawTriangles)> {
        if self.visible && self.num_polygons != 0 && self.num_vertex != 0 {
            let texture = if !wireframe { self.bridge_texture } else { None }; // Set_Texture(0, m_bridgeTexture)
            // Draw all the bridges.
            return Some((
                texture,
                DrawTriangles { start_index: self.first_index, polygon_count: self.num_polygons, min_vertex_index: self.first_vertex, vertex_count: self.num_vertex },
            ));
        }
        None
    }

    /// `W3DBridge::clearBridge` (W3DBridgeBuffer.cpp:128): frees all bridge objects (meshes and texture).
    pub fn clear_bridge(&mut self) {
        self.visible = false;
        self.bridge_texture = None;
        self.left_mesh = None;
        self.section_mesh = None;
        self.right_mesh = None;
    }

    /// `W3DBridge::cullBridge` (W3DBridgeBuffer.cpp:143): culls the bridge to the camera. Returns whether visibility
    /// changed. (`@todo - cull bridges` upstream: every bridge is visible.)
    pub fn cull_bridge(&mut self) -> bool {
        let was_visible = self.visible;

        self.visible = true;

        was_visible != self.visible
    }

    /// `W3DBridge::init` (W3DBridgeBuffer.cpp:162): inits a bridge's location and type so it can be loaded.
    pub fn init(&mut self, from_loc: Vector3, to_loc: Vector3, bridge_template_name: &[u8]) {
        self.start = from_loc;
        self.end = to_loc;
        self.template_name = bridge_template_name.to_vec();
        self.enabled = true;
    }

    /// `W3DBridge::load` (W3DBridgeBuffer.cpp:177): loads a bridge model (if not already loaded) and gets meshes for use
    /// at the specified location. `roads` is `TheTerrainRoads`; `create_render_obj` is
    /// `W3DAssetManager::Create_Render_Obj` and `get_texture` is `Get_Texture(name, MIP_LEVELS_3)`.
    pub fn load(
        &mut self,
        cur_damage_state: BodyDamageType,
        roads: &TerrainRoadCollection,
        create_render_obj: &mut impl FnMut(&[u8]) -> Option<BridgeRenderObj>,
        get_texture: &mut impl FnMut(&[u8]) -> Option<TextureHandle>,
    ) -> bool {
        self.bridge_texture = None;
        self.left_mesh = None;
        self.section_mesh = None;
        self.right_mesh = None;

        // @todo, should these be defaults in INI??? CBD
        // scale = 0.7f; width = 34; length = 170;

        // try to find bridge in INI
        let bridge = match roads.find_bridge(&self.template_name) {
            Some(b) => b,
            None => return false,
        };

        let scale = bridge.bridge_scale;
        let texture_file: Vec<u8>;
        let model_name: Vec<u8>;
        match cur_damage_state {
            BODY_PRISTINE => {
                texture_file = bridge.texture.clone();
                model_name = bridge.bridge_model_name.clone();
            }
            BODY_DAMAGED => {
                texture_file = bridge.texture_damaged.clone();
                model_name = bridge.bridge_model_name_damaged.clone();
            }
            BODY_REALLYDAMAGED => {
                texture_file = bridge.texture_really_damaged.clone();
                model_name = bridge.bridge_model_name_really_damaged.clone();
            }
            BODY_RUBBLE => {
                texture_file = bridge.texture_broken.clone();
                model_name = bridge.bridge_model_name_broken.clone();
            }
            _ => return false,
        }

        let mut left = model_name.clone();
        left.extend_from_slice(b".BRIDGE_LEFT");
        let mut section = model_name.clone();
        section.extend_from_slice(b".BRIDGE_SPAN");
        let mut right = model_name.clone();
        right.extend_from_slice(b".BRIDGE_RIGHT");

        self.bridge_texture = get_texture(&texture_file);
        self.left_mtx.make_identity();
        self.right_mtx.make_identity();
        self.section_mtx.make_identity();

        let p_obj = match create_render_obj(&model_name) {
            Some(o) => o,
            None => return false,
        };
        for (sub_name, mtx) in p_obj.sub_objects.iter() {
            if prefix_matches_no_case(&left, sub_name) {
                self.left_mtx = *mtx;
                left = sub_name.clone();
            }
            if prefix_matches_no_case(&section, sub_name) {
                self.section_mtx = *mtx;
                section = sub_name.clone();
            }
            if prefix_matches_no_case(&right, sub_name) {
                self.right_mtx = *mtx;
                right = sub_name.clone();
            }
        }

        self.left_mesh = create_render_obj(&left).and_then(|o| o.mesh);
        self.section_mesh = create_render_obj(&section).and_then(|o| o.mesh);
        self.right_mesh = create_render_obj(&right).and_then(|o| o.mesh);
        self.scale = scale;

        if self.left_mesh.is_none() {
            self.clear_bridge();
            return false;
        }
        self.bridge_type = TBridgeType::SectionalBridge;

        if self.right_mesh.is_none() || self.section_mesh.is_none() {
            self.bridge_type = TBridgeType::FixedBridge;
        }

        self.left_min_x = f32::MAX;
        self.left_max_x = -f32::MAX;
        self.min_y = f32::MAX;
        self.max_y = -f32::MAX;
        for p_vert in self.left_mesh.as_ref().unwrap().vertices.iter() {
            let vert = Matrix3D::transform_vector(&self.left_mtx, p_vert);
            if self.left_min_x > vert.x {
                self.left_min_x = vert.x;
            }
            if self.min_y > vert.y {
                self.min_y = vert.y;
            }
            if vert.x > self.left_max_x {
                self.left_max_x = vert.x;
            }
            if vert.y > self.max_y {
                self.max_y = vert.y; // Note - we assume all sections are the same width, so we only do maxY for first section.
            }
        }
        if self.bridge_type == TBridgeType::SectionalBridge {
            self.section_min_x = f32::MAX;
            self.section_max_x = -f32::MAX;
            for p_vert in self.section_mesh.as_ref().unwrap().vertices.iter() {
                let vert = Matrix3D::transform_vector(&self.section_mtx, p_vert);
                if self.section_min_x > vert.x {
                    self.section_min_x = vert.x;
                }
                if vert.x > self.section_max_x {
                    self.section_max_x = vert.x;
                }
            }

            self.right_min_x = f32::MAX;
            self.right_max_x = -f32::MAX;
            for p_vert in self.right_mesh.as_ref().unwrap().vertices.iter() {
                let vert = Matrix3D::transform_vector(&self.right_mtx, p_vert);
                if self.right_min_x > vert.x {
                    self.right_min_x = vert.x;
                }
                if vert.x > self.right_max_x {
                    self.right_max_x = vert.x;
                }
            }
        } else {
            self.section_min_x = self.left_max_x;
            self.section_max_x = self.left_max_x;
            self.right_min_x = self.left_max_x;
            self.right_max_x = self.left_max_x;
        }
        let mut length = self.right_max_x - self.left_min_x;
        if length < 1.0 {
            length = 1.0;
        }
        self.length = length;
        if self.bridge_type == TBridgeType::SectionalBridge {
            let allowable_error = 0.05 * length;
            // make sure the sections align.

            if self.left_max_x > self.section_min_x + allowable_error {
                self.bridge_type = TBridgeType::FixedBridge;
            }

            if self.right_min_x < self.section_max_x - allowable_error {
                self.bridge_type = TBridgeType::FixedBridge;
            }
        }
        true
    }

    /// `W3DBridge::getBridgeInfo` (W3DBridgeBuffer.cpp:362): gets the location info for the bridge. Only the fields
    /// upstream sets are written (`bridgeIndex`, `curDamageState` and `bridgeObjectID` are the caller's).
    pub fn get_bridge_info(&self, p_info: &mut BridgeInfo) {
        p_info.from.x = self.start.x;
        p_info.from.y = self.start.y;
        p_info.from.z = self.start.z;
        p_info.to.x = self.end.x;
        p_info.to.y = self.end.y;
        p_info.to.z = self.end.z;
        p_info.bridge_width = (self.max_y - self.min_y) * self.scale;

        let vec = self.end - self.start;
        let mut vec_normal = Vector3::new(-vec.y, vec.x, 0.0);
        vec_normal.normalize();

        // From left = from + vecNormal*maxY*scale
        p_info.from_left.x = self.start.x + vec_normal.x * self.max_y * self.scale;
        p_info.from_left.y = self.start.y + vec_normal.y * self.max_y * self.scale;
        p_info.from_left.z = self.start.z + vec_normal.z * self.max_y * self.scale;

        // From right = from + vecNormal*minY*scale
        p_info.from_right.x = self.start.x + vec_normal.x * self.min_y * self.scale;
        p_info.from_right.y = self.start.y + vec_normal.y * self.min_y * self.scale;
        p_info.from_right.z = self.start.z + vec_normal.z * self.min_y * self.scale;

        // to left = to + vecNormal*maxY*scale
        p_info.to_left.x = self.end.x + vec_normal.x * self.max_y * self.scale;
        p_info.to_left.y = self.end.y + vec_normal.y * self.max_y * self.scale;
        p_info.to_left.z = self.end.z + vec_normal.z * self.max_y * self.scale;

        // to right = to + vecNormal*minY*scale
        p_info.to_right.x = self.end.x + vec_normal.x * self.min_y * self.scale;
        p_info.to_right.y = self.end.y + vec_normal.y * self.min_y * self.scale;
        p_info.to_right.z = self.end.z + vec_normal.z * self.min_y * self.scale;
    }

    /// `W3DBridge::getModelVertices` (W3DBridgeBuffer.cpp:402): gets the vertex values for a section of a bridge.
    /// Returns the number of vertices written, 0 when there is no mesh or no room. `do_the_light` is
    /// `TheTerrainRenderObject->doTheLight(&vb, lightRay, &normal, NULL, 1.0f)` giving the vertex's diffuse.
    pub fn get_model_vertices(
        &self,
        destination_vb: &mut [VertexFormatXYZNDUV1],
        cur_vertex: i32,
        x_offset: f32,
        vec: &Vector3,
        vec_normal: &Vector3,
        vec_z: &Vector3,
        mtx: &Matrix3D,
        p_mesh: Option<&MeshModelData>,
        global_data: &GlobalData,
        do_the_light: &impl Fn(&Vector3, &[Vector3], &Vector3, f32) -> u32,
    ) -> i32 {
        let p_mesh = match p_mesh {
            Some(m) => m,
            None => return 0,
        };

        let num_vertex = p_mesh.vertices.len() as i32;
        let p_vert = &p_mesh.vertices;
        let p_normal = &p_mesh.normals;

        // If we happen to have too many bridges, stop.
        if cur_vertex + num_vertex + 2 >= MAX_BRIDGE_VERTEX {
            return 0;
        }

        let mut light_ray = [Vector3::default(); MAX_GLOBAL_LIGHTS];
        for light_index in 0..global_data.num_global_lights as usize {
            let light_pos: &Coord3D = &global_data.terrain_light_pos[light_index];
            light_ray[light_index].set(-light_pos.x, -light_pos.y, -light_pos.z);
            light_ray[light_index].normalize();
        }
        let n_lights = (global_data.num_global_lights as usize).min(MAX_GLOBAL_LIGHTS);

        let uvs = &p_mesh.uvs;
        let mut cur_vb = cur_vertex as usize;

        for i in 0..num_vertex as usize {
            let vertex = Matrix3D::transform_vector(mtx, &p_vert[i]);
            let mut v_loc = (vertex.x + x_offset) * *vec + vertex.y * *vec_normal + vertex.z * *vec_z;

            v_loc.x += self.start.x;
            v_loc.y += self.start.y;
            v_loc.z += self.start.z;

            destination_vb[cur_vb].x = v_loc.x;
            destination_vb[cur_vb].y = v_loc.y;
            destination_vb[cur_vb].z = v_loc.z;

            let mut normal = Matrix3D::rotate_vector_by(mtx, &p_normal[i]);
            // NO_USE_BRIDGE_NORMALS
            normal = normal.x * *vec + normal.y * *vec_normal + normal.z * *vec_z;
            normal.normalize();
            let diffuse = do_the_light(&v_loc, &light_ray[..n_lights], &normal, 1.0);
            destination_vb[cur_vb].nx = 0.0; // will these to keep AGP write buffer happy.
            destination_vb[cur_vb].ny = 0.0;
            destination_vb[cur_vb].nz = 1.0;
            destination_vb[cur_vb].diffuse = diffuse | 0xFF00_0000;
            destination_vb[cur_vb].u1 = uvs[i].0;
            destination_vb[cur_vb].v1 = uvs[i].1;
            cur_vb += 1;
        }
        num_vertex
    }

    /// `W3DBridge::getModelVerticesFixed` (W3DBridgeBuffer.cpp:497): gets the vertex values for a section of a fixed
    /// bridge.
    pub fn get_model_vertices_fixed(
        &self,
        destination_vb: &mut [VertexFormatXYZNDUV1],
        cur_vertex: i32,
        mtx: &Matrix3D,
        p_mesh: Option<&MeshModelData>,
        global_data: &GlobalData,
        do_the_light: &impl Fn(&Vector3, &[Vector3], &Vector3, f32) -> u32,
    ) -> i32 {
        if p_mesh.is_none() {
            return 0;
        }

        let mut vec = self.end - self.start;
        if vec.length2() < 1.0 {
            vec.normalize();
        }
        let mut vec_normal = Vector3::new(-vec.y, vec.x, 0.0);
        vec_normal.normalize();
        let mut delta_z = self.end.z - self.start.z;
        delta_z /= vec.length();
        let delta_x = (1.0f64 - delta_z as f64 * delta_z as f64).sqrt() as f32;
        let mut vec_z = Vector3::new(-delta_z, 0.0, delta_x);
        vec /= self.length;
        vec_normal *= self.scale;
        vec_z *= self.scale;
        let x_offset = -self.left_min_x;
        self.get_model_vertices(destination_vb, cur_vertex, x_offset, &vec, &vec_normal, &vec_z, mtx, p_mesh, global_data, do_the_light)
    }

    /// `W3DBridge::getIndicesNVertices` (W3DBridgeBuffer.cpp:523): gets the index values and vertex values for a bridge.
    pub fn get_indices_n_vertices(
        &mut self,
        destination_ib: &mut [u16],
        destination_vb: &mut [VertexFormatXYZNDUV1],
        cur_index_p: &mut i32,
        cur_vertex_p: &mut i32,
        global_data: &GlobalData,
        do_the_light: &impl Fn(&Vector3, &[Vector3], &Vector3, f32) -> u32,
    ) {
        self.first_vertex = *cur_vertex_p;
        self.first_index = *cur_index_p;
        self.num_vertex = 0;
        self.num_polygons = 0;
        if self.section_mesh.is_none() {
            let num_v = self.get_model_vertices_fixed(destination_vb, *cur_vertex_p, &self.left_mtx, self.left_mesh.as_ref(), global_data, do_the_light);
            if num_v == 0 {
                // not enough room for vertices
                debug_assert!(num_v != 0, "W3DBridge::GetIndicesNVertices(). Vertex overflow.");
                return;
            }
            let num_i = Self::get_model_indices(destination_ib, *cur_index_p, *cur_vertex_p, self.left_mesh.as_ref());
            if num_i == 0 {
                // not enough room for indices
                debug_assert!(num_i != 0, "W3DBridge::GetIndicesNVertices(). Index overflow.");
                return;
            }
            *cur_index_p += num_i;
            *cur_vertex_p += num_v;
            self.num_vertex += num_v;
            self.num_polygons += num_i / 3;
            return;
        }

        let mut vec = self.end - self.start;
        if vec.length2() < 1.0 {
            vec.normalize();
        }

        let mut vec_normal = Vector3::new(-vec.y, vec.x, 0.0);
        vec_normal.normalize();
        vec_normal *= self.scale;

        // Rotate along the y axis to get the appropriate Z height adjustment.
        let mut delta_z = self.end.z - self.start.z;
        let desired_length = vec.length();
        delta_z /= desired_length;
        let delta_x = (1.0f64 - delta_z as f64 * delta_z as f64).sqrt() as f32;
        let mut vec_z = Vector3::new(-delta_z, 0.0, delta_x);
        vec_z *= self.scale;

        let span_length = self.right_min_x - self.left_max_x;
        let mut num_spans: i32 = 1;
        if self.bridge_type != TBridgeType::FixedBridge {
            let spannable = desired_length - (self.length - span_length);
            num_spans = real_to_int_floor((spannable + span_length / 2.0) / span_length);
            if num_spans < 0 {
                num_spans = 0;
            }
        }

        let bridge_length = self.length + (num_spans - 1) as f32 * span_length;
        let x_offset = -self.left_min_x;

        // Draw the left end.
        vec /= bridge_length;
        let num_v = self.get_model_vertices(destination_vb, *cur_vertex_p, x_offset, &vec, &vec_normal, &vec_z, &self.left_mtx, self.left_mesh.as_ref(), global_data, do_the_light);
        if num_v == 0 {
            // not enough room for vertices
            debug_assert!(num_v != 0, "W3DBridge::GetIndicesNVertices(). Vertex overflow.");
            return;
        }
        let num_i = Self::get_model_indices(destination_ib, *cur_index_p, *cur_vertex_p, self.left_mesh.as_ref());
        if num_i == 0 {
            // not enough room for indices
            debug_assert!(num_i != 0, "W3DBridge::GetIndicesNVertices(). Index overflow.");
            return;
        }
        *cur_index_p += num_i;
        *cur_vertex_p += num_v;
        self.num_vertex += num_v;
        self.num_polygons += num_i / 3;

        // draw the spans.
        for i in 0..num_spans {
            let num_v = self.get_model_vertices(
                destination_vb,
                *cur_vertex_p,
                x_offset + i as f32 * span_length,
                &vec,
                &vec_normal,
                &vec_z,
                &self.section_mtx,
                self.section_mesh.as_ref(),
                global_data,
                do_the_light,
            );
            if num_v == 0 {
                // not enough room for vertices
                debug_assert!(num_v != 0, "W3DBridge::GetIndicesNVertices(). Vertex overflow.");
                return;
            }
            let num_i = Self::get_model_indices(destination_ib, *cur_index_p, *cur_vertex_p, self.section_mesh.as_ref());
            if num_i == 0 {
                // not enough room for indices
                debug_assert!(num_i != 0, "W3DBridge::GetIndicesNVertices(). Index overflow.");
                return;
            }
            *cur_index_p += num_i;
            *cur_vertex_p += num_v;
            self.num_vertex += num_v;
            self.num_polygons += num_i / 3;
        }

        // Draw the right end.
        let num_v = self.get_model_vertices(
            destination_vb,
            *cur_vertex_p,
            x_offset + (num_spans - 1) as f32 * span_length,
            &vec,
            &vec_normal,
            &vec_z,
            &self.right_mtx,
            self.right_mesh.as_ref(),
            global_data,
            do_the_light,
        );
        if num_v == 0 {
            // not enough room for vertices
            debug_assert!(num_v != 0, "W3DBridge::GetIndicesNVertices(). Vertex overflow.");
            return;
        }
        let num_i = Self::get_model_indices(destination_ib, *cur_index_p, *cur_vertex_p, self.right_mesh.as_ref());
        if num_i == 0 {
            // not enough room for indices
            debug_assert!(num_i != 0, "W3DBridge::GetIndicesNVertices(). Index overflow.");
            return;
        }
        *cur_index_p += num_i;
        *cur_vertex_p += num_v;
        self.num_vertex += num_v;
        self.num_polygons += num_i / 3;
    }

    /// `W3DBridge::getModelIndices` (W3DBridgeBuffer.cpp:666): gets the index values for a particular mesh section of the
    /// bridge.
    pub fn get_model_indices(destination_ib: &mut [u16], cur_index: i32, vertex_offset: i32, p_mesh: Option<&MeshModelData>) -> i32 {
        let p_mesh = match p_mesh {
            Some(m) => m,
            None => return 0,
        };
        let num_poly = p_mesh.polygons.len() as i32;
        if cur_index + 3 * num_poly + 6 >= MAX_BRIDGE_INDEX {
            return 0;
        }
        let mut cur_ib = cur_index as usize;
        for p in p_mesh.polygons.iter() {
            destination_ib[cur_ib] = (vertex_offset as u32).wrapping_add(p.0) as u16;
            destination_ib[cur_ib + 1] = (vertex_offset as u32).wrapping_add(p.1) as u16;
            destination_ib[cur_ib + 2] = (vertex_offset as u32).wrapping_add(p.2) as u16;
            cur_ib += 3;
        }
        num_poly * 3
    }

    /// `W3DBridge::getTemplateName` (W3DBridgeBuffer.h).
    pub fn get_template_name(&self) -> &[u8] {
        &self.template_name
    }

    /// `W3DBridge::getStart` (W3DBridgeBuffer.h).
    pub fn get_start(&self) -> &Vector3 {
        &self.start
    }

    /// `W3DBridge::getEnd` (W3DBridgeBuffer.h).
    pub fn get_end(&self) -> &Vector3 {
        &self.end
    }

    /// `W3DBridge::getDamageState` (W3DBridgeBuffer.h).
    pub fn get_damage_state(&self) -> BodyDamageType {
        self.cur_damage_state
    }

    /// `W3DBridge::setDamageState` (W3DBridgeBuffer.h).
    pub fn set_damage_state(&mut self, state: BodyDamageType) {
        self.cur_damage_state = state;
    }

    /// `W3DBridge::isVisible` (W3DBridgeBuffer.h).
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// `W3DBridge::isEnabled` (W3DBridgeBuffer.h).
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// `W3DBridge::setEnabled` (W3DBridgeBuffer.h).
    pub fn set_enabled(&mut self, enable: bool) {
        self.enabled = enable;
    }
}

/// A bridge point pair `loadBridges` finds in the map objects: `FLAG_BRIDGE_POINT1` object then its
/// `FLAG_BRIDGE_POINT2` successor (`MapObject::getName`, `getLocation`).
#[derive(Clone, Debug)]
pub struct BridgeMapObjectPair {
    /// `pMapObj->getName()`.
    pub name: Vec<u8>,
    /// `pMapObj->getLocation()`.
    pub from_xy: (f32, f32),
    /// `pMapObj2->getLocation()`.
    pub to_xy: (f32, f32),
}

/// `class W3DBridgeBuffer` (W3DBridgeBuffer.h): draw buffer for the bridges.
#[derive(Clone, Debug)]
pub struct W3DBridgeBuffer {
    /// `m_vertexBridge`: bridge vertex buffer (`MAX_BRIDGE_VERTEX + 4`); `None` where upstream's is NULL.
    pub vertex_bridge: Option<Vec<VertexFormatXYZNDUV1>>,
    /// `m_indexBridge` (`MAX_BRIDGE_INDEX + 4`).
    pub index_bridge: Option<Vec<u16>>,
    /// `m_curNumBridgeVertices`.
    pub cur_num_bridge_vertices: i32,
    /// `m_curNumBridgeIndices`.
    pub cur_num_bridge_indices: i32,
    /// `m_bridges[MAX_BRIDGES]`: all bridges are stored here.
    pub bridges: Vec<W3DBridge>,
    /// `m_numBridges`.
    pub num_bridges: i32,
    /// `m_initialized`.
    pub initialized: bool,
    /// `m_updateVis`: true if the camera moved, and we need to recalculate visibility.
    pub update_vis: bool,
    /// `m_anythingChanged`: set to true if visibility changed.
    pub anything_changed: bool,
}

impl Default for W3DBridgeBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DBridgeBuffer {
    /// `W3DBridgeBuffer::W3DBridgeBuffer` (W3DBridgeBuffer.cpp:737).
    pub fn new() -> Self {
        let mut b = W3DBridgeBuffer {
            vertex_bridge: None,
            index_bridge: None,
            cur_num_bridge_vertices: 0,
            cur_num_bridge_indices: 0,
            bridges: (0..MAX_BRIDGES).map(|_| W3DBridge::new()).collect(),
            num_bridges: 0,
            initialized: false,
            update_vis: false,
            anything_changed: false,
        };
        b.initialized = false;
        b.cur_num_bridge_vertices = 0;
        b.cur_num_bridge_indices = 0;
        b.clear_all_bridges();
        b.allocate_bridge_buffers();
        b.initialized = true;
        b
    }

    /// `W3DBridgeBuffer::~W3DBridgeBuffer` (W3DBridgeBuffer.cpp:726).
    pub fn destroy(&mut self) {
        self.free_bridge_buffers();
    }

    /// `W3DBridgeBuffer::freeBridgeBuffers` (W3DBridgeBuffer.cpp:759).
    pub fn free_bridge_buffers(&mut self) {
        self.vertex_bridge = None;
        self.index_bridge = None;
        // upstream: REF_PTR_RELEASE(m_vertexMaterial) — device-only, not translated
    }

    /// `W3DBridgeBuffer::allocateBridgeBuffers` (W3DBridgeBuffer.cpp:771). The vertex material is
    /// `VertexMaterialClass::PRELIT_DIFFUSE` (USE_BRIDGE_NORMALS is not defined): the renderer's.
    pub fn allocate_bridge_buffers(&mut self) {
        // DX8VertexBufferClass(DX8_FVF_XYZNDUV1, MAX_BRIDGE_VERTEX+4, USAGE_DYNAMIC)
        self.vertex_bridge = Some(vec![VertexFormatXYZNDUV1::default(); (MAX_BRIDGE_VERTEX + 4) as usize]);
        // DX8IndexBufferClass(MAX_BRIDGE_INDEX+4, USAGE_DYNAMIC)
        self.index_bridge = Some(vec![0u16; (MAX_BRIDGE_INDEX + 4) as usize]);
        self.cur_num_bridge_vertices = 0;
        self.cur_num_bridge_indices = 0;
    }

    /// `W3DBridgeBuffer::clearAllBridges` (W3DBridgeBuffer.cpp:795): removes all bridges.
    pub fn clear_all_bridges(&mut self) {
        for cur_bridge in 0..self.num_bridges as usize {
            self.bridges[cur_bridge].clear_bridge();
        }
        self.cur_num_bridge_indices = 0;
        self.num_bridges = 0;
    }

    /// `W3DBridgeBuffer::doFullUpdate` (W3DBridgeBuffer.h).
    pub fn do_full_update(&mut self) {
        self.update_vis = true;
    }

    /// `W3DBridgeBuffer::cull` (W3DBridgeBuffer.cpp:577): culls the bridges, marking the visible flag. If a bridge changes
    /// visibility, it sets `m_anythingChanged`.
    pub fn cull(&mut self) {
        self.anything_changed = self.update_vis;

        for cur_bridge in 0..self.num_bridges as usize {
            if self.bridges[cur_bridge].cull_bridge() {
                self.anything_changed = true;
            }
        }
    }

    /// `W3DBridgeBuffer::loadBridgesInVertexAndIndexBuffers` (W3DBridgeBuffer.cpp:595): loads the bridges into the vertex
    /// buffer for drawing.
    pub fn load_bridges_in_vertex_and_index_buffers(&mut self, global_data: &GlobalData, do_the_light: &impl Fn(&Vector3, &[Vector3], &Vector3, f32) -> u32) {
        if self.index_bridge.is_none() || self.vertex_bridge.is_none() || !self.initialized {
            return;
        }
        self.cur_num_bridge_vertices = 0;
        self.cur_num_bridge_indices = 0;
        let mut vb = self.vertex_bridge.take().unwrap();
        let mut ib = self.index_bridge.take().unwrap();

        for cur_bridge in 0..self.num_bridges as usize {
            self.bridges[cur_bridge].get_indices_n_vertices(&mut ib, &mut vb, &mut self.cur_num_bridge_indices, &mut self.cur_num_bridge_vertices, global_data, do_the_light);
        }
        self.vertex_bridge = Some(vb);
        self.index_bridge = Some(ib);
    }

    /// `W3DBridgeBuffer::loadBridges` (W3DBridgeBuffer.cpp:812): loads the bridges. When loaded, tells the terrain logic
    /// where each bridge is: `add_bridge_to_logic` is `W3DTerrainLogic::addBridgeToLogic` (called from `addBridge` when
    /// there is a terrain logic) and `height_at` is `TheTerrainRenderObject->getHeightMapHeight(x, y, NULL)`.
    /// `pairs` are the `FLAG_BRIDGE_POINT1` / `FLAG_BRIDGE_POINT2` map object pairs in map-object order.
    pub fn load_bridges(
        &mut self,
        pairs: &[BridgeMapObjectPair],
        have_terrain_logic: bool,
        height_at: &impl Fn(f32, f32) -> f32,
        roads: &TerrainRoadCollection,
        create_render_obj: &mut impl FnMut(&[u8]) -> Option<BridgeRenderObj>,
        get_texture: &mut impl FnMut(&[u8]) -> Option<TextureHandle>,
        add_bridge_to_logic: &mut impl FnMut(&BridgeInfo, &[u8]),
    ) {
        self.clear_all_bridges();
        for pair in pairs {
            let mut from = Vector3::default();
            let mut to = Vector3::default();
            from.set(pair.from_xy.0, pair.from_xy.1, 0.0);
            from.z = height_at(from.x, from.y) + BRIDGE_FLOAT_AMT;
            to.set(pair.to_xy.0, pair.to_xy.1, 0.0);
            to.z = height_at(to.x, to.y) + BRIDGE_FLOAT_AMT;
            self.add_bridge(from, to, &pair.name, have_terrain_logic, roads, create_render_obj, get_texture, add_bridge_to_logic);
        }
        // upstream: if (pTerrainLogic) pTerrainLogic->updateBridgeDamageStates() — the caller's
    }

    /// `W3DBridgeBuffer::addBridge` (W3DBridgeBuffer.cpp:1076): adds a bridge. Name is the GDF object name.
    pub fn add_bridge(
        &mut self,
        from_loc: Vector3,
        to_loc: Vector3,
        name: &[u8],
        have_terrain_logic: bool,
        roads: &TerrainRoadCollection,
        create_render_obj: &mut impl FnMut(&[u8]) -> Option<BridgeRenderObj>,
        get_texture: &mut impl FnMut(&[u8]) -> Option<TextureHandle>,
        add_bridge_to_logic: &mut impl FnMut(&BridgeInfo, &[u8]),
    ) {
        if self.num_bridges as usize >= MAX_BRIDGES {
            return;
        }

        if !self.initialized {
            return;
        }
        let n = self.num_bridges as usize;
        self.bridges[n].init(from_loc, to_loc, name);
        if self.bridges[n].load(BODY_PRISTINE, roads, create_render_obj, get_texture) {
            if have_terrain_logic {
                let mut info = BridgeInfo::default();
                self.bridges[n].get_bridge_info(&mut info);
                info.bridge_index = self.num_bridges;
                add_bridge_to_logic(&info, name);
            }
            self.num_bridges += 1;
        }
    }

    /// `W3DBridgeBuffer::updateCenter` (W3DBridgeBuffer.cpp:1100): updates the drawing buffer, based on the camera
    /// position.
    pub fn update_center(&mut self, global_data: &GlobalData, do_the_light: &impl Fn(&Vector3, &[Vector3], &Vector3, f32) -> u32) {
        self.cull();
        if self.anything_changed || self.cur_num_bridge_indices == 0 {
            self.load_bridges_in_vertex_and_index_buffers(global_data, do_the_light);
        }
        self.update_vis = false;
    }

    /// `W3DBridgeBuffer::drawBridges` (W3DBridgeBuffer.cpp:1114): draws the bridges. `logic_bridges` is
    /// `Some(infos of TheTerrainLogic->getFirstBridge()...)` when `TheTerrainLogic` exists (`None` in the world builder);
    /// `has_shroud` is `TheTerrainRenderObject->getShroud() != NULL`. Returns what the passes draw.
    pub fn draw_bridges(
        &mut self,
        wireframe: bool,
        cloud_texture: bool,
        logic_bridges: Option<&[BridgeInfo]>,
        has_shroud: bool,
        global_data: &GlobalData,
        do_the_light: &impl Fn(&Vector3, &[Vector3], &Vector3, f32) -> u32,
        roads: &TerrainRoadCollection,
        create_render_obj: &mut impl FnMut(&[u8]) -> Option<BridgeRenderObj>,
        get_texture: &mut impl FnMut(&[u8]) -> Option<TextureHandle>,
    ) -> BridgeDrawPlan {
        if let Some(infos) = logic_bridges {
            for cur_bridge in 0..self.num_bridges as usize {
                self.bridges[cur_bridge].set_enabled(false);
            }
            // Check for any changed damage states.
            let mut changed = false;
            for info in infos {
                if info.bridge_index < 0 || info.bridge_index >= self.num_bridges {
                    continue;
                }
                let bi = info.bridge_index as usize;
                self.bridges[bi].set_enabled(true);
                if self.bridges[bi].get_damage_state() != info.cur_damage_state {
                    changed = true;
                    let cur_state = self.bridges[bi].get_damage_state();
                    self.bridges[bi].set_damage_state(info.cur_damage_state);
                    if !self.bridges[bi].load(info.cur_damage_state, roads, create_render_obj, get_texture) {
                        // put the old model back.
                        self.bridges[bi].load(cur_state, roads, create_render_obj, get_texture);
                        self.bridges[bi].set_damage_state(info.cur_damage_state);
                    }
                }
            }
            if changed {
                // upstream passes NULL lights; unused by this code
                self.load_bridges_in_vertex_and_index_buffers(global_data, do_the_light);
            }
        } else {
            // In wb, all are enabled.
            for cur_bridge in 0..self.num_bridges as usize {
                self.bridges[cur_bridge].set_enabled(true);
            }
        }

        let mut plan = BridgeDrawPlan::default();
        if self.cur_num_bridge_indices == 0 {
            return plan;
        }

        // upstream: Set_Material / Set_Index_Buffer / Set_Vertex_Buffer / Set_Shader(detailAlphaShader) — device-only

        if !wireframe && cloud_texture {
            // Force a cloud texture projection into stage 1
            plan.cloud_pass = true;
        }

        for cur_bridge in 0..self.num_bridges as usize {
            if self.bridges[cur_bridge].is_enabled() && self.bridges[cur_bridge].is_visible() {
                if let Some((texture, draw)) = self.bridges[cur_bridge].render_bridge(wireframe) {
                    plan.main.push(BridgeDraw { bridge: cur_bridge, texture, draw });
                }
            }
        }

        // Render shroud pass over all the bridges
        if !wireframe && has_shroud {
            // upstream: reset to _PresetOpaqueShader, W3DShaderManager::setShader(ST_SHROUD_TEXTURE, 0) — device-only
            for cur_bridge in 0..self.num_bridges as usize {
                if self.bridges[cur_bridge].is_enabled() && self.bridges[cur_bridge].is_visible() {
                    // Pretend we're in wireframe so function doesn't reset the shroud texture.
                    if let Some((texture, draw)) = self.bridges[cur_bridge].render_bridge(true) {
                        plan.shroud.push(BridgeDraw { bridge: cur_bridge, texture, draw });
                    }
                }
            }
        }
        plan
    }
}
