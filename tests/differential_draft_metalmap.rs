// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dpure) of the draft `src/drafts/ww3d/metalmap.rs` (`MetalMapManagerClass`)
//! against upstream `WW3D2/metalmap.cpp` and the three `VectorProcessorClass` functions it calls (`WWMath/vp.cpp`), cut
//! VERBATIM (build.rs `drafts_ww3dpure`, differential/wrap_draft_metalmap.cpp): the constructor, `Get_Metal_Map`,
//! `Metal_Map_Count`, `Update_Lighting`, `Update_Textures`, `initialize_normal_table`, `initialize_metal_params`, `DotProduct`,
//! `ClampMin` and `Power`. Stand-ins (for what the cut calls outside itself): `INIClass` answers from the test's script,
//! `WW3D::Get_Device_Resolution` returns the test's bit depth, and the texture/surface/filter/string classes are byte buffers
//! that record the name, format and clamp modes they are given (see the wrapper's header). The destructor (reference
//! releases only) is not compared.
//!
//! # Method
//!
//! 20,000 managers per operation from a fixed seed (`DRAFT_CASES` overrides): 0..=5 `MetalNN` sections, each entry
//! (`AmbientColor`, `DiffuseColor`, `SpecularColor`, `Shininess`) present or absent (an absent one returns upstream's default:
//! white, 0), colours in INI units (0..255, sometimes beyond), shininess below, at and above the clamp (0, 1, 127), a device
//! depth of 8, 16, 24 or 32 bits, then `Update_Lighting` (ambient, light colour, light direction, camera direction: unit,
//! zero, opposite, random) and `Update_Textures`. Compared, as bits: the map count, `Use16Bit`, every `MetalParams` (so the
//! constructor's clamps, scale and the missing clamps are exercised), each texture's name, format and clamp modes, and every
//! pixel byte of every map; plus the 256 normals of `_NormalTable` once per case. `Get_Metal_Map` and `Metal_Map_Count` are
//! compared through the map count and the in-range test.
//!
//! # Excluded inputs
//!
//! - DiffuseColor and SpecularColor below 0 in the INI, and negative or non-finite lighting colours: upstream clamps only
//!   AmbientColor (its three clamps, see the draft's header), so a negative diffuse or specular colour gives a negative
//!   channel, and `(unsigned char)Floor(negative)` (and of NaN) is undefined in C++.
//! - Lighting colours above 10 (the product could overflow to infinity and then NaN, same conversion).
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! 0 mismatches. The place to watch is `Power`: the draft calls `petir::Real::powf` (libm's `powf`, the ARM optimised-routines
//! port), upstream `powf` is glibc's, which is the same algorithm in the same release series, so I expect bit equality; if it
//! is not, that is a ULP difference to record, not a draft defect (the AGENTS rule routes the logic's maths through petir).
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;

use rts_lockstep_engine::drafts::ww3d::metalmap::*;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dpw_metal_normals(out: *mut f32) -> i32;
    fn ref_dpw_metal(input: *const u32, out: *mut u32) -> i32;
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

/// The scripted INI: `script[n]` = 4 present flags (ambient, diffuse, specular, shininess) and 10 values.
struct Script {
    sections: Vec<[f32; 14]>,
}

impl MetalMapIni for Script {
    fn find_section(&self, section: &str) -> bool {
        let n: usize = section.trim_start_matches("Metal").parse().unwrap_or(usize::MAX);
        n < self.sections.len()
    }
    fn get_point(&self, section: &str, entry: &str, default: (f32, f32, f32)) -> (f32, f32, f32) {
        let n: usize = section.trim_start_matches("Metal").parse().unwrap();
        let s = &self.sections[n];
        let k = match entry {
            "AmbientColor" => 0,
            "DiffuseColor" => 1,
            _ => 2,
        };
        if s[k] == 0.0 {
            return default;
        }
        (s[4 + k * 3], s[5 + k * 3], s[6 + k * 3])
    }
    fn get_float(&self, section: &str, _entry: &str, default: f32) -> f32 {
        let n: usize = section.trim_start_matches("Metal").parse().unwrap();
        let s = &self.sections[n];
        if s[3] == 0.0 {
            return default;
        }
        s[13]
    }
}

fn colour(g: &mut Gen) -> f32 {
    match g.below(8) {
        0 => 0.0,
        1 => 255.0,
        2 => 128.0,
        3 => g.uniform(255.0, 600.0),
        _ => g.uniform(0.0, 255.0),
    }
}

fn dir(g: &mut Gen) -> [f32; 3] {
    match g.below(6) {
        0 => [0.0, 0.0, 0.0],
        1 => [1.0, 0.0, 0.0],
        2 => [0.0, 0.0, 1.0],
        _ => {
            let v = [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if l > 0.0 && g.below(3) != 0 { [v[0] / l, v[1] / l, v[2] / l] } else { v }
        }
    }
}

fn light(g: &mut Gen) -> f32 {
    match g.below(6) {
        0 => 0.0,
        1 => 1.0,
        2 => g.uniform(1.0, 10.0),
        _ => g.uniform(0.0, 1.0),
    }
}

#[test]
fn metalmap_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_14_04);
    let mut c_norm = Check::new("MetalMapManagerClass::initialize_normal_table");
    let mut c_ctor = Check::new("MetalMapManagerClass(INIClass) (params, textures, depth)");
    let mut c_upd = Check::new("MetalMapManagerClass::Update_Textures (pixel bytes)");
    let mut c_get = Check::new("Get_Metal_Map / Metal_Map_Count");
    for case in 0..cases() {
        // _NormalTable
        if case % 50 == 0 {
            let mut out = vec![0.0f32; 768];
            unsafe { ref_dpw_metal_normals(out.as_mut_ptr()) };
            let rust = initialize_normal_table();
            let r: Vec<f32> = rust.iter().flat_map(|v| [v.x, v.y, v.z]).collect();
            c_norm.f32s(&r, &out, || format!("case {case}"));
        }
        let count = g.below(6) as usize;
        let bits = [8, 16, 24, 32][g.below(4) as usize];
        let mut sections = Vec::new();
        for _ in 0..count {
            let mut s = [0.0f32; 14];
            for k in 0..4 {
                s[k] = if g.below(4) == 0 { 0.0 } else { 1.0 };
            }
            for k in 0..3 {
                let amb = k == 0;
                for c in 0..3 {
                    // ambient may be anything (it is clamped); diffuse and specular stay >= 0 (see the header)
                    s[4 + k * 3 + c] = if amb && g.below(8) == 0 { -g.uniform(0.0, 300.0) } else { colour(&mut g) };
                }
            }
            s[13] = [0.0, 0.5, 1.0, 1.5, 4.0, 127.0, 127.5, 200.0, -3.0][g.below(9) as usize];
            if g.below(2) == 0 {
                s[13] = g.uniform(0.0, 130.0);
            }
            sections.push(s);
        }
        let amb = [light(&mut g), light(&mut g), light(&mut g)];
        let lc = [light(&mut g), light(&mut g), light(&mut g)];
        let ld = dir(&mut g);
        let cd = if g.below(8) == 0 { [-ld[0], -ld[1], -ld[2]] } else { dir(&mut g) };

        let mut input: Vec<u32> = vec![bits as u32, count as u32];
        for s in &sections {
            input.extend(s.iter().map(|f| f.to_bits()));
        }
        for v in [amb, lc, ld, cd] {
            input.extend(v.iter().map(|f| f.to_bits()));
        }
        let mut out = vec![0u32; 20_000];
        let n = unsafe { ref_dpw_metal(input.as_ptr(), out.as_mut_ptr()) } as usize;
        let cpp = &out[..n];

        let ini = Script { sections };
        let mut m = MetalMapManagerClass::new(&ini, bits);
        m.update_lighting(&Vector3::new(amb[0], amb[1], amb[2]), &Vector3::new(lc[0], lc[1], lc[2]), &Vector3::new(ld[0], ld[1], ld[2]), &Vector3::new(cd[0], cd[1], cd[2]));
        m.update_textures();

        // the constructor's part of the output (everything but the pixel bytes), then the pixel bytes
        let mut ctor_r: Vec<u32> = vec![m.map_count as u32, u32::from(m.use_16_bit)];
        let mut ctor_c: Vec<u32> = vec![cpp[0], cpp[1]];
        let mut pix_r: Vec<u32> = Vec::new();
        let mut pix_c: Vec<u32> = Vec::new();
        let mut at = 2;
        for i in 0..m.map_count as usize {
            let p = &m.metal_parameters[i];
            for v in [p.ambient_color, p.diffuse_color, p.specular_color] {
                ctor_r.extend_from_slice(&[v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]);
            }
            ctor_r.push(p.shininess.to_bits());
            let t = &m.textures[i];
            let mut name = [0u32; 16];
            for (k, b) in t.name.bytes().take(16).enumerate() {
                name[k] = u32::from(b);
            }
            ctor_r.extend_from_slice(&name);
            ctor_r.push(match t.format {
                MetalMapFormat::A4R4G4B4 => 0,
                MetalMapFormat::A8R8G8B8 => 1,
            });
            ctor_r.push(u32::from(t.clamp));
            ctor_r.push(u32::from(t.clamp));
            ctor_c.extend_from_slice(&cpp[at..at + 10 + 16 + 3]);
            at += 10 + 16 + 3;
            let bpp = if t.format == MetalMapFormat::A4R4G4B4 { 2 } else { 4 };
            pix_r.extend(t.pixels.iter().map(|&b| u32::from(b)));
            pix_c.extend_from_slice(&cpp[at..at + 256 * bpp]);
            at += 256 * bpp;
            assert_eq!(t.pixels.len(), 256 * bpp);
        }
        assert_eq!(at, n, "case {case}: the C++ output length");
        c_ctor.compared += ctor_r.len() as u64;
        for (k, (a, b)) in ctor_r.iter().zip(ctor_c.iter()).enumerate() {
            if a != b {
                c_ctor.mismatches += 1;
                if c_ctor.examples.len() < 8 {
                    c_ctor.examples.push(format!("case {case} word {k}: rust {a:#x} != c++ {b:#x}"));
                }
            }
        }
        assert_eq!(ctor_r.len(), ctor_c.len(), "case {case}: constructor output length");
        c_upd.compared += pix_r.len() as u64;
        for (k, (a, b)) in pix_r.iter().zip(pix_c.iter()).enumerate() {
            if a != b {
                c_upd.mismatches += 1;
                if c_upd.examples.len() < 8 {
                    c_upd.examples.push(format!("case {case} byte {k}: rust {a} != c++ {b}"));
                }
            }
        }
        // Get_Metal_Map / Metal_Map_Count: in range or not, against the C++ count
        let cnt = cpp[0] as i32;
        for id in [-1, 0, cnt - 1, cnt, cnt + 1] {
            let inside = id >= 0 && id < cnt;
            c_get.eq((m.get_metal_map(id).is_some(), m.metal_map_count()), (inside, cnt), || format!("case {case} id {id}"));
        }
    }
    finish(vec![c_norm, c_ctor, c_upd, c_get]);
}
