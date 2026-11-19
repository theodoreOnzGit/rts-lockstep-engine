// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dpure) of the draft `src/drafts/ww3d/intersec.rs` (`IntersectionClass`,
//! `IntersectionResultClass`: the first pass's members) against upstream `WW3D2/intersec.h` (the REAL header: `Set`,
//! `Copy_Partial_Results`, `Copy_Results`, `Intersect_Sphere_Quick`, `Intersect_Sphere` are compiled from it as they stand) and
//! `WW3D2/intersec.cpp` (cut VERBATIM: the statics, `Append_Object_Array`, `Intersect_Box`, `Append_Hierarchy_Objects`, both
//! `Intersect_Object_Array` overloads), see differential/wrap_draft_ixpure.cpp. The only stand-in is a scripted
//! `RenderObjClass`. (The scene-walking members of the same file are `src/drafts/sweep_ww3d/intersec.rs`, tested by
//! `tests/differential_draft_intersec.rs`; that test only hosts this draft's types.)
//!
//! # Method
//!
//! 20,000 cases per operation (9 operations) from a fixed seed (`DRAFT_CASES` overrides). A ray (location, direction not
//! necessarily unit, `MaxDistance`, `ConvexTest`, an `IntersectionNormal` present or NULL) against: spheres (centre and radius
//! on a coarse grid so that tangent and `Range == MaxDistance` ties occur, plus `mixed` floats); boxes (the origin inside, on a
//! face, outside in each octant, direction components zero); copy sources with every field set; `Append_Object_Array` with the
//! array empty, partly full, exactly full and over-full (an over-full `CurrentCount` is a caller bug upstream also ignores);
//! `Intersect_Object_Array` over 0..=12 scripted objects (each with scripted sphere and exact answers, ranges that tie and
//! that are negative) for all four flag combinations of `Test_Bounding_Sphere` and `Convex`, with `ConvexTest` on and off;
//! `Append_Hierarchy_Objects` over hierarchies of 0..=300 sub-objects (past the 256 cap). Every result field of the
//! `FinalResult` (26 words), the intersection normal, the returned bool, the array and the ORDER of the scripted calls
//! (`Intersect_Sphere_Quick` / `Intersect`) are compared as bits.
//!
//! # Excluded inputs
//!
//! - `Intersect_Object_Array`: ranges that are NaN or at least 2^29 (the "nearest" search starts from `(float)(2<<28)` and, finding
//!   nothing under it, would copy `TemporaryResults[-1]`, undefined), and an object whose `Intersect` returns true while leaving
//!   `Intersects` false (the scripted objects keep them equal, as the real ones do).
//! - `Release_Ref` calls (upstream releases the reference it did not take): logged by the stand-in, not modelled by the draft, so
//!   the test drops them from the log.
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! 0 mismatches. The riskiest member is `Intersect_Object_Array`: the draft's `nearest_index` for the convex branch stays set from
//! the loop while upstream sets `counter = 0` to leave the loop; the call logs will show a difference if the loops differ.
//!
//! # Results (2026-11-19)
//!
//! **First run: 15,766 mismatches of 620,000 words in `Intersect_Box`**, none elsewhere (the prediction of 0 missed): on a miss
//! upstream writes the components of `Intersection` one at a time (`(*intersection)[counter] = ...`) and returns at the first
//! one outside the box, leaving the later components as the caller had them; the draft built a fresh vector with zeros. **Fixed in the
//! draft** (components written in place), **0 mismatches after**: 20,000 cases of each of the 9 operations. The first plants for
//! 14738 (`Beta <= 0`) and 14740 (`Range >= MaxDistance`) were MISSED, because random floats never hit those ties exactly; the
//! generator now adds axis-aligned rays at whole-number spheres, and both are caught.
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching output words, all operations it affects)
//!   14738: 21,564; 14739: 205,994; 14740: 18,173; 14741: 21,227
//!   14742: 25,279; 14743: 90,894; 14744: 4,152; 14745: 1,701
//!   14746: 4,716; 14747: 8,320; 14748: 35,545; 14749: 46,525
//!   14750: 2,245,250
//!
//! **13 of 13 caught.**

mod support;

