// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dgeo) of the small WW3D container drafts, against upstream's text cut
//! VERBATIM by build.rs (`drafts_ww3dgeo`, differential/wrap_draft_lists.cpp):
//!
//! | draft | upstream function |
//! |---|---|
//! | `predlod::VisibleObjArrays::alloc_visible_obj_arrays` | `PredictiveLODOptimizerClass::AllocVisibleObjArrays` |
//! | `visrasterizer::get_temp_vertex_buffer` | `VisRasterizerClass::Get_Temp_Vertex_Buffer` (`peek_camera` is `return Camera;`, a field read: not tested) |
//! | `seglinerenderer::SegLineVertexBuffer::get_vertex_buffer` | `SegLineRendererClass::getVertexBuffer` |
//! | `streak_render::StreakVertexBuffer::get_vertex_buffer` | `StreakRendererClass::getVertexBuffer` |
//! | `collect::CollectionState::{add_sub_object, snap_point_count, get_snap_point, get_proxy, get_proxy_count}` | `CollectionClass::` the same |
//! | `distlod::DistLodState::{get_num_snap_points, get_snap_point, update_lod, increment_lod, decrement_lod}` | `DistLODClass::` the same |
//!
//! # Method
//!
//! Sequences (default 20,000, fixed seed, `DRAFT_CASES` overrides) of 1 to 10 operations on one object per unit, with the
//! whole observable result of each operation compared as 32-bit words: sizes, offsets, returned values, the contents of
//! buffers, and, for the members that call out (collection, distlod), the full ordered log of the calls made (the Rust host
//! writes the same event codes as the C++ stand-in objects: see the wrapper's header). Buffers are compared by writing a
//! pattern through the returned pointer after every growth and reading it back on the calls that do not grow.
//!
//! Excluded (upstream undefined or unobservable): the contents of a buffer after it grew (`Uninitialised_Grow` and
//! `getVertexBuffer` hand back uninitialised memory; the draft zeroes it), `Get_Snap_Point` / `Get_Proxy` with an index out
//! of range where upstream does not check (`(*SnapPoints)[index]`; `Get_Proxy` does check and is tested out of range), a
//! `getVertexBuffer` count whose `number + (number >> 1)` wraps (above 2^31).
//!
//! Deviation the draft documents and this test does not exercise: `get_temp_vertex_buffer` keeps the old contents when it
//! grows (upstream, a `SimpleVecClass`, does not copy them: they are uninitialised afterwards).
//!
//! # Predictions (before the first run, 2026-11-18)
//!
//! 0 mismatches in every unit: each draft is a few lines. The `VertexFormat` buffers of the draft are generic or Rust-side
//! structs; the sizes are the thing most likely to be off (`number + (number >> 1)`).
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;

use rts_lockstep_engine::drafts::sweep_ww3d::collect::{CollectionHost, CollectionState};
use rts_lockstep_engine::drafts::sweep_ww3d::distlod::{DistLodHost, DistLodState, LodNode};
use rts_lockstep_engine::drafts::sweep_ww3d::hlod::ProxyClass;
use rts_lockstep_engine::drafts::sweep_ww3d::predlod::VisibleObjArrays;
use rts_lockstep_engine::drafts::sweep_ww3d::seglinerenderer::SegLineVertexBuffer;
use rts_lockstep_engine::drafts::sweep_ww3d::streak_render::{StreakVertexBuffer, VertexFormatXYZUV1};
use rts_lockstep_engine::drafts::sweep_ww3d::visrasterizer::get_temp_vertex_buffer;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dls_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 200_000];
    let n = unsafe { ref_dls_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "ref_dls_run op {} failed", input[0]);
    out.truncate(n as usize);
    out
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

fn pattern(seed: u32, i: u32) -> u32 {
    seed.wrapping_mul(2654435761) ^ i.wrapping_mul(40503).wrapping_add(12345)
}

fn count(g: &mut Gen) -> u32 {
    match g.below(8) {
        0 => 0,
        1 => g.below(4),
        2 => g.below(40),
        3 => g.below(300),
        4 => g.below(3000),
        _ => g.below(30),
    }
}

fn mat_words(m: &Matrix3D) -> Vec<u32> {
    let mut v = Vec::new();
    for r in &m.row {
        v.extend([r.x.to_bits(), r.y.to_bits(), r.z.to_bits(), r.w.to_bits()]);
    }
    v
}

