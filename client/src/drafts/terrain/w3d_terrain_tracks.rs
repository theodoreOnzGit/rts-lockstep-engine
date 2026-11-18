//! UNTESTED DRAFT (#202, #204, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DTerrainTracks.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DTerrainTracks.cpp
//!
//! The doubly linked lists of render objects (`m_usedModules`, `m_freeModules`, `m_nextSystem`, `m_prevSystem`) are
//! indices into one `Vec` of modules, with the same list operations. The scene (`SceneClass::Add_Render_Object` /
//! `Remove_Render_Object`) is the renderer's: a module is `in_scene` between `bindTrack` and `releaseTrack`. The
//! `Is_Really_Visible()` test in `flush` reads `hidden` (never set upstream; `Set_Force_Visible(TRUE)` is made in
//! `init`). `WW3D::Get_Sync_Time()`, `ShaderClass::Is_Backface_Culling_Inverted()`, `TheTerrainLogic` heights
//! (`getGroundHeight(x, y, &normal)`, `getLayerHeight(x, y, layer, &normal)`, each returning the height and the normal)
//! and the owner drawable's layer (`m_ownerDrawable->getObject()->getLayer()`) are parameters. `flush` fills `vertex_buffer` and returns the draw calls as data.

use super::w3d_common::{DrawTriangles, TextureHandle, VertexFormatXYZDUV1};
use rts_lockstep_engine::common::base_type::{real_to_int, Coord3D};
use rts_lockstep_engine::common::game_type::{PathfindLayer, LAYER_GROUND};
use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::logic::terrain_visual::MAP_XY_FACTOR;
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use super::w3d_common::{AABoxClass, SphereClass};

/// `MAX_TRACK_EDGE_COUNT`: maximum number of edges or divisions in track mark.
pub const MAX_TRACK_EDGE_COUNT: usize = 100;
/// `MAX_TRACK_OPAQUE_EDGE`: linear fade of edges will begin at this edge.
pub const MAX_TRACK_OPAQUE_EDGE: i32 = 25;
/// `FADE_TIME_FRAMES`: 300 seconds at 30 fps - time to fade out an edge and remove it from the system.
pub const FADE_TIME_FRAMES: i32 = 300000;
/// `BRIDGE_OFFSET_FACTOR`: amount to raise tracks above bridges.
pub const BRIDGE_OFFSET_FACTOR: f32 = 0.25;
/// `DEFAULT_TRACK_SPACING`.
pub const DEFAULT_TRACK_SPACING: f32 = MAP_XY_FACTOR * 1.4;
/// `DEFAULT_TRACK_WIDTH`.
pub const DEFAULT_TRACK_WIDTH: f32 = 4.0;

/// `TerrainTracksRenderObjClass::edgeInfo`.
#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeInfo {
    /// `endPointPos`: the 2 endpoints on the edge.
    pub end_point_pos: [Vector3; 2],
    /// `endPointUV`: uv coordinates at each end point.
    pub end_point_uv: [Vector2; 2],
    /// `timeAdded`: time when edge was created.
    pub time_added: i32,
    /// `alpha`: current alpha value for rendering.
    pub alpha: f32,
}

/// `class TerrainTracksRenderObjClass`: custom render object that draws tracks on the terrain.
#[derive(Clone, Debug)]
pub struct TerrainTracksRenderObjClass {
    /// `m_stageZeroTexture`: primary texture.
    pub stage_zero_texture: Option<TextureHandle>,
    /// `m_boundingSphere`.
    pub bounding_sphere: SphereClass,
    /// `m_boundingBox`.
    pub bounding_box: AABoxClass,
    /// `m_activeEdgeCount`: number of active edges in segment list.
    pub active_edge_count: i32,
    /// `m_totalEdgesAdded`: number of edges ever added to this track.
    pub total_edges_added: i32,
    /// `m_ownerDrawable`: logical object that's laying down tread marks (its drawable id), `None` for NULL.
    pub owner_drawable: Option<u32>,
    /// `m_edges[MAX_TRACK_EDGE_COUNT]`: edges at each segment break.
    pub edges: Vec<EdgeInfo>,
    /// `m_lastAnchor`: location of last edge center.
    pub last_anchor: Vector3,
    /// `m_bottomIndex`: points at oldest edge on track.
    pub bottom_index: i32,
    /// `m_topIndex`: points to newest edge on track.
    pub top_index: i32,
    /// `m_haveAnchor`: set to false until first edge is added.
    pub have_anchor: bool,
    /// `m_bound`: object is bound to owner and accepts new edges.
    pub bound: bool,
    /// `m_width`: track width.
    pub width: f32,
    /// `m_length`: length of each track segment.
    pub length: f32,
    /// `m_airborne`: did the vehicle bounce up into the air?
    pub airborne: bool,
    /// `m_haveCap`: is the segment capped so we can stop and resume at new location.
    pub have_cap: bool,
    /// `m_nextSystem`: next track system (index into the system's modules).
    pub next_system: Option<usize>,
    /// `m_prevSystem`: previous track system.
    pub prev_system: Option<usize>,
    /// Whether the scene holds this object (`Add_Render_Object` / `Remove_Render_Object`).
    pub in_scene: bool,
    /// `Is_Hidden`: never set upstream.
    pub hidden: bool,
    /// `Set_Force_Visible(TRUE)`, made by `init`.
    pub force_visible: bool,
}

