//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdmesh.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdmesh.h
//!
//! `ShdMeshClass`: the render object of a mesh that uses the shader system; it holds one or more
//! [`ShdSubMeshClass`] that each use a single shader. The `RenderObjClass` base is not in the port, so the base state
//! this class reads and writes is held here (`Get_Collision_Type`, hidden, translucent, self-shadowed flags, the cached
//! bounding volumes, the transform). The `RenderInfoClass` is `super::rinfo::RenderInfoClass`.
//!
//! The renderer node a sub-mesh registers with (`ShdRendererNodeClass *Renderer`) is an id into
//! [`super::shdrenderer::ShdRendererClass`]'s node arena. What the nodes read of the mesh at flush time (the world
//! transform, whether the shadow map is being applied, the skin's HTree) is [`ShdMeshRenderState`], shared with the
//! nodes as `Arc<RwLock<..>>`.
//!
//! Not translated: `Init_From_Legacy_Mesh` needs the full `MeshClass` (this takes the pieces: see the function);
//! the legacy `Special_Render` is an empty function upstream; `Add_Dependencies_To_List` is the file-list
//! output of the tools (translated as returning the texture paths); `Clone` (the copy constructor) is
//! [`ShdMeshClass::clone_mesh`].

use super::handles::{LightEnvironmentHandle, TexProjectHandle};
use super::rinfo::{RenderInfoClass, RINFO_OVERRIDE_SHADOW_RENDERING};
use super::shdrenderer::ShdRendererClass;
use super::shdsubmesh::{ShdLoadFailed, ShdShaderInstance, ShdSubMeshClass, Ww3dErrorType, LegacyMeshModel};
use super::shddefmanager::{ShdDefLoad, ShdDefManagerClass};
use super::composite::SphereClass;
use crate::client::mesh_cast::MeshGeometry;
use crate::client::pick_drawable::RayTest;
use crate::formats::w3d::segline::AABox;
use crate::formats::w3d::w3d_file::*;
use crate::ww::chunkio::ChunkLoad;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use std::sync::{Arc, RwLock};

/// `Name("UnNamed")`.
pub const DEFAULT_NAME: &str = "UnNamed";

/// `ShdMeshClass::Legacy_Meshes_Fogged` (static): "Do old .w3d mesh files get fog turned on or off?". Set by the
/// owner (it is never read inside shdmesh.cpp).
pub const LEGACY_MESHES_FOGGED_DEFAULT: bool = false;

/// What the renderer nodes read of their mesh at flush time (`Mesh->Get_Transform()`, `Mesh->Get_Container()->
/// Get_HTree()`, `Applying_Shadow_Map`, `Texture_Projector`).
#[derive(Clone, Debug)]
pub struct ShdMeshRenderState {
    /// `Get_Transform()`.
    pub transform: Matrix3D,
    /// `Is_Applying_Shadow_Map()`.
    pub applying_shadow_map: bool,
    /// `Peek_Texture_Projector()`.
    pub texture_projector: Option<TexProjectHandle>,
    /// `Get_Container()->Get_HTree()` (a handle, `None` for no container).
    pub htree: Option<usize>,
}

/// `struct ShdSubMeshStruct`: the sub-mesh and its renderer node.
#[derive(Clone, Debug)]
pub struct ShdSubMeshStruct<S> {
    /// `Mesh`.
    pub mesh: Arc<RwLock<ShdSubMeshClass<S>>>,
    /// `Renderer`: the node id in the renderer's arena; `None` is NULL.
    pub renderer: Option<usize>,
}

/// `class ShdMeshClass` (shdmesh.h) with the `RenderObjClass` state it uses.
#[derive(Clone, Debug)]
pub struct ShdMeshClass<S> {
    /// `Name`.
    pub name: String,
    /// `SubMeshes`.
    pub sub_meshes: Vec<ShdSubMeshStruct<S>>,
    /// `LightEnvironment` (cached pointer to the light environment for this mesh).
    pub light_environment: Option<LightEnvironmentHandle>,
    /// `Applying_Shadow_Map` / `Texture_Projector` / transform / htree, shared with the renderer nodes.
    pub state: Arc<RwLock<ShdMeshRenderState>>,
    // --- RenderObjClass base state ---
    /// `Get_Collision_Type()`.
    pub collision_type: i32,
    /// `Is_Hidden()` (`Set_Hidden`).
    pub hidden: bool,
    /// `Is_Animation_Hidden()`.
    pub animation_hidden: bool,
    /// `Is_Translucent()`.
    pub translucent: bool,
    /// `Is_Self_Shadowed()`.
    pub self_shadowed: bool,
    /// `CachedBoundingSphere` (object space after `Load_W3D`).
    pub cached_bounding_sphere: SphereClass,
    /// `CachedBoundingBox`.
    pub cached_bounding_box: AABox,
}

