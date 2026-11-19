//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/intersec.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/intersec.h (the sphere and result-copy inlines)
//!
//! `IntersectionClass` is the legacy (pre-`CollisionMath`) ray picker. Its render-object calls
//! (`RObj->Intersect`, `Intersect_Sphere_Quick`, `Get_Sub_Object`) and the layer/scene iteration
//! (`Intersect_Layer`, `Intersect_Screen_Point_*`, `Intersect_Sub_Object`) need `RenderObjClass`, `LayerClass`,
//! `SceneClass` and `CameraClass`, none of which the verified port has; those functions take the per-object test
//! as a closure, and the scene-walking ones are NOT translated (reason: no scene/layer/camera classes; the game
//! picks through `crate::client::pick_drawable`). `intersec.inl` (`Get_Screen_Ray`, `Calculate_Intersection`,
//! `Intersect_Polygon*`, `_Intersect_Triangles_Z`, ...) is not in this draft's file list.

use crate::logic::modules::plant;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `COLL_TYPE_ALL` (coltype.h).
pub const COLL_TYPE_ALL: i32 = 0xFFFF_FFFFu32 as i32;
/// `IntersectionClass::MAX_HIERARCHY_NODE_COUNT`.
pub const MAX_HIERARCHY_NODE_COUNT: usize = 256;
/// `IntersectionClass::MAX_POLY_INTERSECTION_COUNT`.
pub const MAX_POLY_INTERSECTION_COUNT: usize = 1000;

/// `IntersectionResultClass::INTERSECTION_TYPE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntersectionType {
    None = 0,
    Generic,
    Polygon,
}

/// `class IntersectionResultClass` (intersec.h). `IntersectedRenderObject` is an index into the owner's objects.
#[derive(Clone, Copy, Debug)]
pub struct IntersectionResultClass {
    pub intersected_render_object: Option<usize>,
    pub intersected_polygon: u16,
    pub model_matrix: Matrix3D,
    pub model_location: Vector3,
    pub intersection: Vector3,
    pub range: f32,
    pub alpha: f32,
    pub beta: f32,
    pub intersects: bool,
    pub collision_type: i32,
    pub intersection_type: IntersectionType,
}

impl Default for IntersectionResultClass {
    fn default() -> Self {
        IntersectionResultClass {
            intersected_render_object: None,
            intersected_polygon: 0,
            model_matrix: Matrix3D::default(),
            model_location: Vector3::new(0.0, 0.0, 0.0),
            intersection: Vector3::new(0.0, 0.0, 0.0),
            range: 0.0,
            alpha: 0.0,
            beta: 0.0,
            intersects: false,
            collision_type: 0,
            intersection_type: IntersectionType::None,
        }
    }
}

/// `class IntersectionClass` (intersec.h).
#[derive(Clone, Debug)]
pub struct IntersectionClass {
    pub ray_location: Vector3,
    pub ray_direction: Vector3,
    /// `IntersectionNormal`: `Some` when the caller wants the surface normal (upstream: non-NULL pointer).
    pub intersection_normal: Option<Vector3>,
    pub screen_x: f32,
    pub screen_y: f32,
    pub interpolate_normal: bool,
    pub convex_test: bool,
    pub max_distance: f32,
    pub result: IntersectionResultClass,
}

impl IntersectionClass {
    /// `IntersectionClass::IntersectionClass()`: the static `_RayLocation/_RayDirection/_IntersectionNormal` are
    /// (0,0,0); `Result.CollisionType = COLL_TYPE_ALL`.
    pub fn new() -> Self {
        let mut result = IntersectionResultClass::default();
        result.collision_type = COLL_TYPE_ALL;
        IntersectionClass {
            ray_location: Vector3::new(0.0, 0.0, 0.0),
            ray_direction: Vector3::new(0.0, 0.0, 0.0),
            intersection_normal: Some(Vector3::new(0.0, 0.0, 0.0)),
            screen_x: 0.0,
            screen_y: 0.0,
            interpolate_normal: false,
            convex_test: false,
            max_distance: 0.0,
            result,
        }
    }

