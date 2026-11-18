//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdrenderer.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdrenderer.h
//!
//! The shader mesh renderer. Upstream: `ShdRendererClass` (abstract singleton) and `ShdDX8RendererClass`; a
//! `MeshContainerClass` per shader class id; per pass an ordered list of `RendererListContainerClass`es that each
//! hold the `ShdRendererNodeClass`es (one per sub-mesh) that are similar enough to be drawn together; and the
//! visible-node lists `Render` fills and `Flush` empties.
//!
//! Modelling: the `MultiListClass` intrusive lists are `Vec`s of ids. Nodes and list containers live in arenas on
//! [`ShdRendererClass`] (`nodes`, `containers`), so a node's `RendererListContainer[pass]` and the list's node pointers
//! are plain ids. The abstract `ShdRendererClass` + `ShdDX8RendererClass` pair is one struct (there is one
//! implementation). Ref counts (`Add_Ref`, `REF_PTR_*`) are not modelled: the arenas own, `Arc` shares the sub-mesh
//! and the mesh state.
//!
//! Device-only, not translated: `DX8Wrapper::*` (`Apply_Render_State_Changes`, `Invalidate_Cached_Render_States`,
//! `Apply_Default_State`, `Set_Vertex_Buffer`, `Set_Index_Buffer`, `Set_Transform`, `Set_Light_Environment`,
//! `Draw_Triangles`), `SortingRendererClass::Insert_Triangles`, the `DX8VertexBufferClass`/`DX8IndexBufferClass`
//! objects and the shader's `Apply_Shared`/`Apply_Instance`. Their content is kept: `Flush` pushes a
//! [`ShdDrawCall`] with what upstream would have set and drawn (the bytes of each vertex stream, the 16-bit index
//! buffer, the world transform, the counts, the sorting flag) so the client's wgpu path draws from it.
//!
//! **Finding for the differential pass.** `MeshContainerClass::Register_Mesh`'s "not similar enough" branch does
//! `RendererListContainers[pass].Add_After(new_cont, new_cont)`: it adds the new container after *itself*, which is not
//! in the list. `GenericMultiListClass::Internal_Add_After` then reads `existing_list_member->Get_List_Node()` (NULL for
//! a container in no list) and dereferences it (`existing_node->List` is evaluated before the `existing_node` test of its
//! `while`), so upstream crashes there. The intent is evidently to insert after the container being compared
//! against (`cont`); that is what this draft does, and it is flagged where it happens.

use super::handles::LightEnvironmentHandle;
use super::lightenvironment::LightEnvironmentClass;
use super::rinfo::RenderInfoClass;
use super::shdchunk::{VertexStreamStruct, MAX_TEXTURE_STAGES, SHDDEF_CLASSID_DUMMY, SHDDEF_CLASSID_LAST};
use super::shdinterface::ShdInterfaceClass;
use super::shdmesh::ShdMeshRenderState;
use super::shdsubmesh::{ShdShaderInstance, ShdSubMeshClass};
use crate::formats::w3d::mesh::mesh_flags;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use std::sync::{Arc, RwLock};

/// `SHD_MAX_PASSES`.
pub const SHD_MAX_PASSES: usize = ShdInterfaceClass::SHD_MAX_PASSES as usize;