impl<S> ShdMeshClass<S> {
    /// `ShdMeshClass::ShdMeshClass()`.
    pub fn new() -> Self {
        ShdMeshClass {
            name: DEFAULT_NAME.to_string(),
            sub_meshes: Vec::new(),
            light_environment: None,
            state: Arc::new(RwLock::new(ShdMeshRenderState {
                transform: Matrix3D::from_translation(&Vector3::new(0.0, 0.0, 0.0)),
                applying_shadow_map: false,
                texture_projector: None,
                htree: None,
            })),
            collision_type: 0,
            hidden: false,
            animation_hidden: false,
            translucent: false,
            self_shadowed: false,
            cached_bounding_sphere: SphereClass::default(),
            cached_bounding_box: AABox { center: Vector3::new(0.0, 0.0, 0.0), extent: Vector3::new(0.0, 0.0, 0.0) },
        }
    }

    /// `ShdMeshClass::ShdMeshClass(const ShdMeshClass &src)` / `Clone()`: the name, and the sub-meshes shared
    /// (`REF_PTR_SET`) with no renderer nodes; `LightEnvironment` NULL, `Applying_Shadow_Map` false. (The `Free()`
    /// the copy constructor calls first acts on an empty list.) The `RenderObjClass` copy (`RenderObjClass(src)`) is
    /// the owner's: collision type and flags are copied here as that base copy-constructor does.
    pub fn clone_mesh(&self) -> Self {
        let mut m = ShdMeshClass::new();
        m.name = self.name.clone();
        m.collision_type = self.collision_type;
        m.hidden = self.hidden;
        m.animation_hidden = self.animation_hidden;
        m.translucent = self.translucent;
        m.self_shadowed = self.self_shadowed;
        m.cached_bounding_sphere = self.cached_bounding_sphere;
        m.cached_bounding_box = self.cached_bounding_box;
        m.state.write().unwrap().transform = self.state.read().unwrap().transform;
        for sm in &self.sub_meshes {
            m.sub_meshes.push(ShdSubMeshStruct { mesh: Arc::clone(&sm.mesh), renderer: None });
        }
        m
    }

    /// `Class_ID()`: `CLASSID_SHDMESH` (rendobj.h).
    pub const CLASSID_SHDMESH: i32 = 0x0000_000E_u32 as i32;

    /// `Get_Name()`.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// `Set_Name(name)`.
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// `Free()`: releases the sub-meshes and deletes the renderer nodes (`delete SubMeshes[i].Renderer`: the owner
    /// removes the node from the renderer through `unregister`, handed each node id).
    pub fn free<F: FnMut(usize)>(&mut self, mut unregister: F) {
        for sm in self.sub_meshes.iter_mut() {
            // REF_PTR_RELEASE(SubMeshes[i].Mesh): the Arc is dropped with the vector below
            if let Some(node) = sm.renderer.take() {
                unregister(node);
            }
        }
        self.sub_meshes.clear();
    }

    /// `Get_Num_Polys()`.
    pub fn get_num_polys(&self) -> i32 {
        let mut count = 0;
        for sm in &self.sub_meshes {
            count += sm.mesh.read().unwrap().get_polygon_count();
        }
        count
    }

    /// `Get_Num_Vertices()`.
    pub fn get_num_vertices(&self) -> i32 {
        let mut count = 0;
        for sm in &self.sub_meshes {
            count += sm.mesh.read().unwrap().get_vertex_count();
        }
        count
    }

    /// `Get_Sub_Mesh_Count()`.
    pub fn get_sub_mesh_count(&self) -> usize {
        self.sub_meshes.len()
    }

    /// `Peek_Sub_Mesh(i)`.
    pub fn peek_sub_mesh(&self, i: usize) -> &Arc<RwLock<ShdSubMeshClass<S>>> {
        &self.sub_meshes[i].mesh
    }

    /// `Set_Lighting_Environment(light_env)`.
    pub fn set_lighting_environment(&mut self, light_env: Option<LightEnvironmentHandle>) {
        self.light_environment = light_env;
    }