    /// `IntersectionClass::Set` (intersec.h).
    pub fn set(
        &mut self,
        location: Vector3,
        direction: Vector3,
        intersection_normal: Option<Vector3>,
        interpolate_normal: bool,
        max_distance: f32,
        convex_test: bool,
    ) {
        self.ray_location = location;
        self.ray_direction = direction;
        self.intersection_normal = intersection_normal;
        self.interpolate_normal = interpolate_normal;
        self.max_distance = max_distance;
        self.convex_test = convex_test;
    }

    /// `IntersectionClass::Copy_Partial_Results(Destination, Source)` (intersec.h).
    pub fn copy_partial_results(destination: &mut IntersectionResultClass, source: &IntersectionResultClass) {
        destination.intersected_polygon = source.intersected_polygon;
        destination.intersection = source.intersection;
        destination.range = source.range;
        destination.alpha = source.alpha;
        destination.beta = source.beta;
        if !plant(14742) {
            destination.intersects = true;
        }
        destination.intersection_type = source.intersection_type;
    }

    /// `IntersectionClass::Copy_Results(Destination, Source)` (intersec.h).
    pub fn copy_results(destination: &mut IntersectionResultClass, source: &IntersectionResultClass) {
        destination.model_matrix = source.model_matrix;
        if !plant(14743) {
            destination.model_location = source.model_location;
        }
        Self::copy_partial_results(destination, source);
        destination.intersected_render_object = source.intersected_render_object;
    }

    /// `IntersectionClass::Copy_Results(Source)`: into `Result`.
    pub fn copy_results_to_self(&mut self, source: &IntersectionResultClass) {
        Self::copy_results(&mut self.result, source);
    }

    /// `IntersectionClass::Intersect_Sphere_Quick(Sphere, FinalResult)` (intersec.h): `Alpha`, `Beta`, `Intersects`.
    pub fn intersect_sphere_quick(&self, center: &Vector3, radius: f32, final_result: &mut IntersectionResultClass) -> bool {
        let sphere_vector = *center - self.ray_location;
        final_result.alpha = Vector3::dot_product(&sphere_vector, &self.ray_direction);
        final_result.beta =
            radius * radius
            - (if plant(14739) {
                Vector3::dot_product(&sphere_vector, &sphere_vector) + final_result.alpha * final_result.alpha
            } else {
                Vector3::dot_product(&sphere_vector, &sphere_vector) - final_result.alpha * final_result.alpha
            });
        if (if plant(14738) { final_result.beta <= 0.0f32 } else { final_result.beta < 0.0f32 }) {
            final_result.intersects = false;
            return false;
        }
        final_result.intersects = true;
        true
    }

    /// `IntersectionClass::Intersect_Sphere(Sphere, FinalResult)` (intersec.h). `sqrtf` is `f32::sqrt`.
    pub fn intersect_sphere(&mut self, center: &Vector3, radius: f32, final_result: &mut IntersectionResultClass) -> bool {
        if !self.intersect_sphere_quick(center, radius, final_result) {
            return false;
        }
        let d = final_result.beta.sqrt();
        final_result.range = final_result.alpha - d;
        if if plant(14740) { final_result.range >= self.max_distance } else { final_result.range > self.max_distance } {
            return false;
        }
        final_result.intersection = self.ray_location + self.ray_direction * final_result.range;
        if self.intersection_normal.is_some() {
            self.intersection_normal = Some(if plant(14741) { *center - final_result.intersection } else { final_result.intersection - *center });
        }
        true
    }