impl Default for TerrainTracksRenderObjClass {
    fn default() -> Self {
        Self::new()
    }
}

impl TerrainTracksRenderObjClass {
    /// `TerrainTracksRenderObjClass::TerrainTracksRenderObjClass` (W3DTerrainTracks.cpp:74): just nulls out some
    /// variables.
    pub fn new() -> Self {
        TerrainTracksRenderObjClass {
            stage_zero_texture: None,
            bounding_sphere: SphereClass::default(),
            bounding_box: AABoxClass::default(),
            active_edge_count: 0,
            total_edges_added: 0,
            owner_drawable: None,
            edges: vec![EdgeInfo::default(); MAX_TRACK_EDGE_COUNT],
            last_anchor: Vector3::new(0.0, 1.0, 2.25),
            bottom_index: 0,
            top_index: 0,
            have_anchor: false,
            bound: false,
            width: 0.0,
            length: 0.0,
            airborne: false,
            have_cap: true,
            next_system: None,
            prev_system: None,
            in_scene: false,
            hidden: false,
            force_visible: false,
        }
    }

    /// `Get_Obj_Space_Bounding_Sphere` (W3DTerrainTracks.cpp:93).
    pub fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        self.bounding_sphere
    }

    /// `Get_Obj_Space_Bounding_Box` (W3DTerrainTracks.cpp:102).
    pub fn get_obj_space_bounding_box(&self) -> AABoxClass {
        self.bounding_box
    }

    /// `Class_ID` (W3DTerrainTracks.cpp:112): `RenderObjClass::CLASSID_IMAGE3D`.
    pub fn class_id(&self) -> &'static str {
        "CLASSID_IMAGE3D"
    }

    /// `TerrainTracksRenderObjClass::freeTerrainTracksResources` (W3DTerrainTracks.cpp:133): free any W3D resources
    /// associated with this object (the texture reference).
    pub fn free_terrain_tracks_resources(&mut self) -> i32 {
        self.stage_zero_texture = None;
        self.have_anchor = false;
        self.have_cap = true;
        self.top_index = 0;
        self.bottom_index = 0;
        self.active_edge_count = 0;
        self.total_edges_added = 0;
        self.owner_drawable = None;

        0
    }

    /// `TerrainTracksRenderObjClass::init` (W3DTerrainTracks.cpp:150): setup size settings and allocate the W3D texture
    /// (`texture` is `WW3DAssetManager::Get_Texture(texturename)`).
    pub fn init(&mut self, width: f32, length: f32, texture: Option<TextureHandle>) {
        self.free_terrain_tracks_resources(); // free old data and ib/vb

        self.bounding_sphere = SphereClass::new(Vector3::new(0.0, 0.0, 0.0), 400.0 * MAP_XY_FACTOR);
        self.bounding_box.center.set(0.0, 0.0, 0.0);
        self.bounding_box.extent.set(400.0 * MAP_XY_FACTOR, 400.0 * MAP_XY_FACTOR, 1.0);
        self.width = width;
        self.length = length;
        // no sense culling these things since they have very irregular shape and fade out over time.
        self.force_visible = true;
        self.stage_zero_texture = texture;
    }

    /// The edge fill both `addCapEdgeToTrack` and `addEdgeToTrack` do with the same statements: the two end points and
    /// their UVs (every other edge has a different set of UVs). Returns nothing; `alpha` and the counters are the
    /// callers'.
    fn fill_top_edge(&mut self, v_pos: Vector3, v_x: Vector3) {
        let top = self.top_index as usize;
        let odd = (self.total_edges_added & 1) != 0;
        let top_edge = &mut self.edges[top];

        // calculate left end point
        top_edge.end_point_pos[0] = v_pos - ((self.width * 0.5) * v_x); // @todo: try getting height at endpoint
        top_edge.end_point_pos[0].z += 0.2 * MAP_XY_FACTOR; // raise above terrain slightly

        if odd {
            // every other edge has different set of UV's
            top_edge.end_point_uv[0].x = 0.0;
            top_edge.end_point_uv[0].y = 0.0;
        } else {
            top_edge.end_point_uv[0].x = 0.0;
            top_edge.end_point_uv[0].y = 1.0;
        }

        // calculate right end point
        top_edge.end_point_pos[1] = v_pos + ((self.width * 0.5) * v_x); // @todo: try getting height at endpoint
        top_edge.end_point_pos[1].z += 0.2 * MAP_XY_FACTOR; // raise above terrain slightly

        if odd {
            // every other edge has different set of UV's
            top_edge.end_point_uv[1].x = 1.0;
            top_edge.end_point_uv[1].y = 0.0;
        } else {
            top_edge.end_point_uv[1].x = 1.0;
            top_edge.end_point_uv[1].y = 1.0;
        }
    }

    /// `TerrainTracksRenderObjClass::addCapEdgeToTrack` (W3DTerrainTracks.cpp:171): cap the current track (adding a
    /// feathered edge) so we're ready to resume the track at a new location. Used by objects entering FOW where we need
    /// to stop adding edges to the track when they enter the fog boundary but resume elsewhere if they become visible
    /// again. `max_tank_track_edges` is `TheTerrainTracksRenderObjClassSystem->m_maxTankTrackEdges`, `sync_time` is
    /// `WW3D::Get_Sync_Time()`.
    pub fn add_cap_edge_to_track(&mut self, x: f32, y: f32, ground_height: &impl Fn(f32, f32) -> (f32, Coord3D), layer_height: &impl Fn(f32, f32, PathfindLayer) -> (f32, Coord3D), owner_layer: Option<PathfindLayer>, max_tank_track_edges: i32, sync_time: i32) {
        // @todo: Have object pass its height and orientation so we can remove extra calls.

        if self.have_cap {
            // we already have a cap or there are no segments to cap
            return;
        }

        if self.active_edge_count == 1 {
            // if we only have one edge, then it must be the current anchor edge. since achnors are caps, there is not
            // point in adding another.
            self.have_cap = true;
            self.have_anchor = false; // recreate a new anchor when track resumes.
            return;
        }

        let e_height: f32;
        let v_z_tmp;
        if self.owner_drawable.is_some() && owner_layer.is_some() && owner_layer != Some(LAYER_GROUND) {
            let (h, n) = layer_height(x, y, owner_layer.unwrap());
            e_height = BRIDGE_OFFSET_FACTOR + h;
            v_z_tmp = n;
        } else {
            let (h, n) = ground_height(x, y);
            e_height = h;
            v_z_tmp = n;
        }

        let mut v_z = Vector3::default();
        v_z.x = v_z_tmp.x;
        v_z.y = v_z_tmp.y;
        v_z.z = v_z_tmp.z;

        let mut v_pos = Vector3::default();
        v_pos.x = x;
        v_pos.y = y;
        v_pos.z = e_height;

        let mut v_dir = Vector3::new(x, y, e_height) - self.last_anchor;
        let max_edge_count = max_tank_track_edges;

        // avoid sqrt() by checking distance squared since last track mark
        if v_dir.length2() < self.length * self.length {
            // not far enough from anchor to add track
            // since this is a cap, we'll force the previous segment to transparent
            let mut last_added_edge = self.top_index - 1;
            if last_added_edge < 0 {
                last_added_edge = max_edge_count - 1;
            }
            self.edges[last_added_edge as usize].alpha = 0.0; // force the last added edge to transparent.
            self.have_cap = true;
            self.have_anchor = false; // recreate a new anchor when track resumes.
            return;
        }

        if self.active_edge_count >= max_edge_count {
            // no more room in buffer so release oldest edge
            self.bottom_index += 1;
            self.active_edge_count -= 1;

            if self.bottom_index >= max_edge_count {
                self.bottom_index = 0; // roll buffer back to start
            }
        }

        if self.top_index >= max_edge_count {
            self.top_index = 0; // roll around buffer
        }

        // we traveled far enough from last point. accept new point
        v_dir.z = 0.0; // ignore height
        v_dir.normalize();

        let v_x = Vector3::cross_product(&v_dir, &v_z);

        self.fill_top_edge(v_pos, v_x);

        let top = self.top_index as usize;
        self.edges[top].time_added = sync_time;
        self.edges[top].alpha = 0.0; // fully transparent at cap.
        self.last_anchor = v_pos;
        self.active_edge_count += 1;
        self.total_edges_added += 1;
        self.top_index += 1; // make space for new edge
        self.have_cap = true;
        self.have_anchor = false;
    }

    /// `TerrainTracksRenderObjClass::addEdgeToTrack` (W3DTerrainTracks.cpp:297): try to add an additional segment to the
    /// track mark. Will do nothing if the distance from the last edge is too small. Will overwrite the oldest edge if
    /// the maximum track length is reached. Oldest edges should be faded out by that time.
    pub fn add_edge_to_track(&mut self, x: f32, y: f32, ground_height: &impl Fn(f32, f32) -> (f32, Coord3D), layer_height: &impl Fn(f32, f32, PathfindLayer) -> (f32, Coord3D), owner_layer: Option<PathfindLayer>, max_tank_track_edges: i32, sync_time: i32) {
        // @todo: Have object pass its height and orientation so we can remove extra calls.

        let on_layer = self.owner_drawable.is_some() && owner_layer.is_some() && owner_layer != Some(LAYER_GROUND);

        if !self.have_anchor {
            // no anchor yet, make this point an anchor.
            if on_layer {
                self.last_anchor = Vector3::new(x, y, layer_height(x, y, owner_layer.unwrap()).0 + BRIDGE_OFFSET_FACTOR);
            } else {
                self.last_anchor = Vector3::new(x, y, ground_height(x, y).0);
            }

            self.have_anchor = true;
            self.airborne = true;
            self.have_cap = true; // single segment tracks are always capped because nothing is drawn.
            return;
        }

        self.have_cap = false; // have more than 1 segment now so will need to cap if it's interrupted.

        let e_height: f32;
        let v_z_tmp;
        if on_layer {
            let (h, n) = layer_height(x, y, owner_layer.unwrap());
            e_height = BRIDGE_OFFSET_FACTOR + h;
            v_z_tmp = n;
        } else {
            let (h, n) = ground_height(x, y);
            e_height = h;
            v_z_tmp = n;
        }

        let mut v_z = Vector3::default();
        v_z.x = v_z_tmp.x;
        v_z.y = v_z_tmp.y;
        v_z.z = v_z_tmp.z;

        let mut v_pos = Vector3::default();
        v_pos.x = x;
        v_pos.y = y;
        v_pos.z = e_height;

        let mut v_dir = Vector3::new(x, y, e_height) - self.last_anchor;

        // avoid sqrt() by checking distance squared since last track mark
        if v_dir.length2() < self.length * self.length {
            return; // not far enough from anchor to add track
        }

        let max_edge_count = max_tank_track_edges;

        if self.active_edge_count >= max_edge_count {
            // no more room in buffer so release oldest edge
            self.bottom_index += 1;
            self.active_edge_count -= 1;

            if self.bottom_index >= max_edge_count {
                self.bottom_index = 0; // roll buffer back to start
            }
        }

        if self.top_index >= max_edge_count {
            self.top_index = 0; // roll around buffer
        }

        // we traveled far enough from last point. accept new point
        v_dir.z = 0.0; // ignore height
        v_dir.normalize();

        let v_x = Vector3::cross_product(&v_dir, &v_z);

        self.fill_top_edge(v_pos, v_x);

        let top = self.top_index as usize;
        self.edges[top].time_added = sync_time;
        self.edges[top].alpha = 1.0; // fully opaque at start.
        if self.airborne || self.active_edge_count <= 1 {
            self.edges[top].alpha = 0.0; // smooth out track restarts by setting transparent
        }
        self.airborne = false;

        self.last_anchor = v_pos;
        self.active_edge_count += 1;
        self.total_edges_added += 1;
        self.top_index += 1; // make space for new edge
    }

    /// `TerrainTracksRenderObjClass::setAirborne` (W3DTerrainTracks.h): starts a new section of track, generally after
    /// going airborne.
    pub fn set_airborne(&mut self) {
        self.airborne = true;
    }

    /// `TerrainTracksRenderObjClass::setOwnerDrawable` (W3DTerrainTracks.h).
    pub fn set_owner_drawable(&mut self, owner: Option<u32>) {
        self.owner_drawable = owner;
    }

    /// `Is_Really_Visible` as `flush` uses it: in the scene and not hidden.
    pub fn is_really_visible(&self) -> bool {
        self.in_scene && !self.hidden
    }
}

