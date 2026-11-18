//! UNTESTED DRAFT (#202, #204): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DRoadBuffer.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DRoadBuffer.cpp
//!
//! # How the device and the terrain are reached
//!
//! - Textures, vertex buffers and index buffers are the device's: a [`RoadType`] holds `Vec`s of the sizes upstream
//!   allocates (`TheGlobalData->m_maxRoadVertex + 4`, `m_maxRoadIndex + 4`) and an opaque [`TextureHandle`].
//! - The height map (`WorldHeightMap *m_map`) is [`RoadMapInfo`], a plain-data snapshot of the members this file reads.
//!   `TheTerrainRenderObject->getMaxCellHeight(x, y)` and `getStaticDiffuse(x, y)` (BaseHeightMap.cpp, not in this
//!   area) are the closures of [`RoadTerrainFns`].
//! - The dynamic lights (`RefRenderObjListIterator` of `LightClass`) are a slice of [`RoadLight`].
//! - Pointers into `m_roads` (`RoadSegment *`) are indices into `roads`.
//!
//! `LOAD_TEST_ASSETS` is defined (FileSystem.h:92), so the code it guards is translated. `abs(Real)` upstream is
//! the floating point overload (`<cmath>`): `f32::abs`.

use super::w3d_common::{LineSegClass, SphereClass, TextureHandle, VertexFormatXYZDUV1};
use rts_lockstep_engine::common::base_type::{real_to_int, PI};
use rts_lockstep_engine::logic::terrain_visual::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;

/// `TEE_WIDTH_ADJUSTMENT`.
pub const TEE_WIDTH_ADJUSTMENT: f32 = 1.03;
/// `CORNER_RADIUS`: the radius of the center of the road is 1.5 - outer radius 2.0, inner radius 1.0.
pub const CORNER_RADIUS: f32 = 1.5;
/// `TIGHT_CORNER_RADIUS`: the radius of the center of the road is 0.5 - outer radius 1.0, inner radius 0.0.
pub const TIGHT_CORNER_RADIUS: f32 = 0.5;

/// `bottomLeft`.
pub const BOTTOM_LEFT: usize = 0;
/// `bottomRight`.
pub const BOTTOM_RIGHT: usize = 1;
/// `topLeft`.
pub const TOP_LEFT: usize = 2;
/// `topRight`.
pub const TOP_RIGHT: usize = 3;
/// `NUM_CORNERS`.
pub const NUM_CORNERS: usize = 4;
/// `MAX_LINKS`.
pub const MAX_LINKS: i32 = 6;
/// `DEFAULT_ROAD_SCALE`.
pub const DEFAULT_ROAD_SCALE: f32 = 8.0;
/// `MIN_ROAD_SEGMENT`.
pub const MIN_ROAD_SEGMENT: f32 = 0.25;
/// `MAX_SEG_VERTEX`.
pub const MAX_SEG_VERTEX: i32 = 500;
/// `MAX_SEG_INDEX`.
pub const MAX_SEG_INDEX: i32 = 2000;

/// `s_dynamic` (W3DRoadBuffer.cpp): the road buffers are `USAGE_DEFAULT` (static).
pub const S_DYNAMIC: bool = false;

/// `xpSign` (W3DRoadBuffer.cpp:92): the sign of the cross product. If the tails of the vectors are both placed at 0,0,
/// then the cross product can be interpreted as -1 means `v2` is to the right of `v1`, 1 means `v2` is to the left of
/// `v1`, and 0 means `v2` is parallel to `v1`.
pub fn xp_sign(v1: &Vector2, v2: &Vector2) -> i32 {
    let xpdct = v1.x * v2.y - v1.y * v2.x;
    if xpdct < 0.0 {
        return -1;
    }
    if xpdct > 0.0 {
        return 1;
    }
    0
}

/// `struct TRoadPt` (W3DRoadBuffer.h).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TRoadPt {
    /// `loc`.
    pub loc: Vector2,
    /// `top`.
    pub top: Vector2,
    /// `bottom`.
    pub bottom: Vector2,
    /// `count`.
    pub count: i32,
    /// `last`.
    pub last: bool,
    /// `multi`.
    pub multi: bool,
    /// `isAngled`.
    pub is_angled: bool,
    /// `isJoin`.
    pub is_join: bool,
}

/// `struct TRoadSegInfo` (W3DRoadBuffer.h).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TRoadSegInfo {
    /// `loc`.
    pub loc: Vector2,
    /// `roadNormal`.
    pub road_normal: Vector2,
    /// `roadVector`.
    pub road_vector: Vector2,
    /// `corners[NUM_CORNERS]`.
    pub corners: [Vector2; NUM_CORNERS],
    /// `uOffset`.
    pub u_offset: f32,
    /// `vOffset`.
    pub v_offset: f32,
    /// `scale`.
    pub scale: f32,
}

/// `enum TCorner` (W3DRoadBuffer.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TCorner {
    /// `SEGMENT`.
    Segment = 0,
    /// `CURVE`.
    Curve = 1,
    /// `TEE`.
    Tee = 2,
    /// `FOUR_WAY`.
    FourWay = 3,
    /// `THREE_WAY_Y`.
    ThreeWayY = 4,
    /// `THREE_WAY_H`.
    ThreeWayH = 5,
    /// `THREE_WAY_H_FLIP`.
    ThreeWayHFlip = 6,
    /// `ALPHA_JOIN`.
    AlphaJoin = 7,
    /// `NUM_JOINS`.
    NumJoins = 8,
}

/// The `WorldHeightMap` members `W3DRoadBuffer` reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct RoadMapInfo {
    /// `getBorderSizeInline()`.
    pub border_size_inline: i32,
    /// `getXExtent()`.
    pub x_extent: i32,
    /// `getYExtent()`.
    pub y_extent: i32,
    /// `getDrawOrgX()`.
    pub draw_org_x: i32,
    /// `getDrawOrgY()`.
    pub draw_org_y: i32,
    /// `getMaxHeightValue()`.
    pub max_height_value: i32,
    /// `getMinHeightValue()`.
    pub min_height_value: i32,
}

/// `TheTerrainRenderObject->getMaxCellHeight(x, y)` and `->getStaticDiffuse(x, y)` (BaseHeightMap.cpp:1204, 2088).
pub struct RoadTerrainFns<H: Fn(f32, f32) -> f32, D: Fn(i32, i32) -> i32> {
    /// `getMaxCellHeight`: maximum height of the 4 corners containing the given point.
    pub max_cell_height: H,
    /// `getStaticDiffuse`: the static diffuse colour of a terrain vertex.
    pub static_diffuse: D,
}

/// The members of a `LightClass` (a dynamic light) the road lighting reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct RoadLight {
    /// `Get_Position()`.
    pub position: Vector3,
    /// `FarAttenStart` (`Get_Far_Attenuation_Range`'s first value).
    pub far_atten_start: f32,
    /// `FarAttenEnd` (`Get_Attenuation_Range()`, and the second value of `Get_Far_Attenuation_Range`).
    pub far_atten_end: f32,
    /// `Get_Type() == LightClass::POINT`.
    pub is_point: bool,
    /// `Get_Diffuse`.
    pub diffuse: Vector3,
    /// `Get_Ambient`.
    pub ambient: Vector3,
}

impl RoadLight {
    /// `LightClass::Get_Bounding_Sphere`: the light's position and attenuation range.
    pub fn bounding_sphere(&self) -> SphereClass {
        SphereClass::new(self.position, self.far_atten_end)
    }
}

/// `Spheres_Intersect` (sphere.h): test whether two spheres intersect.
pub fn spheres_intersect(s0: &SphereClass, s1: &SphereClass) -> bool {
    let delta = s0.center - s1.center;
    let dist2 = Vector3::dot_product(&delta, &delta);
    dist2 < (s0.radius + s1.radius) * (s0.radius + s1.radius)
}

/// `class RoadType` (W3DRoadBuffer.h).
#[derive(Clone, Debug)]
pub struct RoadType {
    /// `m_roadTexture`.
    pub road_texture: Option<TextureHandle>,
    /// `m_vertexRoad`: road vertex buffer (`m_maxRoadVertex + 4`); `None` where upstream's is NULL.
    pub vertex_road: Option<Vec<VertexFormatXYZDUV1>>,
    /// `m_indexRoad`: indices defining the triangles for the road drawing (`m_maxRoadIndex + 4`).
    pub index_road: Option<Vec<u16>>,
    /// `m_numRoadVertices`.
    pub num_road_vertices: i32,
    /// `m_numRoadIndices`.
    pub num_road_indices: i32,
    /// `m_uniqueID`: ID of the road type in INI.
    pub unique_id: i32,
    /// `m_isAutoLoaded`.
    pub is_auto_loaded: bool,
    /// `m_stackingOrder`: order in the drawing. 0 drawn first, then 1 and so on.
    pub stacking_order: i32,
    /// `m_texturePath` (`LOAD_TEST_ASSETS`).
    pub texture_path: Vec<u8>,
}

impl Default for RoadType {
    fn default() -> Self {
        Self::new()
    }
}

impl RoadType {
    /// `RoadType::RoadType` (W3DRoadBuffer.cpp:131): nulls index and vertex data.
    pub fn new() -> Self {
        RoadType {
            road_texture: None,
            vertex_road: None,
            index_road: None,
            num_road_vertices: 0,
            num_road_indices: 0,
            unique_id: -1,
            is_auto_loaded: false,
            stacking_order: 0,
            texture_path: Vec::new(),
        }
    }

    /// `RoadType::loadTexture` (W3DRoadBuffer.cpp:166): sets the W3D texture. `texture` is
    /// `pMgr->Get_Texture(path, MIP_LEVELS_3)` (mip-mapping `FILTER_TYPE_BEST`, U and V `TEXTURE_ADDRESS_REPEAT`: the
    /// renderer's). `max_road_vertex` / `max_road_index` are `TheGlobalData->m_maxRoadVertex` / `m_maxRoadIndex`.
    pub fn load_texture(&mut self, path: &[u8], id: i32, texture: Option<TextureHandle>, max_road_vertex: i32, max_road_index: i32) {
        // @todo - delay loading textures and only load textures referenced by map.
        self.road_texture = texture;

        // DX8VertexBufferClass(DX8_FVF_XYZDUV1, m_maxRoadVertex+4, s_dynamic ? USAGE_DYNAMIC : USAGE_DEFAULT)
        self.vertex_road = Some(vec![VertexFormatXYZDUV1::default(); (max_road_vertex + 4) as usize]);
        self.index_road = Some(vec![0u16; (max_road_index + 4) as usize]);
        self.num_road_vertices = 0;
        self.num_road_indices = 0;

        self.texture_path = path.to_vec();
        self.unique_id = id;
    }

    /// `RoadType::loadTestTexture` (W3DRoadBuffer.cpp:188, `LOAD_TEST_ASSETS`): loads the texture of an auto-loaded type
    /// from its path. `texture` is `NEW_REF(TextureClass, (path, path, MIP_LEVELS_3))`.
    pub fn load_test_texture(&mut self, texture: impl FnOnce(&[u8]) -> Option<TextureHandle>) {
        if self.is_auto_loaded && self.unique_id > 0 && !self.texture_path.is_empty() {
            // @todo - delay loading textures and only load textures referenced by map.
            self.road_texture = texture(&self.texture_path);
        }
    }

    /// `RoadType::applyTexture` (W3DRoadBuffer.cpp:153): sets the W3D texture, index and vertex buffers (all
    /// device-only: `W3DShaderManager::setTexture(0, m_roadTexture)`, `Set_Index_Buffer`, `Set_Vertex_Buffer`).
    pub fn apply_texture(&self) -> Option<TextureHandle> {
        self.road_texture
    }

    /// `RoadType::getStacking` (W3DRoadBuffer.h).
    pub fn get_stacking(&self) -> i32 {
        self.stacking_order
    }

    /// `RoadType::setStacking` (W3DRoadBuffer.h).
    pub fn set_stacking(&mut self, order: i32) {
        self.stacking_order = order;
    }

    /// `RoadType::getUniqueID` (W3DRoadBuffer.h).
    pub fn get_unique_id(&self) -> i32 {
        self.unique_id
    }

    /// `RoadType::getNumVertices` (W3DRoadBuffer.h).
    pub fn get_num_vertices(&self) -> i32 {
        self.num_road_vertices
    }

    /// `RoadType::setNumIndices` (W3DRoadBuffer.h).
    pub fn set_num_indices(&mut self, num: i32) {
        self.num_road_indices = num;
    }

    /// `RoadType::setNumVertices` (W3DRoadBuffer.h).
    pub fn set_num_vertices(&mut self, num: i32) {
        self.num_road_vertices = num;
    }

    /// `RoadType::getNumIndices` (W3DRoadBuffer.h).
    pub fn get_num_indices(&self) -> i32 {
        self.num_road_indices
    }

    /// `RoadType::setAutoLoaded` (W3DRoadBuffer.h).
    pub fn set_auto_loaded(&mut self) {
        self.is_auto_loaded = true;
    }

    /// `RoadType::isAutoLoaded` (W3DRoadBuffer.h).
    pub fn is_auto_loaded(&self) -> bool {
        self.is_auto_loaded
    }

    /// `RoadType::getPath` (W3DRoadBuffer.h).
    pub fn get_path(&self) -> &[u8] {
        &self.texture_path
    }
}

/// `class RoadSegment` (W3DRoadBuffer.h).
#[derive(Clone, Debug)]
pub struct RoadSegment {
    /// `m_pt1`: drawing location pt1 to pt2.
    pub pt1: TRoadPt,
    /// `m_pt2`: drawing location to.
    pub pt2: TRoadPt,
    /// `m_curveRadius`: radius if it is a curve.
    pub curve_radius: f32,
    /// `m_type`: segment, or curve, or intersection.
    pub type_: TCorner,
    /// `m_scale`.
    pub scale: f32,
    /// `m_widthInTexture`: width of the road in the texture.
    pub width_in_texture: f32,
    /// `m_uniqueID`: road type.
    pub unique_id: i32,
    /// `m_visible`.
    pub visible: bool,
    num_vertex: i32,
    vb: Option<Vec<VertexFormatXYZDUV1>>,
    num_index: i32,
    ib: Option<Vec<u16>>,
    info: TRoadSegInfo,
    bounds: SphereClass,
}

impl Default for RoadSegment {
    fn default() -> Self {
        Self::new()
    }
}

impl RoadSegment {
    /// `RoadSegment::RoadSegment` (W3DRoadBuffer.cpp:213): nulls index and vertex data.
    pub fn new() -> Self {
        RoadSegment {
            pt1: TRoadPt::default(),
            pt2: TRoadPt::default(),
            curve_radius: 0.0,
            type_: TCorner::Segment,
            scale: 1.0,
            width_in_texture: 1.0,
            unique_id: 0,
            visible: false,
            num_vertex: 0,
            vb: None,
            num_index: 0,
            ib: None,
            info: TRoadSegInfo::default(),
            bounds: SphereClass::new(Vector3::new(0.0, 0.0, 0.0), 1.0),
        }
    }

    /// `RoadSegment::~RoadSegment` (W3DRoadBuffer.cpp:233): frees index and vertex data.
    pub fn free_buffers(&mut self) {
        self.num_vertex = 0;
        self.vb = None;
        self.num_index = 0;
        self.ib = None;
    }

    /// `RoadSegment::SetVertexBuffer` (W3DRoadBuffer.cpp:253): allocates and sets the vertex entries.
    pub fn set_vertex_buffer(&mut self, vb: &[VertexFormatXYZDUV1], num_vertex: i32) {
        if self.vb.is_some() {
            self.vb = None;
            self.num_vertex = 0;
        }
        if num_vertex < 1 || num_vertex > MAX_SEG_VERTEX {
            return;
        }
        self.num_vertex = num_vertex;
        let copy: Vec<VertexFormatXYZDUV1> = vb[..num_vertex as usize].to_vec();
        let verts: Vec<Vector3> = copy.iter().map(|v| Vector3::new(v.x, v.y, v.z)).collect();
        self.vb = Some(copy);
        self.bounds = SphereClass::from_points(&verts);
    }

    /// `RoadSegment::SetIndexBuffer` (W3DRoadBuffer.cpp:281): allocates and sets the index entries.
    pub fn set_index_buffer(&mut self, ib: &[u16], num_index: i32) {
        if self.ib.is_some() {
            self.ib = None;
            self.num_index = 0;
        }
        if num_index < 1 || num_index > MAX_SEG_INDEX {
            return;
        }
        self.num_index = num_index;
        self.ib = Some(ib[..num_index as usize].to_vec());
    }

    /// `RoadSegment::SetRoadSegInfo` (W3DRoadBuffer.h).
    pub fn set_road_seg_info(&mut self, info: &TRoadSegInfo) {
        self.info = *info;
    }

    /// `RoadSegment::GetRoadSegInfo` (W3DRoadBuffer.h).
    pub fn get_road_seg_info(&self) -> TRoadSegInfo {
        self.info
    }

    /// `RoadSegment::getBounds` (W3DRoadBuffer.h).
    pub fn get_bounds(&self) -> &SphereClass {
        &self.bounds
    }

    /// `RoadSegment::GetNumVertex` (W3DRoadBuffer.h).
    pub fn get_num_vertex(&self) -> i32 {
        self.num_vertex
    }

    /// `RoadSegment::GetNumIndex` (W3DRoadBuffer.h).
    pub fn get_num_index(&self) -> i32 {
        self.num_index
    }

    /// `RoadSegment::GetVertices` (W3DRoadBuffer.cpp:299): copies vertex entries into `destination_vb`.
    pub fn get_vertices(&self, destination_vb: &mut [VertexFormatXYZDUV1], num_to_copy: i32) -> i32 {
        let vb = match &self.vb {
            Some(v) => v,
            None => return 0,
        };
        if num_to_copy < 1 {
            return 0;
        }
        if num_to_copy > self.num_vertex {
            return 0;
        }
        destination_vb[..num_to_copy as usize].copy_from_slice(&vb[..num_to_copy as usize]);
        num_to_copy
    }

    /// `RoadSegment::GetIndices` (W3DRoadBuffer.cpp:312): copies index entries into `destination_ib`.
    pub fn get_indices(&self, destination_ib: &mut [u16], num_to_copy: i32, offset: i32) -> i32 {
        let ib = match &self.ib {
            Some(v) => v,
            None => return 0,
        };
        if num_to_copy < 1 {
            return 0;
        }
        if num_to_copy > self.num_index {
            return 0;
        }
        for i in 0..num_to_copy as usize {
            destination_ib[i] = ib[i].wrapping_add(offset as u16);
        }
        num_to_copy
    }

