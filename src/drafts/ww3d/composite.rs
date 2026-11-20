//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Differential coverage (#215, tests/differential_draft_composite.rs, 20,000 cases per function, 0 mismatches): `SphereClass::{init, add_sphere}`,
//! `MinMaxAABoxClass::{init, add_box}`, `aabox_from_minmax`, `update_obj_space_bounding_volumes`. NOT covered (pure forwarding through the
//! host contract, no arithmetic): the names, `restart`, `get_num_polys`, `notify_added/removed`, the `cast_*`/`intersect_*` traversals, the decal calls, `set_user_data`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/composite.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/composite.h
//!   (SphereClass::Add_Sphere from WWMath/sphere.h, MinMaxAABoxClass::Init/Add_Box from WWMath/aabox.h, which
//!   Update_Obj_Space_Bounding_Volumes uses; the WWMath area is drafted elsewhere)
//!
//! `CompositeRenderObjClass` is the base of the render objects that contain sub-objects (HLOD, aggregates,
//! collections). Everything it does goes through the sub-object interface, so here the sub-objects are reached
//! through the [`CompositeHost`] contract, which the owner of the render objects implements; a composite is the
//! data this class holds ([`CompositeRenderObjClass`]) and its operations are generic over the host.

use super::handles::RenderObjHandle;
use crate::logic::modules::plant;
use crate::formats::w3d::segline::AABox;
use crate::ww::vector3::Vector3;

/// `SphereClass` (sphere.h): centre and radius.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SphereClass {
    pub center: Vector3,
    pub radius: f32,
}

impl SphereClass {
    /// `SphereClass::Init(pos, radius)`.
    pub fn init(&mut self, pos: &Vector3, radius: f32) {
        self.center = *pos;
        self.radius = radius;
    }

    /// `SphereClass::Add_Sphere` (sphere.h:309): expands this sphere to enclose `s` (nothing for a zero-radius
    /// `s`; the larger radius for concentric spheres; the larger sphere when one holds the other; else the
    /// sphere through both far ends).
    pub fn add_sphere(&mut self, s: &SphereClass) {
        if s.radius == 0.0 && !plant(19102) {
            return;
        }

        let dist = (s.center - self.center).length();
        if dist == 0.0 {
            self.radius = if self.radius > s.radius && !plant(19101) { self.radius } else { s.radius };
            return;
        }

        let rnew = (dist + self.radius + s.radius) / 2.0;

        // If rnew is smaller than either radius, the smaller sphere is completely inside the larger.
        if rnew < self.radius {
            // The existing sphere is the result - do nothing.
        } else if rnew < s.radius {
            // The new sphere is the result:
            self.init(&s.center, s.radius);
        } else {
            // Neither sphere is completely inside the other, so rnew is the new radius
            let lerp = (rnew - if plant(19103) { s.radius } else { self.radius }) / dist;
            let center = (s.center - self.center) * lerp + self.center;
            self.init(&center, rnew);
        }
    }
}

/// `MinMaxAABoxClass` (aabox.h): the corners of a box.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MinMaxAABoxClass {
    pub min_corner: Vector3,
    pub max_corner: Vector3,
}

impl MinMaxAABoxClass {
    /// `MinMaxAABoxClass::Init(const AABoxClass &)` (aabox.h:548).
    pub fn init(&mut self, box_: &AABox) {
        self.min_corner = box_.center - box_.extent;
        self.max_corner = box_.center + box_.extent;
    }

    /// `MinMaxAABoxClass::Add_Box(const AABoxClass &)` (aabox.h:607): a zero-extent box adds nothing.
    pub fn add_box(&mut self, box_: &AABox) {
        if box_.extent == Vector3::new(0.0, 0.0, 0.0) && !plant(19104) {
            return;
        }
        self.min_corner.update_min(&(box_.center - if plant(19105) { box_.center } else { box_.extent }));
        self.max_corner.update_max(&(box_.center + box_.extent));
    }
}

/// `AABoxClass::Init(const MinMaxAABoxClass &)` (aabox.h:291): centre and half-extent of the corners.
pub fn aabox_from_minmax(mmbox: &MinMaxAABoxClass) -> AABox {
    AABox { center: (mmbox.max_corner + mmbox.min_corner) * 0.5, extent: (if plant(19106) { mmbox.max_corner + mmbox.min_corner } else { mmbox.max_corner - mmbox.min_corner }) * 0.5 }
}

