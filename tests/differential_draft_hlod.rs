// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group sweepww3d) of the draft `src/drafts/sweep_ww3d/hlod.rs` (`HLodClass`'s
//! `Get_Max_Screen_Size`, `Get_Lod_Model_Count`, `Get_Lod_Model`, `Get_Lod_Model_Bone`, `Get_Additional_Model_Count`,
//! `Peek_Additional_Model`, `Get_Additional_Model`, `Get_Additional_Model_Bone`, `Is_NULL_Lod_Included`, `Get_Proxy_Count`,
//! `Get_Proxy`, `Add_Sub_Object`, `Get_Num_Sub_Objects_On_Bone`, `Get_Current_LOD`, `Get_Num_Snap_Points`, `Get_Snap_Point`,
//! `Set_HTree`) against upstream `WW3D2/hlod.cpp` cut VERBATIM (build.rs `drafts_sweepww3d`, differential/wrap_draft_hlod.cpp).
//! Stand-ins: `RenderObjClass` (a reference count and `Class_ID`; one object per handle), the HLod's fields, a hierarchy tree that
//! returns a table of bone transforms and logs `Base_Update`/`Get_Transform`, `Set_Hierarchy_Valid`, `Add_Sub_Object_To_Bone` and
//! `Animatable3DObjClass::Set_HTree`, which log their arguments.
//!
//! # Method
//!
//! 20,000 HLods (fixed seed, `DRAFT_CASES` overrides): 0 to 4 LOD levels of 0 to 4 models (handles into a table of 12 objects,
//! some NULL, some of class `CLASSID_NULL`), `MaxScreenSize` values (some `NO_MAX_SCREEN_SIZE`), 0 to 4 additional models, a
//! proxy array or none (names of 0 to 5 characters, bones into a table of 6 transforms), snap points or none, `CurLod` in and
//! out of range; then 1 to 12 operations drawn from all seventeen members. Compared: each returned value, the calls the
//! stand-ins saw, and the change of every object's reference count. The draft does not hold references (`Add_Ref` is the
//! host's, documented); the test compares the C++ reference-count change with `1` when the draft returned a handle (the
//! contract), `0` otherwise.
//!
//! # Excluded inputs
//!
//! Upstream `WWASSERT`s are compiled out, so out-of-range indices are tolerated where the code guards them and read out of bounds
//! where it does not: `Get_Lod_Model` and `Get_Lod_Model_Bone` do not test `model_index >= 0` (a negative index reads before the
//! array), `Is_NULL_Lod_Included` reads `Lod[0][0]` of an empty first LOD, `Get_Proxy` and `Get_Snap_Point` index without a
//! range test. None of those is generated: negative model indices are not used, `Is_NULL_Lod_Included` is only asked when the
//! first LOD is non-empty, proxy and snap-point indices are in range.
//!
//! # Predictions (written before the first run, 2026-11-19)
//!
//! 0 mismatches. Places I am unsure of: the draft's `get_current_lod` (count then model of `CurLod`).
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::sweep_ww3d::hlod::*;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector4::Vector4;
use std::cell::RefCell;
use std::sync::Mutex;
use support::{Check, Gen};
use support_sweep::{b, cases, feq, finish};