/// What one `ShdDX8RendererNodeClass::Flush` would set on the device and draw (see the file header).
#[derive(Clone, Debug)]
pub struct ShdDrawCall {
    /// `cur_pass`.
    pub pass: i32,
    /// The shader class id of the node (what `Apply_Default_State` decisions compare).
    pub shader_class_id: u32,
    /// `Set_Transform(D3DTS_WORLD, ..)`: the mesh transform, or identity for a skin.
    pub world: Matrix3D,
    /// The vertex streams (`Set_Vertex_Buffer(VertexBuffers[n], n)`; for a skin the one dynamic stream).
    pub vertex_streams: Vec<Vec<u8>>,
    /// `Set_Index_Buffer(IndexBuffer, 0)`: 16-bit indices, three per visible polygon.
    pub index_buffer: Vec<u16>,
    /// `Set_Light_Environment(&LightEnvironment)`.
    pub light_environment: LightEnvironmentClass,
    /// `SubMesh->Is_Sorting()`: `SortingRendererClass::Insert_Triangles` instead of `DX8Wrapper::Draw_Triangles`.
    pub sorting: bool,
    /// The polygon count of the draw: `Get_Visible_Polygon_Count()` or, when 0, `Get_Polygon_Count()`.
    pub polygon_count: i32,
    /// `SubMesh->Get_Vertex_Count()`.
    pub vertex_count: i32,
    /// `Apply_Default_State()` is called before this draw's shared settings (`Apply_Shared_Shader_Settings` found
    /// the previous node's class id different).
    pub apply_default_state: bool,
}

/// `class RendererListContainerClass` (shdrenderer.h).
#[derive(Clone, Debug)]
pub struct RendererListContainerClass {
    /// `Pass`.
    pub pass: i32,
    /// `LinkedNodes`: node ids in `Add_Tail` order.
    pub linked_nodes: Vec<usize>,
    /// `VisibleNodes`.
    pub visible_nodes: Vec<usize>,
}

impl RendererListContainerClass {
    /// `RendererListContainerClass::RendererListContainerClass(pass)`.
    pub fn new(pass: i32) -> Self {
        RendererListContainerClass { pass, linked_nodes: Vec::new(), visible_nodes: Vec::new() }
    }

    /// `Add_Visible_Node(node)`: `VisibleNodes.Add_Tail(node)`.
    pub fn add_visible_node(&mut self, node: usize) {
        self.visible_nodes.push(node);
    }

    /// `Peek_Linked_Nodes()`.
    pub fn peek_linked_nodes(&self) -> &[usize] {
        &self.linked_nodes
    }
}

/// `class ShdRendererNodeClass` + `class ShdDX8RendererNodeClass`: the link between a `ShdMeshClass`, a
/// `ShdSubMeshClass` and the rendering system.
#[derive(Clone, Debug)]
pub struct ShdRendererNodeClass<S> {
    /// `RendererListContainer[SHD_MAX_PASSES]`: container ids.
    pub renderer_list_container: [Option<usize>; SHD_MAX_PASSES],
    /// `MaxPass`.
    pub max_pass: i32,
    /// `Container`: the shader class id of the `MeshContainerClass` that made this node.
    pub container: usize,
    /// `Mesh` (what the node reads of its mesh).
    pub mesh: Arc<RwLock<ShdMeshRenderState>>,
    /// `SubMesh`.
    pub sub_mesh: Arc<RwLock<ShdSubMeshClass<S>>>,
    /// `VertexBuffers`: one byte buffer per shader stream; empty for a skin.
    pub vertex_buffers: Vec<Vec<u8>>,
    /// `IndexBuffer` (16-bit, `index_count = polygons * 3`).
    pub index_buffer: Vec<u16>,
    /// Usage chosen from `!Use_HW_Vertex_Processing()` (`USAGE_SOFTWAREPROCESSING`).
    pub software_processing: bool,
    /// `LightEnvironment` (copied at `Render`).
    pub light_environment: LightEnvironmentClass,
    /// `RenderInfo`: a snapshot of the `RenderInfoClass` passed to `Render` (upstream keeps a pointer).
    pub render_info: Option<RenderInfoClass>,
}

/// `ShdDX8RendererClass::MeshContainerClass`: per shader class, per pass, the ordered list of list containers (ids).
#[derive(Clone, Debug, Default)]
pub struct MeshContainerClass {
    /// `RendererListContainers[SHD_MAX_PASSES]`: container ids in list order (`Add` puts at the head).
    pub renderer_list_containers: [Vec<usize>; SHD_MAX_PASSES],
}

