//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/decalsys.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/decalsys.h
//!
//! `DecalGeneratorClass` derives from `ProjectorClass` ([`super::projector`]) and holds it as `projector`.
//! `DecalSystemClass` is overridden for game-specific behaviour (virtual `Lock_Decal_Generator`,
//! `Unlock_Decal_Generator`, `Decal_Mesh_Destroyed`, `Generate_Decal_Id`): the base and the one derived class
//! that decalsys.cpp itself defines, `MultiFixedPoolDecalSystemClass`, are given as the enum
//! [`DecalSystemClass`], and the dispatch is a `match`.

use super::handles::RenderObjHandle;
use super::matpass::MaterialPassClass;
use super::projector::ProjectorClass;
use crate::client::tactical_view::Matrix4x4;
use crate::ww::matrix3d::Matrix3D;
use std::sync::atomic::{AtomicU32, Ordering};

/// `DecalSystemClass::DecalIDGenerator` (decalsys.cpp:35), a class static initialised to 0.
static DECAL_ID_GENERATOR: AtomicU32 = AtomicU32::new(0);

/// `DecalSystemClass::Generate_Unique_Global_Decal_Id` (decalsys.cpp:70): `return DecalIDGenerator++`.
pub fn generate_unique_global_decal_id() -> u32 {
    DECAL_ID_GENERATOR.fetch_add(1, Ordering::SeqCst)
}

/// What the decal system asks of the render objects (`MeshClass::Delete_Decal`).
pub trait DecalHost {
    /// `it.Get_Obj()->Delete_Decal(decal_id)`.
    fn delete_decal(&mut self, mesh: RenderObjHandle, decal_id: u32);
}

/// `class DecalGeneratorClass : public ProjectorClass` (decalsys.h).
#[derive(Clone, Debug)]
pub struct DecalGeneratorClass {
    pub projector: ProjectorClass,
    /// `System`: the owning decal system, by index in the owner's list (`DecalSystemClass *`).
    pub system: usize,
    decal_id: u32,
    backface_val: f32,
    apply_to_translucent_meshes: bool,
    material: MaterialPassClass,
    /// `NonRefRenderObjListClass MeshList`.
    mesh_list: Vec<RenderObjHandle>,
}

impl DecalGeneratorClass {
    /// `DecalGeneratorClass::DecalGeneratorClass(id, system)` (decalsys.cpp:78): backface threshold 0, not applied
    /// to translucent meshes, a new material pass.
    fn new(id: u32, system: usize) -> Self {
        DecalGeneratorClass {
            projector: ProjectorClass::new(),
            system,
            decal_id: id,
            backface_val: 0.0,
            apply_to_translucent_meshes: false,
            material: MaterialPassClass::new(),
            mesh_list: Vec::new(),
        }
    }

    /// `DecalGeneratorClass::Add_Mesh` (decalsys.cpp:98): the caller guarantees `CLASSID_MESH` (WWASSERT).
    pub fn add_mesh(&mut self, mesh: RenderObjHandle) {
        self.mesh_list.push(mesh);
    }

    /// `DecalGeneratorClass::Get_Mesh_List` (decalsys.cpp:104).
    pub fn get_mesh_list(&self) -> &Vec<RenderObjHandle> {
        &self.mesh_list
    }

    /// `Get_Decal_ID` (decalsys.h).
    pub fn get_decal_id(&self) -> u32 {
        self.decal_id
    }

    /// `Set_Backface_Threshhold` (decalsys.h).
    pub fn set_backface_threshhold(&mut self, val: f32) {
        self.backface_val = val;
    }

    /// `Get_Backface_Threshhold` (decalsys.h).
    pub fn get_backface_threshhold(&self) -> f32 {
        self.backface_val
    }

    /// `Apply_To_Translucent_Meshes` (decalsys.h).
    pub fn apply_to_translucent_meshes(&mut self, onoff: bool) {
        self.apply_to_translucent_meshes = onoff;
    }

    /// `Is_Applied_To_Translucent_Meshes` (decalsys.h).
    pub fn is_applied_to_translucent_meshes(&self) -> bool {
        self.apply_to_translucent_meshes
    }

    /// `Get_Material` (decalsys.h): the material pass to modify.
    pub fn get_material(&mut self) -> &mut MaterialPassClass {
        &mut self.material
    }

