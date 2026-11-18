// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dgeo) of the non-drawing members of the procedural-shape render objects
//! (`src/drafts/sweep_ww3d/{ringobj,sphereobj,boxrobj}.rs`) against upstream's text cut VERBATIM from `ringobj.cpp`,
//! `sphereobj.cpp`, `boxrobj.cpp` and `rendobj.cpp` (build.rs `drafts_ww3dgeo`, differential/wrap_draft_shapes.cpp):
//!
//! | draft | upstream |
//! |---|---|
//! | `RingShared::generate_shared_mesh_arrays`, `calculate_value_array`, `init_material`, `get_default_*`, `update_cached_box`, `RingMesh::set_tiling` | `RingRenderObjClass::` the same, `RingMeshClass::Set_Tiling` |
//! | `SphereShared::generate_shared_mesh_arrays`, `calculate_value_array`, `init_material`, `get_default_*`, `update_cached_box` | `SphereRenderObjClass::` the same |
//! | `aabox_update_cached_box`, `obbox_update_cached_box` | `AABoxRenderObjClass::update_cached_box`, `OBBoxRenderObjClass::update_cached_box` |
//! | `AT_MIN_LOD`, `AT_MAX_LOD` (`formats::w3d::part_buf`) | `RenderObjClass::AT_MIN_LOD`, `AT_MAX_LOD` |
//!
//! NOT covered (device or buffers): `render_ring`, `render_sphere`, `render_box`, `vis_render_*` (they fill DX8 dynamic
//! buffers and draw), `obbox_get_box` (a call to `Validate_Transform` then `update_cached_box`, tested through the latter),
//! `RingMeshClass::Generate` and the sphere's (the host contract).
//!
//! # Method
//!
//! 20,000 cases per member (fixed seed, `DRAFT_CASES` overrides). Generation runs twice per case (the second call must do
//! nothing: `Ring_Array_Valid`), the host's `Generate` stand-in gives a polygon count of `(int)(size * 12.0f) + 4`, and the
//! stand-ins log every call with its float arguments (see the wrapper), so the LOD size sequence (`size += step` in `float`)
//! and the cost table are compared exactly. The value array is computed from the generated costs with random screen area and
//! bias (specials among them). `Get_Default_*` is tried with and without a first key. Box updates use random matrices.
//!
//! The draft's `AlphaVectorStruct` is `{angle, intensity, axis}`; upstream's is a Quaternion `angle` (X, Y, Z, W) and an
//! intensity. The test maps (axis0, axis1, axis2, angle) to (X, Y, Z, W): the draft type is NOT the upstream type (a defect of
//! the draft's declaration; `get_default_vector` is generic selection and `set_alpha_vector` a host call, so no behaviour is
//! affected). Noted in the report.
//!
//! # Predictions (before the first run, 2026-11-18)
//!
//! One MISS expected: upstream's `SphereRenderObjClass::Init_Material` also calls `SphereShader.Set_Cull_Mode(CULL_MODE_DISABLE)`;
//! the draft's 7-tuple has no place for it (fixed in this change: `SphereMaterialSetup`). Everything else 0 mismatches.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;

use rts_lockstep_engine::drafts::sweep_ww3d::boxrobj::{aabox_update_cached_box, obbox_update_cached_box};
use rts_lockstep_engine::drafts::sweep_ww3d::ringobj::{self as ring, RingHost, RingMesh, RingShared, VertexXyzNduv2, RING_NUM_LOD};
use rts_lockstep_engine::drafts::sweep_ww3d::sphereobj::{self as sphere, AlphaVectorStruct, SphereHost, SphereShared, SPHERE_NUM_LOD};
use rts_lockstep_engine::drafts::wwlib::aabox::AABoxClass;
use rts_lockstep_engine::drafts::wwlib::obbox::OBBoxClass;
use rts_lockstep_engine::formats::w3d::part_buf::{AT_MAX_LOD, AT_MIN_LOD};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dsh_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 20_000];
    let n = unsafe { ref_dsh_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "op {}", input[0]);
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

struct RHost {
    log: Vec<u32>,
}

impl RingHost for RHost {
    fn generate_ring_mesh(&mut self, mesh: &mut RingMesh, size: f32) {
        self.log.extend([420, 1.0f32.to_bits(), size.to_bits()]);
        mesh.face_ct = (size * 12.0) as usize + 4;
    }
    fn convert_color(&self, _color: [f32; 3], _alpha: f32) -> u32 {
        0
    }
    fn draw_ring(&mut self, _verts: &[VertexXyzNduv2], _inds: &[u16], _sort: bool, _face_ct: usize, _vertex_ct: usize) {}
}