/// `class ShdRendererClass` / `ShdDX8RendererClass`.
#[derive(Clone, Debug)]
pub struct ShdRendererClass<S> {
    /// `MeshCategories[SHDDEF_CLASSID_LAST]`.
    pub mesh_categories: Vec<Option<MeshContainerClass>>,
    /// The node arena (a freed node leaves `None`).
    pub nodes: Vec<Option<ShdRendererNodeClass<S>>>,
    /// The list-container arena.
    pub containers: Vec<Option<RendererListContainerClass>>,
}

impl<S: ShdShaderInstance> ShdRendererClass<S> {
    /// `ShdDX8RendererClass::ShdDX8RendererClass()`: `MeshCategories` of `SHDDEF_CLASSID_LAST` NULLs.
    pub fn new() -> Self {
        ShdRendererClass {
            mesh_categories: (0..SHDDEF_CLASSID_LAST).map(|_| None).collect(),
            nodes: Vec::new(),
            containers: Vec::new(),
        }
    }

    /// `ShdRendererClass::Init()`: `SHD_Force_Links()` (nothing to link), `WWASSERT(!ShdRenderer)`,
    /// `ShdRenderer = new ShdDX8RendererClass()` (this constructor, held by the owner as the singleton), then
    /// `Init_Shaders()` (see [`Self::init_shaders`]).
    pub fn init() -> Self {
        Self::new()
    }

    /// `ShdRendererClass::Init_Shaders()`: for every class id from `SHDDEF_CLASSID_DUMMY+1` to `SHDDEF_CLASSID_LAST`
    /// (exclusive) `def = Create_ShdDefClass_Instance(i); if (def) { def->Init(); release }`: `init_def(i)` is
    /// that body.
    pub fn init_shaders<F: FnMut(u32)>(mut init_def: F) {
        for i in (SHDDEF_CLASSID_DUMMY + 1)..SHDDEF_CLASSID_LAST {
            init_def(i);
        }
    }

    /// `ShdRendererClass::Shutdown_Shaders()`: the same loop with `def->Shutdown()`.
    pub fn shutdown_shaders<F: FnMut(u32)>(mut shutdown_def: F) {
        for i in (SHDDEF_CLASSID_DUMMY + 1)..SHDDEF_CLASSID_LAST {
            shutdown_def(i);
        }
    }

    /// `ShdRendererClass::Shutdown()`: `Shutdown_Shaders()`, then `delete ShdRenderer` (drop `self`). `shutdown_def`
    /// as [`Self::shutdown_shaders`].
    pub fn shutdown<F: FnMut(u32)>(self, shutdown_def: F) {
        Self::shutdown_shaders(shutdown_def);
        drop(self);
    }

    // ---- RendererListContainerClass methods that touch nodes ----

    /// `RendererListContainerClass::Register_Renderer(node)`: `node->Set_Renderer_List_Container(this, Pass)` and
    /// `LinkedNodes.Add_Tail(node)`.
    fn container_register_renderer(&mut self, cont: usize, node: usize) {
        let pass = self.containers[cont].as_ref().unwrap().pass;
        self.node_set_renderer_list_container(node, Some(cont), pass);
        self.containers[cont].as_mut().unwrap().linked_nodes.push(node);
    }

    /// `ShdRendererNodeClass::Set_Renderer_List_Container(cont, pass)`: `MaxPass = max(MaxPass, pass)` when `cont`.
    fn node_set_renderer_list_container(&mut self, node: usize, cont: Option<usize>, pass: i32) {
        let n = self.nodes[node].as_mut().unwrap();
        n.renderer_list_container[pass as usize] = cont;
        if cont.is_some() {
            n.max_pass = n.max_pass.max(pass);
        }
    }

    /// `RendererListContainerClass::Unregister_All()`: removes every linked node and deletes it.
    fn container_unregister_all(&mut self, cont: usize) {
        loop {
            let node = {
                let c = self.containers[cont].as_mut().unwrap();
                if c.linked_nodes.is_empty() {
                    break;
                }
                c.linked_nodes.remove(0)
            };
            self.nodes[node] = None; // delete node
        }
    }

