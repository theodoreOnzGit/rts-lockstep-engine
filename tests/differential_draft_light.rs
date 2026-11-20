// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d5) of the drafts `src/drafts/ww3d/light.rs` and `lightenvironment.rs` against upstream
//! `WW3D2/light.h`, `light.cpp` (constructor, accessors, `Set_Spot_Angle`, bounding volumes, `Load_W3D`) and `lightenvironment.cpp`
//! (`InputLightStruct`, `OutputLightStruct`, `LightEnvironmentClass`), cut verbatim by build.rs `drafts_ww3d5`
//! (differential/wrap_draft_ww3d5_light.cpp lists the one edit and the stand-ins).
//!
//! # Method
//! `DRAFT_CASES` (default 20,000) cases per check from a fixed seed; every output word compared bit for bit.
//! - light state: a light of a random type given random intensity, colours, near/far ranges (doubles, including ranges equal or within
//!   `WWMATH_EPSILON`), flags, spot data and transform; the state read back through every getter.
//! - `Load_W3D`: a well-formed `W3D_CHUNK_LIGHT` (light info first; spot, near and far chunks in any order, repeated or missing;
//!   unknown chunks; every attribute value), 0..=6 trailing chunks.
//! - environment: 0..=7 lights (point, spot, directional) around a centre (some on it), colours around the 0.05 black cut, fill
//!   intensity 0 or not, optional `Pre_Render_Update`, then post-operations (`Add_Fill_Light`, `Calculate_Fill_Light`, `Pre_Render_Update`
//!   in any order and repeated), with the LOD cutoff left at its initial value for the first 2,000 cases and set to a random value
//!   afterwards; `operator==` on pairs that are equal or differ in one place.
//!
//! # Excluded inputs
//! - `Load_W3D` of a chunk shorter than its structure (upstream ignores the failed `Read` and uses an uninitialised struct).
//! - NaN / infinite colours, positions and ranges (float to int undefined behaviour in `HSV_To_RGB`).
//! - LOD cutoff values outside |x| < 40,000 (the int product overflows).
//! - The `m_*` point fields of lights that are not point lights (upstream leaves them uninitialised for a directional light, and
//!   nothing reads them unless `isPointLight`).
//!
//! # Predictions (before the first run, 2026-11-20)
//! One mismatch class expected: the draft keeps `_LightingLODCutoff` as `float` 0.5 / 0.25 with a note "NOT RE-VERIFIED"; upstream's
//! declaration has no type and means `int`, so 0 and 0 until set, and `Set_Lighting_LOD_Cutoff` truncates. Everything else 0.
//!
//! # Results
//! 2026-11-20, `DRAFT_CASES` default: **0 mismatches** in every check after one draft fix. Compared: initial LOD cutoff 1; light state 20,000;
//! `Load_W3D` 20,000; environments 19,290 (1,133 more skipped, see below); `operator==` 19,577; max ULP 0, no NaN-payload-only.
//! **Prediction held** (the one expected class, the LOD cutoff, was the only draft defect): the first run measured 1/1 initial LOD cutoff
//! mismatching (draft 0.5, upstream's implicit `int` 0); the light state and `Load_W3D` first mismatched 13,293 and 13,173 times because the
//! wrapper had not initialised `WWMath`'s sine table (a wrapper defect, `Fast_Cos` read zeros; fixed in the wrapper, not the draft).
//! The environment check then showed NaN cases: when the fill light averages lights with a NaN (a spot of angle 0) or with a zero primary
//! contribution, upstream's `RGB_To_HSV` leaves `Vector3 temp` uninitialised; those cases (1,133 of 20,000) are skipped.
//! Draft fix: `LightingLodCutoff` holds `int`s (truncating `Set_Lighting_LOD_Cutoff`, 0 and 0 initially), as upstream's typeless declarations.
//! Not covered: `Save_W3D`, `Save`, `Load`, the persist factory, `Notify_Added`/`Removed`, `Clone` (not translated or scene-bound).
//!
//! # Planted bugs (`L9_PLANT=<id>`)
//! Mismatching cases per check (initial LOD, state, Load_W3D, environment, equality):
//! 18661 (far attenuation in f32) 0/0/0/1030/0; 18662 (spot direction sign) 0/0/0/3257/97; 18663 (`>=` for the LOD reject) 0/0/0/1093/58;
//! 18664 (black test `<=`) 0/0/0/494/56; 18665 (insert on ties) 0/0/0/5094/12; 18666 (fill averages 4 lights) 0/0/0/3395/9;
//! 18667 (zero output direction sets y) 0/0/0/2827/0; 18668 (ambient z not clamped) 0/0/0/6556/318; 18669 (Set_Flag false xors)
//! 0/2964/0/1126/132; 18670 (`cos` for `Fast_Cos`) 0/9893/13126/1781/0; 18671 (near attenuation sets the far flag) 0/0/4230/0/0;
//! 18672 (spot cos default 0.7071) 0/6707/6827/386/0; 18673 (LOD rounds) 0/0/0/6843/129; 18674 (`==` ignores output direction) 0/0/0/0/1098.
//! **14 of 14 caught.**

