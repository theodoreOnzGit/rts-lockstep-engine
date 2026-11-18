//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/collect.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/collect.h
//!
//! `CollectionClass` members no earlier pass translated: `Add_Sub_Object`, `Snap_Point_Count`, `Get_Snap_Point`,
//! `Get_Proxy`, `Get_Proxy_Count`. The verified `formats::w3d::collect` loads the definition; [`CollectionState`] holds the
//! runtime lists these read. Sub-objects are opaque host handles; `Add_Ref` / `Set_Container` / `Set_Transform` /
//! `Notify_Added` are the [`CollectionHost`] contract. The proxy type is the `hlod` draft's [`ProxyClass`]
//! (proxy.h: a name and a transform), shared with `HLodClass`.

use crate::drafts::sweep_ww3d::hlod::ProxyClass;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// The lists `CollectionClass` keeps.
#[derive(Clone, Debug, Default)]
pub struct CollectionState {
    /// `SubObjects`.
    pub sub_objects: Vec<usize>,
    /// `SnapPoints` (`NULL` when `None`).
    pub snap_points: Option<Vec<Vector3>>,
    /// `ProxyList`.
    pub proxy_list: Vec<ProxyClass>,
    /// `Transform`.
    pub transform: Matrix3D,
}

/// The calls out of `Add_Sub_Object`.
pub trait CollectionHost {
    /// `subobj->Add_Ref()`.
    fn sub_object_add_ref(&mut self, subobj: usize);
    /// `subobj->Set_Container(this)`.
    fn sub_object_set_container(&mut self, subobj: usize);
    /// `subobj->Set_Transform(Transform)`.
    fn sub_object_set_transform(&mut self, subobj: usize, transform: &Matrix3D);
    /// `Update_Sub_Object_Bits()`.
    fn update_sub_object_bits(&mut self);
    /// `Update_Obj_Space_Bounding_Volumes()`.
    fn update_obj_space_bounding_volumes(&mut self);
    /// `Is_In_Scene()`.
    fn is_in_scene(&self) -> bool;
    /// `subobj->Notify_Added(Scene)`.
    fn sub_object_notify_added(&mut self, subobj: usize);
}

impl CollectionState {
    /// `CollectionClass::Add_Sub_Object(subobj)` (collect.cpp:504): takes a reference, parents the object to the
    /// collection and gives it the collection's transform, appends it (`SubObjects.Add`, returning its index... upstream
    /// returns the `DynamicVectorClass::Add` result, `true`/`false` as an int: here 1 on success), refreshes the bits and
    /// the bounding volumes, and tells the object it was added to the scene if the collection is in one.
    pub fn add_sub_object<H: CollectionHost>(&mut self, host: &mut H, subobj: usize) -> i32 {
        host.sub_object_add_ref(subobj);
        host.sub_object_set_container(subobj);
        host.sub_object_set_transform(subobj, &self.transform);
        self.sub_objects.push(subobj);
        let res = 1;
        host.update_sub_object_bits();
        host.update_obj_space_bounding_volumes();
        if host.is_in_scene() {
            host.sub_object_notify_added(subobj);
        }
        res
    }

    /// `CollectionClass::Snap_Point_Count()` (collect.cpp:720): the snap-point count, 0 with none.
    pub fn snap_point_count(&self) -> i32 {
        match &self.snap_points {
            Some(sp) => sp.len() as i32,
            None => 0,
        }
    }

    /// `CollectionClass::Get_Snap_Point(index, set)` (collect.cpp:745): the point, or (0, 0, 0) with no snap points. The
    /// index is not range-checked upstream (`(*SnapPoints)[index]`); out of range panics here.
    pub fn get_snap_point(&self, index: i32, set: &mut Vector3) {
        match &self.snap_points {
            Some(sp) => *set = sp[index as usize],
            None => {
                set.x = 0.0;
                set.y = 0.0;
                set.z = 0.0;
            }
        }
    }

    /// `CollectionClass::Get_Proxy(index, proxy)` (collect.cpp:883): copies the proxy when `0 <= index < count`.
    pub fn get_proxy(&self, index: i32, proxy: &mut ProxyClass) -> bool {
        let mut retval = false;

        if index >= 0 && (index as usize) < self.proxy_list.len() {
            // Return the proxy information to the caller
            *proxy = self.proxy_list[index as usize].clone();
            retval = true;
        }

        retval
    }

    /// `CollectionClass::Get_Proxy_Count()` (collect.cpp:912): `ProxyList.Count()`.
    pub fn get_proxy_count(&self) -> i32 {
        self.proxy_list.len() as i32
    }
}