use rts_lockstep_engine::drafts::ww3d::intersec::*;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dpw_ix_setup(input: *const u32);
    fn ref_dpw_ix_run(input: *const u32, out: *mut u32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp {}", c.name, c.compared, c.mismatches, c.nan_only, c.max_ulp);
        bad += c.mismatches;
        if !planted {
            c.finish();
        }
    }
    if planted {
        println!("PLANTED mismatches {bad}");
        assert!(bad > 0, "plant not caught");
    }
}

fn fb(f: f32) -> u32 {
    f.to_bits()
}

/// A scripted render object (the wrapper's RenderObjClass).
#[derive(Clone)]
struct Obj {
    id: u32,
    quick: bool,
    hit: bool,
    range: f32,
    alpha: f32,
    beta: f32,
    polygon: u32,
    ty: u32,
    isect: [f32; 3],
    mloc: [f32; 3],
    m: [f32; 12],
    subs: Vec<u32>,
}

fn int_type(t: u32) -> IntersectionType {
    match t {
        1 => IntersectionType::Generic,
        2 => IntersectionType::Polygon,
        _ => IntersectionType::None,
    }
}

impl Obj {
    fn intersect(&self, r: &mut IntersectionResultClass) -> bool {
        r.intersected_render_object = Some(self.id as usize);
        r.intersected_polygon = self.polygon as u16;
        r.model_matrix = Matrix3D::from_array(self.m);
        r.model_location = Vector3::new(self.mloc[0], self.mloc[1], self.mloc[2]);
        r.intersection = Vector3::new(self.isect[0], self.isect[1], self.isect[2]);
        r.range = self.range;
        r.alpha = self.alpha;
        r.beta = self.beta;
        r.intersects = self.hit;
        r.intersection_type = int_type(self.ty);
        self.hit
    }
    fn sphere_quick(&self, r: &mut IntersectionResultClass) -> bool {
        r.alpha = self.alpha;
        r.beta = self.beta;
        r.intersects = self.quick;
        self.quick
    }
}

fn setup_words(objs: &[Obj]) -> Vec<u32> {
    let mut w = vec![objs.len() as u32];
    for o in objs {
        w.extend_from_slice(&[o.id, u32::from(o.quick), u32::from(o.hit), fb(o.range), fb(o.alpha), fb(o.beta), o.polygon, o.ty]);
        w.extend(o.isect.iter().map(|&f| fb(f)));
        w.extend(o.mloc.iter().map(|&f| fb(f)));
        w.extend(o.m.iter().map(|&f| fb(f)));
        w.push(o.subs.len() as u32);
        w.extend_from_slice(&o.subs);
    }
    w
}

fn result_words(r: &IntersectionResultClass) -> Vec<u32> {
    let mut w = vec![r.intersected_render_object.map_or(0, |i| i as u32 + 1), u32::from(r.intersected_polygon)];
    w.extend(r.model_matrix.to_array().iter().map(|&f| fb(f)));
    w.extend_from_slice(&[fb(r.model_location.x), fb(r.model_location.y), fb(r.model_location.z), fb(r.intersection.x), fb(r.intersection.y), fb(r.intersection.z)]);
    w.extend_from_slice(&[fb(r.range), fb(r.alpha), fb(r.beta), u32::from(r.intersects), r.collision_type as u32]);
    w.push(match r.intersection_type {
        IntersectionType::None => 0,
        IntersectionType::Generic => 1,
        IntersectionType::Polygon => 2,
    });
    w
}

fn make_result(g: &mut Gen, nobj: usize) -> IntersectionResultClass {
    let mut r = IntersectionResultClass::default();
    r.intersected_render_object = if nobj > 0 && g.below(2) == 0 { Some(g.below(nobj as u32) as usize) } else { None };
    r.intersected_polygon = g.below(100) as u16;
    let m: [f32; 12] = std::array::from_fn(|_| g.uniform(-3.0, 3.0));
    r.model_matrix = Matrix3D::from_array(m);
    r.model_location = Vector3::new(g.uniform(-9.0, 9.0), g.uniform(-9.0, 9.0), g.uniform(-9.0, 9.0));
    r.intersection = Vector3::new(g.uniform(-9.0, 9.0), g.uniform(-9.0, 9.0), g.uniform(-9.0, 9.0));
    r.range = g.uniform(-5.0, 50.0);
    r.alpha = g.uniform(-5.0, 5.0);
    r.beta = g.uniform(-5.0, 5.0);
    r.intersects = g.below(2) == 0;
    r.collision_type = g.i32();
    r.intersection_type = int_type(g.below(3));
    r
}