    /// `Get_Lighting_Environment()`.
    pub fn get_lighting_environment(&self) -> Option<LightEnvironmentHandle> {
        self.light_environment
    }

    /// `Set_Is_Applying_Shadow_Map()`.
    pub fn set_is_applying_shadow_map(&self) {
        self.state.write().unwrap().applying_shadow_map = true;
    }

    /// `Unset_Is_Applying_Shadow_Map()`.
    pub fn unset_is_applying_shadow_map(&self) {
        self.state.write().unwrap().applying_shadow_map = false;
    }

    /// `Is_Applying_Shadow_Map()`.
    pub fn is_applying_shadow_map(&self) -> bool {
        self.state.read().unwrap().applying_shadow_map
    }

    /// `Peek_Texture_Projector()`.
    pub fn peek_texture_projector(&self) -> Option<TexProjectHandle> {
        self.state.read().unwrap().texture_projector
    }

    /// `Get_Transform()`.
    pub fn get_transform(&self) -> Matrix3D {
        self.state.read().unwrap().transform
    }

    /// `Is_Not_Hidden_At_All()` (rendobj.h): not hidden and not animation-hidden.
    pub fn is_not_hidden_at_all(&self) -> bool {
        !self.hidden && !self.animation_hidden
    }

    /// `Set_Collision_Type(type)`.
    pub fn set_collision_type(&mut self, t: i32) {
        self.collision_type = t;
    }
}

impl<S: ShdShaderInstance> ShdMeshClass<S> {
    /// `ShdMeshClass::Render(rinfo)` (shdmesh.cpp). `light_environment` is `rinfo.light_environment` already resolved
    /// by the caller (it is the handle in `rinfo`); `renderer` is `ShdRendererClass::Peek_Instance()`;
    /// `light_env` is the object that handle names (copied into the node at `Render`). `mesh_id` is this mesh's
    /// identity in the owner's tables (what `Register_Mesh(this, ..)` receives).
    pub fn render(&mut self, rinfo: &mut RenderInfoClass, light_env: Option<&super::lightenvironment::LightEnvironmentClass>, renderer: &mut ShdRendererClass<S>) {
        // WWPROFILE("ShdMeshClass::Render");
        if !self.is_not_hidden_at_all() {
            return;
        }
        // DX8_RECORD_MESH_RENDER();
        // TODO: Static sort lists
        self.set_lighting_environment(rinfo.light_environment);
        // const FrustumClass & frustum = rinfo.Camera.Get_Frustum();  (read only by the commented-out frustum test)

        // if rendering shadow remember camera info
        if (*rinfo.current_override_flags() & RINFO_OVERRIDE_SHADOW_RENDERING) != 0 {
            // is generating shadow map
            self.self_shadowed = true; // Set_Is_Self_Shadowed
            // set texture projector
            self.state.write().unwrap().texture_projector = rinfo.texture_projector;
            self.unset_is_applying_shadow_map();
        } else if self.self_shadowed {
            // is applying shadow map
            self.set_is_applying_shadow_map();
            self.self_shadowed = false; // Unset_Is_Self_Shadowed
        } else {
            self.unset_is_applying_shadow_map();
        }

        // TODO: What to do with SKINS?
        // upstream: `if (1) //CollisionMath::Overlap_Test(frustum, Get_Bounding_Box()) != OUTSIDE`
        // TODO: Override flags, decals and material passes (probably in the submesh rendering)
        for i in 0..self.sub_meshes.len() {
            if self.sub_meshes[i].renderer.is_none() {
                let node = renderer.register_mesh(Arc::clone(&self.state), Arc::clone(&self.sub_meshes[i].mesh));
                self.sub_meshes[i].renderer = Some(node);
            }
            let node = self.sub_meshes[i].renderer.unwrap();
            renderer.render_node(node, rinfo, light_env);
        }
        // TODO: RendererDebugger
    }

    /// `ShdMeshClass::Special_Render(rinfo)`: an empty function.
    pub fn special_render(&self) {}