fn mat_of(w: &[f32; 12]) -> Matrix3D {
    Matrix3D {
        row: [Vector4::new(w[0], w[1], w[2], w[3]), Vector4::new(w[4], w[5], w[6], w[7]), Vector4::new(w[8], w[9], w[10], w[11])],
    }
}

fn val(g: &mut Gen) -> f32 {
    if g.below(6) == 0 {
        g.special()
    } else {
        g.uniform(-50.0, 50.0)
    }
}

fn miss<T: PartialEq + std::fmt::Debug>(c: &mut Check, rust: T, cpp: T, what: impl FnOnce() -> String) {
    c.eq(rust, cpp, what);
}

// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn predlod_alloc_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0670);
    let mut c = Check::new("AllocVisibleObjArrays");
    for case in 0..cases() {
        run(&[0]);
        let mut st: VisibleObjArrays<u8> = VisibleObjArrays::default();
        for step in 0..1 + g.below(10) {
            let n = match g.below(6) {
                0 => 0,
                1 => -(g.below(5) as i32),
                2 => g.below(8) as i32,
                _ => count(&mut g) as i32,
            };
            st.array.iter_mut().for_each(|x| *x = 1);
            st.alloc_visible_obj_arrays(n);
            let constructed = st.array.iter().filter(|&&x| x == 0).count() as u32;
            let rust = vec![st.size as u32, st.array2_offset as u32, constructed, u32::from(!st.array.is_empty())];
            let cpp = run(&[1, n as u32]);
            miss(&mut c, rust, cpp, || format!("case {case} step {step}: alloc({n})"));
        }
    }
    finish(vec![c]);
}

#[test]
fn visrasterizer_temp_vertex_buffer_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0675);
    let mut c = Check::new("Get_Temp_Vertex_Buffer");
    for case in 0..cases() {
        run(&[0]);
        let mut v: Vec<Vector3> = Vec::new();
        for step in 0..1 + g.below(10) {
            let n = if g.below(10) == 0 { -(g.below(5) as i32) } else { count(&mut g) as i32 };
            let seed = g.u32();
            let before = v.len();
            let s = get_temp_vertex_buffer(&mut v, n);
            let mut rust = Vec::new();
            // Length is the vector's allocated size: the draft's Vec length (it grows to `count` and never shrinks)
            let len_after = if s.len() > before { s.len() } else { before };
            let grown = len_after != before;
            if grown {
                for (i, e) in s.iter_mut().enumerate() {
                    *e = Vector3::new(f32::from_bits(pattern(seed, 3 * i as u32)), f32::from_bits(pattern(seed, 3 * i as u32 + 1)), f32::from_bits(pattern(seed, 3 * i as u32 + 2)));
                }
                rust.extend([len_after as u32, 1, 0]);
            } else {
                let k = if n > 0 { n as usize } else { 0 };
                rust.extend([len_after as u32, 0, k as u32]);
                for e in s.iter().take(k) {
                    rust.extend([e.x.to_bits(), e.y.to_bits(), e.z.to_bits()]);
                }
            }
            let cpp = run(&[2, n as u32, seed]);
            miss(&mut c, rust, cpp, || format!("case {case} step {step}: get_temp_vertex_buffer({n})"));
        }
    }
    finish(vec![c]);
}

#[derive(Clone, Copy, Default)]
struct V6 {
    w: [u32; 6],
}