fn coord(g: &mut Gen, wild: bool) -> f32 {
    if wild { g.mixed() } else if g.below(3) == 0 { (g.below(11) as f32) - 5.0 } else { g.uniform(-10.0, 10.0) }
}

fn make_obj(g: &mut Gen, id: u32, nsub_pool: usize) -> Obj {
    let hit = g.below(2) == 0;
    Obj {
        id,
        quick: g.below(2) == 0,
        hit,
        range: [0.0, 1.0, 1.0, 2.5, 2.5, -1.0, 10.0][g.below(7) as usize] + if g.below(3) == 0 { g.uniform(0.0, 100.0) } else { 0.0 },
        alpha: g.uniform(-4.0, 4.0),
        beta: g.uniform(-4.0, 4.0),
        polygon: g.below(50),
        ty: g.below(3),
        isect: [g.uniform(-9.0, 9.0), g.uniform(-9.0, 9.0), g.uniform(-9.0, 9.0)],
        mloc: [g.uniform(-9.0, 9.0), g.uniform(-9.0, 9.0), g.uniform(-9.0, 9.0)],
        m: std::array::from_fn(|_| g.uniform(-3.0, 3.0)),
        subs: (0..nsub_pool).map(|_| 0).collect(),
    }
}

const NAMES: [&str; 9] = [
    "IntersectionClass::Intersect_Sphere_Quick", "IntersectionClass::Intersect_Sphere", "IntersectionClass::Copy_Partial_Results",
    "IntersectionClass::Copy_Results", "IntersectionClass::Append_Object_Array", "IntersectionClass::Intersect_Box",
    "IntersectionClass::Intersect_Object_Array (5 args)", "IntersectionClass::Intersect_Object_Array (6 args)",
    "IntersectionClass::Append_Hierarchy_Objects",
];