    /// `DecalGeneratorClass::Set_Mesh_Transform` (decalsys.cpp:110): `Mobj-texture = Projection * Mwrld-texture *
    /// Mobj-wrld`, plugged into the mapper with the texture size (64 if the pass has no texture; upstream asserts
    /// one is there). Only when texturing is enabled (`WW3D::Is_Texturing_Enabled()`, the `texturing_enabled`
    /// argument); `texture_width` is `TextureClass::Get_Width` of the pass's stage-0 texture.
    pub fn set_mesh_transform<F: Fn(usize) -> u32>(&mut self, mesh_transform: &Matrix3D, texturing_enabled: bool, texture_width: F) {
        let world_to_texture = self.projector.transform.get_orthogonal_inverse();
        let tmp = Matrix3D::multiply(&world_to_texture, mesh_transform);
        let mesh_to_texture = Matrix4x4::multiply_matrix_matrix3d(&self.projector.projection, &tmp);

        // Plug the matrix and texture size into the mapper
        if texturing_enabled {
            let mut texsize: f32 = 64.0;
            let tex = self.material.get_texture(0);
            debug_assert!(tex.is_some());
            if let Some(t) = tex {
                texsize = texture_width(t) as f32;
            }
            self.projector.mapper.set_texture_transform(&mesh_to_texture, texsize);
        }
    }
}

/// `MultiFixedPoolDecalSystemClass::encode_decal_id` (decalsys.h).
pub fn encode_decal_id(pool_id: u32, slot_id: u32) -> u32 {
    (slot_id & 0xFFFF) | (pool_id << 16)
}

/// `MultiFixedPoolDecalSystemClass::decode_decal_id` (decalsys.h): `(pool_id, slot_id)`.
pub fn decode_decal_id(decal_id: u32) -> (u32, u32) {
    (decal_id >> 16, decal_id & 0xFFFF)
}

/// `MultiFixedPoolDecalSystemClass::LogicalDecalClass` (decalsys.h).
#[derive(Clone, Debug, Default)]
pub struct LogicalDecalClass {
    pub mesh_list: Vec<RenderObjHandle>,
}

impl LogicalDecalClass {
    /// `LogicalDecalClass::Set` (decalsys.cpp:336): clears the decal, then copies the generator's mesh list.
    pub fn set<H: DecalHost>(&mut self, host: &mut H, generator: &DecalGeneratorClass) {
        self.clear(host, generator.get_decal_id());
        for m in generator.get_mesh_list() {
            self.mesh_list.push(*m);
        }
    }

    /// `LogicalDecalClass::Clear` (decalsys.cpp:350): deletes the decal from every mesh, then empties the list.
    pub fn clear<H: DecalHost>(&mut self, host: &mut H, decal_id: u32) {
        for m in &self.mesh_list {
            host.delete_decal(*m, decal_id);
        }
        self.mesh_list.clear();
    }
}

/// `MultiFixedPoolDecalSystemClass::LogicalDecalPoolClass` (decalsys.h): `Array` and `Size`.
#[derive(Clone, Debug, Default)]
pub struct LogicalDecalPoolClass {
    pub array: Vec<LogicalDecalClass>,
}

impl LogicalDecalPoolClass {
    /// `LogicalDecalPoolClass::Initialize` (decalsys.cpp:381): replaces the array with `size` empty decals
    /// (`assert(Size)`).
    pub fn initialize(&mut self, size: u32) {
        assert!(size != 0);
        self.array = vec![LogicalDecalClass::default(); size as usize];
    }

    /// `Size`.
    pub fn size(&self) -> u32 {
        self.array.len() as u32
    }
}

/// `class DecalSystemClass` and its derived `MultiFixedPoolDecalSystemClass` (decalsys.h).
#[derive(Clone, Debug)]
pub enum DecalSystemClass {
    /// `DecalSystemClass` itself: ids from the global generator.
    Base,
    /// `MultiFixedPoolDecalSystemClass`: several fixed-size pools; the decal id is pool and slot encoded.
    MultiFixedPool(MultiFixedPoolDecalSystemClass),
}

/// `class MultiFixedPoolDecalSystemClass : public DecalSystemClass` (decalsys.h).
#[derive(Clone, Debug)]
pub struct MultiFixedPoolDecalSystemClass {
    /// "These should be set before calling Lock_Decal_Generator()".
    pub generator_pool_id: u32,
    /// See [`Self::generator_pool_id`].
    pub generator_slot_id: u32,
    pools: Vec<LogicalDecalPoolClass>,
}

impl MultiFixedPoolDecalSystemClass {
    /// `MultiFixedPoolDecalSystemClass(num_pools, pool_sizes)` (decalsys.cpp:131): each size must be non-zero.
    pub fn new(pool_sizes: &[u32]) -> Self {
        let mut pools = vec![LogicalDecalPoolClass::default(); pool_sizes.len()];
        for i in 0..pool_sizes.len() {
            assert!(pool_sizes[i] != 0);
            pools[i].initialize(pool_sizes[i]);
        }
        MultiFixedPoolDecalSystemClass { generator_pool_id: 0, generator_slot_id: 0, pools }
    }

    /// Copy constructor (decalsys.cpp:149): same pool sizes with *empty* pools, same generator ids.
    pub fn clone_empty(that: &MultiFixedPoolDecalSystemClass) -> Self {
        let mut pools = vec![LogicalDecalPoolClass::default(); that.pools.len()];
        for i in 0..pools.len() {
            pools[i].initialize(that.pools[i].size());
        }
        MultiFixedPoolDecalSystemClass { generator_pool_id: that.generator_pool_id, generator_slot_id: that.generator_slot_id, pools }
    }