/// `computeTrackSpacing` (W3DTerrainTracks.cpp:424, static): find the distance between the "trackfx" bones of the model.
/// This tells us the correct width for the trackmarks. `bones` is `Some((leftPos, rightPos))` when
/// `Get_Bone_Index("TREADFX01")` and `Get_Bone_Index("TREADFX02")` are both non-zero (the bones' translations).
pub fn compute_track_spacing(bones: Option<(Vector3, Vector3)>) -> f32 {
    let mut track_spacing = DEFAULT_TRACK_SPACING;

    if let Some((left_pos, right_pos)) = bones {
        // both bones found, determine distance between them.
        let mut right_pos = right_pos;
        right_pos -= left_pos; // get distance between centers of tracks
        track_spacing = right_pos.length() + DEFAULT_TRACK_WIDTH; // add width of each track
        // @todo: It's assumed that all tank treads have the same width.
    }

    track_spacing
}

/// One `Set_Texture` + `Draw_Triangles` of `flush`.
#[derive(Clone, Copy, Debug)]
pub struct TrackDraw {
    /// `mod->m_stageZeroTexture`.
    pub texture: Option<TextureHandle>,
    /// `Set_Index_Buffer_Index_Offset(trackStartIndex)`.
    pub index_offset: i32,
    /// `Draw_Triangles(0, (m_activeEdgeCount-1)*2, 0, m_activeEdgeCount*2)`.
    pub draw: DrawTriangles,
}