mod support;

use rts_lockstep_engine::drafts::ww3d::light::{LightClass, LightType, FAR_ATTENUATION, NEAR_ATTENUATION};
use rts_lockstep_engine::drafts::ww3d::lightenvironment::{LightEnvironmentClass, LightingLodCutoff};
use rts_lockstep_engine::formats::w3d::w3d_file::*;
use rts_lockstep_engine::ww::chunkio::{ChunkLoad, GameFile};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dw5_light(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 100_000];
    let n = unsafe { ref_dw5_light(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "wrapper refused op {}", input[0]);
    out.truncate(n as usize);
    out
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp {}", c.name, c.compared, c.mismatches, c.nan_only, c.max_ulp);
        bad += c.mismatches;
    }
    if !planted {
        assert_eq!(bad, 0, "mismatches against the C++");
    }
    if planted {
        println!("PLANTED mismatches {bad}");
        assert!(bad > 0, "plant not caught");
    }
}

fn f(x: f32) -> u32 {
    x.to_bits()
}
fn d_words(x: f64) -> [u32; 2] {
    let b = x.to_bits();
    [b as u32, (b >> 32) as u32]
}

/// A random float of the light's colour/geometry kind: finite, with exact boundary values.
fn val(g: &mut Gen, lo: f32, hi: f32) -> f32 {
    match g.below(12) {
        0 => 0.0,
        1 => 0.05,
        2 => f32::from_bits(0.05f32.to_bits() - 1),
        3 => f32::from_bits(0.05f32.to_bits() + 1),
        4 => 1.0,
        5 => 0.5,
        _ => g.uniform(lo, hi),
    }
}

#[derive(Clone)]
struct Ld {
    ty: u32,
    intensity: f32,
    amb: [f32; 3],
    dif: [f32; 3],
    spec: [f32; 3],
    far: [f64; 2],
    near: [f64; 2],
    far_flag: u32,
    near_flag: u32,
    shadows: u32,
    set_far: u32,
    set_near: u32,
    spot_set: u32,
    angle: f32,
    dir: [f32; 3],
    exp: f32,
    tm: [f32; 12],
}

fn dbl(g: &mut Gen) -> f64 {
    match g.below(4) {
        0 => f64::from(g.uniform(0.0, 200.0)),
        1 => f64::from_bits(f64::to_bits(g.uniform(0.0, 200.0) as f64) + u64::from(g.below(1 << 20))),
        2 => f64::from(g.below(300)),
        _ => f64::from(g.uniform(-10.0, 400.0)),
    }
}