    /// `RoadSegment::updateSegLighting` (W3DRoadBuffer.cpp:329): updates the diffuse lighting in the vertex buffer.
    pub fn update_seg_lighting(&mut self, border_size_inline: i32, static_diffuse: &impl Fn(i32, i32) -> i32) {
        if let Some(vb) = self.vb.as_mut() {
            for i in 0..self.num_vertex as usize {
                let mut x = ((vb[i].x / MAP_XY_FACTOR) as f64 + 0.5) as i32;
                let mut y = ((vb[i].y / MAP_XY_FACTOR) as f64 + 0.5) as i32;
                x += border_size_inline;
                y += border_size_inline;
                vb[i].diffuse = ((255u32) << 24) | (static_diffuse(x, y) as u32);
            }
        }
    }
}

/// The `TColumn` struct local to `loadFloat4PtSection` and `loadLit4PtSection`.
#[derive(Clone, Copy)]
struct TColumn {
    collapsed: bool,
    deleted: bool,
    vtx: [Vector3; MAX_ROWS],
    diffuse_red: i32,
    light_gradient: bool,
    vertex_index: [i32; MAX_ROWS],
    u_index: f32,
}

/// `maxRows` in `loadFloat4PtSection`.
const MAX_ROWS: usize = 100;

impl TColumn {
    fn new() -> Self {
        TColumn { collapsed: false, deleted: false, vtx: [Vector3::default(); MAX_ROWS], diffuse_red: 0, light_gradient: false, vertex_index: [0; MAX_ROWS], u_index: 0.0 }
    }
}

/// `class W3DRoadBuffer` (W3DRoadBuffer.h): draw buffer for the roads.
pub struct W3DRoadBuffer {
    /// `m_roadTypes`.
    pub road_types: Vec<RoadType>,
    /// `m_roads`: the road buffer. All roads are stored here.
    pub roads: Vec<RoadSegment>,
    /// `m_numRoads`: number of roads used in `m_roads`.
    pub num_roads: i32,
    /// `m_initialized`.
    pub initialized: bool,
    /// `m_map`: the height map data (`None` where upstream's is NULL).
    pub map: Option<RoadMapInfo>,
    /// `m_curUniqueID`: road type we are rendering at this pass.
    pub cur_unique_id: i32,
    /// `m_curRoadType`.
    pub cur_road_type: i32,
    /// `m_maxUID` (`LOAD_TEST_ASSETS`): maximum UID.
    pub max_uid: i32,
    /// `m_curOpenRoad` (`LOAD_TEST_ASSETS`): first road type not used.
    pub cur_open_road: i32,
    /// `m_maxRoadSegments`: size of `m_roads`.
    pub max_road_segments: i32,
    /// `m_maxRoadVertex`: size of `m_vertexRoad`.
    pub max_road_vertex: i32,
    /// `m_maxRoadIndex`: size of `m_indexRoad`.
    pub max_road_index: i32,
    /// `m_maxRoadTypes`: size of `m_roadTypes`.
    pub max_road_types: i32,
    /// `m_curNumRoadVertices`: number of vertices used in current road type.
    pub cur_num_road_vertices: i32,
    /// `m_curNumRoadIndices`: number of indices used in current road type.
    pub cur_num_road_indices: i32,
    /// `m_updateBuffers`: if true, update the vertex buffers.
    pub update_buffers: bool,
    /// `warnSegments` (file-scope static in W3DRoadBuffer.cpp).
    pub warn_segments: bool,
}

impl W3DRoadBuffer {
    /// `W3DRoadBuffer::loadTee` (W3DRoadBuffer.cpp:349): loads a tee into the vertex buffer for a road join.
    pub fn load_tee<H: Fn(f32, f32) -> f32, D: Fn(i32, i32) -> i32>(&mut self, p_road: usize, loc1: Vector2, loc2: Vector2, is_4way: bool, scale: f32, terrain: &RoadTerrainFns<H, D>) {
        let mut road_vector = Vector2::new(loc2.x - loc1.x, loc2.y - loc1.y);
        let mut road_normal = Vector2::new(-road_vector.y, road_vector.x);
        if road_vector.x.abs() < MIN_ROAD_SEGMENT && road_vector.y.abs() < MIN_ROAD_SEGMENT {
            road_vector.set(1.0, 0.0);
            road_normal.set(0.0, 1.0);
        } else {
            road_vector.normalize();
            road_normal.normalize();
        }

        let u_offset = 425.0f32 / 512.0;
        let mut v_offset = 255.0f32 / 512.0;
        if is_4way {
            v_offset = 425.0f32 / 512.0;
        }
        let tee_factor = scale * TEE_WIDTH_ADJUSTMENT / 2.0;
        let left = self.roads[p_road].width_in_texture * scale / 2.0;
        self.load_float_section(p_road, loc1, loc2 - loc1, tee_factor, left, tee_factor, u_offset, v_offset, scale, terrain);
    }

    /// `W3DRoadBuffer::loadAlphaJoin` (W3DRoadBuffer.cpp:380): loads an alpha join into the vertex buffer for a road join.
    pub fn load_alpha_join<H: Fn(f32, f32) -> f32, D: Fn(i32, i32) -> i32>(&mut self, p_road: usize, loc1: Vector2, loc2: Vector2, scale: f32, terrain: &RoadTerrainFns<H, D>) {
        let mut road_vector = Vector2::new(loc2.x - loc1.x, loc2.y - loc1.y);
        let mut road_normal = Vector2::new(-road_vector.y, road_vector.x);
        if road_vector.x.abs() < MIN_ROAD_SEGMENT && road_vector.y.abs() < MIN_ROAD_SEGMENT {
            road_vector.set(1.0, 0.0);
            road_normal.set(0.0, 1.0);
        } else {
            road_vector.normalize();
            road_normal.normalize();
        }

        let u_offset = 106.0f32 / 512.0;
        let v_offset = 425.0f32 / 512.0;

        let roadwidth = scale;
        let u_scale = self.roads[p_road].width_in_texture;

        road_vector *= roadwidth * 48.0 / 128.0; // we just want 48 pixels.
        road_normal *= u_scale * (1.0 + 8.0f32 / 128.0); // we want 8 extra pixels.

        let mut corners = [Vector2::default(); NUM_CORNERS];

        corners[TOP_LEFT] = loc1 + road_normal * 0.5 - road_vector * 0.65;
        corners[BOTTOM_LEFT] = corners[TOP_LEFT] - road_normal;
        corners[BOTTOM_RIGHT] = corners[BOTTOM_LEFT] + road_vector;
        corners[TOP_RIGHT] = corners[TOP_LEFT] + road_vector;
        self.load_float_4pt_section(p_road, loc1, road_normal, road_vector, &corners, u_offset, v_offset, scale, u_scale, terrain);
    }

    /// `W3DRoadBuffer::loadY` (W3DRoadBuffer.cpp:421): loads a Y into the vertex buffer for a road join.
    pub fn load_y<H: Fn(f32, f32) -> f32, D: Fn(i32, i32) -> i32>(&mut self, p_road: usize, loc1: Vector2, loc2: Vector2, scale: f32, terrain: &RoadTerrainFns<H, D>) {
        let mut road_vector = Vector2::new(loc2.x - loc1.x, loc2.y - loc1.y);
        let mut road_normal = Vector2::new(-road_vector.y, road_vector.x);
        if road_vector.x.abs() < MIN_ROAD_SEGMENT && road_vector.y.abs() < MIN_ROAD_SEGMENT {
            road_vector.set(1.0, 0.0);
            road_normal.set(0.0, 1.0);
        } else {
            road_vector.normalize();
            road_normal.normalize();
        }

        let u_offset = 255.0f32 / 512.0;
        let v_offset = 226.0f32 / 512.0;

        let roadwidth = scale;

        road_vector *= roadwidth;
        road_normal *= roadwidth;

        let mut corners = [Vector2::default(); NUM_CORNERS];

        road_vector *= 1.59;
        corners[TOP_LEFT] = loc1 + road_normal * 0.29 - road_vector * 0.5;
        corners[BOTTOM_LEFT] = corners[TOP_LEFT] - road_normal * 1.08;
        corners[BOTTOM_RIGHT] = corners[BOTTOM_LEFT] + road_vector;
        corners[TOP_RIGHT] = corners[TOP_LEFT] + road_vector;
        self.load_float_4pt_section(p_road, loc1, road_normal, road_vector, &corners, u_offset, v_offset, scale, scale, terrain);
    }

    /// `W3DRoadBuffer::loadH` (W3DRoadBuffer.cpp:462): loads an h shaped tee into the vertex buffer for a road join.
    pub fn load_h<H: Fn(f32, f32) -> f32, D: Fn(i32, i32) -> i32>(&mut self, p_road: usize, loc1: Vector2, loc2: Vector2, flip: bool, scale: f32, terrain: &RoadTerrainFns<H, D>) {
        let mut road_vector = Vector2::new(loc2.x - loc1.x, loc2.y - loc1.y);
        let mut road_normal = Vector2::new(-road_vector.y, road_vector.x);
        if road_vector.x.abs() < MIN_ROAD_SEGMENT && road_vector.y.abs() < MIN_ROAD_SEGMENT {
            road_vector.set(1.0, 0.0);
            road_normal.set(0.0, 1.0);
        } else {
            road_vector.normalize();
            road_normal.normalize();
        }

        let u_offset = 202.0f32 / 512.0;
        let v_offset = 364.0f32 / 512.0;

        let roadwidth = scale;

        road_vector *= roadwidth;
        road_normal *= roadwidth;

        let mut corners = [Vector2::default(); NUM_CORNERS];

        let width_in_texture = self.roads[p_road].width_in_texture;
        road_normal *= 1.35;
        if flip {
            corners[BOTTOM_LEFT] = loc1 - road_normal * 0.20 - road_vector * width_in_texture / 2.0;
        } else {
            corners[BOTTOM_LEFT] = loc1 - road_normal * 0.8 - road_vector * width_in_texture / 2.0;
        }
        let mut width = road_vector * width_in_texture / 2.0;
        width = width + road_vector * 1.2;
        corners[BOTTOM_RIGHT] = corners[BOTTOM_LEFT] + width;
        corners[TOP_RIGHT] = corners[BOTTOM_RIGHT] + road_normal;
        corners[TOP_LEFT] = corners[BOTTOM_LEFT] + road_normal;
        if flip {
            road_normal = -road_normal;
        }
        self.load_float_4pt_section(p_road, loc1, road_normal, road_vector, &corners, u_offset, v_offset, scale, scale, terrain);
    }

    /// `W3DRoadBuffer::loadFloatSection` (W3DRoadBuffer.cpp:507): loads a section of road using a mesh that floats a
    /// little above the terrain.
    pub fn load_float_section<H: Fn(f32, f32) -> f32, D: Fn(i32, i32) -> i32>(
        &mut self,
        p_road: usize,
        loc: Vector2,
        mut road_vector: Vector2,
        mut half_height: f32,
        left: f32,
        right: f32,
        u_offset: f32,
        v_offset: f32,
        scale: f32,
        terrain: &RoadTerrainFns<H, D>,
    ) {
        if self.map.is_none() {
            return;
        }

        let mut road_normal = Vector2::new(-road_vector.y, road_vector.x);
        road_vector.normalize();
        road_vector *= right;

        road_normal.normalize();
        if half_height < 0.0 {
            half_height = -half_height;
        }
        road_normal *= half_height;

        let mut road_left = road_vector;
        road_left.normalize();
        road_left *= left;
        road_vector += road_left;
        let mut left_center = loc;
        left_center -= road_left;

        let mut corners = [Vector2::default(); NUM_CORNERS];

        corners[BOTTOM_LEFT] = left_center - road_normal;
        corners[BOTTOM_RIGHT] = corners[BOTTOM_LEFT];
        corners[BOTTOM_RIGHT] += road_vector;
        corners[TOP_RIGHT] = corners[BOTTOM_RIGHT];
        corners[TOP_RIGHT] += 2.0 * road_normal;
        corners[TOP_LEFT] = corners[BOTTOM_LEFT];
        corners[TOP_LEFT] += 2.0 * road_normal;

        self.load_float_4pt_section(p_road, loc, road_normal, road_vector, &corners, u_offset, v_offset, scale, scale, terrain);
    }

    /// `W3DRoadBuffer::loadFloat4PtSection` (W3DRoadBuffer.cpp:555): loads a section of road using a mesh that floats a
    /// little above the terrain. The road is loaded into the quadrilateral defined by the 4 corner points. `loc`
    /// specifies the point where u == uOffset && v == vOffset, the road vector gives the direction of the road, and the
    /// road normal is perpendicular to it.
    pub fn load_float_4pt_section<H: Fn(f32, f32) -> f32, D: Fn(i32, i32) -> i32>(
        &mut self,
        p_road: usize,
        loc: Vector2,
        mut road_normal: Vector2,
        mut road_vector: Vector2,
        corners_p: &[Vector2; NUM_CORNERS],
        u_offset: f32,
        v_offset: f32,
        u_scale: f32,
        v_scale: f32,
        terrain: &RoadTerrainFns<H, D>,
    ) {
        let float_amount: f32 = MAP_HEIGHT_SCALE / 8.0;
        let max_error: f32 = MAP_HEIGHT_SCALE * 1.1;
        // upstream's vb is VertexFormatXYZDUV1[MAX_SEG_VERTEX] but the vertex limit it tests is MAX_SEG_INDEX: the
        // buffer here has MAX_SEG_INDEX entries so that case does not run off the end (SetVertexBuffer then refuses it).
        let mut ib = vec![0u16; MAX_SEG_INDEX as usize];
        let mut vb = vec![VertexFormatXYZDUV1::default(); MAX_SEG_INDEX as usize];
        let mut num_road_vertices: i32 = 0;
        let mut num_road_indices: i32 = 0;

        let mut info = TRoadSegInfo::default();
        info.loc = loc;
        info.road_normal = road_normal;
        info.road_vector = road_vector;
        info.corners[BOTTOM_LEFT] = corners_p[BOTTOM_LEFT];
        info.corners[BOTTOM_RIGHT] = corners_p[BOTTOM_RIGHT];
        info.corners[TOP_LEFT] = corners_p[TOP_LEFT];
        info.corners[TOP_RIGHT] = corners_p[TOP_RIGHT];
        info.u_offset = u_offset;
        info.v_offset = v_offset;
        info.scale = u_scale;
        self.roads[p_road].set_road_seg_info(&info);

        let map = match self.map {
            Some(m) => m,
            None => return,
        };

        let road_len = road_vector.length();
        let half_height = road_normal.length();
        road_normal.normalize();
        road_vector.normalize();
        let mut u_count = ((road_len / MAP_XY_FACTOR) + 1.0) as i32;
        if u_count < 2 {
            u_count = 2;
        }
        let mut v_count = (((2.0 * half_height) / MAP_XY_FACTOR) + 1.0) as i32;
        if v_count < 2 {
            v_count = 2;
        }

        let diffuse_limit: i32 = 25; // if more than that, we tesselate :) jba.
        let _ = diffuse_limit;

        if v_count > MAX_ROWS as i32 {
            v_count = MAX_ROWS as i32;
        }
        let mut prev_column = TColumn::new();
        let mut cur_column = TColumn::new();
        let mut next_column = TColumn::new();

        prev_column.deleted = true;
        cur_column.deleted = true;
        let mut v2 = corners_p[BOTTOM_LEFT];
        let origin = Vector3::new(v2.x, v2.y, 0.0);
        v2 = corners_p[BOTTOM_RIGHT] - corners_p[BOTTOM_LEFT];
        let u_vector1 = Vector3::new(v2.x, v2.y, 0.0);
        v2 = corners_p[TOP_RIGHT] - corners_p[TOP_LEFT];
        let mut u_vector2 = Vector3::new(v2.x, v2.y, 0.0);
        v2 = corners_p[TOP_LEFT];
        let _origin2 = Vector3::new(v2.x, v2.y, 0.0);
        v2 = corners_p[TOP_LEFT] - corners_p[BOTTOM_LEFT];
        let v_vector1 = Vector3::new(v2.x, v2.y, 0.0);
        v2 = corners_p[TOP_RIGHT] - corners_p[BOTTOM_RIGHT];
        let v_vector2 = Vector3::new(v2.x, v2.y, 0.0);
        u_vector2 += v_vector1 - v_vector2;
        let mut i: i32 = 0;
        while i <= u_count {
            let i_factor = i as f32 / (u_count - 1) as f32;
            let i_bar_factor = 1.0 - i_factor;
            if i < u_count {
                next_column.collapsed = false;
                next_column.deleted = false;
                next_column.light_gradient = false;
                next_column.u_index = i as f32;

                let mut min_height = map.max_height_value as f32 * MAP_HEIGHT_SCALE;
                let mut max_height = map.min_height_value as f32 * MAP_HEIGHT_SCALE;
                for j in 0..v_count as usize {
                    let j_factor = j as f32 / (v_count - 1) as f32;
                    let j_bar_factor = 1.0 - j_factor;
                    next_column.vtx[j] = origin + (u_vector1 * j_bar_factor * i_factor) + (u_vector2 * j_factor * i_factor) + (v_vector1 * i_bar_factor * j_factor) + (v_vector2 * i_factor * j_factor);
                    let z = (terrain.max_cell_height)(next_column.vtx[j].x, next_column.vtx[j].y);
                    if z < min_height {
                        min_height = z;
                    }
                    if z > max_height {
                        max_height = z;
                    }
                    next_column.vertex_index[j] = -1;
                    next_column.vtx[j].z = z;
                    let diffuse: i32 = 0;
                    if j == 0 {
                        next_column.diffuse_red = diffuse & 0x00ff;
                    } else {
                        let red = diffuse & 0x00ff;
                        if (red - next_column.diffuse_red).abs() > 25 {
                            next_column.light_gradient = true;
                        }
                    }
                }
                let _ = min_height;

                // if (true) { // !nextColumn.lightGradient) {
                next_column.collapsed = true;
                next_column.vtx[0].z = max_height;
                next_column.vtx[1] = next_column.vtx[(v_count - 1) as usize];
                next_column.vtx[1].z = max_height;
                // } else { for (j...) nextColumn.vtx[j].Z = maxHeight; }  (unreachable: the condition is `true`)
                if i < 2 {
                    cur_column = next_column;
                } else if prev_column.collapsed && cur_column.collapsed && next_column.collapsed {
                    let mut ok_to_delete = false;

                    let mut the_z = prev_column.vtx[0].z * (cur_column.u_index - prev_column.u_index) + next_column.vtx[0].z * (next_column.u_index - cur_column.u_index);
                    the_z /= next_column.u_index - prev_column.u_index;
                    if the_z >= cur_column.vtx[0].z && the_z < cur_column.vtx[0].z + max_error {
                        the_z = prev_column.vtx[1].z * (cur_column.u_index - prev_column.u_index) + next_column.vtx[1].z * (next_column.u_index - cur_column.u_index);
                        the_z /= next_column.u_index - prev_column.u_index;
                        if the_z >= cur_column.vtx[1].z && the_z < cur_column.vtx[1].z + max_error {
                            ok_to_delete = true;
                        }
                    }
                    if ok_to_delete {
                        cur_column.deleted = true;
                    }
                }
            }
            if !cur_column.deleted && i != 1 {
                // Write out the vertices.
                for j in 0..v_count as usize {
                    if num_road_vertices >= MAX_SEG_INDEX {
                        break;
                    }
                    let cur_vector = Vector2::new(cur_column.vtx[j].x - loc.x, cur_column.vtx[j].y - loc.y);
                    let v = Vector2::dot_product(&road_normal, &cur_vector);
                    let u = Vector2::dot_product(&road_vector, &cur_vector);
                    let diffuse: u32 = 0;
                    let nv = num_road_vertices as usize;
                    vb[nv].u1 = u_offset + u / (u_scale * 4.0);
                    vb[nv].v1 = v_offset - v / (v_scale * 4.0); // Road is 1/16 texture height.
                    vb[nv].x = cur_column.vtx[j].x;
                    vb[nv].y = cur_column.vtx[j].y;
                    vb[nv].z = cur_column.vtx[j].z + float_amount;
                    vb[nv].diffuse = diffuse;
                    cur_column.vertex_index[j] = num_road_vertices;
                    num_road_vertices += 1;
                    if j == 1 && cur_column.collapsed {
                        break;
                    }
                }
                if num_road_vertices >= MAX_SEG_INDEX {
                    break;
                }
                if i > 1 {
                    // Write out the triangles.
                    let mut j: usize = 0;
                    let mut k: usize = 0;
                    while (j as i32) < v_count - 1 && (k as i32) < v_count - 1 {
                        if num_road_indices >= MAX_SEG_INDEX {
                            break;
                        }
                        let mut cur_ib = num_road_indices as usize;
                        if k == 0 || !prev_column.collapsed {
                            ib[cur_ib] = prev_column.vertex_index[j + 1] as u16;
                            ib[cur_ib + 1] = prev_column.vertex_index[j] as u16;
                            ib[cur_ib + 2] = cur_column.vertex_index[k] as u16;
                            cur_ib += 3;
                            num_road_indices += 3;
                        }
                        if j == 0 || !cur_column.collapsed {
                            let mut offset: usize = 1;
                            if cur_column.collapsed && !prev_column.collapsed {
                                offset = (v_count - 1) as usize;
                            }
                            ib[cur_ib] = prev_column.vertex_index[j + offset] as u16;
                            ib[cur_ib + 1] = cur_column.vertex_index[k] as u16;
                            ib[cur_ib + 2] = cur_column.vertex_index[k + 1] as u16;
                            num_road_indices += 3;
                        }
                        if prev_column.collapsed && cur_column.collapsed {
                            break;
                        }
                        if !prev_column.collapsed {
                            j += 1;
                        }
                        if !cur_column.collapsed {
                            k += 1;
                        }
                    }
                    prev_column = cur_column;
                } else if i == 0 {
                    prev_column = cur_column;
                }
                if num_road_indices >= MAX_SEG_INDEX {
                    break;
                }
            }
            cur_column = next_column;
            i += 1;
        }
        self.roads[p_road].set_vertex_buffer(&vb, num_road_vertices);
        self.roads[p_road].set_index_buffer(&ib, num_road_indices);
    }
}

