//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/intersec.cpp
//!
//! The `IntersectionClass` members the first pass (`drafts::ww3d::intersec`) left out because they walk render objects,
//! layers and cameras: `Intersect_Screen_Point_RenderObject`, `Intersect_RenderObject`, `Intersect_Screen_Point_Layer`,
//! `Intersect_Hierarchy_Sphere_Quick` and `Intersect_Hierarchy_Sphere`. The picker is
//! [`IntersectionClass`] of that draft; render objects are opaque host handles (`usize`) and the virtual calls
//! (`RObj->Intersect`, `Intersect_Sphere_Quick`, `Intersect_Sphere`, `Get_Sub_Object`, `Get_Screen_Ray` from
//! `intersec.inl`, the layer's camera) are the [`IntersectHost`] contract. The game picks through `client::pick_drawable`;
//! this legacy path is kept for completeness.

use crate::drafts::ww3d::intersec::{IntersectionClass, IntersectionResultClass};

/// `ViewportClass` as `Intersect_Screen_Point_Layer` reads it: `Min` and `Max` corners.
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewportView {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

/// What the intersection members call. `layer` is the host's `LayerClass` handle.
pub trait IntersectHost {
    /// `IntersectionClass::Get_Screen_Ray(screen_x, screen_y, Layer)` (intersec.inl:81): sets `ScreenX/ScreenY`, the ray
    /// location (the camera position) and direction on the picker.
    fn get_screen_ray(&mut self, picker: &mut IntersectionClass, screen_x: f32, screen_y: f32, layer: usize);
    /// `Layer.Camera->Get_Viewport()`.
    fn layer_camera_viewport(&self, layer: usize) -> ViewportView;
    /// `Layer.Camera->Get_Depth()`.
    fn layer_camera_depth(&self, layer: usize) -> f32;
    /// `IntersectionClass::Intersect_Layer(Layer, Test_All)` (intersec.cpp:122).
    fn intersect_layer(&mut self, picker: &mut IntersectionClass, layer: usize, test_all: bool) -> bool;
    /// `RObj->Intersect(this, FinalResult)`.
    fn robj_intersect(&mut self, robj: usize, picker: &mut IntersectionClass, final_result: &mut IntersectionResultClass) -> bool;
    /// `Hierarchy->Get_Num_Sub_Objects()`.
    fn get_num_sub_objects(&self, hierarchy: usize) -> i32;
    /// `Hierarchy->Get_Sub_Object(index)` followed by the immediate `Release_Ref()` (the hierarchy owns it).
    fn get_sub_object(&mut self, hierarchy: usize, index: i32) -> usize;
    /// `obj->Intersect_Sphere_Quick(this, FinalResult)`.
    fn robj_intersect_sphere_quick(&mut self, obj: usize, picker: &mut IntersectionClass, final_result: &mut IntersectionResultClass) -> bool;
    /// `obj->Intersect_Sphere(this, FinalResult)`.
    fn robj_intersect_sphere(&mut self, obj: usize, picker: &mut IntersectionClass, final_result: &mut IntersectionResultClass) -> bool;
}

/// `IntersectionClass::Intersect_Screen_Point_RenderObject(screen_x, screen_y, Layer, RObj, FinalResult)`
/// (intersec.cpp:55): builds the screen ray for the layer's camera, then intersects the one object. `final_result` of
/// `None` is upstream's NULL: the picker's own `Result`.
pub fn intersect_screen_point_render_object<H: IntersectHost>(
    picker: &mut IntersectionClass,
    host: &mut H,
    screen_x: f32,
    screen_y: f32,
    layer: usize,
    robj: usize,
    final_result: Option<&mut IntersectionResultClass>,
) -> bool {
    host.get_screen_ray(picker, screen_x, screen_y, layer);
    intersect_render_object(picker, host, robj, final_result)
}

/// `IntersectionClass::Intersect_RenderObject(RObj, FinalResult)` (intersec.cpp:61): `if (FinalResult == 0) FinalResult =
/// &Result; return RObj->Intersect(this, FinalResult);`.
pub fn intersect_render_object<H: IntersectHost>(
    picker: &mut IntersectionClass,
    host: &mut H,
    robj: usize,
    final_result: Option<&mut IntersectionResultClass>,
) -> bool {
    match final_result {
        Some(r) => host.robj_intersect(robj, picker, r),
        None => {
            // FinalResult = &Result: the callee writes into the picker's own result
            let mut own = picker.result;
            let hit = host.robj_intersect(robj, picker, &mut own);
            picker.result = own;
            hit
        }
    }
}

/// `IntersectionClass::Intersect_Screen_Point_Layer(screen_x, screen_y, Layer)` (intersec.cpp:102): marks `Result` as not
/// intersecting, rejects a point outside the layer's viewport ("first, do a test to make sure the screen coords are within
/// the rendering area for this layer"), sets `Result.Range` to the camera depth, builds the ray and intersects the layer
/// (`Test_All = false`).
pub fn intersect_screen_point_layer<H: IntersectHost>(
    picker: &mut IntersectionClass,
    host: &mut H,
    screen_x: f32,
    screen_y: f32,
    layer: usize,
) -> bool {
    // mark this object as not intersecting yet
    picker.result.intersects = false;

    let v = host.layer_camera_viewport(layer);

    if (screen_x < v.min_x) || (screen_x > v.max_x) || (screen_y < v.min_y) || (screen_y > v.max_y) {
        return false;
    }

    picker.result.range = host.layer_camera_depth(layer); // scene->depth * scene->zstop;

    // get the ray for these screen coordinates
    host.get_screen_ray(picker, screen_x, screen_y, layer);

    host.intersect_layer(picker, layer, false)
}

/// `IntersectionClass::Intersect_Hierarchy_Sphere_Quick(Hierarchy, FinalResult)` (intersec.cpp:252): "simply returns true
/// if a ray hits the bounding sphere of any node in a hierarchy. note: Result will only contain range, not the
/// intersection point/normal." Sub-objects are tried last to first (`while(counter--)`).
pub fn intersect_hierarchy_sphere_quick<H: IntersectHost>(
    picker: &mut IntersectionClass,
    host: &mut H,
    hierarchy: usize,
    final_result: &mut IntersectionResultClass,
) -> bool {
    let mut counter = host.get_num_sub_objects(hierarchy);
    while counter > 0 {
        counter -= 1;
        // you already own a reference to this object indirectly..
        let obj = host.get_sub_object(hierarchy, counter);
        if host.robj_intersect_sphere_quick(obj, picker, final_result) {
            return true;
        }
    }
    false
}

/// `IntersectionClass::Intersect_Hierarchy_Sphere(Hierarchy, FinalResult)` (intersec.cpp:267): "returns true if a ray hits
/// the bounding sphere of any node in a hierarchy. note: Result will contain range and the intersection point/normal."
pub fn intersect_hierarchy_sphere<H: IntersectHost>(
    picker: &mut IntersectionClass,
    host: &mut H,
    hierarchy: usize,
    final_result: &mut IntersectionResultClass,
) -> bool {
    let mut counter = host.get_num_sub_objects(hierarchy);
    while counter > 0 {
        counter -= 1;
        // you already own a reference to this object indirectly..
        let obj = host.get_sub_object(hierarchy, counter);
        if host.robj_intersect_sphere(obj, picker, final_result) {
            return true;
        }
    }
    false
}
