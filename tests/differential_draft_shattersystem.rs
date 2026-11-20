// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d4) of `set_plane_from_transform` in the draft `src/drafts/sweep_ww3d/shattersystem.rs`
//! against upstream `WW3D2/shattersystem.cpp` `BSPClass::Set_Plane_From_Transform`, cut VERBATIM (build.rs `drafts_ww3d4`,
//! differential/wrap_draft_dw4_shatter.cpp). Stand-in (C++): `BSPClass` holds only the `PlaneClass Plane` member the cut writes;
//! `Matrix3D` and `PlaneClass` are the real headers.
//!
//! # Method
//!
//! 20,000 transforms (fixed seed, `DRAFT_CASES` overrides) of random finite floats, with zeros, huge and tiny values mixed in
//! (the `Gen::mixed` mix), and a random initial plane (so a normal that was not written shows). `N` and `D` are compared bit for bit.
//!
//! # Not covered
//!
//! `ShatterFragments::{get_fragment_count, get_fragment, peek_fragment}`: the upstream versions read a
//! `DynamicVectorClass<RenderObjClass*>` and `Add_Ref` the render object (the whole renderer); the draft's are an indexing of a
//! `Vec<Option<usize>>` with no arithmetic. In the shipped game the pattern list is empty (see the draft), so they are not reached.
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::sweep_ww3d::shattersystem::set_plane_from_transform;
use rts_lockstep_engine::drafts::ww3d::shattersystem::BSPClass;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::plane::PlaneClass;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use std::sync::Mutex;
use support::{Check, Gen};
use support_sweep::{cases, finish};

extern "C" {
    fn ref_dw4_shatter_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

#[test]
fn set_plane_from_transform_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_d407);
    let mut c = Check::new("BSPClass::Set_Plane_From_Transform");
    for case in 0..cases() {
        let m: [f32; 12] = std::array::from_fn(|_| g.mixed());
        let init: [f32; 4] = std::array::from_fn(|_| g.mixed());
        let mut inp: Vec<u32> = m.iter().map(|x| x.to_bits()).collect();
        inp.extend(init.iter().map(|x| x.to_bits()));
        let tm = Matrix3D { row: [Vector4::new(m[0], m[1], m[2], m[3]), Vector4::new(m[4], m[5], m[6], m[7]), Vector4::new(m[8], m[9], m[10], m[11])] };
        let mut bsp = BSPClass { plane: PlaneClass { n: Vector3::new(init[0], init[1], init[2]), d: init[3] }, front: None, back: None, front_leaf_index: -1, back_leaf_index: -1 };
        set_plane_from_transform(&mut bsp, &tm);
        let rust = [bsp.plane.n.x, bsp.plane.n.y, bsp.plane.n.z, bsp.plane.d];
        let mut out = [0u32; 4];
        let n = unsafe { ref_dw4_shatter_run(inp.as_ptr(), inp.len() as i32, out.as_mut_ptr(), 4) };
        assert_eq!(n, 4);
        let cpp = out.map(f32::from_bits);
        c.f32s(&rust, &cpp, || format!("case {case}: {inp:?}"));
    }
    finish(vec![c]);
}