fn ld(g: &mut Gen, centre: [f32; 3]) -> Ld {
    let ty = g.below(3);
    let mut far = [dbl(g), dbl(g)];
    match g.below(6) {
        0 => far[1] = far[0],
        1 => far[1] = far[0] + 0.00009,
        2 => far[1] = far[0] + 0.0001,
        3 => far[1] = far[0] + 0.00011,
        _ => {}
    }
    let mut tm = [0f32; 12];
    match g.below(4) {
        0 => {
            tm = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        }
        1 => {
            for v in tm.iter_mut() {
                *v = g.uniform(-1.0, 1.0);
            }
        }
        _ => {
            // a rotation about a random axis
            let mut ax = [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)];
            let l = (ax[0] * ax[0] + ax[1] * ax[1] + ax[2] * ax[2]).max(1e-3).sqrt();
            for a in ax.iter_mut() {
                *a /= l;
            }
            let (s, c) = g.uniform(-3.2, 3.2).sin_cos();
            let t = 1.0 - c;
            tm = [
                t * ax[0] * ax[0] + c, t * ax[0] * ax[1] - s * ax[2], t * ax[0] * ax[2] + s * ax[1], 0.0,
                t * ax[0] * ax[1] + s * ax[2], t * ax[1] * ax[1] + c, t * ax[1] * ax[2] - s * ax[0], 0.0,
                t * ax[0] * ax[2] - s * ax[1], t * ax[1] * ax[2] + s * ax[0], t * ax[2] * ax[2] + c, 0.0,
            ];
        }
    }
    // the position: on the object, or somewhere
    if g.below(6) == 0 {
        tm[3] = centre[0];
        tm[7] = centre[1];
        tm[11] = centre[2];
    } else {
        tm[3] = g.uniform(-60.0, 60.0);
        tm[7] = g.uniform(-60.0, 60.0);
        tm[11] = g.uniform(-60.0, 60.0);
    }
    let mut dir = [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)];
    if g.below(3) == 0 {
        dir = [0.0, 0.0, 1.0];
    }
    Ld {
        ty,
        intensity: match g.below(5) {
            0 => 1.0,
            1 => 0.0,
            2 => 2.0,
            _ => g.uniform(0.0, 3.0),
        },
        amb: [val(g, 0.0, 1.0), val(g, 0.0, 1.0), val(g, 0.0, 1.0)],
        dif: [val(g, 0.0, 1.5), val(g, 0.0, 1.5), val(g, 0.0, 1.5)],
        spec: [val(g, 0.0, 1.0), val(g, 0.0, 1.0), val(g, 0.0, 1.0)],
        far,
        near: [dbl(g), dbl(g)],
        far_flag: if g.below(4) == 0 { 0 } else if g.below(5) == 0 { 2 } else { 1 },
        near_flag: g.below(3),
        shadows: g.below(2),
        set_far: u32::from(g.below(5) != 0),
        set_near: g.below(2),
        spot_set: u32::from(g.below(3) != 0),
        angle: match g.below(4) {
            0 => 0.0,
            1 => g.uniform(0.0, 3.2),
            2 => g.uniform(-4.0, 4.0),
            _ => 0.785398,
        },
        dir,
        exp: g.uniform(0.0, 10.0),
        tm,
    }
}

fn ld_words(l: &Ld) -> Vec<u32> {
    let mut w = vec![l.ty, f(l.intensity)];
    w.extend(l.amb.iter().map(|x| f(*x)));
    w.extend(l.dif.iter().map(|x| f(*x)));
    w.extend(l.spec.iter().map(|x| f(*x)));
    for x in [l.far[0], l.far[1], l.near[0], l.near[1]] {
        w.extend(d_words(x));
    }
    w.extend([l.far_flag, l.near_flag, l.shadows, l.set_far, l.set_near, l.spot_set, f(l.angle)]);
    w.extend(l.dir.iter().map(|x| f(*x)));
    w.push(f(l.exp));
    w.extend(l.tm.iter().map(|x| f(*x)));
    w
}

fn v3(a: [f32; 3]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}

fn make_light(l: &Ld) -> LightClass {
    let ty = [LightType::Point, LightType::Directional, LightType::Spot][l.ty as usize];
    let mut x = LightClass::new(ty);
    x.set_intensity(l.intensity);
    x.set_ambient(&v3(l.amb));
    x.set_diffuse(&v3(l.dif));
    x.set_specular(&v3(l.spec));
    if l.set_far != 0 {
        x.set_far_attenuation_range(l.far[0], l.far[1]);
    }
    if l.set_near != 0 {
        x.set_near_attenuation_range(l.near[0], l.near[1]);
    }
    if l.far_flag != 0 {
        x.set_flag(FAR_ATTENUATION, l.far_flag == 1);
    }
    if l.near_flag != 0 {
        x.set_flag(NEAR_ATTENUATION, l.near_flag == 1);
    }
    x.enable_shadows(l.shadows != 0);
    if l.spot_set != 0 {
        x.set_spot_angle(l.angle);
        x.set_spot_direction(&v3(l.dir));
        x.set_spot_exponent(l.exp);
    }
    x.transform = Matrix3D::from_array(l.tm);
    x
}

fn vw(v: &Vector3) -> [u32; 3] {
    [f(v.x), f(v.y), f(v.z)]
}