    /// `ShdMeshClass::Cast_Ray(raytest)` (shdmesh.cpp). `raytest.collided_render_obj` is the owner's (`= this`):
    /// the return value says whether it hit.
    pub fn cast_ray(&self, raytest: &mut RayTest) -> bool {
        if (self.collision_type & raytest.collision_type) == 0 {
            return false;
        }
        if self.translucent && !raytest.check_translucent {
            return false;
        }
        if self.animation_hidden {
            return false;
        }
        if raytest.result.start_bad {
            return false;
        }
        let world = self.get_transform();
        // (the ALIGNED / ORIENTED handling is commented out upstream)
        let world_to_obj = world.get_orthogonal_inverse();
        for sm in &self.sub_meshes {
            // RayCollisionTestClass objray(raytest, world_to_obj): the same result (`Result` is a pointer)
            let mut objray = RayTest { ray: crate::client::pick_drawable::LineSeg::transformed(&raytest.ray, &world_to_obj), ..*raytest };
            let geometry = sm.mesh.read().unwrap().to_mesh_geometry();
            let hit = geometry.cast_ray(&mut objray);
            raytest.result = objray.result;
            if hit {
                // transform result back into original coordinate system
                raytest.result.normal = Matrix3D::rotate_vector_by(&world, &raytest.result.normal);
                if raytest.result.compute_contact_point {
                    raytest.result.contact_point = Matrix3D::transform_vector(&world, &raytest.result.contact_point);
                }
                return true;
            }
        }
        false
    }

    /// `Cast_AABox` / `Cast_OBBox` / `Intersect_AABox` / `Intersect_OBBox`: the loop over the sub-meshes with the
    /// collision-type guards. `hit(sub_mesh_index)` is the sub-mesh's own test with the transform already
    /// applied by the caller (`Cast_World_Space_AABox(boxtest, Get_Transform())`, or `Cast_OBBox(localtest)` /
    /// `Intersect_OBBox(local_test)` against the test moved by `Get_Orthogonal_Inverse`), which need the box-test
    /// classes the port does not have. Returns the first hit; `start_bad` is `boxtest.Result->StartBad` (false for
    /// the intersection tests, which do not read it).
    pub fn box_test_loop<F: FnMut(usize) -> bool>(&self, test_collision_type: i32, start_bad: bool, mut hit: F) -> bool {
        if (self.collision_type & test_collision_type) == 0 {
            return false;
        }
        if start_bad {
            return false;
        }
        for i in 0..self.sub_meshes.len() {
            if hit(i) {
                return true;
            }
        }
        false
    }

    /// `ShdMeshClass::Add_Dependencies_To_List(file_list, textures_only)`: each sub-mesh's shader's textures'
    /// full paths (`texture->Get_Full_Path()`), through `full_path(handle)`. (`RenderObjClass::Add_Dependencies_To_List`,
    /// the base call at the end, is the owner's.)
    pub fn add_dependencies_to_list<F: FnMut(usize) -> String>(&self, file_list: &mut Vec<String>, mut full_path: F) {
        // loop through sub meshes
        for sm in &self.sub_meshes {
            let m = sm.mesh.read().unwrap();
            // upstream dereferences Peek_Shader() unchecked
            let shd = m.peek_shader().unwrap();
            for tidx in 0..shd.texture_count() {
                if let Some(texture) = shd.peek_texture(tidx) {
                    file_list.push(full_path(texture));
                }
            }
        }
    }

    /// `ShdMeshClass::Update_Cached_Bounding_Volumes()`: the object-space sphere moved by the transform; the box
    /// from the object-space box transformed by the transform (`AABoxClass::Transform`). (`Validate_Cached_Bounding_
    /// Volumes` only clears the dirty flag.)
    pub fn update_cached_bounding_volumes(&mut self) {
        let mut sphere = self.get_obj_space_bounding_sphere();
        let tm = self.get_transform();
        sphere.center = Matrix3D::transform_vector(&tm, &sphere.center);
        self.cached_bounding_sphere = sphere;
        // (the ALIGNED/ORIENTED branch is commented out upstream)
        let mut b = self.get_obj_space_bounding_box();
        // AABoxClass::CachedBoundingBox.Transform(Get_Transform()) -> Matrix3D::Transform_Center_Extent_AABox
        let (c, e) = tm.transform_center_extent_aabox(&b.center, &b.extent);
        b.center = c;
        b.extent = e;
        self.cached_bounding_box = b;
    }