#[test]
fn seg_and_streak_vertex_buffers_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0678);
    let mut cs = Check::new("SegLineRendererClass::getVertexBuffer");
    let mut ct = Check::new("StreakRendererClass::getVertexBuffer");
    for case in 0..cases() {
        run(&[0]);
        let mut seg: SegLineVertexBuffer<V6> = SegLineVertexBuffer::default();
        let mut stk = StreakVertexBuffer::default();
        for step in 0..1 + g.below(10) {
            let number = count(&mut g);
            let seed = g.u32();
            // seglinerenderer
            {
                let before = seg.vertex_buffer.len();
                let s = seg.get_vertex_buffer(number);
                let len = s.len();
                let mut rust = vec![len as u32];
                if len != before {
                    rust.extend([1, 0]);
                    for (i, e) in s.iter_mut().enumerate() {
                        for k in 0..6 {
                            e.w[k] = pattern(seed, 6 * i as u32 + k as u32);
                        }
                    }
                } else {
                    rust.extend([0, number]);
                    for e in s.iter().take(number as usize) {
                        rust.extend(e.w);
                    }
                }
                let cpp = run(&[3, number, seed]);
                miss(&mut cs, rust, cpp, || format!("case {case} step {step}: seg getVertexBuffer({number})"));
            }
            // streak
            {
                let before = stk.vertex_buffer.len();
                let s = stk.get_vertex_buffer(number);
                let len = s.len();
                let mut rust = vec![len as u32];
                if len != before {
                    rust.extend([1, 0]);
                    for (i, e) in s.iter_mut().enumerate() {
                        let i = i as u32;
                        *e = VertexFormatXYZUV1 {
                            x: f32::from_bits(pattern(seed, 5 * i)),
                            y: f32::from_bits(pattern(seed, 5 * i + 1)),
                            z: f32::from_bits(pattern(seed, 5 * i + 2)),
                            u1: f32::from_bits(pattern(seed, 5 * i + 3)),
                            v1: f32::from_bits(pattern(seed, 5 * i + 4)),
                        };
                    }
                } else {
                    rust.extend([0, number]);
                    for e in s.iter().take(number as usize) {
                        rust.extend([e.x.to_bits(), e.y.to_bits(), e.z.to_bits(), e.u1.to_bits(), e.v1.to_bits()]);
                    }
                }
                let cpp = run(&[4, number, seed]);
                miss(&mut ct, rust, cpp, || format!("case {case} step {step}: streak getVertexBuffer({number})"));
            }
        }
    }
    finish(vec![cs, ct]);
}

// ---------------------------------------------------------------------------------------------------------------------

struct ColHost {
    log: Vec<u32>,
    in_scene: bool,
    ids: Vec<i32>,
}

impl CollectionHost for ColHost {
    fn sub_object_add_ref(&mut self, subobj: usize) {
        self.log.extend([100, self.ids[subobj] as u32]);
    }
    fn sub_object_set_container(&mut self, subobj: usize) {
        self.log.extend([101, self.ids[subobj] as u32]);
    }
    fn sub_object_set_transform(&mut self, subobj: usize, transform: &Matrix3D) {
        self.log.extend([102, self.ids[subobj] as u32]);
        self.log.extend(mat_words(transform));
    }
    fn update_sub_object_bits(&mut self) {
        self.log.push(110);
    }
    fn update_obj_space_bounding_volumes(&mut self) {
        self.log.push(111);
    }
    fn is_in_scene(&self) -> bool {
        self.in_scene
    }
    fn sub_object_notify_added(&mut self, subobj: usize) {
        self.log.extend([103, self.ids[subobj] as u32]);
    }
}

fn pname(g: &mut Gen) -> Vec<u8> {
    let n = g.below(6) as usize;
    (0..n).map(|_| b'A' + g.below(26) as u8).collect()
}