/// `class TerrainTracksRenderObjClassSystem`: system for drawing, updating, and re-using tread mark render objects.
pub struct TerrainTracksRenderObjClassSystem {
    /// `m_vertexBuffer`: vertex buffer used to draw all tracks (`numModules*m_maxTankTrackEdges*2` vertices).
    pub vertex_buffer: Option<Vec<VertexFormatXYZDUV1>>,
    /// `m_indexBuffer`: indices defining triangles in maximum length track (`(m_maxTankTrackEdges-1)*6`).
    pub index_buffer: Option<Vec<u16>>,
    /// `m_vertexMaterialClass` (`PRELIT_DIFFUSE`) and `m_shaderClass` (`_PresetAlphaShader`) are the renderer's; whether
    /// `init` made them.
    pub material_and_shader_set: bool,
    /// All the track objects the system owns; the lists below index it.
    pub modules: Vec<TerrainTracksRenderObjClass>,
    /// `m_usedModules`: active objects being rendered in the scene.
    pub used_modules: Option<usize>,
    /// `m_freeModules`: unused modules that are free to use again.
    pub free_modules: Option<usize>,
    /// Whether `m_TerrainTracksScene` is set.
    pub scene_set: bool,
    /// `m_edgesToFlush`: number of edges to flush on next render.
    pub edges_to_flush: i32,
    /// `m_maxTankTrackEdges`.
    pub max_tank_track_edges: i32,
    /// `m_maxTankTrackOpaqueEdges`.
    pub max_tank_track_opaque_edges: i32,
    /// `m_maxTankTrackFadeDelay`.
    pub max_tank_track_fade_delay: i32,
}