    /// `Generate_Decal_Id` (decalsys.h): `encode_decal_id(Generator_PoolID, Generator_SlotID)`.
    pub fn generate_decal_id(&self) -> u32 {
        encode_decal_id(self.generator_pool_id, self.generator_slot_id)
    }

    /// `find_logical_decal(pool_id, slot_id)` (decalsys.cpp:262). Upstream asserts the ids are in range and then
    /// clamps with `MIN(id, Count)` (an off-by-one: the clamp still allows one past the end); an out-of-range id is
    /// a panic here, as the out-of-bounds read is undefined there.
    pub fn find_logical_decal(&mut self, pool_id: u32, slot_id: u32) -> &mut LogicalDecalClass {
        assert!((pool_id as usize) < self.pools.len());
        let pool = &mut self.pools[pool_id as usize];
        assert!((slot_id as usize) < pool.array.len());
        &mut pool.array[slot_id as usize]
    }

    /// `find_logical_decal(decal_id)` (decalsys.cpp:277).
    pub fn find_logical_decal_by_id(&mut self, decal_id: u32) -> &mut LogicalDecalClass {
        let (pool_id, slot_id) = decode_decal_id(decal_id);
        self.find_logical_decal(pool_id, slot_id)
    }

    /// `MultiFixedPoolDecalSystemClass::Clear_Decal_Slot` (decalsys.cpp:233).
    pub fn clear_decal_slot<H: DecalHost>(&mut self, host: &mut H, pool_id: u32, slot_id: u32) {
        self.find_logical_decal(pool_id, slot_id).clear(host, encode_decal_id(pool_id, slot_id));
    }

    /// `MultiFixedPoolDecalSystemClass::Clear_Pool` (decalsys.cpp:239).
    pub fn clear_pool<H: DecalHost>(&mut self, host: &mut H, pool_id: u32) {
        let pool = &mut self.pools[pool_id as usize];
        let pool_size = pool.size();
        for slot_id in 0..pool_size {
            pool.array[slot_id as usize].clear(host, encode_decal_id(pool_id, slot_id));
        }
    }

    /// `MultiFixedPoolDecalSystemClass::Clear_All_Decals` (decalsys.cpp:249).
    pub fn clear_all_decals<H: DecalHost>(&mut self, host: &mut H) {
        for pool_id in 0..self.pools.len() as u32 {
            let pool = &mut self.pools[pool_id as usize];
            let pool_size = pool.size();
            for slot_id in 0..pool_size {
                pool.array[slot_id as usize].clear(host, encode_decal_id(pool_id, slot_id));
            }
        }
    }
}

impl DecalSystemClass {
    /// `Generate_Decal_Id` (decalsys.h): the base reroutes to the global generator, the multi-pool system encodes
    /// its pool and slot.
    fn generate_decal_id(&self) -> u32 {
        match self {
            DecalSystemClass::Base => generate_unique_global_decal_id(),
            DecalSystemClass::MultiFixedPool(m) => m.generate_decal_id(),
        }
    }

    /// `DecalSystemClass::Lock_Decal_Generator` (decalsys.cpp:52) and the multi-pool override (decalsys.cpp:201):
    /// the override first clears the slot its ids point at. `system_index` is this system's index in the owner.
    pub fn lock_decal_generator<H: DecalHost>(&mut self, host: &mut H, system_index: usize) -> DecalGeneratorClass {
        if let DecalSystemClass::MultiFixedPool(m) = self {
            let (p, s) = (m.generator_pool_id, m.generator_slot_id);
            m.clear_decal_slot(host, p, s);
        }
        DecalGeneratorClass::new(self.generate_decal_id(), system_index)
    }

    /// `DecalSystemClass::Unlock_Decal_Generator` (decalsys.cpp:60) and the multi-pool override (decalsys.cpp:208):
    /// the override records the generator's meshes in its logical decal first; the generator is then deleted
    /// (consumed).
    pub fn unlock_decal_generator<H: DecalHost>(&mut self, host: &mut H, generator: DecalGeneratorClass) {
        if let DecalSystemClass::MultiFixedPool(m) = self {
            m.find_logical_decal_by_id(generator.get_decal_id()).set(host, &generator);
        }
        drop(generator);
    }

    /// `Decal_Mesh_Destroyed` (decalsys.h: empty for the base; decalsys.cpp:215 for the multi-pool): removes the
    /// destroyed decal mesh's parent mesh (`mesh->Peek_Parent()`, the argument) from the logical decal.
    pub fn decal_mesh_destroyed(&mut self, decal_id: u32, parent: RenderObjHandle) {
        if let DecalSystemClass::MultiFixedPool(m) = self {
            let l = m.find_logical_decal_by_id(decal_id);
            if let Some(pos) = l.mesh_list.iter().position(|x| *x == parent) {
                l.mesh_list.remove(pos);
            }
        }
    }
}
