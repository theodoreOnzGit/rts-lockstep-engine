// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dgeo) of the picking-ray drafts (`src/drafts/sweep_ww3d/intersec.rs`):
//! `Intersect_Screen_Point_RenderObject`, `Intersect_RenderObject`, `Intersect_Screen_Point_Layer`,
//! `Intersect_Hierarchy_Sphere_Quick` and `Intersect_Hierarchy_Sphere`, against upstream's text cut VERBATIM from
//! `WW3D2/intersec.cpp` (build.rs `drafts_ww3dgeo`, differential/wrap_draft_intersec.cpp). The camera, layer and render
//! objects are scripted stand-ins on both sides (they are virtual calls and the layer walk, not part of the cut): see the
//! wrapper's header for what each stand-in does and the event codes.
//!
//! # Method
//!
//! 20,000 worlds (fixed seed, `DRAFT_CASES` overrides): 1 to 6 render objects with random hit flags and random sub-object
//! lists (a sub-object may be any object), a layer with a viewport whose corners are whole numbers, a depth and a scripted
//! `Intersect_Layer` result; then 1 to 8 calls of the five members with random screen points (a quarter of them exactly on a
//! viewport edge), random prefilled `Result` and `FinalResult`, and `FinalResult` NULL (`None`) for the two members that
//! allow it. Compared as words: the return value, `Result.Intersects`, `Result.Range`, `ScreenX`, `ScreenY`, the caller's
//! `FinalResult` and the ordered event log (every virtual call and its arguments).
//!
//! # Predictions (before the first run, 2026-11-18)
//!
//! 0 mismatches: the members are straight call sequences. One thing to watch: with `FinalResult == NULL` upstream passes
//! `&Result` itself, the draft passes a copy and writes it back after the call; the host cannot see the difference unless it
//! reads `Result` during the call (the stand-in does not).
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;

use rts_lockstep_engine::drafts::sweep_ww3d::intersec::*;
use rts_lockstep_engine::drafts::ww3d::intersec::{IntersectionClass, IntersectionResultClass};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dix_setup(input: *const u32, nin: i32);
    fn ref_dix_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Obj {
    id: i32,
    hit: bool,
    hit_quick: bool,
    hit_sphere: bool,
    subs: Vec<usize>,
}

struct World {
    objs: Vec<Obj>,
    layer_id: i32,
    vp: ViewportView,
    depth: f32,
    layer_hit: bool,
    log: Vec<u32>,
}

impl IntersectHost for World {
    fn get_screen_ray(&mut self, picker: &mut IntersectionClass, screen_x: f32, screen_y: f32, _layer: usize) {
        self.log.extend([210, self.layer_id as u32, screen_x.to_bits(), screen_y.to_bits()]);
        picker.screen_x = screen_x;
        picker.screen_y = screen_y;
    }
    fn layer_camera_viewport(&self, _layer: usize) -> ViewportView {
        self.vp
    }
    fn layer_camera_depth(&self, _layer: usize) -> f32 {
        self.depth
    }
    fn intersect_layer(&mut self, picker: &mut IntersectionClass, _layer: usize, test_all: bool) -> bool {
        self.log.extend([211, self.layer_id as u32, u32::from(test_all)]);
        picker.result.intersects = self.layer_hit;
        self.layer_hit
    }
    fn robj_intersect(&mut self, robj: usize, _picker: &mut IntersectionClass, r: &mut IntersectionResultClass) -> bool {
        let o = &self.objs[robj];
        self.log.extend([200, o.id as u32]);
        r.range = o.id as f32 + 0.5;
        r.intersects = o.hit;
        o.hit
    }
    fn get_num_sub_objects(&self, hierarchy: usize) -> i32 {
        self.objs[hierarchy].subs.len() as i32
    }
    fn get_sub_object(&mut self, hierarchy: usize, index: i32) -> usize {
        let s = self.objs[hierarchy].subs[index as usize];
        self.log.extend([201, self.objs[s].id as u32]);
        s
    }
    fn robj_intersect_sphere_quick(&mut self, obj: usize, _picker: &mut IntersectionClass, r: &mut IntersectionResultClass) -> bool {
        let o = &self.objs[obj];
        self.log.extend([202, o.id as u32]);
        r.range = o.id as f32 + 0.5;
        r.intersects = o.hit_quick;
        o.hit_quick
    }
    fn robj_intersect_sphere(&mut self, obj: usize, _picker: &mut IntersectionClass, r: &mut IntersectionResultClass) -> bool {
        let o = &self.objs[obj];
        self.log.extend([203, o.id as u32]);
        r.range = o.id as f32 + 0.5;
        r.intersects = o.hit_sphere;
        o.hit_sphere
    }
}

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp 0", c.name, c.compared, c.mismatches, c.nan_only);
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