impl W3DRoadBuffer {
    /// `W3DRoadBuffer::loadLit4PtSection` (W3DRoadBuffer.cpp:769): loads a section of road lit by the dynamic lights,
    /// into the shared `ib` / `vb` at `m_curNumRoadVertices` / `m_curNumRoadIndices`. Same geometry as
    /// [`W3DRoadBuffer::load_float_4pt_section`], taken from the segment's stored `TRoadSegInfo`.
    pub fn load_lit_4pt_section<H: Fn(f32, f32) -> f32, D: Fn(i32, i32) -> i32>(
        &mut self,
        p_road: usize,
        ib: &mut [u16],
        vb: &mut [VertexFormatXYZDUV1],
        dynamic_lights: &[RoadLight],
        terrain: &RoadTerrainFns<H, D>,
    ) {
        let float_amount: f32 = MAP_HEIGHT_SCALE / 8.0;
        let max_error: f32 = MAP_HEIGHT_SCALE * 1.1;

        if self.roads[p_road].unique_id != self.cur_unique_id {
            return;
        }
        if !self.roads[p_road].visible {
            return;
        }
        let max_lights = 8usize;
        let mut lights: Vec<RoadLight> = Vec::new();

        for p_light in dynamic_lights {
            let bounds = p_light.bounding_sphere();
            if spheres_intersect(self.roads[p_road].get_bounds(), &bounds) {
                lights.push(*p_light);
                if lights.len() == max_lights {
                    break;
                }
            }
        }
        let num_lights = lights.len();

        if num_lights == 0 {
            return;
        }

        let map = match self.map {
            Some(m) => m,
            None => return,
        };

        let mut info = self.roads[p_road].get_road_seg_info();
        let road_len = info.road_vector.length();
        let half_height = info.road_normal.length();
        info.road_normal.normalize();
        info.road_vector.normalize();
        let u_count = ((road_len / MAP_XY_FACTOR) + 1.0) as i32;
        let mut v_count = (((2.0 * half_height) / MAP_XY_FACTOR) + 1.0) as i32;

        if v_count > MAX_ROWS as i32 {
            v_count = MAX_ROWS as i32;
        }
        let mut prev_column = TColumn::new();
        let mut cur_column = TColumn::new();
        let mut next_column = TColumn::new();

        prev_column.deleted = true;
        cur_column.deleted = true;
        let mut v2 = info.corners[BOTTOM_LEFT];
        let origin = Vector3::new(v2.x, v2.y, 0.0);
        v2 = info.corners[BOTTOM_RIGHT] - info.corners[BOTTOM_LEFT];
        let u_vector1 = Vector3::new(v2.x, v2.y, 0.0);
        v2 = info.corners[TOP_RIGHT] - info.corners[TOP_LEFT];
        let mut u_vector2 = Vector3::new(v2.x, v2.y, 0.0);
        v2 = info.corners[TOP_LEFT] - info.corners[BOTTOM_LEFT];
        let v_vector1 = Vector3::new(v2.x, v2.y, 0.0);
        v2 = info.corners[TOP_RIGHT] - info.corners[BOTTOM_RIGHT];
        let v_vector2 = Vector3::new(v2.x, v2.y, 0.0);
        u_vector2 += v_vector1 - v_vector2;
        let mut i: i32 = 0;
        while i <= u_count {
            let i_factor = i as f32 / (u_count - 1) as f32;
            let i_bar_factor = 1.0 - i_factor;
            if i < u_count {
                next_column.collapsed = false;
                next_column.deleted = false;
                next_column.light_gradient = false;
                next_column.u_index = i as f32;

                let mut min_height = map.max_height_value as f32 * MAP_HEIGHT_SCALE;
                let mut max_height = map.min_height_value as f32 * MAP_HEIGHT_SCALE;
                for j in 0..v_count as usize {
                    let j_factor = j as f32 / (v_count - 1) as f32;
                    let j_bar_factor = 1.0 - j_factor;
                    next_column.vtx[j] = origin + (u_vector1 * j_bar_factor * i_factor) + (u_vector2 * j_factor * i_factor) + (v_vector1 * i_bar_factor * j_factor) + (v_vector2 * i_factor * j_factor);
                    let z = (terrain.max_cell_height)(next_column.vtx[j].x, next_column.vtx[j].y);
                    if z < min_height {
                        min_height = z;
                    }
                    if z > max_height {
                        max_height = z;
                    }
                    next_column.vertex_index[j] = -1;
                    next_column.vtx[j].z = z;
                    for light in lights.iter() {
                        let offset = next_column.vtx[j] - light.position;
                        let mut range = light.far_atten_end; // Get_Attenuation_Range()
                        // for culling, expand one cell radius.
                        range += MAP_XY_FACTOR;
                        if offset.length2() < range * range {
                            next_column.light_gradient = true;
                        }
                    }
                }
                let _ = min_height;
                if !next_column.light_gradient {
                    next_column.collapsed = true;
                    next_column.vtx[0].z = max_height;
                    next_column.vtx[1] = next_column.vtx[(v_count - 1) as usize];
                    next_column.vtx[1].z = max_height;
                } else {
                    for j in 0..v_count as usize {
                        next_column.vtx[j].z = max_height;
                    }
                }
                if i < 2 {
                    cur_column = next_column;
                } else if prev_column.collapsed && cur_column.collapsed && next_column.collapsed {
                    let mut ok_to_delete = false;

                    let mut the_z = prev_column.vtx[0].z * (cur_column.u_index - prev_column.u_index) + next_column.vtx[0].z * (next_column.u_index - cur_column.u_index);
                    the_z /= next_column.u_index - prev_column.u_index;
                    if the_z >= cur_column.vtx[0].z && the_z < cur_column.vtx[0].z + max_error {
                        the_z = prev_column.vtx[1].z * (cur_column.u_index - prev_column.u_index) + next_column.vtx[1].z * (next_column.u_index - cur_column.u_index);
                        the_z /= next_column.u_index - prev_column.u_index;
                        if the_z >= cur_column.vtx[1].z && the_z < cur_column.vtx[1].z + max_error {
                            ok_to_delete = true;
                        }
                    }
                    if ok_to_delete {
                        cur_column.deleted = true;
                    }
                }
            }
            if !cur_column.deleted && i != 1 {
                // Write out the vertices.
                for j in 0..v_count as usize {
                    if self.cur_num_road_vertices >= self.max_road_vertex {
                        break;
                    }
                    let cur_vector = Vector2::new(cur_column.vtx[j].x - info.loc.x, cur_column.vtx[j].y - info.loc.y);
                    let v = Vector2::dot_product(&info.road_normal, &cur_vector);
                    let u = Vector2::dot_product(&info.road_vector, &cur_vector);
                    let mut diffuse: i32 = (255i32 << 24)
                        | (terrain.static_diffuse)(
                            ((cur_column.vtx[j].x / MAP_XY_FACTOR) as f64 + 0.5) as i32,
                            ((cur_column.vtx[j].y / MAP_XY_FACTOR) as f64 + 0.5) as i32,
                        );
                    let mut shade_b: f32 = ((diffuse & 0xFF) as f64 / 255.0) as f32;
                    let mut shade_g: f32 = (((diffuse >> 8) & 0xFF) as f64 / 255.0) as f32;
                    let mut shade_r: f32 = (((diffuse >> 16) & 0xFF) as f64 / 255.0) as f32;
                    for light in lights.iter() {
                        if light.is_point {
                            let light_loc = light.position;
                            let vtx = cur_column.vtx[j];
                            let offset = vtx - light_loc;
                            let range: f64 = light.far_atten_end as f64;
                            let mid_range: f64 = light.far_atten_start as f64;
                            if (vtx.x as f64) < light_loc.x as f64 - range {
                                continue;
                            }
                            if (vtx.x as f64) > light_loc.x as f64 + range {
                                continue;
                            }
                            if (vtx.y as f64) < light_loc.y as f64 - range {
                                continue;
                            }
                            if (vtx.y as f64) > light_loc.y as f64 + range {
                                continue;
                            }
                            let dist = offset.length();
                            if dist as f64 >= range {
                                continue;
                            }
                            if mid_range < 0.1 {
                                continue;
                            }
                            let mut factor: f32 = (1.0f64 - (dist as f64 - mid_range) / (range - mid_range)) as f32;
                            factor = factor.clamp(0.0, 1.0); // WWMath::Clamp(factor, 0.0f, 1.0f)
                            let mut shade: f32 = 0.5;
                            shade *= factor;
                            let l_diffuse = light.diffuse;
                            let l_ambient = light.ambient;
                            if shade > 1.0 {
                                shade = 1.0;
                            }
                            if shade < 0.0 {
                                shade = 0.0;
                            }
                            shade_r += shade * l_diffuse.x;
                            shade_g += shade * l_diffuse.y;
                            shade_b += shade * l_diffuse.z;
                            shade_r += factor * l_ambient.x;
                            shade_g += factor * l_ambient.y;
                            shade_b += factor * l_ambient.z;
                        }
                    }
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
                    shade_r *= 255.0;
                    shade_g *= 255.0;
                    shade_b *= 255.0;
                    diffuse = real_to_int(shade_b) | (real_to_int(shade_g) << 8) | (real_to_int(shade_r) << 16) | (255i32 << 24);

                    let nv = self.cur_num_road_vertices as usize;
                    vb[nv].u1 = info.u_offset + u / (info.scale * 4.0);
                    vb[nv].v1 = info.v_offset - v / (info.scale * 4.0); // Road is 1/16 texture height.
                    vb[nv].x = cur_column.vtx[j].x;
                    vb[nv].y = cur_column.vtx[j].y;
                    vb[nv].z = cur_column.vtx[j].z + float_amount;
                    vb[nv].diffuse = diffuse as u32;
                    cur_column.vertex_index[j] = self.cur_num_road_vertices;
                    self.cur_num_road_vertices += 1;
                    if j == 1 && cur_column.collapsed {
                        break;
                    }
                }
                if self.cur_num_road_vertices >= MAX_SEG_INDEX {
                    break;
                }
                if i > 1 && (!prev_column.collapsed || !cur_column.collapsed) {
                    // Write out the triangles.
                    let mut j: usize = 0;
                    let mut k: usize = 0;
                    while (j as i32) < v_count - 1 && (k as i32) < v_count - 1 {
                        if self.cur_num_road_indices >= self.max_road_index {
                            break;
                        }
                        let mut cur_ib = self.cur_num_road_indices as usize;
                        if k == 0 || !prev_column.collapsed {
                            ib[cur_ib] = prev_column.vertex_index[j + 1] as u16;
                            ib[cur_ib + 1] = prev_column.vertex_index[j] as u16;
                            ib[cur_ib + 2] = cur_column.vertex_index[k] as u16;
                            cur_ib += 3;
                            self.cur_num_road_indices += 3;
                        }
                        if j == 0 || !cur_column.collapsed {
                            let mut offset: usize = 1;
                            if cur_column.collapsed && !prev_column.collapsed {
                                offset = (v_count - 1) as usize;
                            }
                            ib[cur_ib] = prev_column.vertex_index[j + offset] as u16;
                            ib[cur_ib + 1] = cur_column.vertex_index[k] as u16;
                            ib[cur_ib + 2] = cur_column.vertex_index[k + 1] as u16;
                            self.cur_num_road_indices += 3;
                        }
                        if prev_column.collapsed && cur_column.collapsed {
                            break;
                        }
                        if !prev_column.collapsed {
                            j += 1;
                        }
                        if !cur_column.collapsed {
                            k += 1;
                        }
                    }
                    prev_column = cur_column;
                } else if i == 0 {
                    prev_column = cur_column;
                }
                if self.cur_num_road_indices >= MAX_SEG_INDEX {
                    break;
                }
            }
            cur_column = next_column;
            i += 1;
        }
    }

    /// `W3DRoadBuffer::loadCurve` (W3DRoadBuffer.cpp:1049): loads a curve segment into the vertex buffer for a road end
    /// cap or join.
    pub fn load_curve<H: Fn(f32, f32) -> f32, D: Fn(i32, i32) -> i32>(&mut self, p_road: usize, loc1: Vector2, loc2: Vector2, scale: f32, terrain: &RoadTerrainFns<H, D>) {
        let u_offset = 4.0f32 / 512.0;
        let mut v_offset = 255.0f32 / 512.0; // CORNER_RADIUS radius 1.5 curve.
        let curve_radius = self.roads[p_road].curve_radius;
        if curve_radius == TIGHT_CORNER_RADIUS {
            v_offset = 425.0f32 / 512.0;
        }

        let mut road_vector = Vector2::new(loc2.x - loc1.x, loc2.y - loc1.y);
        let mut road_normal = Vector2::new(-road_vector.y, road_vector.x);
        let road_len = scale;

        let curve_height = self.roads[p_road].width_in_texture * scale / 2.0;

        road_vector.normalize();
        road_vector *= road_len;

        road_normal.normalize();
        road_normal *= curve_height.abs();

        let mut corners = [Vector2::default(); NUM_CORNERS];

        corners[BOTTOM_LEFT] = loc1 - road_normal;
        corners[BOTTOM_RIGHT] = corners[BOTTOM_LEFT];
        corners[BOTTOM_RIGHT] += road_vector;
        corners[TOP_RIGHT] = corners[BOTTOM_RIGHT];
        corners[TOP_RIGHT] += 2.0 * road_normal;
        corners[TOP_LEFT] = corners[BOTTOM_LEFT];
        corners[TOP_LEFT] += 2.0 * road_normal;

        // Tweak a little
        if curve_radius == TIGHT_CORNER_RADIUS {
            corners[BOTTOM_LEFT] = loc1 - road_normal;
            corners[BOTTOM_RIGHT] = corners[BOTTOM_LEFT];
            corners[BOTTOM_RIGHT] += road_vector * 0.5;
            corners[TOP_RIGHT] = corners[BOTTOM_RIGHT];
            corners[TOP_RIGHT] += 2.0 * road_normal;
            corners[TOP_LEFT] = corners[BOTTOM_LEFT];
            corners[TOP_LEFT] += 2.0 * road_normal;

            corners[BOTTOM_RIGHT] += road_vector * 0.1;
            corners[BOTTOM_RIGHT] += road_normal * 0.2;
            corners[BOTTOM_LEFT] -= road_normal * 0.1;
            corners[BOTTOM_LEFT] -= road_vector * 0.02;
            corners[TOP_LEFT] -= road_vector * 0.02;

            corners[TOP_RIGHT] -= road_vector * 0.4;
            corners[TOP_RIGHT] += road_normal * 0.2;
        } else {
            corners[BOTTOM_LEFT] = loc1 - road_normal;
            corners[BOTTOM_RIGHT] = corners[BOTTOM_LEFT];
            corners[BOTTOM_RIGHT] += road_vector;
            corners[TOP_RIGHT] = corners[BOTTOM_RIGHT];
            corners[TOP_RIGHT] += 2.0 * road_normal;
            corners[TOP_LEFT] = corners[BOTTOM_LEFT];
            corners[TOP_LEFT] += 2.0 * road_normal;

            corners[BOTTOM_RIGHT] += road_vector * 0.1;
            corners[BOTTOM_RIGHT] += road_normal * 0.4;
            corners[BOTTOM_LEFT] -= road_normal * 0.2;
            corners[BOTTOM_LEFT] -= road_vector * 0.02;
            corners[TOP_LEFT] -= road_vector * 0.02;

            corners[TOP_RIGHT] -= road_vector * 0.4;
            corners[TOP_RIGHT] += road_normal * 0.4;
        }

        self.load_float_4pt_section(p_road, loc1, road_normal, road_vector, &corners, u_offset, v_offset, scale, scale, terrain);
    }