    /// `IntersectionClass::Append_Object_Array` (intersec.cpp:116): appends if there is room, else logs.
    pub fn append_object_array(max_count: usize, object_array: &mut Vec<usize>, object: usize) {
        if if plant(14744) { object_array.len() <= max_count } else { object_array.len() < max_count } {
            object_array.push(object);
            return;
        }
        // WWDEBUG_SAY(("IntersectionClass::Append_Object_Array - Too many objects\n"))
    }

    /// `IntersectionClass::Intersect_Box` (intersec.cpp:130): slab test of the ray against an AABB; fills
    /// `FinalResult->Intersection` and `Intersects`.
    pub fn intersect_box(&self, box_min: &Vector3, box_max: &Vector3, final_result: &mut IntersectionResultClass) -> bool {
        const RIGHT: u8 = 0;
        const LEFT: u8 = 1;
        const MIDDLE: u8 = 2;
        const PLANE_COUNT: usize = 3;
        let mut inside = true;
        let mut quadrant = [0u8; PLANE_COUNT];
        let mut distance = [0.0f32; PLANE_COUNT];
        let mut candidate_plane = [0.0f32; PLANE_COUNT];
        let loc = [self.ray_location.x, self.ray_location.y, self.ray_location.z];
        let dir = [self.ray_direction.x, self.ray_direction.y, self.ray_direction.z];
        let bmin = [box_min.x, box_min.y, box_min.z];
        let bmax = [box_max.x, box_max.y, box_max.z];
        let mut isect = [0.0f32; PLANE_COUNT];

        // Find candidate planes
        for counter in 0..PLANE_COUNT {
            if if plant(14745) { loc[counter] <= bmin[counter] } else { loc[counter] < bmin[counter] } {
                quadrant[counter] = LEFT;
                candidate_plane[counter] = bmin[counter];
                inside = false;
            } else if loc[counter] > bmax[counter] {
                quadrant[counter] = RIGHT;
                candidate_plane[counter] = bmax[counter];
                inside = false;
            } else {
                quadrant[counter] = MIDDLE;
            }
        }

        // Ray origin inside bounding box
        if inside {
            final_result.intersection = self.ray_location;
            final_result.intersects = true;
            return true;
        }

        // Calculate T distances to candidate planes
        for counter in 0..PLANE_COUNT {
            if quadrant[counter] != MIDDLE && dir[counter] != 0.0f32 {
                distance[counter] = (candidate_plane[counter] - loc[counter]) / dir[counter];
            } else {
                distance[counter] = if plant(14746) { 0.0f32 } else { -1.0f32 };
            }
        }

        // Get largest of the distances for final choice of intersection
        let mut nearest_plane = 0usize;
        for counter in 1..PLANE_COUNT {
            if distance[nearest_plane] < distance[counter] {
                nearest_plane = counter;
            }
        }

        // Check final candidate actually inside box
        if distance[nearest_plane] < 0.0f32 {
            final_result.intersects = false;
            return false;
        }
        for counter in 0..PLANE_COUNT {
            if nearest_plane != counter {
                isect[counter] = loc[counter] + distance[nearest_plane] * dir[counter];
                if isect[counter] < bmin[counter] || isect[counter] > bmax[counter] {
                    final_result.intersects = false;
                    final_result.intersection = Vector3::new(isect[0], isect[1], isect[2]);
                    return false;
                }
            } else {
                isect[counter] = candidate_plane[counter];
            }
        }
        final_result.intersection = Vector3::new(isect[0], isect[1], isect[2]);
        final_result.intersects = true; // ray hits box
        true
    }