extern "C" {
    fn ref_dsww3d_hlod_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 20_000];
    let n = unsafe { ref_dsww3d_hlod_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

const NOBJ: u32 = 12;

fn handle(g: &mut Gen) -> Option<usize> {
    if g.below(6) == 0 {
        None
    } else {
        Some(1 + g.below(NOBJ) as usize)
    }
}

fn hw(h: Option<usize>) -> u32 {
    h.map_or(0, |v| v as u32)
}

fn mat_of(w: &[f32; 12]) -> Matrix3D {
    Matrix3D { row: [Vector4::new(w[0], w[1], w[2], w[3]), Vector4::new(w[4], w[5], w[6], w[7]), Vector4::new(w[8], w[9], w[10], w[11])] }
}

fn mat_words(m: &Matrix3D) -> Vec<u32> {
    m.row.iter().flat_map(|r| b(&[r.x, r.y, r.z, r.w])).collect()
}

fn val(g: &mut Gen) -> f32 {
    match g.below(6) {
        0 => g.special(),
        1 => 0.0,
        _ => g.uniform(-30.0, 30.0),
    }
}

/// What the C++ would log for one operation, and the reference-count change, in the wrapper's format.
fn tail(log: &[u32], refs: &[(u32, u32)]) -> Vec<u32> {
    let mut w = vec![log.len() as u32];
    w.extend(log);
    for (h, d) in refs {
        w.push(*h);
        w.push(*d);
    }
    w.push(0);
    w
}

#[test]
fn hlod_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0d01);
    let names = [
        "Get_Max_Screen_Size", "Get_Lod_Model_Count", "Get_Lod_Model", "Get_Lod_Model_Bone", "Get_Additional_Model_Count", "Peek_Additional_Model",
        "Get_Additional_Model", "Get_Additional_Model_Bone", "Is_NULL_Lod_Included", "Get_Proxy_Count", "Get_Proxy", "Add_Sub_Object",
        "Get_Num_Sub_Objects_On_Bone", "Get_Current_LOD", "Get_Num_Snap_Points", "Get_Snap_Point", "Set_HTree",
    ];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        let class_ids: Vec<u32> = (0..NOBJ).map(|_| if g.below(4) == 0 { 13 } else { 0 }).collect();
        let lod_count = g.below(5) as i32;
        let cur_lod = g.below(7) as i32 - 1;
        let mut st = HLodState { lod_count, cur_lod, ..Default::default() };
        let mut inp: Vec<u32> = vec![NOBJ];
        inp.extend(&class_ids);
        inp.push(lod_count as u32);
        inp.push(cur_lod as u32);
        for l in 0..lod_count {
            let mut arr = ModelArray::new();
            arr.max_screen_size = if g.below(4) == 0 { NO_MAX_SCREEN_SIZE } else { val(&mut g) };
            let n = if l == 0 && g.below(2) == 0 { 1 + g.below(3) } else { g.below(5) };
            inp.push(arr.max_screen_size.to_bits());
            inp.push(n);
            for _ in 0..n {
                let (h, bone) = (handle(&mut g), g.below(6) as i32);
                arr.models.push(ModelNode { model: h, bone_index: bone });
                inp.extend([hw(h), bone as u32]);
            }
            st.lod.push(arr);
        }
        let nadd = g.below(5);
        inp.push(nadd);
        for _ in 0..nadd {
            let (h, bone) = (handle(&mut g), g.below(6) as i32);
            st.additional_models.push(ModelNode { model: h, bone_index: bone });
            inp.extend([hw(h), bone as u32]);
        }
        let has_proxy = g.below(2) == 0;
        inp.push(u32::from(has_proxy));
        let mut names_of: Vec<String> = Vec::new();
        if has_proxy {
            let np = g.below(5);
            inp.push(np);
            let mut arr = Vec::new();
            for _ in 0..np {
                let bone = g.below(6) as i32;
                let len = g.below(6);
                let name: String = (0..len).map(|_| (b'a' + g.below(26) as u8) as char).collect();
                inp.extend([bone as u32, len]);
                inp.extend(name.bytes().map(u32::from));
                arr.push(ProxyRecord { name: name.clone(), bone_index: bone });
                names_of.push(name);
            }
            st.proxy_array = Some(arr);
        }
        let has_snap = g.below(2) == 0;
        inp.push(u32::from(has_snap));
        if has_snap {
            let ns = g.below(4);
            inp.push(ns);
            let mut pts = Vec::new();
            for _ in 0..ns {
                let p = [val(&mut g), val(&mut g), val(&mut g)];
                inp.extend(b(&p));
                pts.push(p);
            }
            st.snap_points = Some(pts);
        }
        let bones: Vec<[f32; 12]> = (0..6).map(|_| std::array::from_fn(|_| val(&mut g))).collect();
        inp.push(6);
        for m in &bones {
            inp.extend(b(m));
        }
        let identity = Matrix3D::IDENTITY;
        let first_lod_nonempty = st.lod.first().map_or(false, |a| !a.models.is_empty());
        let nops = 1 + g.below(12);
        inp.push(nops);
        let mut rust: Vec<(usize, Vec<u32>)> = Vec::new();
        for _ in 0..nops {
            let mut op = g.below(17) as usize;
            if op == 8 && lod_count > 0 && !first_lod_nonempty {
                op = 9;
            }
            // an index into a present but empty array is out of bounds upstream (no range test): not asked
            if op == 10 && st.proxy_array.as_ref().map_or(false, |a| a.is_empty()) {
                op = 9;
            }
            if op == 15 && st.snap_points.as_ref().map_or(false, |a| a.is_empty()) {
                op = 14;
            }
            inp.push(op as u32);
            let log = RefCell::new(Vec::<u32>::new());
            let mut refs: Vec<(u32, u32)> = Vec::new();
            let mut w: Vec<u32>;
            let ref_of = |h: Option<usize>, refs: &mut Vec<(u32, u32)>| {
                if let Some(h) = h {
                    refs.push((h as u32, 1));
                }
            };
            match op {
                0 => {
                    let i = g.below(7) as i32 - 1;
                    inp.push(i as u32);
                    w = vec![st.get_max_screen_size(i).to_bits()];
                }
                1 => {
                    let i = g.below(7) as i32 - 1;
                    inp.push(i as u32);
                    w = vec![st.get_lod_model_count(i) as u32];
                }
                2 => {
                    let (i, m) = (g.below(7) as i32 - 1, g.below(6) as i32);
                    inp.extend([i as u32, m as u32]);
                    let r = st.get_lod_model(i, m);
                    ref_of(r, &mut refs);
                    w = vec![hw(r)];
                }
                3 => {
                    let (i, m) = (g.below(7) as i32 - 1, g.below(6) as i32);
                    inp.extend([i as u32, m as u32]);
                    w = vec![st.get_lod_model_bone(i, m) as u32];
                }
                4 => w = vec![st.get_additional_model_count() as u32],
                5 => {
                    let i = g.below(7) as i32 - 1;
                    inp.push(i as u32);
                    w = vec![hw(st.peek_additional_model(i))];
                }
                6 => {
                    let i = g.below(7) as i32 - 1;
                    inp.push(i as u32);
                    let r = st.get_additional_model(i);
                    ref_of(r, &mut refs);
                    w = vec![hw(r)];
                }
                7 => {
                    let i = g.below(7) as i32 - 1;
                    inp.push(i as u32);
                    w = vec![st.get_additional_model_bone(i) as u32];
                }
                8 => w = vec![u32::from(st.is_null_lod_included(|h| class_ids[h - 1] == 13))],
                9 => w = vec![st.get_proxy_count() as u32],
                10 => {
                    let n = st.proxy_array.as_ref().map_or(0, |a| a.len());
                    let idx = if n > 0 { g.below(n as u32) } else { g.below(5) };
                    inp.push(idx);
                    let (ok, proxy) = st.get_proxy(
                        idx as i32,
                        |bone| {
                            log.borrow_mut().extend([1, 2, 4]);
                            mat_of(&bones[bone as usize])
                        },
                        identity,
                    );
                    w = vec![u32::from(ok), proxy.name.len() as u32];
                    w.extend(proxy.name.bytes().map(u32::from));
                    w.extend(mat_words(&proxy.transform));
                }
                11 => {
                    let h = 1 + g.below(NOBJ) as usize;
                    inp.push(h as u32);
                    let r = st.add_sub_object(h, |_, sub, bone| {
                        log.borrow_mut().extend([5, bone as u32]);
                        1000 + bone + sub as i32
                    });
                    w = vec![r as u32];
                }
                12 => {
                    let bone = g.below(6) as i32;
                    inp.push(bone as u32);
                    w = vec![st.get_num_sub_objects_on_bone(bone) as u32];
                }
                13 => {
                    let r = st.get_current_lod();
                    ref_of(r, &mut refs);
                    w = vec![hw(r)];
                }
                14 => w = vec![st.get_num_snap_points() as u32],
                15 => {
                    let n = st.snap_points.as_ref().map_or(0, |a| a.len());
                    let idx = if n > 0 { g.below(n as u32) } else { g.below(4) };
                    inp.push(idx);
                    w = b(&st.get_snap_point(idx as i32));
                }
                _ => {
                    let present = g.below(2);
                    inp.push(present);
                    st.set_htree(present, |_, t| log.borrow_mut().extend([3, t]));
                    w = Vec::new();
                }
            }
            w.extend(tail(&log.borrow(), &refs));
            rust.push((op, w));
        }
        let cpp = run(&inp);
        let mut at = 0;
        for (op, w) in rust {
            let end = (at + w.len()).min(cpp.len());
            feq(&mut c[op], w, cpp[at..end].to_vec(), || format!("case {case} op {op}"));
            at = end;
        }
        if at != cpp.len() {
            c[0].eq(at, cpp.len(), || format!("case {case}: output length"));
        }
    }
    finish(c);
}