    /// `W3DRoadBuffer::preloadRoadSegment` (W3DRoadBuffer.cpp:1139): loads a road segment into the vertex buffer for
    /// drawing.
    pub fn preload_road_segment<H: Fn(f32, f32) -> f32, D: Fn(i32, i32) -> i32>(&mut self, p_road: usize, terrain: &RoadTerrainFns<H, D>) {
        let road = self.roads[p_road].clone();
        let road_vector = road.pt2.loc - road.pt1.loc;
        let mut road_normal = Vector2::new(-road_vector.y, road_vector.x);

        let u_offset = 0.0f32;
        let v_offset = 85.0f32 / 512.0;

        let road_height = road.width_in_texture * road.scale / 2.0;
        road_normal.normalize();
        road_normal *= road_height;

        let mut corners = [Vector2::default(); NUM_CORNERS];
        corners[BOTTOM_LEFT] = road.pt1.bottom;
        corners[TOP_LEFT] = road.pt1.top;
        corners[BOTTOM_RIGHT] = road.pt2.bottom;
        corners[TOP_RIGHT] = road.pt2.top;
        self.load_float_4pt_section(p_road, road.pt1.loc, road_normal, road_vector, &corners, u_offset, v_offset, road.scale, road.scale, terrain);
    }

    /// `W3DRoadBuffer::preloadRoadsInVertexAndIndexBuffers` (W3DRoadBuffer.cpp:1165): loads the roads into the vertex
    /// buffer for drawing.
    pub fn preload_roads_in_vertex_and_index_buffers<H: Fn(f32, f32) -> f32, D: Fn(i32, i32) -> i32>(&mut self, terrain: &RoadTerrainFns<H, D>) {
        if !self.initialized {
            return;
        }

        // Do road segments.
        for cur_road in 0..self.num_roads as usize {
            if self.roads[cur_road].type_ == TCorner::Segment {
                self.preload_road_segment(cur_road, terrain);
            }
        }
        // Do curves.
        for cur_road in 0..self.num_roads as usize {
            if self.roads[cur_road].type_ == TCorner::Curve {
                let (l1, l2, sc) = (self.roads[cur_road].pt1.loc, self.roads[cur_road].pt2.loc, self.roads[cur_road].scale);
                self.load_curve(cur_road, l1, l2, sc, terrain);
            }
        }
        // Do tees.
        for cur_road in 0..self.num_roads as usize {
            if self.roads[cur_road].type_ == TCorner::ThreeWayY {
                let (l1, l2, sc) = (self.roads[cur_road].pt1.loc, self.roads[cur_road].pt2.loc, self.roads[cur_road].scale);
                self.load_y(cur_road, l1, l2, sc, terrain);
            }
        }
        // Do tees.
        for cur_road in 0..self.num_roads as usize {
            let ty = self.roads[cur_road].type_;
            if ty == TCorner::ThreeWayH || ty == TCorner::ThreeWayHFlip {
                let (l1, l2, sc) = (self.roads[cur_road].pt1.loc, self.roads[cur_road].pt2.loc, self.roads[cur_road].scale);
                self.load_h(cur_road, l1, l2, ty == TCorner::ThreeWayHFlip, sc, terrain);
            }
        }
        // Do tees.
        for cur_road in 0..self.num_roads as usize {
            let ty = self.roads[cur_road].type_;
            if ty == TCorner::Tee || ty == TCorner::FourWay {
                let (l1, l2, sc) = (self.roads[cur_road].pt1.loc, self.roads[cur_road].pt2.loc, self.roads[cur_road].scale);
                self.load_tee(cur_road, l1, l2, ty == TCorner::FourWay, sc, terrain);
            }
        }
        // Do joins.
        for cur_road in 0..self.num_roads as usize {
            if self.roads[cur_road].type_ == TCorner::AlphaJoin {
                let (l1, l2, sc) = (self.roads[cur_road].pt1.loc, self.roads[cur_road].pt2.loc, self.roads[cur_road].scale);
                self.load_alpha_join(cur_road, l1, l2, sc, terrain);
            }
        }
        self.update_lighting(&terrain.static_diffuse);
    }

    /// `W3DRoadBuffer::loadRoadsInVertexAndIndexBuffers` (W3DRoadBuffer.cpp:1232): loads the roads of the current road
    /// type into its vertex buffer for drawing.
    pub fn load_roads_in_vertex_and_index_buffers(&mut self) {
        if !self.initialized {
            return;
        }
        self.cur_num_road_vertices = 0;
        self.cur_num_road_indices = 0;
        let cur_type = self.cur_road_type as usize;
        // Lock the buffers.
        if self.road_types[cur_type].index_road.is_none() {
            self.road_types[cur_type].set_num_vertices(0);
            self.road_types[cur_type].set_num_indices(0);
            return;
        }
        let mut vb = self.road_types[cur_type].vertex_road.take().unwrap_or_default();
        let mut ib = self.road_types[cur_type].index_road.take().unwrap_or_default();
        // Add to the index buffer & vertex buffer.

        // Do road segments.
        let mut corner = 0;
        while corner < TCorner::NumJoins as i32 {
            for cur_road in 0..self.num_roads as usize {
                if self.roads[cur_road].type_ as i32 == corner {
                    self.load_road_segment(&mut ib, &mut vb, cur_road);
                }
            }
            corner += 1;
        }
        self.road_types[cur_type].vertex_road = Some(vb);
        self.road_types[cur_type].index_road = Some(ib);
        self.road_types[cur_type].set_num_vertices(self.cur_num_road_vertices);
        self.road_types[cur_type].set_num_indices(self.cur_num_road_indices);
    }

    /// `W3DRoadBuffer::visibilityChanged` (W3DRoadBuffer.cpp:1281): returns true if any road segment's visibility
    /// changed. `bounds` is an `IRegion2D` as `(lo.x, lo.y, hi.x, hi.y)`.
    pub fn visibility_changed(&mut self, bounds: (i32, i32, i32, i32)) -> bool {
        if !self.initialized {
            return false;
        }
        let (lo_x, lo_y, hi_x, hi_y) = (bounds.0 as f32, bounds.1 as f32, bounds.2 as f32, bounds.3 as f32);
        let mut vis_changed = false;
        // Do road segments.
        for cur_road in 0..self.num_roads as usize {
            let r = &mut self.roads[cur_road];
            let cur_vis = r.visible;
            let mut new_vis = true;
            let half_scale = r.scale / 2.0;
            // Throw out segs out of view.
            if r.pt1.loc.x + half_scale < lo_x && r.pt2.loc.x + half_scale < lo_x {
                new_vis = false;
            }
            if r.pt1.loc.x - half_scale > hi_x && r.pt2.loc.x - half_scale > hi_x {
                new_vis = false;
            }
            // Throw out segs out of view.
            if r.pt1.loc.y + half_scale < lo_y && r.pt2.loc.y + half_scale < lo_y {
                new_vis = false;
            }
            if r.pt1.loc.y - half_scale > hi_y && r.pt2.loc.y - half_scale > hi_y {
                new_vis = false;
            }
            if new_vis != cur_vis {
                vis_changed = true;
            }
            r.visible = new_vis;
        }
        vis_changed
    }

    /// `W3DRoadBuffer::loadLitRoadsInVertexAndIndexBuffers` (W3DRoadBuffer.cpp:1328): loads the roads lit by the dynamic
    /// lights into the vertex buffer for drawing.
    pub fn load_lit_roads_in_vertex_and_index_buffers<H: Fn(f32, f32) -> f32, D: Fn(i32, i32) -> i32>(&mut self, dynamic_lights: &[RoadLight], terrain: &RoadTerrainFns<H, D>) {
        if !self.initialized {
            return;
        }
        self.cur_num_road_vertices = 0;
        self.cur_num_road_indices = 0;
        let cur_type = self.cur_road_type as usize;
        // Lock the buffers.
        let mut vb = self.road_types[cur_type].vertex_road.take().unwrap_or_default();
        let mut ib = self.road_types[cur_type].index_road.take().unwrap_or_default();
        // Add to the index buffer & vertex buffer.

        // Do road segments.
        let mut corner = 0;
        while corner < TCorner::NumJoins as i32 {
            for cur_road in 0..self.num_roads as usize {
                if self.roads[cur_road].type_ as i32 == corner {
                    self.load_lit_4pt_section(cur_road, &mut ib, &mut vb, dynamic_lights, terrain);
                }
            }
            corner += 1;
        }
        self.road_types[cur_type].vertex_road = Some(vb);
        self.road_types[cur_type].index_road = Some(ib);
        self.road_types[cur_type].set_num_vertices(self.cur_num_road_vertices);
        self.road_types[cur_type].set_num_indices(self.cur_num_road_indices);
    }

    /// `W3DRoadBuffer::loadRoadSegment` (W3DRoadBuffer.cpp:1378): loads a road segment into the vertex buffer for drawing.
    pub fn load_road_segment(&mut self, ib: &mut [u16], vb: &mut [VertexFormatXYZDUV1], p_road: usize) {
        if self.roads[p_road].unique_id != self.cur_unique_id {
            return;
        }
        if !self.roads[p_road].visible {
            return;
        }
        let cur_vertex = self.cur_num_road_vertices;
        if cur_vertex + self.roads[p_road].get_num_vertex() >= self.max_road_vertex {
            return;
        }
        if self.cur_num_road_indices + self.roads[p_road].get_num_index() >= self.max_road_index {
            return;
        }
        let nv = self.roads[p_road].get_num_vertex();
        self.cur_num_road_vertices += self.roads[p_road].get_vertices(&mut vb[cur_vertex as usize..], nv);
        let ni = self.roads[p_road].get_num_index();
        self.cur_num_road_indices += self.roads[p_road].get_indices(&mut ib[self.cur_num_road_indices as usize..], ni, cur_vertex);
    }

    /// `W3DRoadBuffer::flipTheRoad` (W3DRoadBuffer.h): flips the loc1 and loc2 info.
    pub fn flip_the_road(&mut self, p_road: usize) {
        let tmp = self.roads[p_road].pt1;
        self.roads[p_road].pt1 = self.roads[p_road].pt2;
        self.roads[p_road].pt2 = tmp;
    }

    /// `W3DRoadBuffer::moveRoadSegTo` (W3DRoadBuffer.cpp:1399): moves a road segment.
    pub fn move_road_seg_to(&mut self, from_ndx: i32, to_ndx: i32) {
        if from_ndx < 0 || from_ndx >= self.num_roads || to_ndx < 0 || to_ndx >= self.num_roads {
            // upstream (_DEBUG only): DEBUG_LOG("bad moveRoadSegTo")
            return;
        }
        if from_ndx == to_ndx {
            return;
        }
        let cur = self.roads[from_ndx as usize].clone();
        if from_ndx < to_ndx {
            for i in from_ndx..to_ndx {
                self.roads[i as usize] = self.roads[(i + 1) as usize].clone();
            }
        } else {
            let mut i = from_ndx;
            while i > to_ndx {
                self.roads[i as usize] = self.roads[(i - 1) as usize].clone();
                i -= 1;
            }
        }
        self.roads[to_ndx as usize] = cur;
    }

    /// `W3DRoadBuffer::checkLinkBefore` (W3DRoadBuffer.cpp:1429): checks to see if any segments need to link before this
    /// one.
    pub fn check_link_before(&mut self, ndx: i32) {
        // Check for things whose loc == our loc2.
        if self.roads[ndx as usize].pt2.count != 1 {
            return;
        }

        let mut loc2 = self.roads[ndx as usize].pt2.loc;

        let mut end_of_cur_seg = ndx + 1;
        while end_of_cur_seg < self.num_roads - 1 {
            if self.roads[end_of_cur_seg as usize].pt1.loc != self.roads[(end_of_cur_seg + 1) as usize].pt2.loc {
                break;
            }
            end_of_cur_seg += 1;
        }
        let mut check_ndx = end_of_cur_seg + 1;
        while check_ndx < self.num_roads {
            if self.roads[check_ndx as usize].pt1.loc == loc2 {
                self.move_road_seg_to(check_ndx, ndx);
                loc2 = self.roads[ndx as usize].pt2.loc;
                if self.roads[ndx as usize].pt2.count != 1 {
                    return;
                }
                end_of_cur_seg += 1;
            } else if self.roads[check_ndx as usize].pt2.loc == loc2 {
                self.flip_the_road(check_ndx as usize);
                self.move_road_seg_to(check_ndx, ndx);
                loc2 = self.roads[ndx as usize].pt2.loc;
                if self.roads[ndx as usize].pt2.count != 1 {
                    return;
                }
                end_of_cur_seg += 1;
            } else {
                check_ndx += 1;
            }
            if check_ndx <= end_of_cur_seg {
                check_ndx = end_of_cur_seg + 1;
            }
        }
    }

    /// `W3DRoadBuffer::checkLinkAfter` (W3DRoadBuffer.cpp:1488): checks to see if any segments need to link after this
    /// one.
    pub fn check_link_after(&mut self, mut ndx: i32) {
        // Check for things whose loc == our loc1.
        if self.roads[ndx as usize].pt1.count != 1 {
            return;
        }
        if ndx >= self.num_roads - 1 {
            return;
        }

        let mut loc1 = self.roads[ndx as usize].pt1.loc;

        let mut check_ndx = ndx + 1;
        while check_ndx < self.num_roads && ndx < self.num_roads - 1 {
            if self.roads[check_ndx as usize].pt2.loc == loc1 {
                ndx += 1;
                self.move_road_seg_to(check_ndx, ndx);
                loc1 = self.roads[ndx as usize].pt1.loc;
                if self.roads[ndx as usize].pt1.count != 1 {
                    return;
                }
            } else if self.roads[check_ndx as usize].pt1.loc == loc1 {
                self.flip_the_road(check_ndx as usize);
                ndx += 1;
                self.move_road_seg_to(check_ndx, ndx);
                loc1 = self.roads[ndx as usize].pt1.loc;
                if self.roads[ndx as usize].pt1.count != 1 {
                    return;
                }
            } else {
                check_ndx += 1;
            }
        }
    }
}

/// A `TRoadPt *` into `m_roads`: the road's index and which of its two points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PtRef {
    /// Index into `m_roads`.
    pub road: usize,
    /// False for `m_pt1`, true for `m_pt2`.
    pub second: bool,
}

/// A `MapObject` as `addMapObjects` reads it (`getName`, `getLocation`, `getFlag`).
#[derive(Clone, Debug, Default)]
pub struct RoadMapObject {
    /// `getName()`.
    pub name: Vec<u8>,
    /// `getLocation()->x`.
    pub x: f32,
    /// `getLocation()->y`.
    pub y: f32,
    /// `getFlag(FLAG_ROAD_POINT1)`.
    pub road_point1: bool,
    /// `getFlag(FLAG_ROAD_POINT2)`.
    pub road_point2: bool,
    /// `getFlag(FLAG_ROAD_CORNER_ANGLED)`.
    pub corner_angled: bool,
    /// `getFlag(FLAG_ROAD_JOIN)`.
    pub join: bool,
    /// `getFlag(FLAG_ROAD_CORNER_TIGHT)`.
    pub corner_tight: bool,
}

impl W3DRoadBuffer {
    fn pt(&self, p: PtRef) -> TRoadPt {
        if p.second {
            self.roads[p.road].pt2
        } else {
            self.roads[p.road].pt1
        }
    }

    fn set_pt(&mut self, p: PtRef, v: TRoadPt) {
        if p.second {
            self.roads[p.road].pt2 = v;
        } else {
            self.roads[p.road].pt1 = v;
        }
    }

    /// `CHECK_SEGMENTS` (W3DRoadBuffer.cpp:1521, a `#define`): false (after warning once, `warnSegments`) when
    /// `m_numRoads >= m_maxRoadSegments`, in which case the caller returns.
    fn check_segments(&mut self) -> bool {
        if self.num_roads >= self.max_road_segments {
            if self.warn_segments {
                // upstream: DEBUG_LOG("****** Too many road segments.  Need to increase ini values.  See john a.")
            }
            self.warn_segments = false;
            return false;
        }
        true
    }

    /// `W3DRoadBuffer::addMapObject` (W3DRoadBuffer.cpp:1527): adds a road segment, linking it to the others.
    pub fn add_map_object(&mut self, p_road: &RoadSegment, update_the_counts: bool) {
        let mut cur = p_road.clone();
        let loc1 = cur.pt1.loc;
        let loc2 = cur.pt2.loc;
        let road_vector = Vector2::new(loc2.x - loc1.x, loc2.y - loc1.y);
        let mut road_normal = Vector2::new(-road_vector.y, road_vector.x);
        road_normal.normalize();
        road_normal *= cur.scale * cur.width_in_texture / 2.0;
        cur.pt1.top = loc1 + road_normal;
        cur.pt1.bottom = loc1 - road_normal;
        cur.pt2.top = loc2 + road_normal;
        cur.pt2.bottom = loc2 - road_normal;

        if update_the_counts {
            self.update_counts(&mut cur);
        }

        if !self.check_segments() {
            return;
        }
        let mut flip = false;
        let mut add_before = false;
        let mut add_after = false;
        let mut both_match = false;
        let mut i: i32 = 0;
        while i < self.num_roads {
            both_match = false;
            let r = &self.roads[i as usize];
            if (r.pt1.loc == loc1 && r.pt2.loc == loc2) || (r.pt1.loc == loc2 && r.pt2.loc == loc1) {
                both_match = true; // identical segment, so discard.
                break;
            }
            if cur.pt1.count == 1 {
                if r.pt1.loc == loc1 {
                    flip = true;
                    add_after = true;
                }
                if r.pt2.loc == loc1 {
                    flip = false;
                    add_before = true;
                }
            }
            if cur.pt2.count == 1 {
                if r.pt1.loc == loc2 {
                    flip = false;
                    add_after = true;
                }
                if r.pt2.loc == loc2 {
                    flip = true;
                    add_before = true;
                }
            }
            if add_before || add_after {
                break;
            }
            i += 1;
        }
        if both_match {
            return;
        }
        let mut add_index = i;
        if add_after {
            add_index += 1;
        }
        if add_index < self.num_roads {
            let mut k = self.num_roads;
            while k > add_index {
                self.roads[k as usize] = self.roads[(k - 1) as usize].clone();
                k -= 1;
            }
        }

        self.roads[add_index as usize] = cur;
        if flip {
            self.flip_the_road(add_index as usize);
        }
        self.num_roads += 1;
        if add_before {
            self.check_link_before(add_index);
        } else if add_after {
            self.check_link_after(add_index);
        }
    }