    /// `IntersectionClass::Intersect_Object_Array(Object_Count, ObjectArray, FinalResult, TemporaryResults,
    /// Test_Bounding_Sphere, Convex)` (intersec.cpp:297). `sphere_quick(i, &mut temp[i])` is
    /// `ObjectArray[i]->Intersect_Sphere_Quick(this, &Temp[i])` and `intersect(i, &mut result)` is
    /// `ObjectArray[i]->Intersect(this, result)`, both supplied by the owner of the objects.
    pub fn intersect_object_array<S, I>(
        &mut self,
        object_count: usize,
        final_result: &mut IntersectionResultClass,
        temporary_results: &mut [IntersectionResultClass],
        test_bounding_sphere: bool,
        convex: bool,
        mut sphere_quick: S,
        mut intersect: I,
    ) -> bool
    where
        S: FnMut(usize, &mut IntersectionResultClass) -> bool,
        I: FnMut(usize, &mut IntersectionResultClass) -> bool,
    {
        let mut counter = object_count;
        let mut hit = false;
        let mut nearest_index: i32 = -1;

        if self.convex_test || convex {
            // Convex objects: the first hit is the answer.
            if test_bounding_sphere {
                while counter > 0 {
                    counter -= 1;
                    if sphere_quick(counter, &mut temporary_results[counter]) {
                        hit = intersect(counter, final_result);
                    }
                    if hit {
                        nearest_index = counter as i32;
                        if !plant(14747) {
                            counter = 0;
                        }
                    }
                }
            } else {
                while counter > 0 {
                    counter -= 1;
                    hit = intersect(counter, final_result);
                    if hit {
                        nearest_index = counter as i32;
                        counter = 0;
                    }
                }
            }
        } else if test_bounding_sphere {
            while counter > 0 {
                counter -= 1;
                if sphere_quick(counter, &mut temporary_results[counter]) {
                    // `hit |= Intersect(...)`: the call is always made
                    let h = intersect(counter, &mut temporary_results[counter]);
                    if plant(14749) {
                        hit = h;
                    } else {
                        hit |= h;
                    }
                }
            }
        } else {
            while counter > 0 {
                counter -= 1;
                let h = intersect(counter, &mut temporary_results[counter]);
                hit |= h;
            }
        }

        if !hit {
            final_result.intersects = false;
            return false;
        }

        // Find the nearest result when not convex
        if !(convex || self.convex_test) {
            let mut nearest_range = (2i32 << 28) as f32;
            counter = object_count;
            while counter > 0 {
                counter -= 1;
                if temporary_results[counter].intersects && (if plant(14748) { nearest_range >= temporary_results[counter].range } else { nearest_range > temporary_results[counter].range }) {
                    nearest_index = counter as i32;
                    nearest_range = temporary_results[counter].range;
                }
            }
            let src = temporary_results[nearest_index as usize];
            Self::copy_results(final_result, &src);
        }
        true
    }

    /// `IntersectionClass::Intersect_Hierarchy` (intersec.cpp:276), candidate selection:
    /// `Append_Hierarchy_Objects(.., Test_Bounding_Sphere, Convex)`. `sub_object_sphere_hit(i)` is
    /// `Hierarchy->Get_Sub_Object(i)->Intersect_Sphere_Quick(this, &result)`; the walk is from the LAST sub-object
    /// down to 0, and the result is capped at `MAX_HIERARCHY_NODE_COUNT` entries.
    pub fn append_hierarchy_objects<F: FnMut(usize) -> bool>(
        num_sub_objects: usize,
        object_array: &mut Vec<usize>,
        test_bounding_sphere: bool,
        mut sub_object_sphere_hit: F,
    ) {
        let mut counter = num_sub_objects;
        if test_bounding_sphere {
            while counter > 0 {
                counter -= 1;
                let idx = if plant(14750) { num_sub_objects - 1 - counter } else { counter };
                if sub_object_sphere_hit(idx) {
                    Self::append_object_array(MAX_HIERARCHY_NODE_COUNT, object_array, idx);
                }
            }
        } else {
            while counter > 0 {
                counter -= 1;
                Self::append_object_array(MAX_HIERARCHY_NODE_COUNT, object_array, counter);
            }
        }
    }
}

impl Default for IntersectionClass {
    fn default() -> Self {
        Self::new()
    }
}