    /// `ShdRendererNodeClass::Connect()`: for `pass = 0..=MaxPass` adds the node to its container's visible list.
    fn node_connect(&mut self, node: usize) {
        let (max_pass, conts) = {
            let n = self.nodes[node].as_ref().unwrap();
            (n.max_pass, n.renderer_list_container)
        };
        for pass in 0..=max_pass {
            // WWASSERT(RendererListContainer[pass]);
            if let Some(c) = conts[pass as usize] {
                self.containers[c].as_mut().unwrap().add_visible_node(node);
            }
        }
    }

    // ---- ShdDX8RendererClass ----

    /// `ShdDX8RendererClass::Register_Mesh(mesh, sub_mesh)`: finds (or makes) the `MeshContainerClass` of the
    /// sub-mesh's shader class id and registers the sub-mesh with it. Returns the node id.
    pub fn register_mesh(&mut self, mesh: Arc<RwLock<ShdMeshRenderState>>, sub_mesh: Arc<RwLock<ShdSubMeshClass<S>>>) -> usize {
        let class_id = {
            let sm = sub_mesh.read().unwrap();
            let shd = sm.peek_shader().unwrap();
            // const ShdDefClass* def = shd->Peek_Definition(); uint32 class_id = def->Get_Class_ID();
            shd.definition_class_id().unwrap_or_else(|| shd.class_id())
        };
        debug_assert!(class_id < SHDDEF_CLASSID_LAST);
        if self.mesh_categories[class_id as usize].is_none() {
            self.mesh_categories[class_id as usize] = Some(MeshContainerClass::default());
        }
        self.mesh_container_register_mesh(class_id as usize, mesh, sub_mesh)
    }

    /// `ShdDX8RendererClass::Flush()`: for every category with a container, `MeshCategories[i]->Flush()`. The device
    /// state changes around it (`Apply_Render_State_Changes`, `Invalidate_Cached_Render_States`,
    /// `Apply_Default_State`, before and after) are device-only. `deform(sub_mesh, mesh_state)` is the skin's
    /// `SubMesh->Get_Deformed_Vertices(loc, norm, Mesh->Get_Container()->Get_HTree())`.
    pub fn flush<F>(&mut self, deform: &mut F) -> Vec<ShdDrawCall>
    where
        F: FnMut(&ShdSubMeshClass<S>, &ShdMeshRenderState) -> (Vec<Vector3>, Vec<Vector3>),
    {
        // upstream: DX8Wrapper::Apply_Render_State_Changes(); Invalidate_Cached_Render_States(); Apply_Default_State() — device-only, not translated
        let mut out = Vec::new();
        for i in 0..SHDDEF_CLASSID_LAST as usize {
            if self.mesh_categories[i].is_some() {
                self.mesh_container_flush(i, deform, &mut out);
            }
        }
        // upstream: DX8Wrapper::Invalidate_Cached_Render_States(); Apply_Default_State() — device-only, not translated
        out
    }

    /// `MeshContainerClass::~MeshContainerClass()`: per pass, removes each list container, `Unregister_All()`s it.
    /// (`~ShdDX8RendererClass` deletes the categories; here `drop`.)
    pub fn clear(&mut self) {
        for i in 0..self.mesh_categories.len() {
            if let Some(mc) = self.mesh_categories[i].take() {
                for pass in 0..SHD_MAX_PASSES {
                    for &cont in mc.renderer_list_containers[pass].iter() {
                        self.container_unregister_all(cont);
                        self.containers[cont] = None;
                    }
                }
            }
        }
    }

