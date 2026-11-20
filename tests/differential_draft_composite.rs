// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d6) of the draft `src/drafts/ww3d/composite.rs` (`SphereClass::{init, add_sphere}`,
//! `MinMaxAABoxClass::{init, add_box}`, `aabox_from_minmax`, `CompositeRenderObjClass::update_obj_space_bounding_volumes`) against the real
//! `WWMath/sphere.h` and `aabox.h` and `WW3D2/composite.cpp`'s `Update_Obj_Space_Bounding_Volumes` cut VERBATIM (build.rs `drafts_ww3d6`,
//! differential/wrap_draft_w6_comp.cpp). Stand-ins: the class around the function and the sub-objects (scripted spheres and boxes).
//!
//! # Method
//!
//! 20,000 cases per operation (fixed seed, `DRAFT_CASES` overrides): pairs of spheres (zero radius, concentric, nested, tangent, overlapping, apart; centres and radii
//! on a half-integer grid so that equalities occur, or random), lists of 1 to 6 boxes (some with zero extent, or with a single zero component) through
//! `MinMaxAABoxClass`, and composites of 0 to 6 sub-objects. Compared as bits: the resulting sphere, min/max corners, centre and extent, and the number of
//! `Invalidate_Cached_Bounding_Volumes` calls. Not covered: `Set_User_Data`, the name and base-model-name accessors, the traversal functions (`Cast_Ray` ...,
//! pure forwarding through the host contract, no arithmetic) and the container recursion (the host dispatches).
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! 0 mismatches.
//!
//! # Results (2026-11-20)
//!
//! **0 mismatches**: 20,000 cases each of `Add_Sphere`, `MinMaxAABoxClass::Init/Add_Box` + `AABoxClass::Init`, and `Update_Obj_Space_Bounding_Volumes`.
//! **Prediction held.**
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching comparisons over the three checks)
//!   19101: 1,518; 19102: 8,176; 19103: 24,369; 19104: 8,729; 19105: 28,689; 19106: 37,098; 19107: 12,489; 19108: 2,800
//!
//! **8 of 8 caught.**

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::ww3d::composite::*;
use rts_lockstep_engine::drafts::ww3d::handles::RenderObjHandle;
use rts_lockstep_engine::formats::w3d::segline::AABox;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_w6_comp_run(input: *const u32, out: *mut u32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn b(f: f32) -> u32 {
    f.to_bits()
}

struct Host {
    spheres: Vec<SphereClass>,
    boxes: Vec<AABox>,
    invalidates: u32,
}

impl CompositeHost for Host {
    type RayTest = ();
    type AABoxTest = ();
    type OBBoxTest = ();
    type AABoxIntersectionTest = ();
    type OBBoxIntersectionTest = ();
    type Scene = ();
    type DecalGenerator = ();
    fn get_num_sub_objects(&self, _obj: RenderObjHandle) -> usize {
        self.spheres.len()
    }
    fn get_sub_object(&self, _obj: RenderObjHandle, i: usize) -> RenderObjHandle {
        i
    }
    fn restart(&mut self, _robj: RenderObjHandle) {}
    fn get_num_polys(&self, _robj: RenderObjHandle) -> i32 {
        0
    }
    fn notify_added(&mut self, _robj: RenderObjHandle, _scene: &mut ()) {}
    fn notify_removed(&mut self, _robj: RenderObjHandle, _scene: &mut ()) {}
    fn cast_ray(&mut self, _robj: RenderObjHandle, _raytest: &mut ()) -> bool {
        false
    }
    fn cast_aabox(&mut self, _robj: RenderObjHandle, _boxtest: &mut ()) -> bool {
        false
    }
    fn cast_obbox(&mut self, _robj: RenderObjHandle, _boxtest: &mut ()) -> bool {
        false
    }
    fn intersect_aabox(&mut self, _robj: RenderObjHandle, _boxtest: &mut ()) -> bool {
        false
    }
    fn intersect_obbox(&mut self, _robj: RenderObjHandle, _boxtest: &mut ()) -> bool {
        false
    }
    fn create_decal(&mut self, _robj: RenderObjHandle, _generator: &mut ()) {}
    fn delete_decal(&mut self, _robj: RenderObjHandle, _decal_id: u32) {}
    fn get_obj_space_bounding_sphere(&self, robj: RenderObjHandle) -> SphereClass {
        self.spheres[robj]
    }
    fn get_obj_space_bounding_box(&self, robj: RenderObjHandle) -> AABox {
        self.boxes[robj]
    }
    fn set_user_data_one(&mut self, _robj: RenderObjHandle, _value: usize) {}
    fn invalidate_cached_bounding_volumes(&mut self, _robj: RenderObjHandle) {
        self.invalidates += 1;
    }
    fn get_container(&self, _robj: RenderObjHandle) -> Option<RenderObjHandle> {
        None
    }
}

fn grid(g: &mut Gen) -> f32 {
    (g.below(17) as f32 - 8.0) * 0.5
}

fn vec3(g: &mut Gen, grid_values: bool) -> Vector3 {
    if grid_values {
        Vector3::new(grid(g), grid(g), grid(g))
    } else {
        Vector3::new(g.uniform(-9.0, 9.0), g.uniform(-9.0, 9.0), g.uniform(-9.0, 9.0))
    }
}

fn sphere(g: &mut Gen, grid_values: bool) -> SphereClass {
    let radius = match g.below(6) {
        0 => 0.0,
        _ if grid_values => g.below(9) as f32 * 0.5,
        _ => g.uniform(0.0, 6.0),
    };
    SphereClass { center: vec3(g, grid_values), radius }
}

fn aabox(g: &mut Gen, grid_values: bool) -> AABox {
    let mut extent = if grid_values { Vector3::new(g.below(5) as f32 * 0.5, g.below(5) as f32 * 0.5, g.below(5) as f32 * 0.5) } else { Vector3::new(g.uniform(0.0, 4.0), g.uniform(0.0, 4.0), g.uniform(0.0, 4.0)) };
    match g.below(6) {
        0 => extent = Vector3::new(0.0, 0.0, 0.0),
        1 => extent.y = 0.0,
        _ => {}
    }
    AABox { center: vec3(g, grid_values), extent }
}

fn sw(s: &SphereClass) -> Vec<u32> {
    vec![b(s.center.x), b(s.center.y), b(s.center.z), b(s.radius)]
}

fn bw(x: &AABox) -> Vec<u32> {
    vec![b(x.center.x), b(x.center.y), b(x.center.z), b(x.extent.x), b(x.extent.y), b(x.extent.z)]
}

#[test]
fn composite_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_61_05);
    let names = ["SphereClass::Add_Sphere", "MinMaxAABoxClass::Init/Add_Box + AABoxClass::Init", "Update_Obj_Space_Bounding_Volumes"];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    let mut out = vec![0u32; 256];
    for case in 0..support_sweep::cases() {
        let grid_values = g.below(2) == 0;
        // --- Add_Sphere
        {
            let mut a = sphere(&mut g, grid_values);
            let s = match g.below(5) {
                0 => SphereClass { center: a.center, radius: g.below(9) as f32 * 0.5 }, // concentric
                1 => SphereClass { center: a.center + Vector3::new(1.0, 0.0, 0.0), radius: (a.radius - 1.0).max(0.0) }, // internally tangent or nested
                _ => sphere(&mut g, grid_values),
            };
            let mut input = vec![0u32];
            input.extend(sw(&a));
            input.extend(sw(&s));
            let n = unsafe { ref_w6_comp_run(input.as_ptr(), out.as_mut_ptr()) } as usize;
            let before = a;
            a.add_sphere(&s);
            c[0].eq(sw(&a), out[..n].to_vec(), || format!("case {case} {before:?} + {s:?}"));
        }
        // --- boxes
        {
            let n = 1 + g.below(6) as usize;
            let boxes: Vec<AABox> = (0..n).map(|_| aabox(&mut g, grid_values)).collect();
            let mut input = vec![1u32, n as u32];
            for x in &boxes {
                input.extend(bw(x));
            }
            let k = unsafe { ref_w6_comp_run(input.as_ptr(), out.as_mut_ptr()) } as usize;
            let mut mm = MinMaxAABoxClass::default();
            mm.init(&boxes[0]);
            for x in &boxes[1..] {
                mm.add_box(x);
            }
            let res = aabox_from_minmax(&mm);
            let mut r = vec![b(mm.min_corner.x), b(mm.min_corner.y), b(mm.min_corner.z), b(mm.max_corner.x), b(mm.max_corner.y), b(mm.max_corner.z)];
            r.extend(bw(&res));
            c[1].eq(r, out[..k].to_vec(), || format!("case {case} boxes {boxes:?}"));
        }
        // --- composite
        {
            let n = g.below(7) as usize;
            let spheres: Vec<SphereClass> = (0..n).map(|_| sphere(&mut g, grid_values)).collect();
            let boxes: Vec<AABox> = (0..n).map(|_| aabox(&mut g, grid_values)).collect();
            let mut input = vec![2u32, n as u32];
            for i in 0..n {
                input.extend(sw(&spheres[i]));
                input.extend(bw(&boxes[i]));
            }
            let k = unsafe { ref_w6_comp_run(input.as_ptr(), out.as_mut_ptr()) } as usize;
            let mut host = Host { spheres, boxes, invalidates: 0 };
            let mut comp = CompositeRenderObjClass::new();
            // what the object held before, as in the wrapper (the empty case overwrites it)
            comp.obj_sphere = SphereClass { center: Vector3::new(7.0, 8.0, 9.0), radius: 5.0 };
            comp.obj_box = AABox { center: Vector3::new(1.0, 2.0, 3.0), extent: Vector3::new(4.0, 5.0, 6.0) };
            comp.update_obj_space_bounding_volumes(&mut host, 0);
            let mut r = sw(&comp.obj_sphere);
            r.extend(vec![b(comp.obj_box.center.x), b(comp.obj_box.center.y), b(comp.obj_box.center.z), b(comp.obj_box.extent.x), b(comp.obj_box.extent.y), b(comp.obj_box.extent.z)]);
            r.push(host.invalidates);
            c[2].eq(r, out[..k].to_vec(), || format!("case {case} {n} sub-objects {:?}", host.spheres));
        }
    }
    support_sweep::finish(c);
}