impl TerrainTracksRenderObjClassSystem {
    /// `TerrainTracksRenderObjClassSystem::TerrainTracksRenderObjClassSystem` (W3DTerrainTracks.cpp:529): just nulls out
    /// some variables.
    pub fn new(global_data: &GlobalData) -> Self {
        TerrainTracksRenderObjClassSystem {
            vertex_buffer: None,
            index_buffer: None,
            material_and_shader_set: false,
            modules: Vec::new(),
            used_modules: None,
            free_modules: None,
            scene_set: false,
            edges_to_flush: 0,
            max_tank_track_edges: global_data.max_tank_track_edges,
            max_tank_track_opaque_edges: global_data.max_tank_track_opaque_edges,
            max_tank_track_fade_delay: global_data.max_tank_track_fade_delay,
        }
    }

    /// `~TerrainTracksRenderObjClassSystem` (W3DTerrainTracks.cpp:548): free all data.
    pub fn destroy(&mut self) {
        self.shutdown();
        self.material_and_shader_set = false;
        self.scene_set = false;
    }

    /// `TerrainTracksRenderObjClassSystem::ReAcquireResources` (W3DTerrainTracks.cpp:561): (re)allocates all W3D assets
    /// after a reset.
    pub fn re_acquire_resources(&mut self, global_data: &GlobalData) {
        let num_modules = global_data.max_terrain_tracks;

        // just for paranoia's sake.
        self.index_buffer = None;
        self.vertex_buffer = None;

        // Create static index buffers.  These will index the vertex buffers holding the track segments
        let mut ib = vec![0u16; ((self.max_tank_track_edges - 1) * 6) as usize];

        // Fill up the IB
        {
            let mut p = 0usize;
            for i in 0..(self.max_tank_track_edges - 1) {
                ib[p + 3] = (i * 2) as u16;
                ib[p] = (i * 2) as u16;
                ib[p + 1] = (i * 2 + 1) as u16;
                ib[p + 4] = ((i + 1) * 2 + 1) as u16;
                ib[p + 2] = ((i + 1) * 2 + 1) as u16;
                ib[p + 5] = ((i + 1) * 2) as u16;
                p += 6; // skip the 6 indices we just filled
            }
        }
        self.index_buffer = Some(ib);

        // DEBUG_ASSERTCRASH(numModules*m_maxTankTrackEdges*2 < 65535, ("Too many terrain track edges"));

        // DX8VertexBufferClass(DX8_FVF_XYZDUV1, numModules*m_maxTankTrackEdges*2, USAGE_DYNAMIC)
        self.vertex_buffer = Some(vec![VertexFormatXYZDUV1::default(); (num_modules * self.max_tank_track_edges * 2) as usize]);
    }

    /// `TerrainTracksRenderObjClassSystem::ReleaseResources` (W3DTerrainTracks.cpp:605): releases the dx8 buffers so the
    /// device can be reset (the material needs no release).
    pub fn release_resources(&mut self) {
        self.index_buffer = None;
        self.vertex_buffer = None;
    }

