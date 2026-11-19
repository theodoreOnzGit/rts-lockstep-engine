// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group sweepww3d) of four small drafts of `src/drafts/sweep_ww3d` against upstream text cut
//! VERBATIM (build.rs `drafts_sweepww3d`, differential/wrap_draft_smallw3d.cpp):
//!
//! | draft | upstream |
//! |---|---|
//! | `layer.rs`: `LayerClass::set_scene`, `get_scene`, `peek_camera` | `layer.cpp`: `LayerClass::Set_Scene`, `Get_Scene`, `Peek_Camera` |
//! | `scene.rs`: `SimpleScene::remove_all_render_objects`, `create_iterator`, `destroy_iterator`; `SimpleSceneIterator::new`, `first`, `next`, `is_done`, `current_item` | `scene.cpp`: `SimpleSceneClass::Remove_All_Render_Objects`, `Create_Iterator`, `Destroy_Iterator`; `SimpleSceneIterator::` the same |
//! | `vertmaterial.rs`: `set_emissive`, `set_emissive_rgb`, `get_uv_source`, `apply_null` | `vertmaterial.cpp`: `VertexMaterialClass::Set_Emissive` (both), `Get_UV_Source`, `Apply_Null` |
//! | `w3d_util.rs`: `convert_quaternion_from_w3d`, `convert_quaternion_to_w3d` | `w3d_util.cpp`: `W3dUtilityClass::Convert_Quaternion` (both overloads) |
//!
//! Stand-ins are listed in the wrapper: reference-counted scene and object handles that log `Add_Ref` / `Release_Ref`, a circular
//! `RefRenderObjListClass` with a head node (the shape of WWLib's `multilist.h`), a `DX8Wrapper` that logs
//! `Set_DX8_Render_State` / `Set_DX8_Material` / `Set_DX8_Texture_Stage_State`, and `D3DRS_*` / `D3DTSS_*` / `D3DMCS_MATERIAL`
//! values read from Wine's `include/d3d8types.h` on 2026-11-19 (verified against it: LIGHTING 137, DIFFUSEMATERIALSOURCE 145,
//! AMBIENTMATERIALSOURCE 147, EMISSIVEMATERIALSOURCE 148, TEXCOORDINDEX 11, TEXTURETRANSFORMFLAGS 24, TCI_PASSTHRU 0,
//! TTFF_DISABLE 0, MCS_MATERIAL 0). The verified `formats::w3d::vertmaterial::D3DMCS_MATERIAL` is 0, as Wine's.
//!
//! # Method
//!
//! 20,000 sequences each (fixed seed, `DRAFT_CASES` overrides): layers with and without a scene or camera and 1 to 8 of
//! `Set_Scene` (same scene, other scene, NULL), `Get_Scene`, `Peek_Camera`; scenes of 0 to 8 objects with 1 to 14 of
//! `Remove_All_Render_Objects`, `Create_Iterator`, `First`, `Next` (also at the end), `Is_Done`, `Current_Item`,
//! `Destroy_Iterator`; vertex materials with random emissive, UV sources and `CRCDirty`, 1 to 8 of the four operations (the
//! `Apply_Null` host log is compared event by event, in order); quaternions of random floats both ways. Every float by bits.
//!
//! # Differences found and fixed in the draft
//!
//! `SimpleSceneIterator` kept a position that `next` increased without bound, `is_done` was `>=` and `current_item` returned
//! `None` past the end. Upstream's iterator walks WWLib's CIRCULAR list: `Next` from the head node goes to the first object, so
//! `Is_Done` is false again afterwards. The draft now wraps (`position = (position + 1) % (len + 1)`, done at `len`). Plant 13926
//! re-creates the old draft and must be caught. The draft's iterator walks a COPY of the list (upstream walks the live list); that
//! design difference is observable only when the list changes during iteration and is not tested: the sequences here do not remove
//! objects while an iterator is live (`Remove_All_Render_Objects` ends the sequence of iterator operations).
//!
//! # Predictions (written before the first run, 2026-11-19)
//!
//! One MISS expected, the iterator's `Next` at the end (found by reading `multilist.h`, fixed in the draft before the first run;
//! plant 13926 is the old behaviour). Everything else 0 mismatches.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::sweep_ww3d::layer::{LayerClass, LayerRefHost};
use rts_lockstep_engine::drafts::sweep_ww3d::scene::{SceneHost, SimpleScene, SimpleSceneIterator};
use rts_lockstep_engine::drafts::sweep_ww3d::vertmaterial::*;
use rts_lockstep_engine::drafts::sweep_ww3d::w3d_util::*;
use rts_lockstep_engine::formats::w3d::vertmaterial::VertexMaterialState;
use rts_lockstep_engine::ww::quat::Quaternion;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};
use support_sweep::{b, cases, feq, finish};