fn state(x: &LightClass) -> Vec<u32> {
    let mut w = vec![x.get_type() as u32, u32::from(x.get_flag(NEAR_ATTENUATION)), u32::from(x.get_flag(FAR_ATTENUATION)), u32::from(x.are_shadows_enabled()), f(x.get_intensity())];
    w.extend(vw(&x.get_ambient()));
    w.extend(vw(&x.get_diffuse()));
    w.extend(vw(&x.get_specular()));
    let (a, b) = x.get_near_attenuation_range();
    w.extend(d_words(a));
    w.extend(d_words(b));
    let (a, b) = x.get_far_attenuation_range();
    w.extend(d_words(a));
    w.extend(d_words(b));
    w.push(f(x.get_attenuation_range()));
    w.push(f(x.get_spot_angle()));
    w.push(f(x.get_spot_angle_cos()));
    w.extend(vw(&x.get_spot_direction()));
    w.push(f(x.get_spot_exponent()));
    let (c, r) = x.get_obj_space_bounding_sphere();
    w.extend(vw(&c));
    w.push(f(r));
    let (c, e) = x.get_obj_space_bounding_box();
    w.extend(vw(&c));
    w.extend(vw(&e));
    w.push(u32::from(x.force_visible));
    w.push(u32::from(x.is_vertex_processor()));
    w
}

// --- Load_W3D blobs ---
fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
    let mut v = id.to_le_bytes().to_vec();
    v.extend((body.len() as u32).to_le_bytes());
    v.extend(body);
    v
}
fn fl(x: f32) -> [u8; 4] {
    x.to_le_bytes()
}

fn light_blob(g: &mut Gen) -> Vec<u8> {
    let attr = match g.below(8) {
        0 => 1,
        1 => 2,
        2 => 3,
        3 => g.below(8),
        4 => 0x101,
        5 => 0x103 | (g.below(4) << 12),
        _ => g.below(3) + 1 + if g.below(2) == 0 { 0x100 } else { 0 },
    };
    let mut info = Vec::new();
    info.extend(attr.to_le_bytes());
    info.extend(0u32.to_le_bytes());
    for _ in 0..9 {
        info.push(g.below(256) as u8);
        if info.len() % 4 == 0 && info.len() < 20 {}
    }
    // 3 colours of 4 bytes each (r, g, b, pad)
    info.truncate(8);
    for _ in 0..3 {
        info.extend([g.below(256) as u8, g.below(256) as u8, g.below(256) as u8, g.below(256) as u8]);
    }
    info.extend(fl(g.uniform(0.0, 3.0)));
    assert_eq!(info.len(), 24);
    let mut body = chunk(W3D_CHUNK_LIGHT_INFO, &info);
    for _ in 0..g.below(7) {
        match g.below(5) {
            0 | 1 => {
                let mut b = Vec::new();
                for _ in 0..3 {
                    b.extend(fl(g.uniform(-1.0, 1.0)));
                }
                b.extend(fl(g.uniform(0.0, 3.2)));
                b.extend(fl(g.uniform(0.0, 10.0)));
                body.extend(chunk(W3D_CHUNK_SPOT_LIGHT_INFO, &b));
            }
            2 => {
                let mut b = fl(g.uniform(0.0, 100.0)).to_vec();
                b.extend(fl(g.uniform(0.0, 200.0)));
                body.extend(chunk(W3D_CHUNK_NEAR_ATTENUATION, &b));
            }
            3 => {
                let mut b = fl(g.uniform(0.0, 100.0)).to_vec();
                b.extend(fl(g.uniform(0.0, 200.0)));
                body.extend(chunk(W3D_CHUNK_FAR_ATTENUATION, &b));
            }
            _ => {
                let n = g.below(9) as usize;
                body.extend(chunk(0x7777 + g.below(3), &vec![g.below(256) as u8; n]));
            }
        }
    }
    chunk(W3D_CHUNK_LIGHT, &body)
}

// --- environment ---
struct Env {
    centre: [f32; 3],
    amb: [f32; 3],
    fill: f32,
    lights: Vec<Ld>,
    camera: [f32; 12],
    pre: u32,
}