    /// `TerrainTracksRenderObjClassSystem::init` (W3DTerrainTracks.cpp:618): initialize the system, allocate all the
    /// render objects we will need.
    pub fn init(&mut self, global_data: &GlobalData) {
        let num_modules = global_data.max_terrain_tracks;

        self.scene_set = true;

        self.re_acquire_resources(global_data);
        // go with a preset material for now: VertexMaterialClass::Get_Preset(PRELIT_DIFFUSE)
        // use a multi-texture shader: (text1*diffuse)*text2: ShaderClass::_PresetAlphaShader
        self.material_and_shader_set = true;

        // we cannot initialize a system that is already initialized
        if self.free_modules.is_some() || self.used_modules.is_some() {
            // system already online!
            // assert( 0 );
            return;
        }

        // allocate our modules for this system
        for _i in 0..num_modules {
            let mut m = TerrainTracksRenderObjClass::new();
            let idx = self.modules.len();
            m.prev_system = None;
            m.next_system = self.free_modules;
            self.modules.push(m);
            if let Some(f) = self.free_modules {
                self.modules[f].prev_system = Some(idx);
            }
            self.free_modules = Some(idx);
        }
    }

    /// `TerrainTracksRenderObjClassSystem::shutdown` (W3DTerrainTracks.cpp:677): shutdown and free all memory for this
    /// system.
    pub fn shutdown(&mut self) {
        // release unbound tracks that may still be fading out
        let mut m = self.used_modules;

        while let Some(idx) = m {
            let next_mod = self.modules[idx].next_system;

            if !self.modules[idx].bound {
                self.release_track(Some(idx));
            }

            m = next_mod;
        }

        // free all attached things and used modules
        // assert( m_usedModules == NULL );

        // free all module storage
        while let Some(f) = self.free_modules {
            let next_mod = self.modules[f].next_system;
            self.free_modules = next_mod;
        }
        self.modules.clear();

        self.index_buffer = None;
        self.material_and_shader_set = false;
        self.vertex_buffer = None;
    }

    /// `TerrainTracksRenderObjClassSystem::bindTrack` (W3DTerrainTracks.cpp:394): grab a track from the free store. If
    /// no free tracks exist, return `None`. As long as a track is bound to an object (like a tank) it is ready to accept
    /// updates with additional edges. Once it is unbound, it will expire and return to the free store once all tracks
    /// have faded out. `track_bones` is `computeTrackSpacing`'s input, `texture` is the texture for the tracks (image
    /// should be symmetrical and include an alpha channel).
    pub fn bind_track(&mut self, track_bones: Option<(Vector3, Vector3)>, length: f32, texture: Option<TextureHandle>) -> Option<usize> {
        let m = self.free_modules;
        if let Some(idx) = m {
            // take module off the free list
            if let Some(n) = self.modules[idx].next_system {
                self.modules[n].prev_system = self.modules[idx].prev_system;
            }
            if let Some(p) = self.modules[idx].prev_system {
                self.modules[p].next_system = self.modules[idx].next_system;
            } else {
                self.free_modules = self.modules[idx].next_system;
            }

            // put module on the used list
            self.modules[idx].prev_system = None;
            self.modules[idx].next_system = self.used_modules;
            if let Some(u) = self.used_modules {
                self.modules[u].prev_system = Some(idx);
            }
            self.used_modules = Some(idx);

            self.modules[idx].init(compute_track_spacing(track_bones), length, texture);
            self.modules[idx].bound = true;
            self.modules[idx].in_scene = true; // m_TerrainTracksScene->Add_Render_Object( mod)
        }

        m
    }

    /// `TerrainTracksRenderObjClassSystem::unbindTrack` (W3DTerrainTracks.cpp:437): called when an object (i.e Tank) will
    /// not lay down any more tracks and doesn't need this object anymore. The track-laying object will be returned to the
    /// pool of available tracks as soon as any remaining track edges have faded out.
    pub fn unbind_track(&mut self, m: usize) {
        // this object should return to free store as soon as there is nothing left to render.
        self.modules[m].bound = false;
        self.modules[m].owner_drawable = None;
    }

    /// `TerrainTracksRenderObjClassSystem::releaseTrack` (W3DTerrainTracks.cpp:450): returns a track laying object to
    /// free store to be used again later.
    pub fn release_track(&mut self, m: Option<usize>) {
        let idx = match m {
            Some(i) => i,
            None => return,
        };

        // DEBUG_ASSERTCRASH(mod->m_bound == false, ("mod is bound."));

        // remove module from used list
        if let Some(n) = self.modules[idx].next_system {
            self.modules[n].prev_system = self.modules[idx].prev_system;
        }
        if let Some(p) = self.modules[idx].prev_system {
            self.modules[p].next_system = self.modules[idx].next_system;
        } else {
            self.used_modules = self.modules[idx].next_system;
        }

        // add module to free list
        self.modules[idx].prev_system = None;
        self.modules[idx].next_system = self.free_modules;
        if let Some(f) = self.free_modules {
            self.modules[f].prev_system = Some(idx);
        }
        self.free_modules = Some(idx);
        self.modules[idx].free_terrain_tracks_resources();
        self.modules[idx].in_scene = false; // m_TerrainTracksScene->Remove_Render_Object(mod)
    }