extern "C" {
    fn ref_dsww3d_layer_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
    fn ref_dsww3d_scene_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
    fn ref_dsww3d_vmat_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
    fn ref_dsww3d_wutil_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

type Entry = unsafe extern "C" fn(*const u32, i32, *mut u32, i32) -> i32;

fn run(f: Entry, input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 8192];
    let n = unsafe { f(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

fn val(g: &mut Gen) -> f32 {
    match g.below(6) {
        0 => g.special(),
        1 => 0.0,
        _ => g.uniform(-10.0, 10.0),
    }
}

/// Compares per operation: `parts` holds (check index, Rust words); the C++ output is split by the Rust lengths.
fn compare(c: &mut [Check], parts: Vec<(usize, Vec<u32>)>, cpp: Vec<u32>, case: u64) {
    let mut at = 0;
    for (k, w) in parts {
        let end = (at + w.len()).min(cpp.len());
        feq(&mut c[k], w, cpp[at..end].to_vec(), || format!("case {case}"));
        at = end;
    }
    if at != cpp.len() {
        c[0].eq(at, cpp.len(), || format!("case {case}: output length"));
    }
}

struct LayerLog(Vec<u32>);
impl LayerRefHost for LayerLog {
    fn scene_add_ref(&mut self, scene: usize) {
        self.0.extend([1, scene as u32]);
    }
    fn scene_release_ref(&mut self, scene: usize) {
        self.0.extend([2, scene as u32]);
    }
}

struct SceneLog(Vec<u32>);
impl SceneHost for SceneLog {
    fn scene_remove_render_object_base(&mut self, obj: usize) {
        self.0.extend([20, obj as u32]);
    }
    fn release_ref(&mut self, obj: usize) {
        self.0.extend([3, obj as u32]);
    }
}

/// The events `Apply_Null` makes, in the wrapper's format: 30 state value (render state), 31 and 17 floats (material), 32 stage
/// type value (texture stage state). Render-state and texture-stage ids are d3d8types.h's (see the module comment).
struct NullLog(Vec<u32>);
impl NullMaterialHost for NullLog {
    fn set_lighting(&mut self, on: bool) {
        self.0.extend([30, 137, u32::from(on)]);
    }
    fn set_default_material(&mut self) {
        self.0.push(31);
        // diffuse (1,1,1,1), ambient (1,1,1,1), specular 0, emissive 0, power 1
        self.0.extend(b(&[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]));
    }
    fn set_material_sources(&mut self, ambient: u32, diffuse: u32, emissive: u32) {
        self.0.extend([30, 147, ambient, 30, 145, diffuse, 30, 148, emissive]);
    }
    fn set_stage_passthru(&mut self, stage: i32) {
        // D3DTSS_TCI_PASSTHRU | stage, then D3DTTFF_DISABLE
        self.0.extend([32, stage as u32, 11, stage as u32, 32, stage as u32, 24, 0]);
    }
}

fn tail_log(log: &[u32]) -> Vec<u32> {
    let mut w = vec![log.len() as u32];
    w.extend(log);
    w
}

#[test]
fn small_drafts_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0e01);
    let mut lay: Vec<Check> = ["Set_Scene", "Get_Scene", "Peek_Camera"].iter().map(|n| Check::new(&format!("LayerClass::{n}"))).collect();
    let mut sc: Vec<Check> = ["Remove_All_Render_Objects", "Create_Iterator", "First", "Next", "Is_Done", "Current_Item", "Destroy_Iterator"]
        .iter()
        .map(|n| Check::new(&format!("SimpleScene {n}")))
        .collect();
    let mut vm: Vec<Check> = ["Set_Emissive(Vector3)", "Set_Emissive(r,g,b)", "Get_UV_Source", "Apply_Null"].iter().map(|n| Check::new(&format!("VertexMaterialClass::{n}"))).collect();
    let mut wu: Vec<Check> = ["Convert_Quaternion(W3d -> Quaternion)", "Convert_Quaternion(Quaternion -> W3d)"].iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        // ---- layer
        {
            let s0 = if g.below(3) == 0 { None } else { Some(1 + g.below(8) as usize) };
            let cam = if g.below(3) == 0 { None } else { Some(1 + g.below(8) as usize) };
            let mut l = LayerClass { scene: s0, camera: cam };
            let mut inp = vec![s0.map_or(0, |v| v as u32), cam.map_or(0, |v| v as u32)];
            let nops = 1 + g.below(8);
            inp.push(nops);
            let mut parts = Vec::new();
            for _ in 0..nops {
                let mut host = LayerLog(Vec::new());
                let (k, ret) = match g.below(3) {
                    0 => {
                        let ns = match g.below(4) {
                            0 => None,
                            1 => l.scene,
                            _ => Some(1 + g.below(8) as usize),
                        };
                        inp.extend([0, ns.map_or(0, |v| v as u32)]);
                        l.set_scene(&mut host, ns);
                        (0, 0)
                    }
                    1 => {
                        inp.push(1);
                        (1, l.get_scene(&mut host).map_or(0, |v| v as u32))
                    }
                    _ => {
                        inp.push(2);
                        (2, l.peek_camera().map_or(0, |v| v as u32))
                    }
                };
                let mut w = tail_log(&host.0);
                w.push(ret);
                w.push(l.scene.map_or(0, |v| v as u32));
                parts.push((k, w));
            }
            let cpp = run(ref_dsww3d_layer_run, &inp);
            compare(&mut lay, parts, cpp, case);
        }
        // ---- scene
        {
            let n = g.below(9) as usize;
            let mut s = SimpleScene { render_list: (1..=n).collect() };
            let mut inp = vec![n as u32, 0];
            let nops = 1 + g.below(14);
            let mut done_ops = 0u32;
            let mut it: Option<SimpleSceneIterator> = None;
            let mut parts = Vec::new();
            let mut removed = false;
            for _ in 0..nops {
                let op = g.below(7);
                // an iterator is not used after the list is emptied (the draft walks a copy; see the module comment)
                if removed && op != 0 && op != 1 {
                    continue;
                }
                match op {
                    0 => {
                        inp.push(0);
                        done_ops += 1;
                        let mut host = SceneLog(Vec::new());
                        s.remove_all_render_objects(&mut host);
                        it = None;
                        removed = true;
                        parts.push((0, tail_log(&host.0)));
                    }
                    1 => {
                        let v = g.below(2);
                        inp.extend([1, v]);
                        done_ops += 1;
                        removed = false;
                        if let Some(old) = it.take() {
                            s.destroy_iterator(old);
                        }
                        it = Some(s.create_iterator(v != 0));
                        parts.push((1, tail_log(&[])));
                    }
                    _ => {
                        let Some(i) = it.as_mut() else { continue };
                        inp.push(op as u32);
                        done_ops += 1;
                        let w = match op {
                            2 => {
                                i.first();
                                Vec::new()
                            }
                            3 => {
                                i.next();
                                Vec::new()
                            }
                            4 => vec![u32::from(i.is_done())],
                            5 => vec![i.current_item().map_or(0, |v| v as u32)],
                            _ => {
                                let old = it.take().unwrap();
                                s.destroy_iterator(old);
                                Vec::new()
                            }
                        };
                        let mut w = w;
                        w.extend(tail_log(&[]));
                        parts.push((op as usize, w));
                    }
                }
            }
            inp[1] = done_ops;
            let cpp = run(ref_dsww3d_scene_run, &inp);
            compare(&mut sc, parts, cpp, case);
        }
        // ---- vertex material
        {
            let em = [val(&mut g), val(&mut g), val(&mut g)];
            let uv: [i32; 8] = std::array::from_fn(|_| if g.below(8) == 0 { g.i32() } else { g.below(8) as i32 });
            let mut dirty = g.below(2) == 0;
            let mut m = VertexMaterialState::default();
            m.emissive = em;
            m.uv_source = uv;
            let mut inp: Vec<u32> = b(&em);
            inp.extend(uv.iter().map(|&v| v as u32));
            inp.push(u32::from(dirty));
            let nops = 1 + g.below(8);
            inp.push(nops);
            let mut parts = Vec::new();
            for _ in 0..nops {
                let op = g.below(4) as usize;
                inp.push(op as u32);
                let mut host = NullLog(Vec::new());
                let mut w: Vec<u32> = Vec::new();
                match op {
                    0 => {
                        let v = Vector3::new(val(&mut g), val(&mut g), val(&mut g));
                        inp.extend(b(&[v.x, v.y, v.z]));
                        set_emissive(&mut m, &mut dirty, &v);
                    }
                    1 => {
                        let v = [val(&mut g), val(&mut g), val(&mut g)];
                        inp.extend(b(&v));
                        set_emissive_rgb(&mut m, &mut dirty, v[0], v[1], v[2]);
                    }
                    2 => {
                        let stage = g.below(8) as i32;
                        inp.push(stage as u32);
                        w.push(get_uv_source(&m, stage) as u32);
                    }
                    _ => apply_null(&mut host),
                }
                w.extend(tail_log(&host.0));
                w.extend(b(&m.emissive));
                w.push(u32::from(dirty));
                parts.push((op, w));
            }
            let cpp = run(ref_dsww3d_vmat_run, &inp);
            compare(&mut vm, parts, cpp, case);
        }
        // ---- w3d_util
        {
            let q = [val(&mut g), val(&mut g), val(&mut g), val(&mut g)];
            let mut inp = vec![0];
            inp.extend(b(&q));
            let r = convert_quaternion_from_w3d(&q);
            feq(&mut wu[0], b(&[r.x, r.y, r.z, r.w]), run(ref_dsww3d_wutil_run, &inp), || format!("case {case}"));
            let mut inp = vec![1];
            inp.extend(b(&q));
            let r = convert_quaternion_to_w3d(&Quaternion { x: q[0], y: q[1], z: q[2], w: q[3] });
            feq(&mut wu[1], b(&r), run(ref_dsww3d_wutil_run, &inp), || format!("case {case}"));
        }
    }
    let mut all = lay;
    all.extend(sc);
    all.extend(vm);
    all.extend(wu);
    finish(all);
}