fn env(g: &mut Gen) -> Env {
    let centre = [g.uniform(-60.0, 60.0), g.uniform(-60.0, 60.0), g.uniform(-60.0, 60.0)];
    let n = g.below(8) as usize;
    let mut lights = Vec::new();
    for _ in 0..n {
        let mut l = ld(g, centre);
        if g.below(8) == 0 {
            l.dif = [g.uniform(0.0, 0.06), g.uniform(0.0, 0.06), g.uniform(0.0, 0.06)];
        }
        if g.below(4) == 0 {
            l.set_far = 1;
            l.far_flag = 1;
            let a = g.uniform(0.0, 40.0) as f64;
            l.far = [a, a + f64::from(g.uniform(1.0, 80.0))];
        }
        lights.push(l);
    }
    let mut camera = [0f32; 12];
    for v in camera.iter_mut() {
        *v = g.uniform(-1.0, 1.0);
    }
    Env {
        centre,
        amb: [val(g, 0.0, 1.0), val(g, 0.0, 1.0), val(g, 0.0, 1.0)],
        fill: if g.below(2) == 0 { 0.0 } else { g.uniform(0.0, 1.5) },
        lights,
        camera,
        pre: g.below(2),
    }
}

fn env_words(e: &Env) -> Vec<u32> {
    let mut w: Vec<u32> = e.centre.iter().chain(e.amb.iter()).map(|x| f(*x)).collect();
    w.push(f(e.fill));
    w.push(e.lights.len() as u32);
    for l in &e.lights {
        w.extend(ld_words(l));
    }
    w.extend(e.camera.iter().map(|x| f(*x)));
    w.push(e.pre);
    w
}

/// `Calculate_Fill_Light` divides by the primary light's contribution; when that is 0 (a point light whose diffuse was folded
/// into the ambient) and a second light exists, the ratio is NaN, and upstream's `RGB_To_HSV` then leaves its `Vector3 temp`
/// uninitialised and reads it. Those calls are excluded (the case is skipped on both sides).
fn fill_risk(env: &LightEnvironmentClass, fill: f32) -> bool {
    if fill == 0.0 || env.get_light_count() == 0 {
        return false;
    }
    // the same uninitialised read when a light the fill light averages is NaN (a spot cone of angle 0 divides 0 by 0)
    let nan = (0..env.get_light_count().min(3)).any(|i| {
        let d = env.get_light_diffuse(i);
        let r = env.get_light_direction(i);
        d.x.is_nan() || d.y.is_nan() || d.z.is_nan() || r.x.is_nan() || r.y.is_nan() || r.z.is_nan()
    });
    nan || (env.get_light_count() >= 2 && env.get_light_diffuse(0).length2() == 0.0)
}

fn build(e: &Env, lod: &LightingLodCutoff) -> Option<(LightEnvironmentClass, Matrix3D, bool)> {
    let mut env = LightEnvironmentClass::new();
    env.reset(&v3(e.centre), &v3(e.amb));
    env.set_fill_intensity(e.fill);
    let lights: Vec<LightClass> = e.lights.iter().map(make_light).collect();
    for l in &lights {
        env.add_light(l, lod);
    }
    let cam = Matrix3D::from_array(e.camera);
    let mut rendered = false;
    if e.pre != 0 {
        if fill_risk(&env, e.fill) {
            return None;
        }
        env.pre_render_update(&cam);
        rendered = true;
    }
    Some((env, cam, rendered))
}

fn dump_env(env: &LightEnvironmentClass, rendered: bool) -> Vec<u32> {
    let mut w = vec![env.get_light_count() as u32];
    w.extend(vw(env.get_equivalent_ambient()));
    for i in 0..env.get_light_count() {
        w.extend(vw(env.get_light_direction(i)));
        w.extend(vw(env.get_light_diffuse(i)));
        w.push(u32::from(env.is_point_light(i)));
        if env.is_point_light(i) {
            w.push(f(env.get_point_irad(i)));
            w.push(f(env.get_point_orad(i)));
            w.extend(vw(env.get_point_diffuse(i)));
            w.extend(vw(env.get_point_ambient(i)));
            w.extend(vw(env.get_point_center(i)));
        }
    }
    if rendered {
        for i in 0..env.get_light_count() {
            w.extend(vw(&env.output_light(i).direction));
            w.extend(vw(&env.output_light(i).diffuse));
        }
    }
    w
}