    /// `TerrainTracksRenderObjClass::Render` (W3DTerrainTracks.cpp:277): does nothing but increment the counter of how
    /// many track edges were requested for rendering this frame. Actual rendering is done in `flush`. `m` is the
    /// module the scene renders.
    pub fn render(&mut self, m: usize, global_data: &GlobalData) {
        // @todo: After adding track mark visibility tests, add visible marks to another list.
        if global_data.make_track_marks && self.modules[m].active_edge_count >= 2 {
            self.edges_to_flush += self.modules[m].active_edge_count;
        }
    }

    /// `TerrainTracksRenderObjClassSystem::update` (W3DTerrainTracks.cpp:701): update the state of all active track
    /// marks - fade, expire, etc. `sync_time` is `WW3D::Get_Sync_Time()`.
    pub fn update(&mut self, global_data: &GlobalData, sync_time: i32) {
        let i_time = sync_time;
        let mut m = self.used_modules;

        // first update all the tracks
        while let Some(idx) = m {
            let next_mod = self.modules[idx].next_system;

            if !global_data.make_track_marks {
                self.modules[idx].have_anchor = false; // force a track restart next time around.
            }

            let mut i = 0;
            let mut index = self.modules[idx].bottom_index;
            while i < self.modules[idx].active_edge_count {
                if index >= self.max_tank_track_edges {
                    index = 0;
                }

                let mut i_diff: f32 = (i_time - self.modules[idx].edges[index as usize].time_added) as f32;
                i_diff = 1.0 - i_diff / self.max_tank_track_fade_delay as f32;
                if i_diff < 0.0 {
                    i_diff = 0.0;
                }
                if self.modules[idx].edges[index as usize].alpha > 0.0 {
                    self.modules[idx].edges[index as usize].alpha = i_diff;
                }

                if i_diff == 0.0 {
                    // this edge was invisible, we can remove it
                    self.modules[idx].bottom_index += 1;
                    self.modules[idx].active_edge_count -= 1;

                    if self.modules[idx].bottom_index >= self.max_tank_track_edges {
                        self.modules[idx].bottom_index = 0; // roll buffer back to start
                    }
                }
                if self.modules[idx].active_edge_count == 0 && !self.modules[idx].bound {
                    self.release_track(Some(idx));
                }
                i += 1;
                index += 1;
            }
            m = next_mod;
        }
    }