    /// `MeshContainerClass::Register_Mesh(mesh, sub_mesh)` (shdrenderer.cpp): new node, then per pass of the
    /// shader find where it sorts among the containers.
    fn mesh_container_register_mesh(&mut self, category: usize, mesh: Arc<RwLock<ShdMeshRenderState>>, sub_mesh: Arc<RwLock<ShdSubMeshClass<S>>>) -> usize {
        let node = self.new_node(category, mesh, sub_mesh.clone());
        let pass_count = sub_mesh.read().unwrap().peek_shader().unwrap().pass_count(); // WWASSERT(shdi)
        for pass in 0..pass_count {
            let list: Vec<usize> = self.mesh_categories[category].as_ref().unwrap().renderer_list_containers[pass as usize].clone();
            // RendererListContainerIterator ite(&RendererListContainers[pass]); ite.Last();
            let mut pos = list.len() as isize - 1;
            let mut broke = false;
            while pos >= 0 {
                let cont = list[pos as usize];
                // ShdRendererNodeListIterator ite2(&cont->Peek_Linked_Nodes()); ite2.First();
                // "Container should delete itself when the last node is removed, so we must never have an empty node."
                let first = self.containers[cont].as_ref().unwrap().linked_nodes.first().copied();
                debug_assert!(first.is_some());
                if let Some(obj) = first {
                    if self.node_greater_than(node, obj, pass) {
                        // If similar enough, add to the same renderer container, otherwise create a new one
                        if self.node_similar_enough(node, obj, pass) {
                            self.container_register_renderer(cont, node);
                        } else {
                            let new_cont = self.new_container(pass);
                            self.container_register_renderer(new_cont, node);
                            // upstream: RendererListContainers[pass].Add_After(new_cont, new_cont) — see the file header
                            // (it adds the container after itself, which crashes upstream); inserted after `cont` here.
                            let l = &mut self.mesh_categories[category].as_mut().unwrap().renderer_list_containers[pass as usize];
                            l.insert(pos as usize + 1, new_cont);
                        }
                        broke = true;
                        break;
                    }
                }
                pos -= 1; // ite.Prev()
            }
            if !broke {
                // if (ite.Is_Done())
                let new_cont = self.new_container(pass);
                self.container_register_renderer(new_cont, node);
                // RendererListContainers[pass].Add(new_cont): Add puts it at the head of the list
                self.mesh_categories[category].as_mut().unwrap().renderer_list_containers[pass as usize].insert(0, new_cont);
            }
        }
        node
    }

    /// `MeshContainerClass::Flush()`: every container of every pass in list order.
    fn mesh_container_flush<F>(&mut self, category: usize, deform: &mut F, out: &mut Vec<ShdDrawCall>)
    where
        F: FnMut(&ShdSubMeshClass<S>, &ShdMeshRenderState) -> (Vec<Vector3>, Vec<Vector3>),
    {
        // SNAPSHOT_SAY(("ShdDX8RendererClass::MeshContainerClass::Flush()\n"));
        for pass in 0..SHD_MAX_PASSES {
            let list: Vec<usize> = self.mesh_categories[category].as_ref().unwrap().renderer_list_containers[pass].clone();
            for cont in list {
                self.container_flush(cont, deform, out);
            }
        }
    }

    /// `RendererListContainerClass::Flush()`: pops each visible node, applies its shared settings relative to the
    /// previous node, flushes it.
    fn container_flush<F>(&mut self, cont: usize, deform: &mut F, out: &mut Vec<ShdDrawCall>)
    where
        F: FnMut(&ShdSubMeshClass<S>, &ShdMeshRenderState) -> (Vec<Vector3>, Vec<Vector3>),
    {
        let pass = self.containers[cont].as_ref().unwrap().pass;
        let mut prev_node: Option<usize> = None;
        loop {
            let node = {
                let c = self.containers[cont].as_mut().unwrap();
                if c.visible_nodes.is_empty() {
                    break;
                }
                c.visible_nodes.remove(0)
            };
            let default_state = self.node_apply_shared_shader_settings(node, prev_node, pass);
            self.node_flush(node, pass, default_state, deform, out);
            prev_node = Some(node);
        }
    }

    fn new_container(&mut self, pass: i32) -> usize {
        self.containers.push(Some(RendererListContainerClass::new(pass)));
        self.containers.len() - 1
    }