#[test]
fn light_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_15_01);
    let mut c_state = Check::new("LightClass state (ctor, setters, getters, bounds)");
    let mut c_load = Check::new("LightClass::Load_W3D");
    let mut c_env = Check::new("LightEnvironmentClass");
    let mut c_eq = Check::new("LightEnvironmentClass::operator==");
    let mut c_lod = Check::new("Get_Lighting_LOD_Cutoff (initial)");
    let n = cases();

    // the LOD cutoff as the process started: first, before any case sets it
    {
        let cpp = run(&[4]);
        c_lod.f32(LightingLodCutoff::default().get_lighting_lod_cutoff(), f32::from_bits(cpp[0]), || "initial".into());
    }
    let mut lod = LightingLodCutoff::default();
    let mut skipped = 0u32;
    for case in 0..n {
        // light state
        {
            let l = ld(&mut g, [0.0; 3]);
            let mut inp = vec![0];
            inp.extend(ld_words(&l));
            let cpp = run(&inp);
            c_state.eq(state(&make_light(&l)), cpp, || format!("case {case}"));
        }
        // Load_W3D
        {
            let blob = light_blob(&mut g);
            let ty = g.below(3);
            let mut inp = vec![1, ty, blob.len() as u32];
            inp.extend(blob.iter().map(|b| u32::from(*b)));
            let cpp = run(&inp);
            let mut x = LightClass::new([LightType::Point, LightType::Directional, LightType::Spot][ty as usize]);
            let mut c = ChunkLoad::new(GameFile::new(&blob).unwrap());
            assert!(c.open_chunk().unwrap());
            let r = x.load_w3d(&mut c);
            let mut mine = vec![u32::from(r.is_err())];
            mine.extend(state(&x));
            c_load.eq(mine, cpp, || format!("blob {blob:?}"));
        }
        // environment
        {
            let (set, val_lod) = if case < 2000 {
                (0u32, 0.0f32)
            } else {
                (
                    1,
                    match g.below(6) {
                        0 => 0.5,
                        1 => 1.0,
                        2 => g.uniform(-3.0, 3.0),
                        3 => g.uniform(0.0, 1.9),
                        4 => 0.0,
                        _ => g.uniform(-300.0, 300.0),
                    },
                )
            };
            let e = env(&mut g);
            let posts: Vec<u32> = (0..g.below(5)).map(|_| g.below(3)).collect();
            let mut try_lod = lod;
            if set != 0 {
                try_lod.set_lighting_lod_cutoff(val_lod);
            }
            let mut mine = None;
            if let Some((mut me, cam, mut rendered)) = build(&e, &try_lod) {
                let mut ok = true;
                for p in &posts {
                    if *p != 0 && fill_risk(&me, e.fill) {
                        ok = false;
                        break;
                    }
                    match p {
                        0 => me.add_fill_light(),
                        1 => me.calculate_fill_light(),
                        _ => {
                            me.pre_render_update(&cam);
                            rendered = true;
                        }
                    }
                }
                if ok {
                    let mut w = dump_env(&me, rendered);
                    w.push(0xFFFF_FFFF);
                    w.push(f(try_lod.get_lighting_lod_cutoff()));
                    mine = Some(w);
                }
            }
            if let Some(mine) = mine {
                let mut inp = vec![2, set, f(val_lod)];
                inp.extend(env_words(&e));
                inp.push(posts.len() as u32);
                inp.extend(&posts);
                let cpp = run(&inp);
                lod = try_lod;
                c_env.eq(mine, cpp, || format!("case {case} lod {set}/{val_lod}"));
            } else {
                skipped += 1;
            }
        }
        // operator==
        {
            let a = env(&mut g);
            let mut b = Env { centre: a.centre, amb: a.amb, fill: a.fill, lights: a.lights.clone(), camera: a.camera, pre: a.pre };
            match g.below(5) {
                0 => {}
                1 => b.centre[g.below(3) as usize] += 1.0,
                2 => b.amb[g.below(3) as usize] += 0.25,
                3 => {
                    if let Some(l) = b.lights.last_mut() {
                        l.dif[0] += 0.5;
                    }
                }
                _ => b.camera[g.below(12) as usize] += 0.5,
            }
            let (Some((ea, _, _)), Some((eb, _, _))) = (build(&a, &lod), build(&b, &lod)) else {
                skipped += 1;
                continue;
            };
            let mut inp = vec![3];
            inp.extend(env_words(&a));
            inp.extend(env_words(&b));
            let cpp = run(&inp);
            c_eq.eq(vec![u32::from(ea.equals(&eb)), u32::from(eb.equals(&ea)), u32::from(ea.equals(&ea))], cpp, || format!("case {case}"));
        }
    }
    println!("skipped (fill-light NaN risk): {skipped}");
    for c in [&c_state, &c_load, &c_env, &c_eq] { for e in c.examples.iter().take(2) { println!("EX {}: {}", c.name, &e[..e.len().min(1500)]); } }
    finish(vec![c_lod, c_state, c_load, c_env, c_eq]);
}