    /// `W3DRoadBuffer::addMapObjects` (W3DRoadBuffer.cpp:1612): loads the roads from the map objects. `map_objects` is
    /// `MapObject::getFirstMapObject()` and its `getNext()` chain; `roads` is `TheTerrainRoads`.
    pub fn add_map_objects(&mut self, map_objects: &[RoadMapObject], roads: &rts_lockstep_engine::common::terrain_roads::TerrainRoadCollection) {
        let mut idx = 0usize;
        while idx < map_objects.len() {
            let p_map_obj = &map_objects[idx];
            if self.num_roads >= self.max_road_segments {
                break;
            }
            if p_map_obj.road_point1 {
                let p_map_obj2 = match map_objects.get(idx + 1) {
                    Some(o) => o,
                    None => break,
                };
                if !p_map_obj2.road_point2 {
                    idx += 1;
                    continue;
                }
                let mut loc1 = Vector2::default();
                let mut loc2 = Vector2::default();
                loc1.set(p_map_obj.x, p_map_obj.y);
                loc2.set(p_map_obj2.x, p_map_obj2.y);
                if loc1.x == loc2.x && loc1.y == loc2.y {
                    loc2.x += 0.25;
                }
                let mut cur_road = RoadSegment::new();
                cur_road.scale = DEFAULT_ROAD_SCALE;
                cur_road.width_in_texture = 1.0;
                cur_road.unique_id = 1;
                let mut found = false;
                if let Some(road) = roads.find_road(&p_map_obj.name) {
                    cur_road.width_in_texture = road.road_width_in_texture;
                    cur_road.scale = road.road_width;
                    cur_road.unique_id = road.id as i32;
                    found = true;
                }
                // LOAD_TEST_ASSETS
                let default_scale: f32 = 30.0;
                if !found {
                    for i in 0..self.max_road_types as usize {
                        if p_map_obj.name == self.road_types[i].get_path() {
                            cur_road.scale = default_scale;
                            cur_road.unique_id = self.road_types[i].get_unique_id();
                            found = true;
                        }
                    }
                }
                if !found && self.cur_open_road < self.max_road_types {
                    self.max_uid += 1;
                    cur_road.scale = default_scale;
                    cur_road.unique_id = self.max_uid;
                    // upstream: m_roadTypes[m_curOpenRoad].loadTexture(pMapObj->getName(), m_maxUID) — the texture is
                    // fetched by the renderer (`W3DAssetManager::Get_Texture`); the type's id and path are set here
                    let (mv, mi, uid) = (self.max_road_vertex, self.max_road_index, self.max_uid);
                    self.road_types[self.cur_open_road as usize].load_texture(&p_map_obj.name, uid, None, mv, mi);
                    self.cur_open_road += 1;
                }
                cur_road.pt1.loc = loc1;
                cur_road.pt1.is_angled = p_map_obj.corner_angled;
                cur_road.pt1.is_join = p_map_obj.join;
                cur_road.pt2.loc = loc2;
                cur_road.pt2.is_angled = p_map_obj2.corner_angled;
                cur_road.pt2.is_join = p_map_obj2.join;
                cur_road.type_ = TCorner::Segment;
                cur_road.curve_radius = if p_map_obj.corner_tight { TIGHT_CORNER_RADIUS } else { CORNER_RADIUS };

                self.add_map_object(&cur_road, true);
                idx += 1; // pMapObj = pMapObj2
            }
            idx += 1;
        }
        let cur_count = self.num_roads;
        self.num_roads = 0;
        for i in 0..cur_count as usize {
            let cur_road = self.roads[i].clone();
            self.add_map_object(&cur_road, false);
        }
    }

    /// `W3DRoadBuffer::updateCounts` (W3DRoadBuffer.cpp:1700): updates the count and last fields.
    pub fn update_counts(&mut self, p_road: &mut RoadSegment) {
        p_road.pt1.last = true;
        p_road.pt2.last = true;
        p_road.pt1.multi = false;
        p_road.pt2.multi = false;
        p_road.pt1.count = 0;
        p_road.pt2.count = 0;
        let loc1 = p_road.pt1.loc;
        let loc2 = p_road.pt2.loc;
        for i in 0..self.num_roads as usize {
            // Only connect same type of road.
            if self.roads[i].unique_id != p_road.unique_id {
                continue;
            }
            if self.roads[i].pt1.loc == loc1 {
                self.roads[i].pt1.count += 1;
                p_road.pt1.count += 1;
            }
            if self.roads[i].pt1.loc == loc2 {
                self.roads[i].pt1.count += 1;
                p_road.pt2.count += 1;
            }
            if self.roads[i].pt2.loc == loc1 {
                self.roads[i].pt2.count += 1;
                p_road.pt1.count += 1;
            }
            if self.roads[i].pt2.loc == loc2 {
                self.roads[i].pt2.count += 1;
                p_road.pt2.count += 1;
            }
            self.roads[i].pt1.multi = self.roads[i].pt1.count > 1;
            self.roads[i].pt2.multi = self.roads[i].pt2.count > 1;
        }
        p_road.pt1.multi = p_road.pt1.count > 1;
        p_road.pt2.multi = p_road.pt2.count > 1;
    }

    /// `W3DRoadBuffer::updateCountsAndFlags` (W3DRoadBuffer.cpp:1745): updates the count and last fields.
    pub fn update_counts_and_flags(&mut self) {
        for i in 0..self.num_roads as usize {
            self.roads[i].pt1.last = true;
            self.roads[i].pt2.last = true;
            self.roads[i].pt1.count = 0;
            self.roads[i].pt2.count = 0;
        }
        let mut j = self.num_roads - 1;
        while j > 0 {
            let ju = j as usize;
            let loc1 = self.roads[ju].pt1.loc;
            let loc2 = self.roads[ju].pt2.loc;
            for i in 0..ju {
                if self.roads[i].unique_id != self.roads[ju].unique_id {
                    continue;
                }
                if self.roads[i].pt1.loc == loc1 {
                    self.roads[i].pt1.last = false;
                    self.roads[i].pt1.count += 1;
                    self.roads[ju].pt1.count += 1;
                }
                if self.roads[i].pt1.loc == loc2 {
                    self.roads[i].pt1.last = false;
                    self.roads[i].pt1.count += 1;
                    self.roads[ju].pt2.count += 1;
                }
                if self.roads[i].pt2.loc == loc1 {
                    self.roads[i].pt2.last = false;
                    self.roads[i].pt2.count += 1;
                    self.roads[ju].pt1.count += 1;
                }
                if self.roads[i].pt2.loc == loc2 {
                    self.roads[i].pt2.last = false;
                    self.roads[i].pt2.count += 1;
                    self.roads[ju].pt2.count += 1;
                }
            }
            j -= 1;
        }
    }

    /// `W3DRoadBuffer::insertTee` (W3DRoadBuffer.cpp:1788): inserts a Tee intersection.
    pub fn insert_tee(&mut self, loc: Vector2, index1: i32, scale: f32) {
        if self.insert_y(loc, index1, scale) {
            return;
        }
        let index1u = index1 as usize;

        // pr1-3 point to the points on the segments that form the tee. They are the points on the segments that are != loc.
        let pr1: PtRef;
        let mut pr2: Option<PtRef> = None;
        let mut pr3: Option<PtRef> = None;

        // pc1-3 point to the center points of the segments. These are the points that are at loc.
        let pc1: PtRef;
        let mut pc2: Option<PtRef> = None;
        let mut pc3: Option<PtRef> = None;

        if self.roads[index1u].pt1.loc == loc {
            pr1 = PtRef { road: index1u, second: true };
            pc1 = PtRef { road: index1u, second: false };
        } else {
            pr1 = PtRef { road: index1u, second: false };
            pc1 = PtRef { road: index1u, second: true };
        }
        for i in (index1 + 1)..self.num_roads {
            let iu = i as usize;
            if self.roads[iu].pt1.loc == loc {
                self.roads[iu].pt1.count = -2;
                if pr2.is_none() {
                    pr2 = Some(PtRef { road: iu, second: true });
                    pc2 = Some(PtRef { road: iu, second: false });
                } else {
                    pr3 = Some(PtRef { road: iu, second: true });
                    pc3 = Some(PtRef { road: iu, second: false });
                }
            }
            if self.roads[iu].pt2.loc == loc {
                self.roads[iu].pt2.count = -2;
                if pr2.is_none() {
                    pr2 = Some(PtRef { road: iu, second: false });
                    pc2 = Some(PtRef { road: iu, second: true });
                } else {
                    pr3 = Some(PtRef { road: iu, second: false });
                    pc3 = Some(PtRef { road: iu, second: true });
                }
            }
        }
        let (pr2, pr3) = match (pr2, pr3) {
            (Some(a), Some(b)) => (a, b),
            _ => return,
        };
        let (pc2, pc3) = (pc2.unwrap(), pc3.unwrap());

        let mut v1 = self.pt(pr1).loc - loc;
        v1.normalize();
        let mut v2 = self.pt(pr2).loc - loc;
        v2.normalize();
        let mut v3 = self.pt(pr3).loc - loc;
        v3.normalize();
        let dot12 = Vector2::dot_product(&v1, &v2);
        let dot13 = Vector2::dot_product(&v1, &v3);
        let dot32 = Vector2::dot_product(&v3, &v2);
        // The greatest negative dot product is the pair that is heading most opposite each other.
        let mut do12 = false;
        let mut do13 = false;
        let mut do32 = false;

        if dot12 < dot13 {
            if dot12 < dot32 {
                do12 = true;
            } else {
                do32 = true;
            }
        } else if dot13 < dot32 {
            do13 = true;
        } else {
            do32 = true;
        }

        let mut up_vector = Vector2::default();
        let mut decider = Vector2::default();
        if do12 {
            up_vector = v2 - v1;
            decider = v3;
        }
        if do13 {
            up_vector = v3 - v1;
            decider = v2;
        }
        if do32 {
            up_vector = v2 - v3;
            decider = v1;
        }
        up_vector.normalize();

        // Check to see if the Tee side is slanted.
        let cos60: f32 = 0.5;
        let dot = Vector2::dot_product(&up_vector, &decider).abs();
        let width_in_texture = self.roads[index1u].width_in_texture;
        if dot > cos60 {
            // The arm of the tee is slanted, so do a slant tee.
            let mut angle = PI / 2.0; // 90 degrees.
            let xpdct = Vector3::cross_product_z(&Vector3::new(up_vector.x, up_vector.y, 0.0), &Vector3::new(decider.x, decider.y, 0.0));
            let mut mirror = false;
            if xpdct < 0.0 {
                angle = -angle;
                mirror = true;
            }
            up_vector.normalize();
            up_vector *= (0.5f64 * scale as f64) as f32; // we are offseting one half road width.
            let mut tee_vector = up_vector;
            tee_vector.rotate(angle);

            let mut flip = false;
            if do12 {
                flip = xp_sign(&tee_vector, &v3) == 1;
                self.offset_h(pc1, pc2, pc3, loc, up_vector, tee_vector, flip, mirror, width_in_texture);
            }
            if do13 {
                flip = xp_sign(&tee_vector, &v2) == 1;
                self.offset_h(pc1, pc3, pc2, loc, up_vector, tee_vector, flip, mirror, width_in_texture);
            }
            if do32 {
                flip = xp_sign(&tee_vector, &v1) == 1;
                self.offset_h(pc3, pc2, pc1, loc, up_vector, tee_vector, flip, mirror, width_in_texture);
            }

            for p in [pc1, pc2, pc3] {
                let mut v = self.pt(p);
                v.last = true;
                v.count = 0;
                self.set_pt(p, v);
            }

            if !self.check_segments() {
                return;
            }
            let n = self.num_roads as usize;
            let (r_scale, r_width, r_uid) = (self.roads[index1u].scale, self.roads[index1u].width_in_texture, self.roads[index1u].unique_id);
            self.roads[n].pt1.loc.set(loc.x, loc.y);
            self.roads[n].pt2.loc.set((loc + tee_vector).x, (loc + tee_vector).y);
            self.roads[n].pt1.last = true; // if not, that one will clear flag in prior loop.
            self.roads[n].pt2.last = true; // if not, that one will clear flag in prior loop.
            self.roads[n].scale = r_scale;
            self.roads[n].width_in_texture = r_width;
            self.roads[n].pt1.count = -3;
            self.roads[n].type_ = if flip { TCorner::ThreeWayHFlip } else { TCorner::ThreeWayH };
            self.roads[n].unique_id = r_uid;
            self.num_roads += 1;
        } else {
            // Do a not slanted tee.
            let mut angle = PI / 2.0; // 90 degrees.
            let xpdct = Vector3::cross_product_z(&Vector3::new(up_vector.x, up_vector.y, 0.0), &Vector3::new(decider.x, decider.y, 0.0));
            if xpdct < 0.0 {
                angle = -angle;
            }
            up_vector.normalize();
            up_vector *= (0.5f64 * scale as f64) as f32; // we are offseting one half road width.
            let mut tee_vector = up_vector;
            tee_vector.rotate(angle);

            if do12 {
                self.offset_3way(pc1, pc2, pc3, loc, up_vector, tee_vector, width_in_texture);
            }
            if do13 {
                self.offset_3way(pc1, pc3, pc2, loc, up_vector, tee_vector, width_in_texture);
            }
            if do32 {
                self.offset_3way(pc3, pc2, pc1, loc, up_vector, tee_vector, width_in_texture);
            }
            for p in [pc1, pc2, pc3] {
                let mut v = self.pt(p);
                v.last = true;
                v.count = 0;
                self.set_pt(p, v);
            }

            if !self.check_segments() {
                return;
            }
            let n = self.num_roads as usize;
            let (r_scale, r_width, r_uid) = (self.roads[index1u].scale, self.roads[index1u].width_in_texture, self.roads[index1u].unique_id);
            self.roads[n].pt1.loc.set(loc.x, loc.y);
            self.roads[n].pt2.loc.set((loc + tee_vector).x, (loc + tee_vector).y);
            self.roads[n].pt1.last = true; // if not, that one will clear flag in prior loop.
            self.roads[n].pt2.last = true; // if not, that one will clear flag in prior loop.
            self.roads[n].scale = r_scale;
            self.roads[n].width_in_texture = r_width;
            self.roads[n].pt1.count = -3;
            self.roads[n].type_ = TCorner::Tee;
            self.roads[n].unique_id = r_uid;
            self.num_roads += 1;
        }
    }