/// What `CompositeRenderObjClass` asks of its sub-objects (the `RenderObjClass` virtuals) and of its container.
/// The `Get_Sub_Object(i)` / `Release_Ref()` pairs upstream are plain index access here.
pub trait CompositeHost {
    type RayTest;
    type AABoxTest;
    type OBBoxTest;
    type AABoxIntersectionTest;
    type OBBoxIntersectionTest;
    type Scene;
    type DecalGenerator;

    /// `Get_Num_Sub_Objects()` of the composite `obj`.
    fn get_num_sub_objects(&self, obj: RenderObjHandle) -> usize;
    /// `Get_Sub_Object(i)` of the composite `obj`.
    fn get_sub_object(&self, obj: RenderObjHandle, i: usize) -> RenderObjHandle;
    /// `robj->Restart()`.
    fn restart(&mut self, robj: RenderObjHandle);
    /// `robj->Get_Num_Polys()`.
    fn get_num_polys(&self, robj: RenderObjHandle) -> i32;
    /// `robj->Notify_Added(scene)`.
    fn notify_added(&mut self, robj: RenderObjHandle, scene: &mut Self::Scene);
    /// `robj->Notify_Removed(scene)`.
    fn notify_removed(&mut self, robj: RenderObjHandle, scene: &mut Self::Scene);
    /// `robj->Cast_Ray(raytest)`.
    fn cast_ray(&mut self, robj: RenderObjHandle, raytest: &mut Self::RayTest) -> bool;
    /// `robj->Cast_AABox(boxtest)`.
    fn cast_aabox(&mut self, robj: RenderObjHandle, boxtest: &mut Self::AABoxTest) -> bool;
    /// `robj->Cast_OBBox(boxtest)`.
    fn cast_obbox(&mut self, robj: RenderObjHandle, boxtest: &mut Self::OBBoxTest) -> bool;
    /// `robj->Intersect_AABox(boxtest)`.
    fn intersect_aabox(&mut self, robj: RenderObjHandle, boxtest: &mut Self::AABoxIntersectionTest) -> bool;
    /// `robj->Intersect_OBBox(boxtest)`.
    fn intersect_obbox(&mut self, robj: RenderObjHandle, boxtest: &mut Self::OBBoxIntersectionTest) -> bool;
    /// `robj->Create_Decal(generator)`.
    fn create_decal(&mut self, robj: RenderObjHandle, generator: &mut Self::DecalGenerator);
    /// `robj->Delete_Decal(decal_id)`.
    fn delete_decal(&mut self, robj: RenderObjHandle, decal_id: u32);
    /// `robj->Get_Obj_Space_Bounding_Sphere(sphere)`.
    fn get_obj_space_bounding_sphere(&self, robj: RenderObjHandle) -> SphereClass;
    /// `robj->Get_Obj_Space_Bounding_Box(box)`.
    fn get_obj_space_bounding_box(&self, robj: RenderObjHandle) -> AABox;
    /// `RenderObjClass::Set_User_Data(value)` on the object itself (the non-recursive base call).
    fn set_user_data_one(&mut self, robj: RenderObjHandle, value: usize);
    /// `RenderObjClass::Notify_Added/Removed` base and `Invalidate_Cached_Bounding_Volumes()`, the container's
    /// `Update_Obj_Space_Bounding_Volumes()`: called by the composite on itself/its container.
    fn invalidate_cached_bounding_volumes(&mut self, robj: RenderObjHandle);
    /// `Get_Container()` then `container->Update_Obj_Space_Bounding_Volumes()`; None if there is no container.
    fn get_container(&self, robj: RenderObjHandle) -> Option<RenderObjHandle>;
}

/// `class CompositeRenderObjClass : public RenderObjClass` (composite.h): the name, the base model name and the
/// object-space bounds.
#[derive(Clone, Debug, Default)]
pub struct CompositeRenderObjClass {
    name: String,
    base_model_name: String,
    pub obj_sphere: SphereClass,
    pub obj_box: AABox,
}