#[test]
fn collection_members_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0684);
    let names = ["Add_Sub_Object", "Snap_Point_Count", "Get_Snap_Point", "Get_Proxy", "Get_Proxy_Count"];
    let mut chk: Vec<Check> = names.iter().map(|n| Check::new(&format!("CollectionClass::{n}"))).collect();
    for case in 0..cases() {
        run(&[0]);
        let mut st = CollectionState { sub_objects: Vec::new(), snap_points: None, proxy_list: Vec::new(), transform: Matrix3D::default() };
        // the C++ collection starts with an identity transform
        st.transform = mat_of(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        let mut host = ColHost { log: Vec::new(), in_scene: false, ids: Vec::new() };
        let mut next_id = 1i32;
        for step in 0..1 + g.below(12) {
            match g.below(9) {
                0 => {
                    host.in_scene = g.below(2) == 0;
                    run(&[10, u32::from(host.in_scene)]);
                }
                1 => {
                    let w: [f32; 12] = std::array::from_fn(|_| val(&mut g));
                    st.transform = mat_of(&w);
                    let mut inp = vec![11];
                    inp.extend(w.iter().map(|x| x.to_bits()));
                    run(&inp);
                }
                2 | 3 => {
                    let id = next_id;
                    next_id += 1;
                    host.ids.push(id);
                    host.log.clear();
                    let h = host.ids.len() - 1;
                    let ret = st.add_sub_object(&mut host, h);
                    let mut rust = vec![ret as u32, st.sub_objects.len() as u32];
                    rust.extend(host.log.iter().copied());
                    let cpp = run(&[13, id as u32]);
                    miss(&mut chk[0], rust, cpp, || format!("case {case} step {step}: add_sub_object"));
                }
                4 => {
                    let has = g.below(3) != 0;
                    let n = g.below(6) as usize;
                    let pts: Vec<Vector3> = (0..n).map(|_| Vector3::new(val(&mut g), val(&mut g), val(&mut g))).collect();
                    st.snap_points = if has { Some(pts.clone()) } else { None };
                    let mut inp = vec![14, u32::from(has), n as u32];
                    for p in &pts {
                        inp.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
                    }
                    run(&inp);
                    miss(&mut chk[1], vec![st.snap_point_count() as u32], run(&[15]), || format!("case {case} step {step}: snap_point_count"));
                }
                5 => {
                    let n = st.snap_points.as_ref().map_or(0, |s| s.len());
                    if st.snap_points.is_some() && n == 0 {
                        continue;
                    }
                    let idx = if st.snap_points.is_some() { g.below(n as u32) } else { g.below(4) };
                    let pre = [val(&mut g), val(&mut g), val(&mut g)];
                    let mut set = Vector3::new(pre[0], pre[1], pre[2]);
                    st.get_snap_point(idx as i32, &mut set);
                    let cpp = run(&[16, idx, pre[0].to_bits(), pre[1].to_bits(), pre[2].to_bits()]);
                    miss(&mut chk[2], vec![set.x.to_bits(), set.y.to_bits(), set.z.to_bits()], cpp, || format!("case {case} step {step}: get_snap_point({idx})"));
                }
                6 => {
                    let n = g.below(5) as usize;
                    st.proxy_list.clear();
                    let mut inp = vec![17, n as u32];
                    for _ in 0..n {
                        let name = pname(&mut g);
                        let w: [f32; 12] = std::array::from_fn(|_| val(&mut g));
                        inp.push(name.len() as u32);
                        inp.extend(name.iter().map(|&b| u32::from(b)));
                        inp.extend(w.iter().map(|x| x.to_bits()));
                        st.proxy_list.push(ProxyClass { name: String::from_utf8(name).unwrap(), transform: mat_of(&w) });
                    }
                    run(&inp);
                    miss(&mut chk[4], vec![st.get_proxy_count() as u32], run(&[19]), || format!("case {case} step {step}: get_proxy_count"));
                }
                _ => {
                    let idx = g.below(8) as i32 - 2;
                    let pre = pname(&mut g);
                    let mut px = ProxyClass { name: String::from_utf8(pre.clone()).unwrap(), transform: mat_of(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]) };
                    let r = st.get_proxy(idx, &mut px);
                    let mut rust = vec![u32::from(r), px.name.len() as u32];
                    rust.extend(px.name.bytes().map(u32::from));
                    rust.extend(mat_words(&px.transform));
                    let mut inp = vec![18, idx as u32, pre.len() as u32];
                    inp.extend(pre.iter().map(|&b| u32::from(b)));
                    miss(&mut chk[3], rust, run(&inp), || format!("case {case} step {step}: get_proxy({idx})"));
                }
            }
        }
    }
    finish(chk);
}

// ---------------------------------------------------------------------------------------------------------------------

struct Model {
    id: i32,
    center: Vector3,
    snaps: Vec<Vector3>,
}

struct DlHost {
    models: Vec<Model>,
    in_scene: bool,
    log: Vec<u32>,
}

impl DistLodHost for DlHost {
    fn model_get_num_snap_points(&self, model: usize) -> i32 {
        self.models[model].snaps.len() as i32
    }
    fn model_get_snap_point(&self, model: usize, index: i32, set: &mut Vector3) {
        *set = self.models[model].snaps[index as usize];
    }
    fn model_bounding_sphere_center(&self, model: usize) -> Vector3 {
        self.models[model].center
    }
    fn is_in_scene(&self) -> bool {
        self.in_scene
    }
    fn model_notify_removed(&mut self, model: usize) {
        self.log.extend([104, self.models[model].id as u32]);
    }
    fn model_notify_added(&mut self, model: usize) {
        self.log.extend([103, self.models[model].id as u32]);
    }
}