struct SHost {
    log: Vec<u32>,
}

impl SphereHost for SHost {
    fn generate_sphere_mesh(&mut self, mesh: &mut ring::RingMesh, size: f32) {
        self.log.extend([421, 1.0f32.to_bits(), size.to_bits(), size.to_bits()]);
        mesh.face_ct = (size * 12.0) as usize + 4;
    }
    fn set_alpha_vector(&mut self, _mesh: &mut ring::RingMesh, v: &AlphaVectorStruct) {
        self.log.extend([422, v.axis[0].to_bits(), v.axis[1].to_bits(), v.axis[2].to_bits(), v.angle.to_bits(), v.intensity.to_bits(), 0, 0]);
    }
    fn convert_dcg_color(&self, _mesh: &ring::RingMesh, _index: usize) -> u32 {
        0
    }
    fn draw_sphere(&mut self, _verts: &[VertexXyzNduv2], _inds: &[u16], _sort: bool, _face_ct: usize, _vertex_ct: usize) {}
}

fn val(g: &mut Gen) -> f32 {
    match g.below(8) {
        0 => g.special(),
        1 => 0.0,
        _ => g.uniform(-30.0, 30.0),
    }
}

fn b(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn mat12(g: &mut Gen) -> [f32; 12] {
    std::array::from_fn(|_| val(g))
}

fn mat_of(w: &[f32; 12]) -> Matrix3D {
    Matrix3D { row: [Vector4::new(w[0], w[1], w[2], w[3]), Vector4::new(w[4], w[5], w[6], w[7]), Vector4::new(w[8], w[9], w[10], w[11])] }
}

fn mat_log(rows: &[(u32, [f32; 3])]) -> Vec<u32> {
    let mut v = Vec::new();
    for (code, x) in rows {
        v.push(*code);
        v.extend(b(x));
    }
    v
}

/// Word-wise comparison where a pair of NaNs with different sign or payload bits is counted apart (tests/support/mod.rs rule).
fn feq(c: &mut Check, rust: Vec<u32>, cpp: Vec<u32>, what: impl FnOnce() -> String) {
    let nan = |w: u32| f32::from_bits(w).is_nan();
    let same = rust.len() == cpp.len() && rust.iter().zip(&cpp).all(|(a, b)| a == b || (nan(*a) && nan(*b)));
    if same && rust != cpp {
        c.compared += 1;
        c.nan_only += 1;
    } else {
        c.eq(rust, cpp, what);
    }
}

fn is_ok<T>(opt: &Option<T>) -> u32 {
    u32::from(opt.is_some())
}

#[test]
fn shapes_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0727);
    let names = [
        "AT_MIN_LOD/AT_MAX_LOD", "Ring Generate_Shared_Mesh_Arrays", "Ring calculate_value_array", "Ring Init_Material",
        "Ring Get_Default_*", "Ring update_cached_box", "RingMeshClass::Set_Tiling", "Sphere Generate_Shared_Mesh_Arrays",
        "Sphere calculate_value_array", "Sphere Init_Material", "Sphere Get_Default_*", "Sphere update_cached_box",
        "AABoxRenderObjClass::update_cached_box", "OBBoxRenderObjClass::update_cached_box",
    ];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    feq(&mut c[0], vec![AT_MIN_LOD.to_bits(), AT_MAX_LOD.to_bits()], run(&[14]), || "constants".into());
    for case in 0..cases() {
        // ---- ring generation, twice, then the value array
        run(&[0]);
        let mut rs = RingShared::default();
        let mut rh = RHost { log: Vec::new() };
        rs.generate_shared_mesh_arrays(&mut rh);
        let cpp = run(&[1]);
        let mut rust: Vec<u32> = rs.lod_costs.iter().map(|x| x.to_bits()).collect();
        rust.extend(rh.log.iter().copied());
        feq(&mut c[1], rust, cpp, || format!("case {case}: first generation"));
        rh.log.clear();
        rs.generate_shared_mesh_arrays(&mut rh);
        let cpp = run(&[1]);
        let mut rust: Vec<u32> = rs.lod_costs.iter().map(|x| x.to_bits()).collect();
        rust.extend(rh.log.iter().copied());
        feq(&mut c[1], rust, cpp, || format!("case {case}: second generation"));
        let (area, bias) = (val(&mut g), val(&mut g));
        let mut values = [0.0f32; RING_NUM_LOD + 2];
        rs.calculate_value_array(area, bias, &mut values);
        feq(&mut c[2], b(&values), run(&[2, area.to_bits(), bias.to_bits()]), || format!("case {case}: area {area} bias {bias}"));
        // ---- ring material
        {
            let (a, d, s, e, o, sh) = ring::init_material();
            let log = mat_log(&[(400, a), (401, d), (402, s), (403, e)]);
            let mut rust = vec![0u32];
            rust.extend(log);
            rust.extend([404, o.to_bits(), 405, sh.to_bits()]);
            feq(&mut c[3], rust, run(&[3]), || "ring material".into());
        }
        // ---- ring defaults
        {
            let kc = if g.below(2) == 0 { Some([val(&mut g), val(&mut g), val(&mut g)]) } else { None };
            let cc = [val(&mut g), val(&mut g), val(&mut g)];
            let ka = if g.below(2) == 0 { Some(val(&mut g)) } else { None };
            let ca = val(&mut g);
            let ki = if g.below(2) == 0 { Some([val(&mut g), val(&mut g)]) } else { None };
            let ci = [val(&mut g), val(&mut g)];
            let ko = if g.below(2) == 0 { Some([val(&mut g), val(&mut g)]) } else { None };
            let co = [val(&mut g), val(&mut g)];
            let mut inp = vec![4, is_ok(&kc)];
            inp.extend(b(&kc.unwrap_or([0.0; 3])));
            inp.extend(b(&cc));
            inp.extend([is_ok(&ka), ka.unwrap_or(0.0).to_bits(), ca.to_bits(), is_ok(&ki)]);
            inp.extend(b(&ki.unwrap_or([0.0; 2])));
            inp.extend(b(&ci));
            inp.push(is_ok(&ko));
            inp.extend(b(&ko.unwrap_or([0.0; 2])));
            inp.extend(b(&co));
            let mut rust = b(&ring::get_default_color(kc, cc));
            rust.push(ring::get_default_alpha(ka, ca).to_bits());
            rust.extend(b(&ring::get_default_inner_scale(ki, ci)));
            rust.extend(b(&ring::get_default_outer_scale(ko, co)));
            feq(&mut c[4], rust, run(&inp), || format!("case {case}: ring defaults"));
        }
        // ---- ring cached box
        {
            let m = mat12(&mut g);
            let (ce, ex) = ([val(&mut g), val(&mut g), val(&mut g)], [val(&mut g), val(&mut g), val(&mut g)]);
            let tr = [m[3], m[7], m[11]];
            let (rc, re) = ring::update_cached_box(tr, ce, ex);
            let mut inp = vec![5];
            inp.extend(b(&m));
            inp.extend(b(&ce));
            inp.extend(b(&ex));
            let mut rust = b(&rc);
            rust.extend(b(&re));
            feq(&mut c[5], rust, run(&inp), || format!("case {case}: ring cached box"));
            // the sphere's, the AABox's and the OBBox's with the same operands
            let (sc, se) = sphere::update_cached_box(tr, ce, ex);
            inp[0] = 11;
            let mut rust = b(&sc);
            rust.extend(b(&se));
            feq(&mut c[11], rust, run(&inp), || format!("case {case}: sphere cached box"));
            let mut ab = AABoxClass::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0));
            aabox_update_cached_box(&mut ab, &mat_of(&m), &Vector3::new(ce[0], ce[1], ce[2]), &Vector3::new(ex[0], ex[1], ex[2]));
            inp[0] = 12;
            feq(&mut c[12], b(&[ab.center.x, ab.center.y, ab.center.z, ab.extent.x, ab.extent.y, ab.extent.z]), run(&inp), || format!("case {case}: AABox cached box"));
            let mut ob = OBBoxClass::new(&Vector3::new(0.0, 0.0, 0.0), &Vector3::new(0.0, 0.0, 0.0));
            obbox_update_cached_box(&mut ob, &mat_of(&m), &Vector3::new(ce[0], ce[1], ce[2]), &Vector3::new(ex[0], ex[1], ex[2]));
            inp[0] = 13;
            let mut rust = b(&[ob.center.x, ob.center.y, ob.center.z, ob.extent.x, ob.extent.y, ob.extent.z]);
            for r in 0..3 {
                for col in 0..3 {
                    rust.push(ob.basis[r][col].to_bits());
                }
            }
            feq(&mut c[13], rust, run(&inp), || format!("case {case}: OBBox cached box"));
        }
        // ---- Set_Tiling
        {
            let slices = g.below(70) as i32 - 3;
            let tile = g.below(8) as i32;
            let count = if g.below(4) == 0 { tile } else { g.below(12) as i32 };
            let vct = 2 * g.below(20) as usize;
            let uv: Vec<[f32; 2]> = (0..vct).map(|_| [val(&mut g), val(&mut g)]).collect();
            let mut m = RingMesh { slices, tile_count: tile, vertex_ct: vct, vtx_uv: uv.clone(), ..Default::default() };
            m.set_tiling(count);
            let mut inp = vec![6, slices as u32, tile as u32, vct as u32, count as u32];
            for p in &uv {
                inp.extend(b(p));
            }
            let mut rust = vec![m.tile_count as u32];
            for p in &m.vtx_uv {
                rust.extend(b(p));
            }
            feq(&mut c[6], rust, run(&inp), || format!("case {case}: slices {slices} tile {tile} count {count} vertices {vct}"));
        }
        // ---- sphere generation, twice, then the value array
        {
            let av = AlphaVectorStruct { angle: val(&mut g), intensity: val(&mut g), axis: [val(&mut g), val(&mut g), val(&mut g)] };
            let mut inp = vec![7];
            inp.extend(b(&av.axis));
            inp.extend([av.angle.to_bits(), av.intensity.to_bits()]);
            let mut ss = SphereShared::default();
            let mut sh = SHost { log: Vec::new() };
            ss.generate_shared_mesh_arrays(&mut sh, &av);
            let mut rust: Vec<u32> = ss.lod_costs.iter().map(|x| x.to_bits()).collect();
            rust.extend(sh.log.iter().copied());
            feq(&mut c[7], rust, run(&inp), || format!("case {case}: sphere first generation"));
            sh.log.clear();
            ss.generate_shared_mesh_arrays(&mut sh, &av);
            let mut rust: Vec<u32> = ss.lod_costs.iter().map(|x| x.to_bits()).collect();
            rust.extend(sh.log.iter().copied());
            feq(&mut c[7], rust, run(&inp), || format!("case {case}: sphere second generation"));
            let (area, bias) = (val(&mut g), val(&mut g));
            let mut values = [0.0f32; SPHERE_NUM_LOD + 2];
            ss.calculate_value_array(area, bias, &mut values);
            feq(&mut c[8], b(&values), run(&[8, area.to_bits(), bias.to_bits()]), || format!("case {case}: sphere area {area} bias {bias}"));
        }
        // ---- sphere material
        {
            let m = sphere::init_material();
            let mut rust = vec![0u32];
            rust.extend(mat_log(&[(400, m.ambient), (401, m.diffuse), (402, m.specular), (403, m.emissive)]));
            rust.extend([404, m.opacity.to_bits(), 405, m.shininess.to_bits(), 406, u32::from(m.lighting)]);
            if m.cull_mode_disabled {
                rust.extend([410, 0]);
            }
            feq(&mut c[9], rust, run(&[9]), || "sphere material".into());
        }
        // ---- sphere defaults
        {
            let kc = if g.below(2) == 0 { Some([val(&mut g), val(&mut g), val(&mut g)]) } else { None };
            let cc = [val(&mut g), val(&mut g), val(&mut g)];
            let ka = if g.below(2) == 0 { Some(val(&mut g)) } else { None };
            let ca = val(&mut g);
            let ks = if g.below(2) == 0 { Some([val(&mut g), val(&mut g), val(&mut g)]) } else { None };
            let cs = [val(&mut g), val(&mut g), val(&mut g)];
            let mk = |g: &mut Gen| AlphaVectorStruct { angle: val(g), intensity: val(g), axis: [val(g), val(g), val(g)] };
            let kv = if g.below(2) == 0 { Some(mk(&mut g)) } else { None };
            let cv = mk(&mut g);
            let words = |v: &AlphaVectorStruct| vec![v.axis[0].to_bits(), v.axis[1].to_bits(), v.axis[2].to_bits(), v.angle.to_bits(), v.intensity.to_bits()];
            let mut inp = vec![10, is_ok(&kc)];
            inp.extend(b(&kc.unwrap_or([0.0; 3])));
            inp.extend(b(&cc));
            inp.extend([is_ok(&ka), ka.unwrap_or(0.0).to_bits(), ca.to_bits(), is_ok(&ks)]);
            inp.extend(b(&ks.unwrap_or([0.0; 3])));
            inp.extend(b(&cs));
            inp.push(is_ok(&kv));
            inp.extend(words(&kv.unwrap_or(AlphaVectorStruct { angle: 0.0, intensity: 0.0, axis: [0.0; 3] })));
            inp.extend(words(&cv));
            let mut rust = b(&sphere::get_default_color(kc, cc));
            rust.push(sphere::get_default_alpha(ka, ca).to_bits());
            rust.extend(b(&sphere::get_default_scale(ks, cs)));
            rust.extend(words(&sphere::get_default_vector(kv, cv)));
            feq(&mut c[10], rust, run(&inp), || format!("case {case}: sphere defaults"));
        }
    }
    finish(c);
}