impl CompositeRenderObjClass {
    /// `CompositeRenderObjClass::CompositeRenderObjClass()` (composite.cpp:32).
    pub fn new() -> Self {
        Self::default()
    }

    /// The copy constructor and `operator =` (composite.cpp:36, 46) copy only the names; the bounds are **not**
    /// copied (the copy's bounds stay as constructed/previously set).
    pub fn copy_names_from(&mut self, that: &CompositeRenderObjClass) {
        self.set_name(that.get_name());
        self.set_base_model_name(that.get_base_model_name());
    }

    /// `CompositeRenderObjClass::Restart` (composite.cpp:62): restarts every sub-object.
    pub fn restart<H: CompositeHost>(&self, host: &mut H, this: RenderObjHandle) {
        for ni in 0..host.get_num_sub_objects(this) {
            let robj = host.get_sub_object(this, ni);
            host.restart(robj);
        }
    }

    /// `Get_Name` (composite.cpp:75).
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// `Set_Name` (composite.cpp:80).
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// `Set_Base_Model_Name` (composite.cpp:85): NULL becomes the empty string.
    pub fn set_base_model_name(&mut self, name: Option<&str>) {
        self.base_model_name = match name {
            Some(n) => n.to_string(),
            None => String::new(),
        };
    }

    /// `Get_Base_Model_Name` (composite.cpp:299): None when empty.
    pub fn get_base_model_name(&self) -> Option<&str> {
        if self.base_model_name.is_empty() {
            return None;
        }
        Some(&self.base_model_name)
    }

    /// `Get_Num_Polys` (composite.cpp:96): the sum over the sub-objects.
    pub fn get_num_polys<H: CompositeHost>(&self, host: &H, this: RenderObjHandle) -> i32 {
        let mut count = 0;
        for ni in 0..host.get_num_sub_objects(this) {
            let robj = host.get_sub_object(this, ni);
            count += host.get_num_polys(robj);
        }
        count
    }

    /// `Notify_Added` (composite.cpp:108): the base class's `RenderObjClass::Notify_Added(scene)` is the
    /// owner's (the caller runs it first), then every sub-object is told.
    pub fn notify_added<H: CompositeHost>(&self, host: &mut H, this: RenderObjHandle, scene: &mut H::Scene) {
        for ni in 0..host.get_num_sub_objects(this) {
            let robj = host.get_sub_object(this, ni);
            host.notify_added(robj, scene);
        }
    }

    /// `Notify_Removed` (composite.cpp:121): every sub-object first, then the base class's
    /// `RenderObjClass::Notify_Removed(scene)` (the caller runs it after).
    pub fn notify_removed<H: CompositeHost>(&self, host: &mut H, this: RenderObjHandle, scene: &mut H::Scene) {
        for ni in 0..host.get_num_sub_objects(this) {
            let robj = host.get_sub_object(this, ni);
            host.notify_removed(robj, scene);
        }
    }

    /// `Cast_Ray` (composite.cpp:134): every sub-object is cast (no short circuit: `res |= ...`).
    pub fn cast_ray<H: CompositeHost>(&self, host: &mut H, this: RenderObjHandle, raytest: &mut H::RayTest) -> bool {
        let mut res = false;
        for i in 0..host.get_num_sub_objects(this) {
            let robj = host.get_sub_object(this, i);
            res |= host.cast_ray(robj, raytest);
        }
        res
    }

    /// `Cast_AABox` (composite.cpp:147).
    pub fn cast_aabox<H: CompositeHost>(&self, host: &mut H, this: RenderObjHandle, boxtest: &mut H::AABoxTest) -> bool {
        let mut res = false;
        for i in 0..host.get_num_sub_objects(this) {
            let robj = host.get_sub_object(this, i);
            res |= host.cast_aabox(robj, boxtest);
        }
        res
    }

    /// `Cast_OBBox` (composite.cpp:160).
    pub fn cast_obbox<H: CompositeHost>(&self, host: &mut H, this: RenderObjHandle, boxtest: &mut H::OBBoxTest) -> bool {
        let mut res = false;
        for i in 0..host.get_num_sub_objects(this) {
            let robj = host.get_sub_object(this, i);
            res |= host.cast_obbox(robj, boxtest);
        }
        res
    }