    /// `W3DRoadBuffer::insertY` (W3DRoadBuffer.cpp:1990): inserts a Y intersection if the corner meets "Y" criteria.
    pub fn insert_y(&mut self, loc: Vector2, index1: i32, scale: f32) -> bool {
        let index1u = index1 as usize;
        let pr1: PtRef;
        let mut pr2: Option<PtRef> = None;
        let mut pr3: Option<PtRef> = None;
        let pc1: PtRef;
        let mut pc2: Option<PtRef> = None;
        let mut pc3: Option<PtRef> = None;

        if self.roads[index1u].pt1.loc == loc {
            pr1 = PtRef { road: index1u, second: true };
            pc1 = PtRef { road: index1u, second: false };
        } else {
            pr1 = PtRef { road: index1u, second: false };
            pc1 = PtRef { road: index1u, second: true };
        }
        for i in (index1 + 1)..self.num_roads {
            let iu = i as usize;
            if self.roads[iu].pt1.loc == loc {
                self.roads[iu].pt1.count = -2;
                if pr2.is_none() {
                    pr2 = Some(PtRef { road: iu, second: true });
                    pc2 = Some(PtRef { road: iu, second: false });
                } else {
                    pr3 = Some(PtRef { road: iu, second: true });
                    pc3 = Some(PtRef { road: iu, second: false });
                }
            }
            if self.roads[iu].pt2.loc == loc {
                self.roads[iu].pt2.count = -2;
                if pr2.is_none() {
                    pr2 = Some(PtRef { road: iu, second: false });
                    pc2 = Some(PtRef { road: iu, second: true });
                } else {
                    pr3 = Some(PtRef { road: iu, second: false });
                    pc3 = Some(PtRef { road: iu, second: true });
                }
            }
        }
        let (pr2, pr3) = match (pr2, pr3) {
            (Some(a), Some(b)) => (a, b),
            _ => return false,
        };
        let (pc2, pc3) = (pc2.unwrap(), pc3.unwrap());

        let mut v1 = self.pt(pr1).loc - loc;
        v1.normalize();
        let mut v2 = self.pt(pr2).loc - loc;
        v2.normalize();
        let mut v3 = self.pt(pr3).loc - loc;
        v3.normalize();

        let mut do12 = false;
        let mut do13 = false;
        let mut do32 = false;

        let dot12 = Vector2::dot_product(&v1, &v2);
        let dot13 = Vector2::dot_product(&v1, &v3);
        let dot32 = Vector2::dot_product(&v3, &v2);
        let mut score12: f32 = 2.0;
        let mut score13: f32 = 2.0;
        let mut score32: f32 = 2.0;

        let cos30: f32 = 0.866;
        let cos45: f32 = 0.707;

        if dot12 < (-cos30) {
            return false; // Too close to a straigh line, do a straight side tee.
        }
        if dot13 < (-cos30) {
            return false; // Too close to a straigh line, do a straight side tee.
        }
        if dot32 < (-cos30) {
            return false; // Too close to a straigh line, to a straight side tee.
        }

        let mut s1: i32;
        let mut s2 = xp_sign(&v1, &v2);
        let mut s3 = xp_sign(&v1, &v3);

        if s2 != s3 && (s2 + s3 == 0) {
            let v1_90 = Vector2::new(-v1.y, v1.x);
            if xp_sign(&v1_90, &v2) == 1 && xp_sign(&v1_90, &v3) == 1 {
                // s2 & s3 could be Y legs.
                do32 = true;
                score32 = (dot12 + cos45).abs() + (dot13 + cos45).abs();
            }
        }

        s1 = xp_sign(&v3, &v1);
        s2 = xp_sign(&v3, &v2);
        if s2 != s1 && (s2 + s1 == 0) {
            // s2 & s3 could be Y legs.
            let v3_90 = Vector2::new(-v3.y, v3.x);
            if xp_sign(&v3_90, &v2) == 1 && xp_sign(&v3_90, &v1) == 1 {
                do12 = true;
                score12 = (dot13 + cos45).abs() + (dot32 + cos45).abs();
            }
        }

        s1 = xp_sign(&v2, &v1);
        s3 = xp_sign(&v2, &v3);
        if s3 != s1 && (s3 + s1 == 0) {
            // s2 & s3 could be Y legs.
            let v2_90 = Vector2::new(-v2.y, v2.x);
            if xp_sign(&v2_90, &v3) == 1 && xp_sign(&v2_90, &v1) == 1 {
                do13 = true;
                score13 = (dot12 + cos45).abs() + (dot32 + cos45).abs();
            }
        }

        if score12 < score13 {
            do13 = false;
            if score12 < score32 {
                do32 = false;
            } else {
                do12 = false;
            }
        } else {
            do12 = false;
            if score13 < score32 {
                do32 = false;
            } else {
                do13 = false;
            }
        }

        let mut up_vector: Vector2;
        if do12 {
            up_vector = v3;
        } else if do13 {
            up_vector = v2;
        } else if do32 {
            up_vector = v1;
        } else {
            return false;
        }

        let angle = -(PI / 2.0); // -90 degrees.
        up_vector.normalize();
        up_vector *= (0.5f64 * scale as f64) as f32; // we are offseting one half road width.
        let mut tee_vector = up_vector;
        tee_vector.rotate(angle);

        let width_in_texture = self.roads[index1u].width_in_texture;
        // Offset the ends of the road to meet the stumps of the y.
        if do12 {
            if xp_sign(&v3, &v1) == -1 {
                self.offset_y(pc1, pc2, pc3, loc, up_vector, width_in_texture);
            } else {
                self.offset_y(pc2, pc1, pc3, loc, up_vector, width_in_texture);
            }
        }
        if do13 {
            if xp_sign(&v2, &v1) == -1 {
                self.offset_y(pc1, pc3, pc2, loc, up_vector, width_in_texture);
            } else {
                self.offset_y(pc3, pc1, pc2, loc, up_vector, width_in_texture);
            }
        }
        if do32 {
            if xp_sign(&v1, &v3) == -1 {
                self.offset_y(pc3, pc2, pc1, loc, up_vector, width_in_texture);
            } else {
                self.offset_y(pc2, pc3, pc1, loc, up_vector, width_in_texture);
            }
        }

        for p in [pc1, pc2, pc3] {
            let mut v = self.pt(p);
            v.last = true;
            v.count = 0;
            self.set_pt(p, v);
        }

        if self.num_roads >= self.max_road_segments {
            return false;
        }
        let n = self.num_roads as usize;
        let (r_scale, r_width, r_uid) = (self.roads[index1u].scale, self.roads[index1u].width_in_texture, self.roads[index1u].unique_id);
        self.roads[n].pt1.loc.set(loc.x, loc.y);
        self.roads[n].pt2.loc.set((loc + tee_vector).x, (loc + tee_vector).y);
        self.roads[n].pt1.last = true; // if not, that one will clear flag in prior loop.
        self.roads[n].pt2.last = true; // if not, that one will clear flag in prior loop.
        self.roads[n].scale = r_scale;
        self.roads[n].width_in_texture = r_width;
        self.roads[n].pt1.count = -3;
        self.roads[n].type_ = TCorner::ThreeWayY;
        self.roads[n].unique_id = r_uid;
        self.num_roads += 1;

        true
    }

    /// `W3DRoadBuffer::offset3Way` (W3DRoadBuffer.cpp:2183): offsets the points coming into a 3 way intersection so that
    /// they move to the join points of the 3 way intersection.
    pub fn offset_3way(&mut self, pc1: PtRef, pc2: PtRef, pc3: PtRef, loc: Vector2, up_vector: Vector2, tee_vector: Vector2, width_in_texture: f32) {
        let mut up_vector = up_vector;
        let mut c1 = self.pt(pc1);
        let mut c2 = self.pt(pc2);
        let mut c3 = self.pt(pc3);
        c1.loc = loc - up_vector;
        c2.loc = loc + up_vector;
        c3.loc = loc + tee_vector;

        // Adjust the top & bottoms, so they merge smoothly into the tee.

        // Make sure the t vector goes right with respect to the up vector.
        let mut xpdct = Vector3::cross_product_z(&Vector3::new(up_vector.x, up_vector.y, 0.0), &Vector3::new(tee_vector.x, tee_vector.y, 0.0));
        let mut right_tee = tee_vector;
        if xpdct < 0.0 {
            right_tee.x = -tee_vector.x;
            right_tee.y = -tee_vector.y;
        }
        right_tee *= width_in_texture;

        xpdct = Vector3::cross_product_z(&Vector3::new(up_vector.x, up_vector.y, 0.0), &Vector3::new(c1.top.x - c1.loc.x, c1.top.y - c1.loc.y, 0.0));
        if xpdct > 0.0 {
            c1.bottom = c1.loc - right_tee;
            c1.top = c1.loc + right_tee;
        } else {
            c1.bottom = c1.loc + right_tee;
            c1.top = c1.loc - right_tee;
        }
        xpdct = Vector3::cross_product_z(&Vector3::new(up_vector.x, up_vector.y, 0.0), &Vector3::new(c2.top.x - c2.loc.x, c2.top.y - c2.loc.y, 0.0));
        if xpdct > 0.0 {
            c2.bottom = c2.loc - right_tee;
            c2.top = c2.loc + right_tee;
        } else {
            c2.bottom = c2.loc + right_tee;
            c2.top = c2.loc - right_tee;
        }
        up_vector *= width_in_texture;
        xpdct = Vector3::cross_product_z(&Vector3::new(right_tee.x, right_tee.y, 0.0), &Vector3::new(c3.top.x - c3.loc.x, c3.top.y - c3.loc.y, 0.0));
        if xpdct < 0.0 {
            c3.bottom = c3.loc - up_vector;
            c3.top = c3.loc + up_vector;
        } else {
            c3.bottom = c3.loc + up_vector;
            c3.top = c3.loc - up_vector;
        }
        self.set_pt(pc1, c1);
        self.set_pt(pc2, c2);
        self.set_pt(pc3, c3);
    }

    /// `W3DRoadBuffer::offsetH` (W3DRoadBuffer.cpp:2234): offsets the points coming into a 3 way intersection so that
    /// they move to the join points of the 3 way intersection.
    pub fn offset_h(&mut self, pc1: PtRef, pc2: PtRef, pc3: PtRef, loc: Vector2, up_vector: Vector2, tee_vector: Vector2, flip: bool, mirror: bool, width_in_texture: f32) {
        let mut c1 = self.pt(pc1);
        let mut c2 = self.pt(pc2);
        let mut c3 = self.pt(pc3);
        if flip != mirror {
            c1.loc = loc - up_vector * 2.05;
            c2.loc = loc + up_vector * 0.46;
        } else {
            c1.loc = loc - up_vector * 0.46;
            c2.loc = loc + up_vector * 2.05;
        }
        // pc3->loc = loc + teeVector;

        // Adjust the top & bottoms, so they merge smoothly into the tee.

        // Make sure the t vector goes right with respect to the up vector.
        let mut xpdct = Vector3::cross_product_z(&Vector3::new(up_vector.x, up_vector.y, 0.0), &Vector3::new(tee_vector.x, tee_vector.y, 0.0));
        let mut right_tee = tee_vector;
        if xpdct < 0.0 {
            right_tee.x = -tee_vector.x;
            right_tee.y = -tee_vector.y;
        }
        right_tee *= width_in_texture;

        xpdct = Vector3::cross_product_z(&Vector3::new(up_vector.x, up_vector.y, 0.0), &Vector3::new(c1.top.x - c1.loc.x, c1.top.y - c1.loc.y, 0.0));
        if xpdct > 0.0 {
            c1.bottom = c1.loc - right_tee;
            c1.top = c1.loc + right_tee;
        } else {
            c1.bottom = c1.loc + right_tee;
            c1.top = c1.loc - right_tee;
        }
        xpdct = Vector3::cross_product_z(&Vector3::new(up_vector.x, up_vector.y, 0.0), &Vector3::new(c2.top.x - c2.loc.x, c2.top.y - c2.loc.y, 0.0));
        if xpdct > 0.0 {
            c2.bottom = c2.loc - right_tee;
            c2.top = c2.loc + right_tee;
        } else {
            c2.bottom = c2.loc + right_tee;
            c2.top = c2.loc - right_tee;
        }

        let mut arm = tee_vector;
        if flip {
            arm.rotate(PI / 4.0);
        } else {
            arm.rotate(-PI / 4.0);
        }
        let mut arm_normal = Vector2::new(-arm.y, arm.x);
        arm_normal *= width_in_texture;

        c3.loc += arm * 2.10;
        let xpdct_i = xp_sign(&arm, &(c3.top - loc));
        if xpdct_i == 1 {
            c3.top = c3.loc + arm_normal;
            c3.bottom = c3.loc - arm_normal;
        } else {
            c3.top = c3.loc - arm_normal;
            c3.bottom = c3.loc + arm_normal;
        }
        self.set_pt(pc1, c1);
        self.set_pt(pc2, c2);
        self.set_pt(pc3, c3);
    }

    /// `W3DRoadBuffer::offsetY` (W3DRoadBuffer.cpp:2295): offsets the points coming into a 3 way intersection so that
    /// they move to the join points of the 3 way intersection.
    pub fn offset_y(&mut self, pc1: PtRef, pc2: PtRef, pc3: PtRef, loc: Vector2, up_vector: Vector2, width_in_texture: f32) {
        let mut c1 = self.pt(pc1);
        let mut c2 = self.pt(pc2);
        let mut c3 = self.pt(pc3);
        c3.loc += up_vector * 0.55;
        c3.top += up_vector * 0.55;
        c3.bottom += up_vector * 0.55;

        // Adjust the bottom, so they merge smoothly into the tee.

        // Make sure the t vector goes right with respect to the up vector.
        let mut arm = up_vector;
        arm.rotate(3.0 * PI / 4.0);
        let mut arm_normal = Vector2::new(-arm.y, arm.x);
        arm_normal *= width_in_texture;
        c2.loc += arm * 1.1;
        let mut xpdct = xp_sign(&arm, &(c2.top - loc));
        if xpdct == 1 {
            c2.top = c2.loc + arm_normal;
            c2.bottom = c2.loc - arm_normal;
        } else {
            c2.top = c2.loc - arm_normal;
            c2.bottom = c2.loc + arm_normal;
        }

        arm = up_vector;
        arm.rotate(-3.0 * PI / 4.0);
        arm_normal.set(-arm.y, arm.x);
        arm_normal *= width_in_texture;

        c1.loc += arm * 1.1;
        xpdct = xp_sign(&arm, &(c1.top - loc));
        if xpdct == 1 {
            c1.top = c1.loc + arm_normal;
            c1.bottom = c1.loc - arm_normal;
        } else {
            c1.top = c1.loc - arm_normal;
            c1.bottom = c1.loc + arm_normal;
        }
        self.set_pt(pc1, c1);
        self.set_pt(pc2, c2);
        self.set_pt(pc3, c3);
    }

    /// `W3DRoadBuffer::offset4Way` (W3DRoadBuffer.cpp:2342): offsets the points coming into a 4 way intersection so that
    /// they move to the join points of the 4 way intersection. `pr3` is only read (upstream reads it after writing
    /// `pc1` and `pc2`; it is a different point).
    pub fn offset_4way(&mut self, pc1: PtRef, pc2: PtRef, pc3: PtRef, pr3: PtRef, pc4: PtRef, loc: Vector2, align_vector: Vector2, width_in_texture: f32) {
        let mut align_vector = align_vector;
        let mut c1 = self.pt(pc1);
        let mut c2 = self.pt(pc2);
        let mut c3 = self.pt(pc3);
        let mut c4 = self.pt(pc4);
        c1.loc = loc - align_vector;
        c2.loc = loc + align_vector;

        let v3 = self.pt(pr3).loc - loc;
        let mut angle = PI / 2.0; // 90 degrees.
        let mut xpdct = Vector3::cross_product_z(&Vector3::new(align_vector.x, align_vector.y, 0.0), &Vector3::new(v3.x, v3.y, 0.0));
        if xpdct < 0.0 {
            angle = -angle;
        }
        let mut tee_vector = align_vector;
        tee_vector.rotate(angle);
        c3.loc = loc + tee_vector;
        c4.loc = loc - tee_vector;

        let mut real_tee = align_vector;
        real_tee.rotate(PI / 2.0);
        real_tee *= width_in_texture;

        // Fix up the top & bottom points.
        let align3 = Vector3::new(align_vector.x, align_vector.y, 0.0);

        xpdct = Vector3::cross_product_z(&align3, &Vector3::new(c1.top.x - c1.loc.x, c1.top.y - c1.loc.y, 0.0));
        if xpdct > 0.0 {
            c1.bottom = c1.loc - real_tee;
            c1.top = c1.loc + real_tee;
        } else {
            c1.bottom = c1.loc + real_tee;
            c1.top = c1.loc - real_tee;
        }
        xpdct = Vector3::cross_product_z(&align3, &Vector3::new(c2.top.x - c2.loc.x, c2.top.y - c2.loc.y, 0.0));
        if xpdct > 0.0 {
            c2.bottom = c2.loc - real_tee;
            c2.top = c2.loc + real_tee;
        } else {
            c2.bottom = c2.loc + real_tee;
            c2.top = c2.loc - real_tee;
        }
        align_vector *= width_in_texture;

        let tee3 = Vector3::new(real_tee.x, real_tee.y, 0.0);
        xpdct = Vector3::cross_product_z(&tee3, &Vector3::new(c3.top.x - c3.loc.x, c3.top.y - c3.loc.y, 0.0));
        if xpdct < 0.0 {
            c3.bottom = c3.loc - align_vector;
            c3.top = c3.loc + align_vector;
        } else {
            c3.bottom = c3.loc + align_vector;
            c3.top = c3.loc - align_vector;
        }
        xpdct = Vector3::cross_product_z(&tee3, &Vector3::new(c4.top.x - c4.loc.x, c4.top.y - c4.loc.y, 0.0));
        if xpdct < 0.0 {
            c4.bottom = c4.loc - align_vector;
            c4.top = c4.loc + align_vector;
        } else {
            c4.bottom = c4.loc + align_vector;
            c4.top = c4.loc - align_vector;
        }

        c1.last = true;
        c1.count = 0;
        c2.last = true;
        c2.count = 0;
        c3.last = true;
        c3.count = 0;
        c4.last = true;
        c4.count = 0;
        self.set_pt(pc1, c1);
        self.set_pt(pc2, c2);
        self.set_pt(pc3, c3);
        self.set_pt(pc4, c4);
    }
}

/// What `drawRoads` sets for a road type: its texture and the shader passes it draws the whole index range with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoadTypeDraw {
    /// Index into `m_roadTypes`.
    pub road_type: usize,
    /// `applyTexture()`: the road texture (texture 0 is NULL in wireframe).
    pub texture: Option<TextureHandle>,
    /// `Draw_Triangles(0, getNumIndices()/3, 0, getNumVertices())`, repeated once per device pass.
    pub draw: super::w3d_common::DrawTriangles,
}

/// `W3DShaderManager::ShaderTypes` values `drawRoads` chooses between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoadShaderType {
    /// `ST_ROAD_BASE`.
    RoadBase,
    /// `ST_ROAD_BASE_NOISE1`.
    RoadBaseNoise1,
    /// `ST_ROAD_BASE_NOISE2`.
    RoadBaseNoise2,
    /// `ST_ROAD_BASE_NOISE12`.
    RoadBaseNoise12,
}