#[test]
fn picking_members_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0699);
    let names = ["Intersect_Screen_Point_RenderObject", "Intersect_RenderObject", "Intersect_Screen_Point_Layer", "Intersect_Hierarchy_Sphere_Quick", "Intersect_Hierarchy_Sphere"];
    let mut chk: Vec<Check> = names.iter().map(|n| Check::new(&format!("IntersectionClass::{n}"))).collect();
    let (mut outside, mut hits) = (0u64, 0u64);
    for case in 0..cases() {
        let n = 1 + g.below(6) as usize;
        let mut objs = Vec::new();
        let mut setup = vec![n as u32];
        for i in 0..n {
            let nsub = g.below(5) as usize;
            let subs: Vec<usize> = (0..nsub).map(|_| g.below(n as u32) as usize).collect();
            let o = Obj { id: 100 + i as i32, hit: g.below(2) == 0, hit_quick: g.below(3) == 0, hit_sphere: g.below(3) == 0, subs };
            setup.extend([o.id as u32, u32::from(o.hit), u32::from(o.hit_quick), u32::from(o.hit_sphere), nsub as u32]);
            setup.extend(o.subs.iter().map(|&s| s as u32));
            objs.push(o);
        }
        let min_x = g.uniform(-20.0, 5.0).round();
        let min_y = g.uniform(-20.0, 5.0).round();
        let vp = ViewportView { min_x, min_y, max_x: min_x + g.uniform(0.0, 30.0).round(), max_y: min_y + g.uniform(0.0, 30.0).round() };
        let depth = g.uniform(1.0, 5000.0);
        let layer_hit = g.below(2) == 0;
        setup.extend([7, vp.min_x.to_bits(), vp.min_y.to_bits(), vp.max_x.to_bits(), vp.max_y.to_bits(), depth.to_bits(), u32::from(layer_hit)]);
        unsafe { ref_dix_setup(setup.as_ptr(), setup.len() as i32) };
        let mut world = World { objs, layer_id: 7, vp, depth, layer_hit, log: Vec::new() };
        for step in 0..1 + g.below(8) {
            let op = g.below(5) as usize;
            let edge = |g: &mut Gen, lo: f32, hi: f32| -> f32 {
                match g.below(4) {
                    0 => lo,
                    1 => hi,
                    2 => lo - 1.0,
                    _ => g.uniform(lo - 3.0, hi + 3.0),
                }
            };
            let (sx, sy) = (edge(&mut g, vp.min_x, vp.max_x), edge(&mut g, vp.min_y, vp.max_y));
            let robj = g.below(n as u32) as usize;
            let has_final = g.below(2) == 0;
            let (fi, fr) = (g.below(2) == 0, g.uniform(-9.0, 9.0));
            let (ri, rr) = (g.below(2) == 0, g.uniform(-9.0, 9.0));
            let mut picker = IntersectionClass::new();
            picker.result.intersects = ri;
            picker.result.range = rr;
            let mut fin = IntersectionResultClass::default();
            fin.intersects = fi;
            fin.range = fr;
            world.log.clear();
            let ret = match op {
                0 => intersect_screen_point_render_object(&mut picker, &mut world, sx, sy, 0, robj, if has_final { Some(&mut fin) } else { None }),
                1 => intersect_render_object(&mut picker, &mut world, robj, if has_final { Some(&mut fin) } else { None }),
                2 => intersect_screen_point_layer(&mut picker, &mut world, sx, sy, 0),
                3 => intersect_hierarchy_sphere_quick(&mut picker, &mut world, robj, &mut fin),
                _ => intersect_hierarchy_sphere(&mut picker, &mut world, robj, &mut fin),
            };
            let mut rust = vec![u32::from(ret), u32::from(picker.result.intersects), picker.result.range.to_bits(), picker.screen_x.to_bits(), picker.screen_y.to_bits(), u32::from(fin.intersects), fin.range.to_bits()];
            rust.extend(world.log.iter().copied());
            let input = [op as u32, sx.to_bits(), sy.to_bits(), robj as u32, u32::from(has_final), u32::from(fi), fr.to_bits(), u32::from(ri), rr.to_bits()];
            let mut out = vec![0u32; 1000];
            let cnt = unsafe { ref_dix_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
            assert!(cnt >= 0);
            out.truncate(cnt as usize);
            if op == 2 {
                if rust[3..5] == [0, 0] && world.log.is_empty() {
                    outside += 1;
                }
            }
            hits += u64::from(ret);
            chk[op].eq(rust, out, || format!("case {case} step {step}: op {op} screen ({sx}, {sy}) robj {robj} has_final {has_final}"));
        }
    }
    println!("{hits} calls returned true; {outside} layer picks rejected outside the viewport");
    finish(chk);
}