    /// `ShdDX8RendererNodeClass::ShdDX8RendererNodeClass(container, mesh, sub_mesh)`: builds the index buffer (16-bit)
    /// from the visible polygons, and, unless the mesh is a skin, one vertex buffer per shader stream.
    fn new_node(&mut self, container: usize, mesh: Arc<RwLock<ShdMeshRenderState>>, sub_mesh: Arc<RwLock<ShdSubMeshClass<S>>>) -> usize {
        let mut node = ShdRendererNodeClass {
            renderer_list_container: [None; SHD_MAX_PASSES],
            max_pass: 0,
            container,
            mesh,
            sub_mesh: sub_mesh.clone(),
            vertex_buffers: Vec::new(),
            index_buffer: Vec::new(),
            software_processing: false,
            light_environment: LightEnvironmentClass::new(),
            render_info: None,
        };
        {
            let sm = sub_mesh.read().unwrap();
            let shader = sm.peek_shader().unwrap();

            // create usage depending on associated shader's processing behaviour KM
            // (DX8IndexBufferClass::USAGE_DEFAULT, or USAGE_SOFTWAREPROCESSING)
            node.software_processing = !shader.use_hw_vertex_processing();

            let mut count = sm.get_visible_polygon_count();
            if count == 0 {
                count = sm.get_polygon_count();
            }
            let index_count = count as usize * 3;
            // IndexBuffer = new SortingIndexBufferClass(index_count) / DX8IndexBufferClass(index_count, ib_usage)
            node.index_buffer = vec![0u16; index_count];
            let indices = &sm.geometry.poly;
            let mut j = 0usize;
            let first = sm.get_first_visible_polygon();
            let mut i = first;
            while i < count + first {
                node.index_buffer[j] = indices[i as usize][0] as u16;
                j += 1;
                node.index_buffer[j] = indices[i as usize][1] as u16;
                j += 1;
                node.index_buffer[j] = indices[i as usize][2] as u16;
                j += 1;
                i += 1;
            }

            // Don't use static vertex buffers if skin
            if !sm.get_flag(mesh_flags::SKIN) {
                // Compose the vertex and index buffers
                let vss = vertex_stream_of(&sm, None);
                let vertex_count = sm.get_vertex_count() as usize;
                let stream_count = shader.vertex_stream_count();
                debug_assert!(stream_count > 0);
                // vb_usage as for ib_usage; Sorting: SortingVertexBufferClass(vertex_count) else DX8VertexBufferClass
                for n in 0..stream_count {
                    // let vertex_size = shader.vertex_size(n);  (the byte buffer carries its own size)
                    node.vertex_buffers.push(shader.copy_vertex_stream(n, &vss, vertex_count));
                }
            }
        }
        self.nodes.push(Some(node));
        self.nodes.len() - 1
    }

    /// `ShdDX8RendererNodeClass::Greater_Than(s, pass)`: the shaders' `Greater_Than`.
    fn node_greater_than(&self, a: usize, b: usize, pass: i32) -> bool {
        let (na, nb) = (self.nodes[a].as_ref().unwrap(), self.nodes[b].as_ref().unwrap());
        let (sa, sb) = (na.sub_mesh.read().unwrap(), nb.sub_mesh.read().unwrap());
        sa.peek_shader().unwrap().greater_than(sb.peek_shader().unwrap(), pass)
    }

    /// `ShdDX8RendererNodeClass::Similar_Enough(s, pass)`.
    fn node_similar_enough(&self, a: usize, b: usize, pass: i32) -> bool {
        let (na, nb) = (self.nodes[a].as_ref().unwrap(), self.nodes[b].as_ref().unwrap());
        let (sa, sb) = (na.sub_mesh.read().unwrap(), nb.sub_mesh.read().unwrap());
        sa.peek_shader().unwrap().similar_enough(sb.peek_shader().unwrap(), pass)
    }