impl W3DRoadBuffer {
    /// `W3DRoadBuffer::insert4Way` (W3DRoadBuffer.cpp:2422): inserts a 4 way intersection.
    pub fn insert_4way(&mut self, loc: Vector2, index1: i32, scale: f32) {
        let index1u = index1 as usize;
        let pr1: PtRef;
        let mut pr2: Option<PtRef> = None;
        let mut pr3: Option<PtRef> = None;
        let mut pr4: Option<PtRef> = None;
        let pc1: PtRef;
        let mut pc2: Option<PtRef> = None;
        let mut pc3: Option<PtRef> = None;
        let mut pc4: Option<PtRef> = None;

        if self.roads[index1u].pt1.loc == loc {
            pr1 = PtRef { road: index1u, second: true };
            pc1 = PtRef { road: index1u, second: false };
        } else {
            pr1 = PtRef { road: index1u, second: false };
            pc1 = PtRef { road: index1u, second: true };
        }
        for i in (index1 + 1)..self.num_roads {
            let iu = i as usize;
            if self.roads[iu].pt1.loc == loc {
                self.roads[iu].pt1.count = -2;
                if pr2.is_none() {
                    pr2 = Some(PtRef { road: iu, second: true });
                    pc2 = Some(PtRef { road: iu, second: false });
                } else if pr3.is_none() {
                    pr3 = Some(PtRef { road: iu, second: true });
                    pc3 = Some(PtRef { road: iu, second: false });
                } else {
                    pr4 = Some(PtRef { road: iu, second: true });
                    pc4 = Some(PtRef { road: iu, second: false });
                }
            }
            if self.roads[iu].pt2.loc == loc {
                self.roads[iu].pt2.count = -2;
                if pr2.is_none() {
                    pr2 = Some(PtRef { road: iu, second: false });
                    pc2 = Some(PtRef { road: iu, second: true });
                } else if pr3.is_none() {
                    pr3 = Some(PtRef { road: iu, second: false });
                    pc3 = Some(PtRef { road: iu, second: true });
                } else {
                    pr4 = Some(PtRef { road: iu, second: false });
                    pc4 = Some(PtRef { road: iu, second: true });
                }
            }
        }
        let (pr2, pr3, pr4) = match (pr2, pr3, pr4) {
            (Some(a), Some(b), Some(c)) => (a, b, c),
            _ => return,
        };
        let (pc2, pc3, pc4) = (pc2.unwrap(), pc3.unwrap(), pc4.unwrap());

        let mut v1 = self.pt(pr1).loc - loc;
        v1.normalize();
        let mut v2 = self.pt(pr2).loc - loc;
        v2.normalize();
        let mut v3 = self.pt(pr3).loc - loc;
        v3.normalize();
        let mut v4 = self.pt(pr4).loc - loc;
        v4.normalize();

        let dot12 = Vector2::dot_product(&v1, &v2);
        let dot13 = Vector2::dot_product(&v1, &v3);
        let dot14 = Vector2::dot_product(&v1, &v4);
        let dot23 = Vector2::dot_product(&v2, &v3);
        let dot24 = Vector2::dot_product(&v2, &v4);
        let dot34 = Vector2::dot_product(&v3, &v4);
        // The most negative dot product is the pair that is heading most opposite each other.

        let mut cur_pair = 12;
        let mut cur_dot = dot12;
        if dot13 < cur_dot {
            cur_pair = 13;
            cur_dot = dot13;
        }
        if dot14 < cur_dot {
            cur_pair = 14;
            cur_dot = dot14;
        }
        if dot23 < cur_dot {
            cur_pair = 23;
            cur_dot = dot23;
        }
        if dot24 < cur_dot {
            cur_pair = 24;
            cur_dot = dot24;
        }
        if dot34 < cur_dot {
            cur_pair = 34;
        }
        let do12 = cur_pair == 12; // Do segment 1 to 2
        let do13 = cur_pair == 13; // Do segment 1 to 3 etc.
        let do14 = cur_pair == 14;
        let do23 = cur_pair == 23;
        let do24 = cur_pair == 24;
        let do34 = cur_pair == 34;

        let mut align_vector = Vector2::default();
        if do12 {
            align_vector = v2 - v1;
        }
        if do13 {
            align_vector = v3 - v1;
        }
        if do14 {
            align_vector = v4 - v1;
        }
        if do23 {
            align_vector = v3 - v2;
        }
        if do24 {
            align_vector = v4 - v2;
        }
        if do34 {
            align_vector = v4 - v3;
        }
        align_vector.normalize();
        align_vector *= (0.5f64 * scale as f64) as f32; // we are offseting one half road width.

        let w = self.roads[index1u].width_in_texture;
        if do12 {
            self.offset_4way(pc1, pc2, pc3, pr3, pc4, loc, align_vector, w);
        }
        if do13 {
            self.offset_4way(pc1, pc3, pc2, pr2, pc4, loc, align_vector, w);
        }
        if do14 {
            self.offset_4way(pc1, pc4, pc3, pr3, pc2, loc, align_vector, w);
        }
        if do23 {
            self.offset_4way(pc2, pc3, pc1, pr1, pc4, loc, align_vector, w);
        }
        if do24 {
            self.offset_4way(pc2, pc4, pc1, pr1, pc3, loc, align_vector, w);
        }
        if do34 {
            self.offset_4way(pc3, pc4, pc1, pr1, pc2, loc, align_vector, w);
        }
        if align_vector.x < 0.0 {
            // Since the 4 way intersection is symmetrical, make it go right (pos x)
            align_vector.x = -align_vector.x;
            align_vector.y = -align_vector.y;
        }
        if !self.check_segments() {
            return;
        }
        let n = self.num_roads as usize;
        let (r_scale, r_uid) = (self.roads[index1u].scale, self.roads[index1u].unique_id);
        self.roads[n].pt1.loc.set(loc.x, loc.y);
        self.roads[n].pt2.loc.set((loc + align_vector).x, (loc + align_vector).y);
        self.roads[n].pt1.last = true; // if not, that one will clear flag in prior loop.
        self.roads[n].pt2.last = true; // if not, that one will clear flag in prior loop.
        self.roads[n].scale = r_scale;
        self.roads[n].width_in_texture = TEE_WIDTH_ADJUSTMENT; // It is symmetrical.
        self.roads[n].pt1.count = -4;
        self.roads[n].type_ = TCorner::FourWay;
        self.roads[n].unique_id = r_uid;
        self.num_roads += 1;
    }

    /// `W3DRoadBuffer::insertTeeIntersections` (W3DRoadBuffer.cpp:2568): inserts Tee intersections at 3 way
    /// intersections.
    pub fn insert_tee_intersections(&mut self) {
        // Insert the tees.
        let num_road_segments = self.num_roads;
        for i in 0..num_road_segments {
            let iu = i as usize;
            if self.roads[iu].type_ != TCorner::Segment {
                continue;
            }
            if self.roads[iu].pt1.count == 2 {
                let (l, s) = (self.roads[iu].pt1.loc, self.roads[iu].scale);
                self.insert_tee(l, i, s);
            }
            if self.roads[iu].pt2.count == 2 {
                let (l, s) = (self.roads[iu].pt2.loc, self.roads[iu].scale);
                self.insert_tee(l, i, s);
            }
            if self.roads[iu].pt1.count == 3 {
                let (l, s) = (self.roads[iu].pt1.loc, self.roads[iu].scale);
                self.insert_4way(l, i, s);
            }
            if self.roads[iu].pt2.count == 3 {
                let (l, s) = (self.roads[iu].pt2.loc, self.roads[iu].scale);
                self.insert_4way(l, i, s);
            }
        }
    }

    /// `W3DRoadBuffer::insertCurveSegments` (W3DRoadBuffer.cpp:2596): inserts curved segments along connected segments.
    pub fn insert_curve_segments(&mut self) {
        // Insert the curve segments.
        let num_road_segments = self.num_roads;
        let mut segment_start_index: i32 = -1;
        for i in 0..num_road_segments {
            let iu = i as usize;
            if i < num_road_segments - 1 && self.roads[iu].pt1.loc == self.roads[iu + 1].pt2.loc {
                if self.roads[iu + 1].pt2.count == 1 && self.roads[iu].pt1.count == 1 {
                    self.insert_curve_segment_at(i, i + 1);
                    if segment_start_index < 0 {
                        segment_start_index = i;
                    }
                }
            } else if segment_start_index >= 0 {
                if self.roads[iu].pt1.loc == self.roads[segment_start_index as usize].pt2.loc {
                    if self.roads[segment_start_index as usize].pt2.count == 1 && self.roads[iu].pt1.count == 1 {
                        self.insert_curve_segment_at(i, segment_start_index);
                    }
                }
                segment_start_index = -1;
            }
        }
    }

    /// `W3DRoadBuffer::findCrossTypeJoinVector` (W3DRoadBuffer.cpp:2619): finds a road segment of different type and sets
    /// the join vector. Returns the other road's unique id, 0 when none.
    pub fn find_cross_type_join_vector(&self, loc: Vector2, join_vector: &mut Vector2, unique_id: i32) -> i32 {
        // Insert the curve segments.
        let num_road_segments = self.num_roads;
        for i in 0..num_road_segments as usize {
            // we only want different road types.
            if self.roads[i].unique_id == unique_id {
                continue;
            }
            // Only join to straight line segments.
            if self.roads[i].type_ != TCorner::Segment {
                continue;
            }
            let loc1 = self.roads[i].pt1.loc;
            let loc2 = self.roads[i].pt2.loc;
            let mut lo = Vector2::new(loc1.x, loc1.y);
            let mut hi = lo;
            if lo.x > loc2.x {
                lo.x = loc2.x;
            }
            if lo.y > loc2.y {
                lo.y = loc2.y;
            }
            if hi.x < loc2.x {
                hi.x = loc2.x;
            }
            if hi.y < loc2.y {
                hi.y = loc2.y;
            }
            lo.x -= self.roads[i].scale / 2.0;
            lo.y -= self.roads[i].scale / 2.0;
            hi.x += self.roads[i].scale / 2.0;
            hi.y += self.roads[i].scale / 2.0;
            if loc.x >= lo.x && loc.y >= lo.y && loc.x <= hi.x && loc.y <= hi.y {
                let v1 = Vector3::new(loc1.x, loc1.y, 0.0);
                let v2 = Vector3::new(loc2.x, loc2.y, 0.0);
                let road_line = LineSegClass::new(v1, v2);
                let v_loc = Vector3::new(loc.x, loc.y, 0.0);
                let pt_on_line = road_line.find_point_closest_to(&v_loc);
                let dist = Vector3::distance(&pt_on_line, &v_loc);
                if dist < self.roads[i].scale * 0.55 {
                    let mut road_vec = loc2 - loc1;
                    if xp_sign(&road_vec, join_vector) == 1 {
                        road_vec.rotate(PI / 2.0);
                    } else {
                        road_vec.rotate(-PI / 2.0);
                    }
                    *join_vector = road_vec;
                    return self.roads[i].unique_id;
                }
            }
        }
        0
    }

    /// `W3DRoadBuffer::adjustStacking` (W3DRoadBuffer.cpp:2680): adjusts the stacking order.
    pub fn adjust_stacking(&mut self, top_unique_id: i32, bottom_unique_id: i32) {
        let mut i = 0;
        while i < self.max_road_types {
            if self.road_types[i as usize].get_unique_id() == top_unique_id {
                break;
            }
            i += 1;
        }
        // upstream: DEBUG_ASSERTLOG(i < m_maxRoadTypes, "***** Wrong unique id- john a should fix.")
        if i >= self.max_road_types {
            return;
        }

        let mut j = 0;
        while j < self.max_road_types {
            if self.road_types[j as usize].get_unique_id() == bottom_unique_id {
                break;
            }
            j += 1;
        }
        if j >= self.max_road_types {
            return;
        }

        if self.road_types[i as usize].get_stacking() > self.road_types[j as usize].get_stacking() {
            return; // It's already on top.
        }
        let new_stacking = self.road_types[j as usize].get_stacking();
        for j in 0..self.max_road_types as usize {
            if self.road_types[j].get_stacking() > new_stacking {
                let s = self.road_types[j].get_stacking();
                self.road_types[j].set_stacking(s + 1);
            }
        }
        self.road_types[i as usize].set_stacking(new_stacking + 1);
    }

    /// `W3DRoadBuffer::insertCrossTypeJoins` (W3DRoadBuffer.cpp:2713): inserts alpha blend type joins at open ends.
    pub fn insert_cross_type_joins(&mut self) {
        // Insert the curve segments.
        let num_road_segments = self.num_roads;
        for i in 0..num_road_segments as usize {
            let loc1: Vector2;
            let loc2: Vector2;
            let mut is_pt1 = false;
            if self.roads[i].pt2.count == 0 && self.roads[i].pt2.is_join {
                loc1 = self.roads[i].pt2.loc;
                loc2 = self.roads[i].pt1.loc;
            } else if self.roads[i].pt1.count == 0 && self.roads[i].pt1.is_join {
                loc1 = self.roads[i].pt1.loc;
                loc2 = self.roads[i].pt2.loc;
                is_pt1 = true;
            } else {
                continue;
            }
            let mut road_vector = Vector2::new(loc2.x - loc1.x, loc2.y - loc1.y);
            road_vector.normalize();
            let mut join_vector = road_vector;
            let other_id = self.find_cross_type_join_vector(loc1, &mut join_vector, self.roads[i].unique_id);
            if other_id == 0 {
                join_vector *= 100.0;
            }
            let road_normal = Vector2::new(-road_vector.y, road_vector.x);
            let join_normal = Vector2::new(-join_vector.y, join_vector.x);

            let (r_scale, r_wit) = (self.roads[i].scale, self.roads[i].width_in_texture);
            let mut p1 = loc1 + road_normal * r_scale * r_wit / 2.0;
            let mut p2 = loc2 + road_normal * r_scale * r_wit / 2.0;

            let mut v1 = Vector3::new(p1.x, p1.y, 0.0);
            let mut v2 = Vector3::new(p2.x, p2.y, 0.0);
            let mut road_line = LineSegClass::new(v1, v2);
            let v_loc1 = Vector3::new(loc1.x, loc1.y, 0.0);
            v1 = Vector3::new(join_normal.x, join_normal.y, 0.0) + v_loc1;
            let join_line = LineSegClass::new(v_loc1, v1);

            let mut top = self.roads[i].pt1.top;
            if let Some((p_int1, _, _, _)) = join_line.find_intersection(&road_line) {
                if is_pt1 {
                    self.roads[i].pt1.top.set(p_int1.x, p_int1.y);
                    top = self.roads[i].pt1.top;
                } else {
                    self.roads[i].pt2.bottom.set(p_int1.x, p_int1.y);
                    top = self.roads[i].pt2.bottom;
                }
            }
            p1 = loc1 - road_normal * r_scale * r_wit / 2.0;
            p2 = loc2 - road_normal * r_scale * r_wit / 2.0;
            v1.set(p1.x, p1.y, 0.0);
            v2.set(p2.x, p2.y, 0.0);
            road_line.set(v1, v2);
            let mut bottom = self.roads[i].pt1.bottom;
            if let Some((p_int1, _, _, _)) = join_line.find_intersection(&road_line) {
                if is_pt1 {
                    self.roads[i].pt1.bottom.set(p_int1.x, p_int1.y);
                    bottom = self.roads[i].pt1.bottom;
                } else {
                    self.roads[i].pt2.top.set(p_int1.x, p_int1.y);
                    bottom = self.roads[i].pt2.top;
                }
            }

            bottom = bottom - top;
            let scale_adjustment = bottom.length() / (r_scale * r_wit);
            if other_id != 0 {
                let uid = self.roads[i].unique_id;
                self.adjust_stacking(uid, other_id);
            }
            if !self.check_segments() {
                return;
            }
            let n = self.num_roads as usize;
            self.roads[n].pt1.loc.set(loc1.x, loc1.y);
            self.roads[n].pt2.loc.set((loc1 + join_vector).x, (loc1 + join_vector).y);
            self.roads[n].pt1.last = true; // if not, that one will clear flag in prior loop.
            self.roads[n].pt2.last = true; // if not, that one will clear flag in prior loop.
            self.roads[n].scale = r_scale;
            self.roads[n].width_in_texture = r_scale * scale_adjustment;
            self.roads[n].pt1.count = 0;
            self.roads[n].type_ = TCorner::AlphaJoin;
            self.roads[n].unique_id = self.roads[i].unique_id;
            self.num_roads += 1;
        }
    }

    /// `W3DRoadBuffer::miter` (W3DRoadBuffer.cpp:2826): adjusts the end points to create a smooth miter join between road
    /// segments.
    pub fn miter(&mut self, ndx1: i32, ndx2: i32) {
        let (n1, n2) = (ndx1 as usize, ndx2 as usize);
        // adjust a mitered join.  jba.
        let mut p1 = Vector3::new(self.roads[n1].pt1.top.x, self.roads[n1].pt1.top.y, 0.0);
        let mut p2 = Vector3::new(self.roads[n1].pt2.top.x, self.roads[n1].pt2.top.y, 0.0);
        let mut offset_line1 = LineSegClass::new(p1, p2);
        p1 = Vector3::new(self.roads[n2].pt1.top.x, self.roads[n2].pt1.top.y, 0.0);
        p2 = Vector3::new(self.roads[n2].pt2.top.x, self.roads[n2].pt2.top.y, 0.0);
        let mut offset_line2 = LineSegClass::new(p1, p2);
        if let Some((p_int1, _, _, _)) = offset_line1.find_intersection(&offset_line2) {
            self.roads[n2].pt2.top.x = p_int1.x;
            self.roads[n2].pt2.top.y = p_int1.y;
            self.roads[n1].pt1.top.x = p_int1.x;
            self.roads[n1].pt1.top.y = p_int1.y;
        }
        p1 = Vector3::new(self.roads[n1].pt1.bottom.x, self.roads[n1].pt1.bottom.y, 0.0);
        p2 = Vector3::new(self.roads[n1].pt2.bottom.x, self.roads[n1].pt2.bottom.y, 0.0);
        offset_line1 = LineSegClass::new(p1, p2);
        p1 = Vector3::new(self.roads[n2].pt1.bottom.x, self.roads[n2].pt1.bottom.y, 0.0);
        p2 = Vector3::new(self.roads[n2].pt2.bottom.x, self.roads[n2].pt2.bottom.y, 0.0);
        offset_line2 = LineSegClass::new(p1, p2);
        if let Some((p_int1, _, _, _)) = offset_line1.find_intersection(&offset_line2) {
            self.roads[n2].pt2.bottom.x = p_int1.x;
            self.roads[n2].pt2.bottom.y = p_int1.y;
            self.roads[n1].pt1.bottom.x = p_int1.x;
            self.roads[n1].pt1.bottom.y = p_int1.y;
        }
    }

    fn loc_of(&self, p: PtRef) -> Vector2 {
        self.pt(p).loc
    }

    fn set_loc_of(&mut self, p: PtRef, v: Vector2) {
        let mut pt = self.pt(p);
        pt.loc = v;
        self.set_pt(p, pt);
    }

