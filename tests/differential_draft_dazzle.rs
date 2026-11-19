// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group sweepww3d) of the draft `src/drafts/sweep_ww3d/dazzle.rs` against upstream
//! `WW3D2/dazzle.cpp` cut VERBATIM (build.rs `drafts_sweepww3d`, differential/wrap_draft_dazzle.cpp):
//!
//! | draft | upstream |
//! |---|---|
//! | `get_vector2`, `get_vector3`, `get_vector4` | `DazzleINIClass::Get_Vector2`, `Get_Vector3`, `Get_Vector4` (`sscanf` of the entry's text) |
//! | `lensflare_generate_vertex_buffers` | `LensflareTypeClass::Generate_Vertex_Buffers` |
//! | `DazzleTypeClass::calculate_intensities` | `DazzleTypeClass::Calculate_Intensities` |
//!
//! NOT covered: `render_dazzle` and `vis_render_dazzle` (device and rasteriser host calls; `dazzle_index_buffer` is the index
//! fill INSIDE `Render_Dazzle`, which is not separable from the dynamic index buffer lock), the texture getters and shader
//! setters (host calls and assignments), the type tables (`DazzleTables`: name lookups over `StringClass` and the static
//! tables of the whole object), the visible lists and `set_current_dazzle_layer` (list and static assignments).
//!
//! Stand-ins: the INI entry's text, `DX8Wrapper::Convert_Color` (a hash of the colour and alpha bits, so the colour passed is
//! observable), the vertex format reduced to the six fields filled, and the two init classes reduced to the fields read.
//! `pow`/`powf`/`sqrt` are interposed in this test binary to forward to `petir::Real` (like the other C math), so that the draft
//! and the C++ are not compared across two libms; the wrapper asserts at compile time that `pow(float, float)` and
//! `sqrt(float)` are the C `double` functions (the upstream was written for MSVC 6, which has no `float` overloads).
//!
//! # Method
//!
//! 20,000 cases per member (fixed seed, `DRAFT_CASES` overrides). Vector text: 1 to 5 fields drawn from numbers (signed,
//! with exponents, `.5`, `5.`, hexadecimal, `inf`, `nan`, overflow), words, empty strings, with separators `,`, `, `, ` ,`, `,,`
//! and none, leading and trailing blanks; present or absent. Lens flares: 0 to 4 flares of random location, size, colour (above
//! and below the clamp of 1 once multiplied by the intensity) and UV. Intensities: area, direction area, powers (including the
//! `WWMATH_EPSILON` boundary), fade distances (equal start and end among them), unit-ish directions and dot products at 0 and
//! the extremes, squared distances on both sides of the fade bounds.
//!
//! # Predictions (written before the first run, 2026-11-19)
//!
//! Two MISSES expected, neither found by running: (1) `get_vector*` parse each comma-separated part with `trim()` then Rust's
//! `str::parse`, where `sscanf("%f,%f")` skips blanks BEFORE a number but needs the comma right after it, accepts hexadecimal
//! floats and a number followed by garbage up to the comma only if nothing else follows; "1 ,2" is rejected by `sscanf` and
//! accepted by the draft; (2) `Calculate_Intensities` calls the C `double` `pow` and narrows, the draft calls `f32::powf`.
//! `lensflare_generate_vertex_buffers` (`sqrt` of a float expression is the C double `sqrt`, then `+ 1.0f` in float) 0 expected.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use petir::Real;
use rts_lockstep_engine::drafts::sweep_ww3d::dazzle::*;
use std::sync::Mutex;
use support::{Check, Gen};
use support_sweep::{b, cases, feq, finish};