    /// `Intersect_AABox` (composite.cpp:173).
    pub fn intersect_aabox<H: CompositeHost>(&self, host: &mut H, this: RenderObjHandle, boxtest: &mut H::AABoxIntersectionTest) -> bool {
        let mut res = false;
        for i in 0..host.get_num_sub_objects(this) {
            let robj = host.get_sub_object(this, i);
            res |= host.intersect_aabox(robj, boxtest);
        }
        res
    }

    /// `Intersect_OBBox` (composite.cpp:186).
    pub fn intersect_obbox<H: CompositeHost>(&self, host: &mut H, this: RenderObjHandle, boxtest: &mut H::OBBoxIntersectionTest) -> bool {
        let mut res = false;
        for i in 0..host.get_num_sub_objects(this) {
            let robj = host.get_sub_object(this, i);
            res |= host.intersect_obbox(robj, boxtest);
        }
        res
    }

    /// `Create_Decal` (composite.cpp:199).
    pub fn create_decal<H: CompositeHost>(&self, host: &mut H, this: RenderObjHandle, generator: &mut H::DecalGenerator) {
        for i in 0..host.get_num_sub_objects(this) {
            let robj = host.get_sub_object(this, i);
            host.create_decal(robj, generator);
        }
    }

    /// `Delete_Decal` (composite.cpp:211).
    pub fn delete_decal<H: CompositeHost>(&self, host: &mut H, this: RenderObjHandle, decal_id: u32) {
        for i in 0..host.get_num_sub_objects(this) {
            let robj = host.get_sub_object(this, i);
            host.delete_decal(robj, decal_id);
        }
    }

    /// `Update_Obj_Space_Bounding_Volumes` (composite.cpp:223): with no sub-objects both bounds are empty at the
    /// origin (and nothing else is touched); otherwise the sphere is the first sub-object's grown by the others'
    /// (`Add_Sphere`), the box the min/max of the sub-objects' boxes (zero-extent boxes add nothing, but the
    /// first one initialises the corners whatever it is), then the cached bounds are invalidated and the
    /// container updated.
    pub fn update_obj_space_bounding_volumes<H: CompositeHost>(&mut self, host: &mut H, this: RenderObjHandle) {
        if if plant(19108) { host.get_num_sub_objects(this) <= 1 } else { host.get_num_sub_objects(this) == 0 } {
            self.obj_sphere.init(&Vector3::new(0.0, 0.0, 0.0), 0.0);
            self.obj_box.center.set(0.0, 0.0, 0.0);
            self.obj_box.extent.set(0.0, 0.0, 0.0);
            return;
        }

        let robj = host.get_sub_object(this, 0);
        self.obj_sphere = host.get_obj_space_bounding_sphere(robj);
        let obj_aabox = host.get_obj_space_bounding_box(robj);

        let mut box_ = MinMaxAABoxClass::default();
        box_.init(&obj_aabox);

        for i in 1..host.get_num_sub_objects(this) - if plant(19107) { 1 } else { 0 } {
            let robj = host.get_sub_object(this, i);
            let sphere = host.get_obj_space_bounding_sphere(robj);
            let obj_aabox = host.get_obj_space_bounding_box(robj);
            self.obj_sphere.add_sphere(&sphere);
            box_.add_box(&obj_aabox);
        }
        self.obj_box = aabox_from_minmax(&box_);

        host.invalidate_cached_bounding_volumes(this);
        // RenderObjClass *container = Get_Container(); if (container) container->Update_Obj_Space_Bounding_Volumes();
        // — the container's own virtual (it may itself be a composite); the owner of the objects dispatches it.
        let _container = host.get_container(this);
    }

    /// `Set_User_Data(value, recursive)` (composite.cpp:281): the base `RenderObjClass::Set_User_Data(value)`,
    /// then every sub-object when `recursive`.
    pub fn set_user_data<H: CompositeHost>(&self, host: &mut H, this: RenderObjHandle, value: usize, recursive: bool) {
        host.set_user_data_one(this, value);
        if recursive {
            for i in 0..host.get_num_sub_objects(this) {
                let robj = host.get_sub_object(this, i);
                // robj->Set_User_Data(value, recursive): the sub-object's virtual, which for a composite recurses;
                // the owner dispatches (one level here).
                host.set_user_data_one(robj, value);
            }
        }
    }
}