    /// `W3DRoadBuffer::insertCurveSegmentAt` (W3DRoadBuffer.cpp:2870): inserts curves at the corner of 2 segments.
    pub fn insert_curve_segment_at(&mut self, ndx1: i32, ndx2: i32) {
        let dot_limit: f32 = 0.5; // If the dot product of the new line is less than this, abort.
        let (n1, n2) = (ndx1 as usize, ndx2 as usize);
        let radius = self.roads[n1].curve_radius * self.roads[n1].scale;
        let original_pt = self.roads[n1].pt1.loc;
        // we got a segment.
        let mut line1 = LineSegClass::new(
            Vector3::new(self.roads[n1].pt1.loc.x, self.roads[n1].pt1.loc.y, 0.0),
            Vector3::new(self.roads[n1].pt2.loc.x, self.roads[n1].pt2.loc.y, 0.0),
        );
        let mut line2 = LineSegClass::new(
            Vector3::new(self.roads[n2].pt1.loc.x, self.roads[n2].pt1.loc.y, 0.0),
            Vector3::new(self.roads[n2].pt2.loc.x, self.roads[n2].pt2.loc.y, 0.0),
        );
        let pr1: PtRef;
        let pr2: PtRef;
        let pr3: PtRef;
        let pr4: PtRef;

        if self.roads[n1].unique_id != self.roads[n2].unique_id {
            return;
        }
        let cur_sin = Vector3::dot_product(&line1.get_dir(), &line2.get_dir());
        let xpdct = Vector3::cross_product_z(&line1.get_dir(), &line2.get_dir());
        if xpdct > 0.0 {
            pr1 = PtRef { road: n1, second: false };
            pr2 = PtRef { road: n1, second: true };
            pr3 = PtRef { road: n2, second: false };
            pr4 = PtRef { road: n2, second: true };
        } else {
            pr4 = PtRef { road: n1, second: false };
            pr3 = PtRef { road: n1, second: true };
            pr2 = PtRef { road: n2, second: false };
            pr1 = PtRef { road: n2, second: true };
            let (a, b, c, d) = (self.loc_of(pr1), self.loc_of(pr2), self.loc_of(pr3), self.loc_of(pr4));
            line1.set(Vector3::new(a.x, a.y, 0.0), Vector3::new(b.x, b.y, 0.0));
            line2.set(Vector3::new(c.x, c.y, 0.0), Vector3::new(d.x, d.y, 0.0));
        }
        let angle = rts_lockstep_engine::ww::wwmath::acos(cur_sin);
        let count = angle / (PI / 6.0); // number of 30 degree steps.
        if (count as f64) < 0.9 || self.roads[n1].pt1.is_angled {
            self.miter(ndx1, ndx2);
            return;
        }

        let mut offset1 = radius * line1.get_dir();
        let mut offset2 = radius * line2.get_dir();
        offset1.rotate_z(-PI / 2.0);
        offset2.rotate_z(-PI / 2.0);

        let (l1, l2, l3, l4) = (self.loc_of(pr1), self.loc_of(pr2), self.loc_of(pr3), self.loc_of(pr4));
        let mut p1 = Vector3::new(l1.x, l1.y, 0.0) + offset1;
        let mut p2 = Vector3::new(l2.x, l2.y, 0.0) + offset1;
        let offset_line1 = LineSegClass::new(p1, p2);
        p1 = Vector3::new(l3.x, l3.y, 0.0) + offset2;
        p2 = Vector3::new(l4.x, l4.y, 0.0) + offset2;
        let offset_line2 = LineSegClass::new(p1, p2);
        let mut p_int1: Vector3;
        let mut p_int3 = Vector3::default();
        if let Some((a, _, _, _)) = offset_line1.find_intersection(&offset_line2) {
            p_int1 = a;
            self.roads[n2].pt2.last = true;
            let cross1 = LineSegClass::new(p_int1, p_int1 - offset2);
            let cross2 = LineSegClass::new(p_int1, p_int1 - offset1);
            if let Some((a, _, _, _)) = cross1.find_intersection(&line2) {
                p_int1 = a;
            }
            if let Some((a, _, _, _)) = cross2.find_intersection(&line1) {
                p_int3 = a;
            }
            // Make sure the lines didn't clip out of existence.
            let mut the_dot = Vector3::dot_product(&line2.get_dir(), &(p_int1 - Vector3::new(l3.x, l3.y, 0.0)));
            if the_dot < dot_limit {
                self.set_loc_of(pr1, original_pt);
                self.set_loc_of(pr4, original_pt);
                self.miter(ndx1, ndx2);
                return;
            }
            the_dot = Vector3::dot_product(&line1.get_dir(), &(Vector3::new(l2.x, l2.y, 0.0) - p_int3));
            if the_dot < dot_limit {
                self.set_loc_of(pr1, original_pt);
                self.set_loc_of(pr4, original_pt);
                self.miter(ndx1, ndx2);
                return;
            }
            self.set_loc_of(pr4, Vector2::new(p_int1.x, p_int1.y));
            let angle = -PI / 6.0; // -30 degrees.
            let mut pt2 = self.loc_of(pr4);
            let mut pt1 = self.loc_of(pr3);
            let mut direction = Vector2::new(pt1.x - pt2.x, pt1.y - pt2.y);
            // offset = normal of the vector from pt1 to pt2.
            let mut center_of_curve = Vector2::new(-direction.y, direction.x);
            center_of_curve.normalize();
            center_of_curve *= self.roads[n1].curve_radius * self.roads[n1].scale;
            center_of_curve += pt2;

            Self::rotate_about(&mut pt2, center_of_curve, angle);
            direction.rotate(angle);
            pt1 = pt2 + direction;

            // upstream writes m_roads[m_numRoads] before CHECK_SEGMENTS (past the end when full); `roads` has one spare entry
            let nr = self.num_roads as usize;
            self.roads[nr].pt1.loc = pt2;
            self.roads[nr].pt2.loc = pt1;

            if !self.check_segments() {
                return;
            }
            let (c_scale, c_wit, c_rad, c_uid) = (self.roads[n1].scale, self.roads[n1].width_in_texture, self.roads[n1].curve_radius, self.roads[n1].unique_id);
            self.roads[nr].pt1.last = true; // if not, that one will clear flag in prior loop.
            self.roads[nr].pt2.last = true; // if not, that one will clear flag in prior loop.
            self.roads[nr].scale = c_scale;
            self.roads[nr].width_in_texture = c_wit;
            self.roads[nr].type_ = TCorner::Curve;
            self.roads[nr].curve_radius = c_rad;
            self.roads[nr].unique_id = c_uid;
            self.num_roads += 1;
            if count > 2.0 {
                let mut i = 2;
                while (i as f32) < count {
                    // offset = normal of the vector from pt1 to pt2.
                    direction.rotate(angle);
                    Self::rotate_about(&mut pt2, center_of_curve, angle);
                    pt1 = pt2 + direction;
                    if !self.check_segments() {
                        return;
                    }
                    let nr = self.num_roads as usize;
                    self.roads[nr].pt1.loc.set(pt2.x, pt2.y);
                    self.roads[nr].pt2.loc.set(pt1.x, pt1.y);
                    self.roads[nr].pt1.last = true; // if not, that one will clear flag in prior loop.
                    self.roads[nr].pt2.last = true; // if not, that one will clear flag in prior loop.
                    self.roads[nr].scale = c_scale;
                    self.roads[nr].width_in_texture = c_wit;
                    self.roads[nr].type_ = TCorner::Curve;
                    self.roads[nr].curve_radius = c_rad;
                    self.roads[nr].unique_id = c_uid;
                    self.num_roads += 1;
                    i += 1;
                }
            }

            self.set_loc_of(pr1, Vector2::new(p_int3.x, p_int3.y));

            self.roads[n1].pt1.last = true;
            if count > 1.0 {
                pt2 = self.loc_of(pr1);
                pt1 = self.loc_of(pr1) + self.loc_of(pr1) - self.loc_of(pr2);
                direction.set(pt1.x - pt2.x, pt1.y - pt2.y);
                pt1 = pt2 + direction;
                if !self.check_segments() {
                    return;
                }
                let nr = self.num_roads as usize;
                self.roads[nr].pt1.loc.set(pt2.x, pt2.y);
                self.roads[nr].pt2.loc.set(pt1.x, pt1.y);
                self.roads[nr].pt1.last = true; // if not, that one will clear flag in prior loop.
                self.roads[nr].pt2.last = true; // if not, that one will clear flag in prior loop.
                self.roads[nr].scale = c_scale;
                self.roads[nr].width_in_texture = c_wit;
                self.roads[nr].type_ = TCorner::Curve;
                self.roads[nr].curve_radius = c_rad;
                self.roads[nr].unique_id = c_uid;
                self.num_roads += 1;
            }

            // Recalculate top & bottom.
            let mut road_vector = self.roads[n1].pt2.loc - self.roads[n1].pt1.loc;
            let mut road_normal = Vector2::new(-road_vector.y, road_vector.x);
            road_normal.normalize();
            road_normal *= self.roads[n1].scale * self.roads[n1].width_in_texture / 2.0;
            self.roads[n1].pt1.top = self.roads[n1].pt1.loc + road_normal;
            self.roads[n1].pt1.bottom = self.roads[n1].pt1.loc - road_normal;

            road_vector = self.roads[n2].pt2.loc - self.roads[n2].pt1.loc;
            road_normal = Vector2::new(-road_vector.y, road_vector.x);
            road_normal.normalize();
            road_normal *= self.roads[n2].scale * self.roads[n2].width_in_texture / 2.0;
            self.roads[n2].pt2.top = self.roads[n2].pt2.loc + road_normal;
            self.roads[n2].pt2.bottom = self.roads[n2].pt2.loc - road_normal;
        }
    }

    /// `W3DRoadBuffer::rotateAbout` (W3DRoadBuffer.cpp:3056): rotates `pt_p` about center.
    pub fn rotate_about(pt_p: &mut Vector2, center: Vector2, angle: f32) {
        let mut offset = Vector2::default();
        offset.x = pt_p.x - center.x;
        offset.y = pt_p.y - center.y;
        let org_offset = offset;
        offset.rotate(angle);
        offset.y -= org_offset.y;
        offset.x -= org_offset.x;
        *pt_p += offset;
    }
}

impl W3DRoadBuffer {
    /// `W3DRoadBuffer::W3DRoadBuffer` (W3DRoadBuffer.cpp:3092): constructor; `allocateRoadBuffers` reads the global data
    /// and `TheTerrainRoads`. `get_texture` is `W3DAssetManager::Get_Texture(path, MIP_LEVELS_3)`.
    pub fn new(
        global_data: &rts_lockstep_engine::common::global_data::GlobalData,
        terrain_roads: &rts_lockstep_engine::common::terrain_roads::TerrainRoadCollection,
        get_texture: &mut impl FnMut(&[u8]) -> Option<TextureHandle>,
    ) -> Self {
        let mut b = W3DRoadBuffer {
            road_types: Vec::new(),
            roads: Vec::new(),
            num_roads: 0,
            initialized: false,
            map: None,
            cur_unique_id: 0,
            cur_road_type: 0,
            max_uid: 0,
            cur_open_road: 0,
            max_road_segments: 500,
            max_road_vertex: 1000,
            max_road_index: 2000,
            max_road_types: 8,
            cur_num_road_vertices: 0,
            cur_num_road_indices: 0,
            update_buffers: false,
            warn_segments: true,
        };
        b.allocate_road_buffers(global_data, terrain_roads, get_texture);
        b
    }

    /// `W3DRoadBuffer::~W3DRoadBuffer` (W3DRoadBuffer.cpp:3081).
    pub fn destroy(&mut self) {
        self.free_road_buffers();
        self.map = None;
    }

    /// `W3DRoadBuffer::freeRoadBuffers` (W3DRoadBuffer.cpp:3115): frees the index and vertex buffers.
    pub fn free_road_buffers(&mut self) {
        self.roads = Vec::new();
        self.road_types = Vec::new();
    }

    /// `W3DRoadBuffer::allocateRoadBuffers` (W3DRoadBuffer.cpp:3132): allocates the index and vertex buffers.
    pub fn allocate_road_buffers(
        &mut self,
        global_data: &rts_lockstep_engine::common::global_data::GlobalData,
        terrain_roads: &rts_lockstep_engine::common::terrain_roads::TerrainRoadCollection,
        get_texture: &mut impl FnMut(&[u8]) -> Option<TextureHandle>,
    ) {
        // save data for max limits
        self.max_road_segments = global_data.max_road_segments;
        self.max_road_vertex = global_data.max_road_vertex;
        self.max_road_index = global_data.max_road_index;
        self.max_road_types = global_data.max_road_types;

        // LOAD_TEST_ASSETS
        self.max_road_types += 4;

        self.cur_num_road_vertices = 0;
        self.cur_num_road_indices = 0;
        // one spare segment: insertCurveSegmentAt writes m_roads[m_numRoads] before testing the limit
        self.roads = (0..(self.max_road_segments + 1).max(0)).map(|_| RoadSegment::new()).collect();
        self.road_types = (0..self.max_road_types.max(0)).map(|_| RoadType::new()).collect();

        // load roads from INI (TheTerrainRoads->firstRoad() is the newest: the list is kept oldest first)
        let mut i: i32 = 0;
        for road in terrain_roads.roads.iter().rev() {
            // get a path to the texture file
            if i < self.max_road_types {
                let id = road.id as i32;
                let tex = get_texture(&road.texture);
                let (mv, mi) = (self.max_road_vertex, self.max_road_index);
                self.road_types[i as usize].load_texture(&road.texture, id, tex, mv, mi);
                i += 1;
                if self.max_uid < id {
                    self.max_uid = id;
                }
            }
        }

        while i < self.max_road_types && self.road_types[i as usize].is_auto_loaded() {
            let tp = self.road_types[i as usize].texture_path.clone();
            self.road_types[i as usize].load_test_texture(|_| get_texture(&tp));
            i += 1;
        }
        self.cur_open_road = i;

        self.initialized = true;
    }

    /// `W3DRoadBuffer::clearAllRoads` (W3DRoadBuffer.cpp:3188): removes all roads.
    pub fn clear_all_roads(&mut self) {
        if !self.roads.is_empty() {
            for i in 0..self.num_roads as usize {
                // SetIndexBuffer(NULL, 0) and SetVertexBuffer(NULL, 0): free the segment's buffers
                self.roads[i].free_buffers();
            }
        }
        self.num_roads = 0;
        if !self.road_types.is_empty() {
            for i in 0..self.max_road_types as usize {
                self.road_types[i].set_stacking(0); // Reset stacking orders
                self.road_types[i].set_num_vertices(0);
                self.road_types[i].set_num_indices(0);
            }
        }
    }

    /// `W3DRoadBuffer::setMap` (W3DRoadBuffer.cpp:3210): sets the height map.
    pub fn set_map(&mut self, p_map: Option<RoadMapInfo>) {
        self.map = p_map;
    }

    /// `W3DRoadBuffer::loadRoads` (W3DRoadBuffer.cpp:3219): loads the roads from the map objects.
    pub fn load_roads<H: Fn(f32, f32) -> f32, D: Fn(i32, i32) -> i32>(
        &mut self,
        map_objects: &[RoadMapObject],
        terrain_roads: &rts_lockstep_engine::common::terrain_roads::TerrainRoadCollection,
        terrain: &RoadTerrainFns<H, D>,
    ) {
        if !self.initialized {
            return;
        }
        // Free any existing road segments.
        self.clear_all_roads();
        self.add_map_objects(map_objects, terrain_roads);
        self.update_counts_and_flags();
        self.insert_tee_intersections();
        self.insert_curve_segments();
        self.insert_cross_type_joins();
        self.preload_roads_in_vertex_and_index_buffers(terrain);
        self.update_buffers = true;
    }

    /// `W3DRoadBuffer::updateLighting` (W3DRoadBuffer.cpp:3242): updates the diffuse lighting in the buffers.
    /// (The `if (!m_roads) return` guard is the alt-tab crash fix, 08/21/03.) The border size is `m_map`'s: upstream
    /// asks `TheTerrainRenderObject->getMap()`.
    pub fn update_lighting(&mut self, static_diffuse: &impl Fn(i32, i32) -> i32) {
        if self.roads.is_empty() {
            return;
        }
        let border = self.map.map(|m| m.border_size_inline).unwrap_or(0);
        // Do road segments.
        for cur_road in 0..self.num_roads as usize {
            self.roads[cur_road].update_seg_lighting(border, static_diffuse);
        }
        self.update_buffers = true;
    }

    /// `W3DRoadBuffer::updateCenter` (W3DRoadBuffer.cpp:3282): sets the flag to reload the vertex buffer.
    pub fn update_center(&mut self) {
        self.update_buffers = true;
    }

    /// `W3DRoadBuffer::drawRoads` (W3DRoadBuffer.cpp:3292): draws the roads. Returns the shader upstream picks, the
    /// device passes it is drawn with (`device_passes` is `W3DShaderManager::getShaderPasses(st)`) and each road type's
    /// draw, in stacking order. `NO_TEST_CULL` is defined, so the `TEST_CULL` shrink of the bounds is not translated;
    /// `LOG_STATS` is off. The `#if 0` dynamic light pass is not translated ([`W3DRoadBuffer::load_lit_roads_in_vertex_and_index_buffers`]
    /// is, but unused).
    pub fn draw_roads(&mut self, cloud_texture: bool, noise_texture: bool, wireframe: bool, min_x: i32, max_x: i32, min_y: i32, max_y: i32) -> (RoadShaderType, Vec<RoadTypeDraw>) {
        let bounds = (
            (min_x as f32 * MAP_XY_FACTOR) as i32,
            (min_y as f32 * MAP_XY_FACTOR) as i32,
            (max_x as f32 * MAP_XY_FACTOR) as i32,
            (max_y as f32 * MAP_XY_FACTOR) as i32,
        );

        let mut max_stacking = 0;
        for i in 0..self.max_road_types as usize {
            if self.road_types[i].get_stacking() > max_stacking {
                max_stacking = self.road_types[i].get_stacking();
            }
        }
        let mut st = RoadShaderType::RoadBase; // set default shader
        if cloud_texture {
            st = RoadShaderType::RoadBaseNoise1;
            if noise_texture {
                st = RoadShaderType::RoadBaseNoise12;
            }
        } else if noise_texture {
            st = RoadShaderType::RoadBaseNoise2;
        }
        // upstream: devicePasses = W3DShaderManager::getShaderPasses(st); setTexture(1, cloud); setTexture(2, noise)

        let mut load_buffers = false;
        if self.update_buffers {
            if self.visibility_changed(bounds) {
                load_buffers = true;
            }
        }
        self.update_buffers = false;

        let mut draws = Vec::new();
        for stacking in 0..=max_stacking {
            for i in 0..self.max_road_types as usize {
                if stacking != self.road_types[i].get_stacking() {
                    continue;
                }
                self.cur_unique_id = self.road_types[i].get_unique_id();
                self.cur_road_type = i as i32;
                if load_buffers {
                    self.load_roads_in_vertex_and_index_buffers();
                }
                if self.road_types[i].get_num_indices() == 0 {
                    continue;
                }
                let texture = if wireframe { None } else { self.road_types[i].apply_texture() };
                // upstream, per device pass: if (!wireframe) W3DShaderManager::setShader(st, pass); Draw_Triangles(...)
                // afterwards: if (!wireframe) W3DShaderManager::resetShader(st)
                draws.push(RoadTypeDraw {
                    road_type: i,
                    texture,
                    draw: super::w3d_common::DrawTriangles {
                        start_index: 0,
                        polygon_count: self.road_types[i].get_num_indices() / 3,
                        min_vertex_index: 0,
                        vertex_count: self.road_types[i].get_num_vertices(),
                    },
                });
            }
        }
        self.cur_road_type = 0;
        (st, draws)
    }
}