extern "C" {
    fn ref_dsww3d_dz_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

/// C's double `pow` for the C++ reference (the draft's own maths goes through `petir`).
#[no_mangle]
pub extern "C" fn pow(x: f64, y: f64) -> f64 {
    Real::powf(x, y)
}
/// C's `powf`.
#[no_mangle]
pub extern "C" fn powf(x: f32, y: f32) -> f32 {
    Real::powf(x, y)
}

static CPP: Mutex<()> = Mutex::new(());

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 4096];
    let n = unsafe { ref_dsww3d_dz_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

fn val(g: &mut Gen) -> f32 {
    match g.below(8) {
        0 => g.special(),
        1 => 0.0,
        2 => (g.below(5) as f32) - 2.0,
        _ => g.uniform(-5.0, 5.0),
    }
}

fn number(g: &mut Gen) -> String {
    match g.below(17) {
        0 => "1".into(),
        1 => "-2.5".into(),
        2 => "+3.25".into(),
        3 => ".5".into(),
        4 => "5.".into(),
        5 => "1e3".into(),
        6 => "2E-2".into(),
        7 => "0x10".into(),
        8 => "inf".into(),
        9 => "nan".into(),
        10 => "1e999".into(),
        11 => "abc".into(),
        12 => String::new(),
        13 => "1.5f".into(),
        14 => ["1e", "1e+", "0x", ".", "-", "+.5", "nan(abc)", "INF", "Infinity", "1_0", "0x1.8p1", "0X.8", "-nan", "infinit", "1e-"][g.below(15) as usize].into(),
        _ => format!("{}", g.uniform(-100.0, 100.0)),
    }
}

fn vector_text(g: &mut Gen) -> String {
    let n = 1 + g.below(5);
    let mut s = String::new();
    if g.below(5) == 0 {
        s.push(' ');
    }
    for i in 0..n {
        if i > 0 {
            s.push_str(["," , ", ", " ,", ",,", " ", ";"][g.below(6) as usize]);
        }
        s.push_str(&number(g));
        if g.below(12) == 0 {
            s.push_str("x");
        }
    }
    if g.below(5) == 0 {
        s.push(' ');
    }
    s
}

/// `DX8Wrapper::Convert_Color` of the wrapper.
fn convert_color(c: [f32; 3], a: f32) -> u32 {
    31u32.wrapping_mul(c[0].to_bits()).wrapping_add(131u32.wrapping_mul(c[1].to_bits())).wrapping_add(1031u32.wrapping_mul(c[2].to_bits())).wrapping_add(a.to_bits())
}

fn unit(g: &mut Gen) -> [f32; 3] {
    match g.below(6) {
        0 => [0.0, 0.0, 1.0],
        1 => [0.0, 0.0, -1.0],
        2 => [1.0, 0.0, 0.0],
        _ => {
            let v = [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
            [v[0] / l, v[1] / l, v[2] / l]
        }
    }
}

#[test]
fn dazzle_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0f01);
    let names = ["Get_Vector2", "Get_Vector3", "Get_Vector4", "Generate_Vertex_Buffers", "Calculate_Intensities"];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        // ---- the INI vector readers
        for k in 0..3usize {
            let dim = 2 + k;
            let has = g.below(6) != 0;
            let text = vector_text(&mut g);
            let def: Vec<f32> = (0..dim).map(|_| val(&mut g)).collect();
            let mut inp = vec![k as u32, u32::from(has), text.len() as u32];
            inp.extend(text.bytes().map(u32::from));
            inp.extend(b(&def));
            let t = has.then_some(text.as_str());
            let rust = match k {
                0 => get_vector2(t, [def[0], def[1]]).to_vec(),
                1 => get_vector3(t, [def[0], def[1], def[2]]).to_vec(),
                _ => get_vector4(t, [def[0], def[1], def[2], def[3]]).to_vec(),
            };
            feq(&mut c[k], b(&rust), run(&inp), || format!("case {case}: {text:?} (present {has})"));
        }
        // ---- Generate_Vertex_Buffers
        {
            let n = g.below(5) as usize;
            let intensity = g.uniform(0.0, 3.0);
            let lic = LensflareInit {
                flare_count: n,
                flare_locations: (0..n).map(|_| val(&mut g)).collect(),
                flare_sizes: (0..n).map(|_| val(&mut g)).collect(),
                flare_colors: (0..n).map(|_| [g.uniform(0.0, 2.0), g.uniform(0.0, 2.0), if g.below(8) == 0 { val(&mut g) } else { g.uniform(0.0, 2.0) }]).collect(),
                flare_uv: (0..n).map(|_| [val(&mut g), val(&mut g), val(&mut g), val(&mut g)]).collect(),
                texture_name: String::new(),
            };
            let (sx, sy) = (val(&mut g), val(&mut g));
            let tl = [val(&mut g), val(&mut g), val(&mut g), val(&mut g)];
            let mut inp = vec![3, n as u32];
            for a in 0..n {
                inp.push(lic.flare_locations[a].to_bits());
                inp.push(lic.flare_sizes[a].to_bits());
                inp.extend(b(&lic.flare_colors[a]));
                inp.extend(b(&lic.flare_uv[a]));
            }
            inp.extend(b(&[sx, sy, intensity]));
            inp.extend(b(&tl));
            let mut vb = Vec::new();
            let count = lensflare_generate_vertex_buffers(&lic, &mut vb, sx, sy, intensity, tl, convert_color);
            let mut rust = vec![count as u32];
            for v in &vb {
                rust.extend(b(&[v.x, v.y, v.z, v.u1, v.v1]));
                rust.push(v.diffuse);
            }
            feq(&mut c[3], rust, run(&inp), || format!("case {case}: {n} flares"));
        }
        // ---- Calculate_Intensities
        {
            let ic = DazzleInit {
                dazzle_area: match g.below(5) {
                    0 => 1.0,
                    1 => 0.0,
                    _ => g.uniform(0.01, 1.2),
                },
                dazzle_direction_area: if g.below(3) == 0 { 0.0 } else { g.uniform(0.0, 1.2) },
                dazzle_size_pow: [0.0, 0.5, 1.0, 2.0][g.below(4) as usize] + if g.below(2) == 0 { g.uniform(0.0, 3.0) } else { 0.0 },
                dazzle_intensity_pow: [0.0, 0.5, 1.0, 2.0][g.below(4) as usize] + if g.below(2) == 0 { g.uniform(0.0, 3.0) } else { 0.0 },
                dazzle_intensity: g.uniform(0.0, 3.0),
                halo_intensity_pow: [0.0, 0.00005, 0.0001, 0.0002, 1.0][g.below(5) as usize] + if g.below(3) == 0 { g.uniform(0.0, 4.0) } else { 0.0 },
                halo_intensity: g.uniform(0.0, 3.0),
                use_camera_translation: g.below(2) == 0,
                fadeout_start: g.uniform(0.0, 100.0),
                fadeout_end: 0.0,
                ..Default::default()
            };
            let mut ic = ic;
            ic.fadeout_end = if g.below(6) == 0 { ic.fadeout_start } else { ic.fadeout_start + g.uniform(0.0, 200.0) };
            let (fs, fe) = if g.below(5) == 0 { (g.uniform(0.0, 1.0e4), g.uniform(0.0, 4.0e4)) } else { (ic.fadeout_start * ic.fadeout_start, ic.fadeout_end * ic.fadeout_end) };
            let ty = DazzleTypeClass {
                name: String::new(),
                ic: ic.clone(),
                lensflare_id: 0,
                dazzle_shader: 0,
                halo_shader: 0,
                fadeout_start_sqr: fs,
                fadeout_end_sqr: fe,
                primary_texture: None,
                secondary_texture: None,
            };
            let halo_in = g.uniform(0.0, 3.0);
            let (cam, daz, dir) = (unit(&mut g), unit(&mut g), unit(&mut g));
            let distance = match g.below(4) {
                0 => fs,
                1 => fe,
                _ => g.uniform(0.0, 5.0e4),
            };
            let mut inp = vec![4];
            inp.extend(b(&[ic.dazzle_area, ic.dazzle_direction_area, ic.dazzle_size_pow, ic.dazzle_intensity_pow, ic.dazzle_intensity, ic.halo_intensity_pow, ic.halo_intensity]));
            inp.push(u32::from(ic.use_camera_translation));
            inp.extend(b(&[ic.fadeout_start, ic.fadeout_end, fs, fe, halo_in]));
            inp.extend(b(&cam));
            inp.extend(b(&daz));
            inp.extend(b(&dir));
            inp.push(distance.to_bits());
            let (di, ds, hi) = ty.calculate_intensities(halo_in, cam, daz, dir, distance);
            feq(&mut c[4], b(&[di, ds, hi]), run(&inp), || format!("case {case}: ic {ic:?} fs {fs} fe {fe} halo {halo_in} distance {distance}"));
        }
    }
    finish(c);
}
