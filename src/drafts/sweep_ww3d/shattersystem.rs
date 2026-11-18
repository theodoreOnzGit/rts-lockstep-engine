//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/shattersystem.cpp
//!
//! `BSPClass::Set_Plane_From_Transform` and the `ShatterSystem` fragment accessors the first pass
//! (`drafts::ww3d::shattersystem`) left out. The BSP node is that draft's [`BSPClass`]; the fragments (`MeshFragments`, a
//! `DynamicVectorClass<RenderObjClass*>`) are opaque host handles (`None` is NULL). In the shipped game the pattern list is
//! empty (`ShatterSystem::Init` loads none), so these are not reached; see the first pass's note.

use crate::drafts::ww3d::shattersystem::BSPClass;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `BSPClass::Set_Plane_From_Transform(tm)` (shattersystem.cpp:740): `Plane.Set(tm.Get_Z_Vector(), tm.Get_Translation())`,
/// i.e. `N = z axis` and `D = dot(N, translation)`.
pub fn set_plane_from_transform(bsp: &mut BSPClass, tm: &Matrix3D) {
    let normal = tm.get_z_vector();
    let point = tm.get_translation();
    bsp.plane.n = normal;
    bsp.plane.d = Vector3::dot_product(&normal, &point);
}

/// `ShatterSystem`'s `MeshFragments`.
#[derive(Clone, Debug, Default)]
pub struct ShatterFragments {
    pub mesh_fragments: Vec<Option<usize>>,
}

impl ShatterFragments {
    /// `ShatterSystem::Get_Fragment_Count()` (shattersystem.cpp:1047): `MeshFragments.Count()`.
    pub fn get_fragment_count(&self) -> i32 {
        self.mesh_fragments.len() as i32
    }

    /// `ShatterSystem::Get_Fragment(fragment_index)` (shattersystem.cpp:1052): the fragment with an `Add_Ref` when not NULL
    /// (the reference is the host's). Unchecked index upstream; out of range panics here.
    pub fn get_fragment(&self, fragment_index: i32) -> Option<usize> {
        self.mesh_fragments[fragment_index as usize]
    }

    /// `ShatterSystem::Peek_Fragment(fragment_index)` (shattersystem.cpp:1060): the fragment without a reference.
    pub fn peek_fragment(&self, fragment_index: i32) -> Option<usize> {
        self.mesh_fragments[fragment_index as usize]
    }
}