#[test]
fn distlod_members_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0691);
    let names = ["Get_Num_Snap_Points", "Get_Snap_Point", "Update_Lod", "Increment_Lod", "Decrement_Lod"];
    let mut chk: Vec<Check> = names.iter().map(|n| Check::new(&format!("DistLODClass::{n}"))).collect();
    let (mut ups, mut downs, mut stays) = (0u64, 0u64, 0u64);
    for case in 0..cases() {
        run(&[0]);
        let n = 1 + g.below(5) as usize;
        let cur = g.below(n as u32) as i32;
        let in_scene = g.below(2) == 0;
        let mut models = Vec::new();
        let mut st = DistLodState { lods: Vec::new(), cur_lod: cur };
        let mut inp = vec![30, n as u32, cur as u32, u32::from(in_scene)];
        for i in 0..n {
            let center = Vector3::new(g.uniform(-20.0, 20.0).round(), g.uniform(-20.0, 20.0).round(), g.uniform(-20.0, 20.0).round());
            let ns = g.below(4) as usize;
            let snaps: Vec<Vector3> = (0..ns).map(|_| Vector3::new(val(&mut g), val(&mut g), val(&mut g))).collect();
            let up = g.uniform(0.0, 40.0).round();
            let down = up + g.uniform(0.0, 40.0).round();
            models.push(Model { id: 10 + i as i32, center, snaps: snaps.clone() });
            st.lods.push(LodNode { model: i, res_up_dist: up, res_down_dist: down });
            inp.extend([10 + i as u32, up.to_bits(), down.to_bits(), center.x.to_bits(), center.y.to_bits(), center.z.to_bits(), ns as u32]);
            for p in &snaps {
                inp.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
            }
        }
        run(&inp);
        let mut host = DlHost { models, in_scene, log: Vec::new() };
        for step in 0..1 + g.below(10) {
            host.log.clear();
            match g.below(6) {
                0 => {
                    miss(&mut chk[0], vec![st.get_num_snap_points(&host) as u32], run(&[34]), || format!("case {case} step {step}: get_num_snap_points"));
                }
                1 => {
                    let k = host.models[0].snaps.len();
                    if k > 0 {
                        let idx = g.below(k as u32);
                        let pre = [val(&mut g), val(&mut g), val(&mut g)];
                        let mut set = Vector3::new(pre[0], pre[1], pre[2]);
                        st.get_snap_point(&host, idx as i32, &mut set);
                        let cpp = run(&[35, idx, pre[0].to_bits(), pre[1].to_bits(), pre[2].to_bits()]);
                        miss(&mut chk[1], vec![set.x.to_bits(), set.y.to_bits(), set.z.to_bits()], cpp, || format!("case {case} step {step}: get_snap_point({idx})"));
                    }
                }
                2 | 3 | 4 => {
                    // a camera at a whole-number position; sometimes the distance is set to land exactly on a switch distance
                    let cur = st.cur_lod as usize;
                    let c = host.models[cur].center;
                    let mut cam = Vector3::new(g.uniform(-30.0, 30.0).round(), g.uniform(-30.0, 30.0).round(), g.uniform(-30.0, 30.0).round());
                    if g.below(4) == 0 {
                        // put the camera on an axis at exactly ResUpDist or ResDownDist from the centre (Quick_Length of (d,0,0) is d)
                        let d = if g.below(2) == 0 { st.lods[cur].res_up_dist } else { st.lods[cur].res_down_dist };
                        cam = Vector3::new(c.x + d, c.y, c.z);
                    }
                    let before = st.cur_lod;
                    st.update_lod(&mut host, &cam);
                    match st.cur_lod.cmp(&before) {
                        std::cmp::Ordering::Less => ups += 1,
                        std::cmp::Ordering::Greater => downs += 1,
                        std::cmp::Ordering::Equal => stays += 1,
                    }
                    let mut rust = vec![st.cur_lod as u32];
                    rust.extend(host.log.iter().copied());
                    miss(&mut chk[2], rust, run(&[31, cam.x.to_bits(), cam.y.to_bits(), cam.z.to_bits()]), || format!("case {case} step {step}: update_lod from {before}"));
                }
                _ => {
                    let inc = g.below(2) == 0;
                    if inc {
                        st.increment_lod(&mut host);
                    } else {
                        st.decrement_lod(&mut host);
                    }
                    let mut rust = vec![st.cur_lod as u32];
                    rust.extend(host.log.iter().copied());
                    miss(&mut chk[if inc { 3 } else { 4 }], rust, run(&[if inc { 32 } else { 33 }]), || format!("case {case} step {step}: {}", if inc { "increment" } else { "decrement" }));
                }
            }
        }
    }
    println!("update_lod outcomes: up {ups}, down {downs}, stay {stays}");
    finish(chk);
}