    /// `ShdDX8RendererNodeClass::Render(rinfo)`: remembers the render info, copies (or resets) the light
    /// environment, and `Connect()`s into the visible lists. `light_env` is the object `rinfo.light_environment`
    /// names; with `rinfo.light_environment` unset the copy is `Reset(Vector3(0,0,0), Vector3(0,0,0))`.
    pub fn render_node(&mut self, node: usize, rinfo: &RenderInfoClass, light_env: Option<&LightEnvironmentClass>) {
        {
            let n = self.nodes[node].as_mut().unwrap();
            n.render_info = Some(rinfo.clone());
            let handle: Option<LightEnvironmentHandle> = rinfo.light_environment;
            if let (Some(_), Some(le)) = (handle, light_env) {
                // memcpy(&LightEnvironment, rinfo.light_environment, sizeof(LightEnvironmentClass))
                n.light_environment = le.clone();
            } else {
                n.light_environment.reset(&Vector3::new(0.0, 0.0, 0.0), &Vector3::new(0.0, 0.0, 0.0));
            }
        }
        self.node_connect(node);
    }

    /// `ShdDX8RendererNodeClass::Apply_Shared_Shader_Settings(prev_node, pass)`: returns whether
    /// `DX8Wrapper::Apply_Default_State()` is called first (the previous node's shader is a different class);
    /// `SubMesh->Peek_Shader()->Apply_Shared(pass, *RenderInfo)` is device-only.
    fn node_apply_shared_shader_settings(&self, node: usize, prev_node: Option<usize>, _pass: i32) -> bool {
        let mut default_state = false;
        // If the previously set shader is not the same type as this, we need to set all render states to default state.
        if let Some(prev) = prev_node {
            let p = self.nodes[prev].as_ref().unwrap().sub_mesh.read().unwrap();
            let cur = self.nodes[node].as_ref().unwrap().sub_mesh.read().unwrap();
            if p.peek_shader().unwrap().class_id() != cur.peek_shader().unwrap().class_id() {
                // upstream: DX8Wrapper::Apply_Default_State() — device-only, not translated
                default_state = true;
            }
        }
        // upstream: SubMesh->Peek_Shader()->Apply_Shared(pass, *RenderInfo) — device-only, not translated
        default_state
    }