#[test]
fn ixpure_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_14_06);
    let mut chk: Vec<Check> = NAMES.iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        for op in 0..9usize {
            let wild = g.below(16) == 0;
            // the scripted objects
            let nobj = match op {
                6 | 7 => g.below(13) as usize,
                8 => g.below(301) as usize,
                4 => 1 + g.below(12) as usize,
                _ => 1,
            };
            let mut objs: Vec<Obj> = (0..nobj.max(1)).map(|i| make_obj(&mut g, i as u32, 0)).collect();
            if op == 8 {
                // object 0 is the hierarchy: its sub-objects are the others (1..nobj), in order
                let n = nobj;
                objs = (0..n + 1).map(|i| make_obj(&mut g, i as u32, 0)).collect();
                objs[0].subs = (1..=n as u32).collect();
            }
            let loc = [coord(&mut g, wild), coord(&mut g, wild), coord(&mut g, wild)];
            let dir = [coord(&mut g, wild), coord(&mut g, wild), coord(&mut g, wild)];
            let dir = if g.below(4) == 0 { [dir[0], 0.0, dir[2]] } else { dir };
            let maxd = if g.below(6) == 0 { f32::MAX } else { g.uniform(0.0, 40.0) };
            let convex_test = g.below(3) == 0;
            let has_normal = g.below(2) == 0;
            let normal = [g.uniform(-2.0, 2.0), g.uniform(-2.0, 2.0), g.uniform(-2.0, 2.0)];
            let fin = make_result(&mut g, objs.len());
            let src = make_result(&mut g, objs.len());
            let center = [coord(&mut g, wild), coord(&mut g, wild), coord(&mut g, wild)];
            let radius = if g.below(6) == 0 { 0.0 } else if wild { g.mixed() } else { g.uniform(0.0, 12.0) };
            // a box: the ray origin inside, on a face, or outside
            let mut bmin = [coord(&mut g, false), coord(&mut g, false), coord(&mut g, false)];
            let mut bmax = [bmin[0] + g.uniform(0.0, 8.0), bmin[1] + g.uniform(0.0, 8.0), bmin[2] + g.uniform(0.0, 8.0)];
            if g.below(4) == 0 {
                for k in 0..3 {
                    bmin[k] = bmin[k].round();
                    bmax[k] = bmax[k].round();
                }
            }
            let loc = if op == 5 && g.below(4) == 0 {
                let k = g.below(3) as usize;
                let mut l = [(bmin[0] + bmax[0]) * 0.5, (bmin[1] + bmax[1]) * 0.5, (bmin[2] + bmax[2]) * 0.5];
                l[k] = if g.below(2) == 0 { bmin[k] } else { bmax[k] };
                l
            } else {
                loc
            };
            // exact ties for the sphere members: an axis-aligned unit ray from an integer point at a sphere on whole numbers, so that
            // Beta == 0 (tangent) and Range == MaxDistance happen exactly
            let (loc, dir, maxd, center, radius) = if (op == 0 || op == 1) && g.below(3) == 0 {
                let k = g.below(3) as usize;
                let sign = if g.below(2) == 0 { 1.0f32 } else { -1.0 };
                let mut d = [0.0f32; 3];
                d[k] = sign;
                let l = [(g.below(7) as f32) - 3.0, (g.below(7) as f32) - 3.0, (g.below(7) as f32) - 3.0];
                let a = 1 + g.below(8);
                let (p, r) = [(0u32, 1u32), (0, 3), (3, 5), (4, 5), (5, 5), (2, 2)][g.below(6) as usize];
                let j = (k + 1 + g.below(2) as usize) % 3;
                let mut c = l;
                c[k] += sign * a as f32;
                c[j] += p as f32;
                let dd = ((r * r - p * p) as f32).sqrt();
                let m = if g.below(2) == 0 { a as f32 - dd } else { maxd };
                (l, d, m, c, r as f32)
            } else {
                (loc, dir, maxd, center, radius)
            };
            let tbs = g.below(2) == 0;
            let convex = g.below(2) == 0;
            let maxcount = g.below(14) as usize;
            let cur = if op == 4 { g.below(maxcount as u32 + 3) as usize } else { g.below(maxcount as u32 + 1) as usize };
            let cur = if op == 8 { 0 } else { cur };
            let prefill: Vec<u32> = (0..cur).map(|_| g.below(objs.len() as u32)).collect();
            let hier_max = 256usize;

            // the C++ call
            let setup = setup_words(&objs);
            unsafe { ref_dpw_ix_setup(setup.as_ptr()) };
            let mut input: Vec<u32> = vec![op as u32, fb(loc[0]), fb(loc[1]), fb(loc[2]), fb(dir[0]), fb(dir[1]), fb(dir[2]), fb(maxd), u32::from(convex_test), u32::from(has_normal)];
            input.extend(normal.iter().map(|&f| fb(f)));
            input.extend(result_words(&fin));
            input.extend(result_words(&src));
            input.extend(center.iter().map(|&f| fb(f)));
            input.push(fb(radius));
            input.extend(bmin.iter().map(|&f| fb(f)));
            input.extend(bmax.iter().map(|&f| fb(f)));
            input.extend_from_slice(&[u32::from(tbs), u32::from(convex)]);
            let count_arg = match op {
                4 => g.below(objs.len() as u32), // the object appended
                6 | 7 => nobj as u32,
                _ => 0, // 8: the hierarchy is object 0
            };
            input.push(count_arg);
            input.push(if op == 8 { hier_max as u32 } else { maxcount as u32 });
            input.push(cur as u32);
            input.extend_from_slice(&prefill);
            let mut out = vec![0u32; 4000];
            let n = unsafe { ref_dpw_ix_run(input.as_ptr(), out.as_mut_ptr()) } as usize;
            let cpp = &out[..n];

            // the Rust call
            let mut ic = IntersectionClass::new();
            ic.set(Vector3::new(loc[0], loc[1], loc[2]), Vector3::new(dir[0], dir[1], dir[2]), if has_normal { Some(Vector3::new(normal[0], normal[1], normal[2])) } else { None }, false, maxd, convex_test);
            let mut f = fin;
            let mut ret = false;
            let mut arr: Vec<usize> = Vec::new();
            let mut is_append = false;
            let mut log: Vec<u32> = Vec::new();
            match op {
                0 => ret = ic.intersect_sphere_quick(&Vector3::new(center[0], center[1], center[2]), radius, &mut f),
                1 => ret = ic.intersect_sphere(&Vector3::new(center[0], center[1], center[2]), radius, &mut f),
                2 => IntersectionClass::copy_partial_results(&mut f, &src),
                3 => IntersectionClass::copy_results(&mut f, &src),
                4 => {
                    is_append = true;
                    arr = prefill.iter().map(|&i| i as usize).collect();
                    IntersectionClass::append_object_array(maxcount, &mut arr, count_arg as usize);
                }
                5 => ret = ic.intersect_box(&Vector3::new(bmin[0], bmin[1], bmin[2]), &Vector3::new(bmax[0], bmax[1], bmax[2]), &mut f),
                6 | 7 => {
                    let mut temp = vec![IntersectionResultClass::default(); nobj.max(1)];
                    let objs_ref = &objs;
                    let log_ref = std::cell::RefCell::new(Vec::<u32>::new());
                    ret = ic.intersect_object_array(
                        nobj,
                        &mut f,
                        &mut temp,
                        tbs,
                        convex,
                        |i, r| {
                            log_ref.borrow_mut().extend_from_slice(&[202, i as u32]);
                            objs_ref[i].sphere_quick(r)
                        },
                        |i, r| {
                            log_ref.borrow_mut().extend_from_slice(&[200, i as u32]);
                            objs_ref[i].intersect(r)
                        },
                    );
                    log = log_ref.into_inner();
                }
                _ => {
                    is_append = true;
                    let log_ref = std::cell::RefCell::new(Vec::<u32>::new());
                    let subs = objs[0].subs.clone();
                    let objs_ref = &objs;
                    let mut idx: Vec<usize> = Vec::new();
                    IntersectionClass::append_hierarchy_objects(subs.len(), &mut idx, tbs, |i| {
                        log_ref.borrow_mut().extend_from_slice(&[202, subs[i]]);
                        let mut r = IntersectionResultClass::default();
                        objs_ref[subs[i] as usize].sphere_quick(&mut r)
                    });
                    arr = idx.iter().map(|&i| subs[i] as usize).collect();
                    log = log_ref.into_inner();
                }
            }
            let mut w: Vec<u32> = vec![u32::from(ret)];
            w.extend(result_words(&f));
            let nrm = ic.intersection_normal.unwrap_or(Vector3::new(normal[0], normal[1], normal[2]));
            w.extend_from_slice(&[fb(nrm.x), fb(nrm.y), fb(nrm.z)]);
            if is_append {
                w.push(arr.len() as u32);
                w.extend(arr.iter().map(|&i| i as u32));
            } else {
                w.push(0);
            }
            w.extend(log);
            // drop the Release_Ref events (201 id) from the C++ log
            let head = 1 + 26 + 3 + 1 + if is_append { cpp[30] as usize } else { 0 };
            let mut cw: Vec<u32> = cpp[..head].to_vec();
            let mut k = head;
            while k + 1 < cpp.len() {
                if cpp[k] != 201 {
                    cw.push(cpp[k]);
                    cw.push(cpp[k + 1]);
                }
                k += 2;
            }
            let lenmatch = w.len() == cw.len();
            chk[op].compared += cw.len() as u64;
            if !lenmatch {
                chk[op].mismatches += 1;
                if chk[op].examples.len() < 8 {
                    chk[op].examples.push(format!("case {case} op {op}: {} words, c++ {}", w.len(), cw.len()));
                }
            } else {
                for (i, (a, b)) in w.iter().zip(cw.iter()).enumerate() {
                    if a != b {
                        let nan = |x: u32| (x & 0x7F80_0000) == 0x7F80_0000 && (x & 0x007F_FFFF) != 0;
                        if nan(*a) && nan(*b) {
                            chk[op].nan_only += 1;
                        } else {
                            chk[op].mismatches += 1;
                            if chk[op].examples.len() < 8 {
                                chk[op].examples.push(format!("case {case} op {op} word {i}: rust {a:#x} != c++ {b:#x}"));
                            }
                        }
                    }
                }
            }
        }
    }
    finish(chk);
}