    /// `Get_Obj_Space_Bounding_Sphere(sphere)`: the first sub-mesh's sphere, merged (`sphere += tmp_s`,
    /// `SphereClass::Add_Sphere`) with the others'; no sub-meshes: centre 0, radius 1.
    pub fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        let mut sphere = SphereClass::default();
        if !self.sub_meshes.is_empty() {
            let (c, r) = self.sub_meshes[0].mesh.read().unwrap().get_bounding_sphere();
            sphere = SphereClass { center: c, radius: r };
            // If there are more than one submesh, merge all bounding spheres
            for i in 1..self.sub_meshes.len() {
                let (c, r) = self.sub_meshes[i].mesh.read().unwrap().get_bounding_sphere();
                sphere.add_sphere(&SphereClass { center: c, radius: r });
            }
        } else {
            sphere.center = Vector3::new(0.0, 0.0, 0.0);
            sphere.radius = 1.0;
        }
        sphere
    }

    /// `Get_Obj_Space_Bounding_Box(box)`: the first sub-mesh's box, `Add_Box`ed with the others'; no sub-meshes:
    /// `Init(Vector3(0,0,0), Vector3(1,1,1))` (centre 0, extent 1).
    pub fn get_obj_space_bounding_box(&self) -> AABox {
        if !self.sub_meshes.is_empty() {
            let (mn, mx) = self.sub_meshes[0].mesh.read().unwrap().get_bounding_box();
            let mut bx = AABox::init_min_max(&mn, &mx);
            // If there are more than one submesh, merge all bounding boxes
            for i in 1..self.sub_meshes.len() {
                let (mn, mx) = self.sub_meshes[i].mesh.read().unwrap().get_bounding_box();
                let tmp_b = AABox::init_min_max(&mn, &mx);
                // AABoxClass::Add_Box(const AABoxClass &): grow to enclose
                let mut mm = super::composite::MinMaxAABoxClass::default();
                mm.init(&bx);
                mm.add_box(&tmp_b);
                bx = AABox::init_min_max(&mm.min_corner, &mm.max_corner);
            }
            bx
        } else {
            AABox { center: Vector3::new(0.0, 0.0, 0.0), extent: Vector3::new(1.0, 1.0, 1.0) }
        }
    }

    /// `ShdMeshClass::Init_From_Legacy_Mesh(mesh)`: split a legacy mesh model into sub-meshes by shared state. The
    /// legacy mesh's name, collision type, hidden and translucent flags are passed in (`mesh->Get_Name()`,
    /// `Get_Collision_Type()`, `Is_Hidden()`, `Is_Translucent()`); `model` is `mesh->Peek_Model()`.
    #[allow(clippy::too_many_arguments)]
    pub fn init_from_legacy_mesh<M, F>(
        &mut self,
        name: &str,
        model: Option<&M>,
        collision_type: i32,
        hidden: bool,
        translucent: bool,
        sorting_enabled: bool,
        mut make_shader: F,
    ) where
        M: LegacyMeshModel,
        F: FnMut(super::shdlegacyw3d::Shd6LegacyW3DClass) -> S,
    {
        self.set_name(name);
        if let Some(model) = model {
            let mut first_poly = 0;
            let mut sub_mesh_count = 0;
            while first_poly < model.get_polygon_count() {
                sub_mesh_count += 1;
                let mut sub_mesh = ShdSubMeshClass::<S>::new();
                sub_mesh.init_from_legacy_mesh_model(model, first_poly, sorting_enabled, &mut make_shader);
                let visible = sub_mesh.get_visible_polygon_count();
                // SubMeshes.Resize(sub_mesh_count); SubMeshes[sub_mesh_count-1].Mesh = sub_mesh
                self.sub_meshes.push(ShdSubMeshStruct { mesh: Arc::new(RwLock::new(sub_mesh)), renderer: None });
                if visible == 0 {
                    break;
                }
                first_poly += visible;
            }
            let _ = sub_mesh_count;
        }
        // Pull interesting stuff out of the w3d attributes bits
        self.set_collision_type(collision_type);
        self.hidden = hidden;
        self.translucent = translucent;
    }

    /// `ShdMeshClass::Load_W3D(cload)` (shdmesh.cpp): the name chunk, the header, then `NumSubMeshes` sub-mesh chunks
    /// (a `W3D_CHUNK_SHDMESH_USER_TEXT` chunk among them is read and dropped: "todo"). `create_shader` is
    /// `shddef->Create()`.
    pub fn load_w3d<D, F>(&mut self, cload: &mut ChunkLoad<'_>, defs: &ShdDefManagerClass<D>, mut create_shader: F) -> Ww3dErrorType
    where
        D: ShdDefLoad,
        F: FnMut(D) -> S,
    {
        // Open the first chunk, it should be the shader mesh name
        cload.open_chunk()?;
        if cload.cur_chunk_id()? != W3D_CHUNK_SHDMESH_NAME {
            // WWDEBUG_SAY(("Invalid format shader mesh.\n"));
            return Ok(Err(ShdLoadFailed));
        }
        // read name
        let len = cload.cur_chunk_length()?;
        let mut buf = vec![0u8; len as usize];
        cload.read(&mut buf, len)?;
        cload.close_chunk()?;
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        self.set_name(&String::from_utf8_lossy(&buf[..end]));

        // open header
        cload.open_chunk()?;
        let mut hb = [0u8; 80];
        if cload.read_into(&mut hb)? != 80 {
            return Ok(Err(ShdLoadFailed));
        }
        let hdr = W3dShdMeshHeaderStruct::from_bytes(&hb);
        cload.close_chunk()?;

        // Process the header
        // Set Bounding Info
        let min = Vector3::new(hdr.box_min.x, hdr.box_min.y, hdr.box_min.z);
        let max = Vector3::new(hdr.box_max.x, hdr.box_max.y, hdr.box_max.z);
        self.cached_bounding_box = AABox::init_min_max(&min, &max);
        let cntr = Vector3::new(hdr.sph_center.x, hdr.sph_center.y, hdr.sph_center.z);
        self.cached_bounding_sphere.init(&cntr, hdr.sph_radius);

        // Flags (todo)
        // user text

        // next are the sub meshes
        self.free(|_| {});
        let n = hdr.num_sub_meshes as usize;
        let mut i = 0usize;
        while i < n {
            cload.open_chunk()?;
            if cload.cur_chunk_id()? == W3D_CHUNK_SHDMESH_USER_TEXT {
                // todo
                let len = cload.cur_chunk_length()?;
                let mut tmp = vec![0u8; len as usize];
                cload.read(&mut tmp, len)?;
            } else {
                let mut ssmesh = ShdSubMeshClass::<S>::new();
                // upstream ignores Load_W3D's result
                let _ = ssmesh.load_w3d(cload, defs, &mut create_shader)?;
                // assign each sub-mesh with a name in the format: <parentmesh>.<index>
                ssmesh.set_name(&format!("{}.{}", self.name, i));
                self.sub_meshes.push(ShdSubMeshStruct { mesh: Arc::new(RwLock::new(ssmesh)), renderer: None });
                i += 1;
            }
            cload.close_chunk()?;
        }

        // Pull interesting stuff out of the w3d attributes bits
        let col_bits = ((hdr.attributes & W3D_MESH_FLAG_COLLISION_TYPE_MASK) >> W3D_MESH_FLAG_COLLISION_TYPE_SHIFT) as i32;
        self.set_collision_type(col_bits << 1);
        self.hidden = (hdr.attributes & W3D_MESH_FLAG_HIDDEN) != 0;

        for sm in &self.sub_meshes {
            let shadow = (hdr.attributes & W3D_MESH_FLAG_CAST_SHADOW) == W3D_MESH_FLAG_CAST_SHADOW;
            sm.mesh.write().unwrap().set_flag(crate::formats::w3d::mesh::mesh_flags::CAST_SHADOW, shadow);
        }

        // Indicate whether this mesh is translucent.  The mesh is considered translucent
        // if sorting has been enabled (alpha blending on pass 0) or if pass0 contains alpha-test.
        // This flag is used to determine if the mesh can cast a geometric shadow.
        let _is_translucent = false; // (unused upstream)
        for sm in &self.sub_meshes {
            let m = sm.mesh.read().unwrap();
            if m.is_sorting() {
                self.translucent = true;
            }
            if let Some(shader) = m.peek_shader() {
                if !shader.is_opaque() {
                    self.translucent = true;
                }
            }
        }
        Ok(Ok(()))
    }
}

impl<S> Default for ShdMeshClass<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> ShdSubMeshClass<S> {
    /// The geometry `MeshGeometryClass::Cast_Ray` reads (no AABTree: `CullTree` is not read for sub-meshes, so the
    /// brute-force cast runs).
    pub fn to_mesh_geometry(&self) -> MeshGeometry {
        MeshGeometry {
            flags: self.geometry.flags,
            vertices: self.geometry.vertex.iter().map(|v| [v.x, v.y, v.z]).collect(),
            polys: self.geometry.poly.iter().map(|p| [p[0] as u16, p[1] as u16, p[2] as u16]).collect(),
            surface_types: self.geometry.poly_surface_type.clone(),
            cull_tree: None,
        }
    }
}