    /// `ShdDX8RendererNodeClass::Flush(cur_pass)`: pass selection, then the vertex data (a skin re-deforms and rebuilds
    /// its single dynamic stream every flush), then the draw. Pushes a [`ShdDrawCall`] for what would be drawn.
    fn node_flush<F>(&mut self, node: usize, cur_pass: i32, apply_default_state: bool, deform: &mut F, out: &mut Vec<ShdDrawCall>)
    where
        F: FnMut(&ShdSubMeshClass<S>, &ShdMeshRenderState) -> (Vec<Vector3>, Vec<Vector3>),
    {
        let n = self.nodes[node].as_ref().unwrap();
        debug_assert!(n.render_info.is_some()); // WWASSERT(RenderInfo)
        let mesh_arc = Arc::clone(&n.mesh);
        let sub_arc = Arc::clone(&n.sub_mesh);
        let light_environment = n.light_environment.clone();
        let index_buffer = n.index_buffer.clone();
        let static_streams = n.vertex_buffers.clone();

        // pass selection for this instance
        {
            let mesh_state = mesh_arc.read().unwrap();
            let mut sm = sub_arc.write().unwrap();
            if !sm.shader.as_mut().unwrap().pass_selection(&mesh_state, cur_pass) {
                return;
            }
        }
        // SNAPSHOT_SAY(("ShdDX8RendererNodeClass::Flush(%d) - %s\n", cur_pass, SubMesh->Get_Name()));
        let sm = sub_arc.read().unwrap();
        let mesh_state = mesh_arc.read().unwrap();
        let shader = sm.peek_shader().unwrap();
        let draw_polys = if sm.get_visible_polygon_count() != 0 { sm.get_visible_polygon_count() } else { sm.get_polygon_count() };

        // BEGIN OF SKIN CODE
        if sm.get_flag(mesh_flags::SKIN) {
            // upstream: DX8Wrapper::Set_Vertex_Buffer(NULL) — device-only, not translated
            let vertex_count = sm.get_vertex_count() as usize;
            let stream_count = shader.vertex_stream_count();
            debug_assert!(stream_count > 0);
            // For now there are restrictions in vertex buffer format with skinning:
            // WWASSERT(Get_Vertex_Size(0) == FVFInfoClass(dynamic_fvf_type).Get_FVF_Size());
            // DynamicVBAccessClass vb(sorting ? BUFFER_TYPE_DYNAMIC_SORTING : BUFFER_TYPE_DYNAMIC_DX8, dynamic_fvf_type, vertex_count)
            // (_TempVertexBuffer / _TempNormalBuffer grow to vertex_count: the closure returns vectors of that size)
            debug_assert!(mesh_state.htree.is_some()); // WWASSERT(Mesh->Get_Container())
            let (loc, norm) = deform(&sm, &mesh_state);

            // Compose the vertex and index buffers
            let vss = vertex_stream_of(&sm, Some((loc, norm)));
            debug_assert!(stream_count == 1);
            let bytes = shader.copy_vertex_stream(0, &vss, sm.get_vertex_count() as usize);
            let _ = vertex_count;
            // upstream: Set_Vertex_Buffer(vb), Set_Index_Buffer(IndexBuffer, 0), Set_Transform(D3DTS_WORLD, Matrix3D(true)),
            // Set_Light_Environment(&LightEnvironment), Apply_Instance(cur_pass, *RenderInfo) — device-only, not translated
            out.push(ShdDrawCall {
                pass: cur_pass,
                shader_class_id: shader.class_id(),
                world: Matrix3D::from_translation(&Vector3::new(0.0, 0.0, 0.0)), // Matrix3D(true): identity
                vertex_streams: vec![bytes],
                index_buffer,
                light_environment,
                sorting: sm.is_sorting(),
                polygon_count: draw_polys,
                vertex_count: sm.get_vertex_count(),
                apply_default_state,
            });
            return;
        }
        // END OF SKIN CODE

        // upstream: Set_Vertex_Buffer(VertexBuffers[n], n) per stream, Set_Index_Buffer, Set_Transform(D3DTS_WORLD,
        // Mesh->Get_Transform()), Set_Light_Environment, Apply_Instance — device-only, not translated
        out.push(ShdDrawCall {
            pass: cur_pass,
            shader_class_id: shader.class_id(),
            world: mesh_state.transform,
            vertex_streams: static_streams,
            index_buffer,
            light_environment,
            sorting: sm.is_sorting(),
            polygon_count: draw_polys,
            vertex_count: sm.get_vertex_count(),
            apply_default_state,
        });
    }
}

impl<S: ShdShaderInstance> Default for ShdRendererClass<S> {
    fn default() -> Self {
        Self::new()
    }
}

/// The `VertexStreamStruct vss` both the node constructor and the skin flush fill: `Locations`/`Normals` from the
/// sub-mesh (or the deformed arrays), `UV[stage]` for each stage, `DiffuseInt`, `S`, `T`, `SxT`.
fn vertex_stream_of<S>(sm: &ShdSubMeshClass<S>, deformed: Option<(Vec<Vector3>, Vec<Vector3>)>) -> VertexStreamStruct {
    let mut vss = VertexStreamStruct::default();
    match deformed {
        Some((loc, norm)) => {
            vss.locations = Some(loc);
            vss.normals = Some(norm);
        }
        None => {
            vss.locations = Some(sm.geometry.vertex.clone());
            // Get_Vertex_Normal_Array(): NULL when the sub-mesh has none
            vss.normals = if sm.geometry.vertex_norm.is_empty() { None } else { Some(sm.geometry.vertex_norm.clone()) };
        }
    }
    for stage in 0..MAX_TEXTURE_STAGES {
        vss.uv[stage] = sm.get_uv_array(stage).map(|a| a.to_vec());
    }
    vss.diffuse_int = sm.get_diffuse_array().map(|a| a.to_vec());
    vss.s = sm.get_tangent_basis_s_array().map(|a| a.to_vec());
    vss.t = sm.get_tangent_basis_t_array().map(|a| a.to_vec());
    vss.sxt = sm.get_tangent_basis_sxt_array().map(|a| a.to_vec());
    vss
}