    /// `TerrainTracksRenderObjClassSystem::flush` (W3DTerrainTracks.cpp:766): draw all active track marks for this
    /// frame. Fills `vertex_buffer` and returns the draws (one per visible module). `backface_culling_inverted` is
    /// `ShaderClass::Is_Backface_Culling_Inverted()`. The `Set_Material`/`Set_Shader`/buffers/world transform set-up is
    /// device-only.
    pub fn flush(&mut self, global_data: &GlobalData, backface_culling_inverted: bool) -> Vec<TrackDraw> {
        // @todo: Optimize system by drawing tracks as triangle strips and use dynamic vertex buffer access.
        let mut draws = Vec::new();

        let mut m = self.used_modules;
        if m.is_none() {
            return draws; // nothing to render
        }

        if backface_culling_inverted {
            return draws; // don't render track marks in reflections.
        }

        // adjust shading for time of day.
        let mut shade_r = global_data.terrain_ambient[0].red;
        let mut shade_g = global_data.terrain_ambient[0].green;
        let mut shade_b = global_data.terrain_ambient[0].blue;
        shade_r += global_data.terrain_diffuse[0].red / 2.0;
        shade_g += global_data.terrain_diffuse[0].green / 2.0;
        shade_b += global_data.terrain_diffuse[0].blue / 2.0;
        shade_r *= 255.0;
        shade_g *= 255.0;
        shade_b *= 255.0;

        let diffuse_light: i32 = real_to_int(shade_b) | (real_to_int(shade_g) << 8) | (real_to_int(shade_r) << 16);
        let num_faded_edges: f32 = (self.max_tank_track_edges - self.max_tank_track_opaque_edges) as f32;

        // check if there is anything to draw and fill vertex buffer
        if self.edges_to_flush >= 2 {
            let mut verts = self.vertex_buffer.take().unwrap();
            let mut v = 0usize;

            m = self.used_modules;
            // Fill our vertex buffer with all the tracks
            while let Some(idx) = m {
                let md = &self.modules[idx];
                if md.active_edge_count >= 2 && md.is_really_visible() {
                    let mut i = 0;
                    let mut index = md.bottom_index;
                    while i < md.active_edge_count {
                        if index >= self.max_tank_track_edges {
                            index = 0;
                        }

                        let e = &md.edges[index as usize];

                        let mut distance_fade: f32 = 1.0;

                        if (md.active_edge_count - 1 - i) >= self.max_tank_track_opaque_edges {
                            // we're getting close to the limit on the number of track pieces allowed so force it to
                            // fade out.
                            distance_fade = 1.0 - ((md.active_edge_count - i) - self.max_tank_track_opaque_edges) as f32 / num_faded_edges;
                        }

                        distance_fade *= e.alpha; // adjust fade with distance from start of track

                        // left endpoint
                        verts[v].x = e.end_point_pos[0].x;
                        verts[v].y = e.end_point_pos[0].y;
                        verts[v].z = e.end_point_pos[0].z;

                        verts[v].u1 = e.end_point_uv[0].x;
                        verts[v].v1 = e.end_point_uv[0].y;

                        // fade the alpha channel with distance
                        verts[v].diffuse = (diffuse_light | (real_to_int(distance_fade * 255.0) << 24)) as u32;
                        v += 1;

                        // right endpoint
                        verts[v].x = e.end_point_pos[1].x;
                        verts[v].y = e.end_point_pos[1].y;
                        verts[v].z = e.end_point_pos[1].z;

                        verts[v].u1 = e.end_point_uv[1].x;
                        verts[v].v1 = e.end_point_uv[1].y; // @todo: Add diffuse lighting.

                        verts[v].diffuse = (diffuse_light | (real_to_int(distance_fade * 255.0) << 24)) as u32;
                        v += 1;

                        i += 1;
                        index += 1;
                    }
                } // mod has edges to render
                m = md.next_system;
            } // while (mod)
            self.vertex_buffer = Some(verts);
        } // edges to flush

        // draw the filled vertex buffers
        if self.edges_to_flush >= 2 {
            // upstream: ShaderClass::Invalidate, Set_Material, Set_Shader, Set_Index_Buffer, Set_Vertex_Buffer,
            // Set_Transform(D3DTS_WORLD, mod->Transform) — device-only, not translated
            let mut track_start_index = 0;
            m = self.used_modules;
            while let Some(idx) = m {
                let md = &self.modules[idx];
                if md.active_edge_count >= 2 && md.is_really_visible() {
                    // Set_Texture(0, mod->m_stageZeroTexture); Set_Index_Buffer_Index_Offset(trackStartIndex)
                    draws.push(TrackDraw {
                        texture: md.stage_zero_texture,
                        index_offset: track_start_index,
                        draw: DrawTriangles {
                            start_index: 0,
                            polygon_count: (md.active_edge_count - 1) * 2,
                            min_vertex_index: 0,
                            vertex_count: md.active_edge_count * 2,
                        },
                    });

                    track_start_index += md.active_edge_count * 2;
                }
                m = md.next_system;
            }
        } // there are some edges to render in pool.

        self.edges_to_flush = 0; // reset count for next flush
        draws
    }

    /// `TerrainTracksRenderObjClassSystem::Reset` (W3DTerrainTracks.cpp:797): removes all remaining tracks from the
    /// rendering system.
    pub fn reset(&mut self) {
        let mut m = self.used_modules;

        while let Some(idx) = m {
            let next_mod = self.modules[idx].next_system;

            self.release_track(Some(idx));

            m = next_mod;
        }

        // free all attached things and used modules
        // assert( m_usedModules == NULL );
        self.edges_to_flush = 0;
    }

    /// `TerrainTracksRenderObjClassSystem::clearTracks` (W3DTerrainTracks.cpp:817): reset the amount of visible track
    /// marks of each object without freeing the objects. Mostly used when user changed LOD level.
    pub fn clear_tracks(&mut self) {
        let mut m = self.used_modules;

        while let Some(idx) = m {
            let md = &mut self.modules[idx];
            md.have_anchor = false;
            md.have_cap = true;
            md.top_index = 0;
            md.bottom_index = 0;
            md.active_edge_count = 0;
            md.total_edges_added = 0;

            m = md.next_system;
        }

        self.edges_to_flush = 0;
    }

    /// `TerrainTracksRenderObjClassSystem::setDetail` (W3DTerrainTracks.cpp:838): adjust various parameters which affect
    /// the cost of rendering tracks on the map. Parameters are passed via `GlobalData`.
    pub fn set_detail(&mut self, global_data: &GlobalData) {
        // Remove all existing track segments from screen.
        self.clear_tracks();
        self.release_resources();

        self.max_tank_track_edges = global_data.max_tank_track_edges;
        self.max_tank_track_opaque_edges = global_data.max_tank_track_opaque_edges;
        self.max_tank_track_fade_delay = global_data.max_tank_track_fade_delay;

        // We changed the maximum number of visible edges so re-allocate our resources to match.
        self.re_acquire_resources(global_data);
    }
}
