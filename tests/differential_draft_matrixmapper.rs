// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dlogic) of the draft `src/drafts/ww3d/matrixmapper.rs`
//! (`MatrixMapperClass` and `CompositeMatrixMapperClass`) against upstream `WW3D2/matrixmapper.cpp` compiled WHOLE and
//! unchanged (differential/wrap_draft_matrixmapper.cpp, build.rs `drafts_ww3dlogic`). Stand-ins: `TextureMapperClass`
//! (stage, refcount, virtuals) and `DX8Wrapper::Set_Transform`/`Set_DX8_Texture_Stage_State`, which log their arguments;
//! the draft's `apply_state` returns what it would install, and the test compares that to the log.
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides): a plain mapper or a composite one (with or without an inner
//! mapper whose texture matrix is a random 4x4), a stage, then 1 to 12 random operations: `Set_Flag`, `Set_Type`,
//! `Set_Gradient_U_Coord`, `Set_Texture_Transform` (Matrix4x4 or Matrix3D, texture size among 64..1024, 0, 1, 2, 3 and
//! mixed values), `Compute_Texture_Coordinate`, `Apply` (every mapping type), `Calculate_Texture_Matrix` and the getters.
//! Every output float is compared by bits. `Update_View_To_Pixel_Transform` is not a public member: it is reached through
//! every one of them (all read `ViewToPixel` or `ViewSpaceProjectionNormal`).
//!
//! # Predictions (written before the first run, 2026-11-19)
//!
//! 0 mismatches expected. Places I am unsure of: `k = 0.5 * (tex_size - 2.0f) / tex_size` (the `0.5` is a double, so the
//! whole expression is double and narrowed: the draft does this), and `Normalize()` of a zero or NaN normal (WWMath's
//! `Vector3::Normalize` divides by `Length`, with a guard for zero length: the verified `Vector3` is used by the draft).
//!
//! # Excluded inputs
//!
//! `MatrixMapperClass::Apply` leaves the rows of its local `Matrix4x4 m` that it does not assign uninitialised (the default
//! constructor does nothing), so the device matrix's unset rows are garbage in C++: row 3 for the perspective type and rows
//! 2 and 3 for the two gradient types. The draft sets them to identity. Those rows are not compared (they are never read:
//! the texture transform with D3DTTFF_COUNT2/COUNT3 reads the first two or three rows).
//! `CompositeMatrixMapperClass(const CompositeMatrixMapperClass&)` and `Clone` need `TextureMapperClass::Clone`/refcounts
//! (an object model the draft does not have): not covered. `MatrixMapperClass::Clone` asserts and returns NULL.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;

use rts_lockstep_engine::client::tactical_view::Matrix4x4;
use rts_lockstep_engine::drafts::ww3d::matrixmapper::*;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dww3dl_mm_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
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

/// What the test needs of either class.
enum Mapper {
    Plain(MatrixMapperClass),
    Composite(CompositeMatrixMapperClass<Inner>),
}

#[derive(Clone)]
struct Inner(Matrix4x4);
impl TextureMatrixSource for Inner {
    fn calculate_texture_matrix(&self) -> Matrix4x4 {
        self.0
    }
}

impl Mapper {
    fn base(&mut self) -> &mut MatrixMapperClass {
        match self {
            Mapper::Plain(m) => m,
            Mapper::Composite(c) => &mut c.base,
        }
    }
}

fn mat4(g: &mut Gen) -> [f32; 16] {
    let mut a = [0.0f32; 16];
    let kind = g.below(10);
    for v in a.iter_mut() {
        *v = match kind {
            0 => g.mixed(),
            1 => (g.below(5) as f32) - 2.0,
            _ => g.uniform(-4.0, 4.0),
        };
    }
    a
}

fn texsize(g: &mut Gen) -> f32 {
    match g.below(10) {
        0 => 64.0,
        1 => 128.0,
        2 => 256.0,
        3 => 512.0,
        4 => 1024.0,
        5 => [0.0, 1.0, 2.0, 3.0, -2.0][g.below(5) as usize],
        6 => g.mixed(),
        _ => g.uniform(4.0, 2048.0),
    }
}

fn from4(a: &[f32]) -> Matrix4x4 {
    let mut m = Matrix4x4::identity();
    for i in 0..4 {
        for j in 0..4 {
            m.row[i][j] = a[i * 4 + j];
        }
    }
    m
}

fn words(f: &[f32]) -> Vec<u32> {
    f.iter().map(|x| x.to_bits()).collect()
}

fn code_tci(s: &str) -> u32 {
    match s {
        "D3DTSS_TCI_CAMERASPACEPOSITION" => 0x20000,
        "D3DTSS_TCI_CAMERASPACENORMAL" => 0x10000,
        _ => panic!("tci {s}"),
    }
}

fn code_flags(s: &str) -> u32 {
    match s {
        "D3DTTFF_COUNT2" => 2,
        "D3DTTFF_PROJECTED|D3DTTFF_COUNT3" => 256 | 3,
        _ => panic!("flags {s}"),
    }
}

#[test]
fn matrix_mapper_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_13_01);
    let names = ["Getters (Set_Flag, Set_Type, Set_Gradient_U_Coord, Get_Texture_Transform)", "Compute_Texture_Coordinate", "Apply", "Calculate_Texture_Matrix"];
    let mut chk: Vec<Check> = Vec::new();
    for cls in ["MatrixMapperClass", "CompositeMatrixMapperClass"] {
        for n in names {
            chk.push(Check::new(&format!("{cls}::{n}")));
        }
    }
    for case in 0..cases() {
        let composite = g.below(2) == 0;
        let has_inner = g.below(4) != 0;
        let stage = g.below(8);
        let inner_m = mat4(&mut g);
        let mut input: Vec<u32> = vec![stage, u32::from(composite), u32::from(has_inner)];
        input.extend(words(&inner_m));
        let mut mapper = if composite {
            Mapper::Composite(CompositeMatrixMapperClass::new(if has_inner { Some(Inner(from4(&inner_m))) } else { None }, stage))
        } else {
            Mapper::Plain(MatrixMapperClass::new(stage as i32))
        };
        let nops = 1 + g.below(12);
        input.push(nops);
        // each expectation: (check index offset 0..4, words)
        let mut expect: Vec<(usize, Vec<u32>, Vec<bool>)> = Vec::new();
        let base_idx = if composite { 4 } else { 0 };
        let mut tex_set = false;
        for _ in 0..nops {
            let op = g.below(9);
            match op {
                0 => {
                    let flag = [1u32, 0, 2, 3, 0x8000_0000, g.u32()][g.below(6) as usize];
                    let on = g.below(2) == 0;
                    input.extend([0, flag, u32::from(on)]);
                    mapper.base().set_flag(flag, on);
                }
                1 => {
                    let t = g.below(4);
                    input.extend([1, t]);
                    let ty = [MappingType::OrthoProjection, MappingType::PerspectiveProjection, MappingType::DepthGradient, MappingType::NormalGradient][t as usize];
                    mapper.base().set_type(ty);
                }
                2 => {
                    let f = if g.below(5) == 0 { g.mixed() } else { g.uniform(-1.0, 2.0) };
                    input.extend([2, f.to_bits()]);
                    mapper.base().set_gradient_u_coord(f);
                }
                3 => {
                    let m = mat4(&mut g);
                    let ts = texsize(&mut g);
                    input.push(3);
                    input.extend(words(&m));
                    input.push(ts.to_bits());
                    mapper.base().set_texture_transform(&from4(&m), ts);
                    tex_set = true;
                }
                4 => {
                    let mut a = [0.0f32; 12];
                    for v in a.iter_mut() {
                        *v = if g.below(10) == 0 { g.mixed() } else { g.uniform(-4.0, 4.0) };
                    }
                    let ts = texsize(&mut g);
                    input.push(4);
                    input.extend(words(&a));
                    input.push(ts.to_bits());
                    mapper.base().set_texture_transform_3d(&Matrix3D::from_array(a), ts);
                    tex_set = true;
                }
                5 => {
                    let p = [g.geom(), g.geom(), g.geom()];
                    input.extend([5]);
                    input.extend(words(&p));
                    let r = mapper.base().compute_texture_coordinate(&Vector3::new(p[0], p[1], p[2]));
                    expect.push((1 + base_idx, words(&[r.x, r.y, r.z]), vec![true; 3]));
                }
                6 => {
                    let uv = g.below(4) as i32;
                    input.extend([6, uv as u32]);
                    let (st, ty) = match &mut mapper {
                        Mapper::Plain(m) => (m.apply_state(uv), m.get_type()),
                        Mapper::Composite(c) => (c.apply_state(uv), c.base.get_type()),
                    };
                    let stage_w = 16 + stage;
                    // rows compared: all four, three, or two (see "Excluded inputs")
                    let rows = match ty {
                        MappingType::OrthoProjection => 4,
                        MappingType::PerspectiveProjection => 3,
                        _ => 2,
                    };
                    let mut w: Vec<u32> = vec![26, 1000, stage_w];
                    let mut mask: Vec<bool> = vec![true; 3];
                    for i in 0..4 {
                        for j in 0..4 {
                            w.push(st.texture_transform.row[i][j].to_bits());
                            mask.push(i < rows);
                        }
                    }
                    w.extend([1001, stage, 11, code_tci(st.tex_coord_index), 1001, stage, 24, code_flags(st.texture_transform_flags)]);
                    mask.extend(vec![true; 8]);
                    // the C++ log prefix: length 26 words, then 1000 stage m[16] ...
                    expect.push((2 + base_idx, w, mask));
                }
                7 => {
                    input.extend([7]);
                    let m = match &mapper {
                        Mapper::Plain(m) => m.calculate_texture_matrix(),
                        Mapper::Composite(c) => c.calculate_texture_matrix(),
                    };
                    let mut w = Vec::new();
                    for i in 0..4 {
                        for j in 0..4 {
                            w.push(m.row[i][j].to_bits());
                        }
                    }
                    expect.push((3 + base_idx, w, vec![true; 16]));
                }
                _ => {
                    let flag = [1u32, 0, 2, 3, 0x8000_0000, g.u32()][g.below(6) as usize];
                    input.extend([8, flag]);
                    let b = mapper.base();
                    let mut w = vec![u32::from(b.get_flag(flag)), b.get_type() as u32, b.get_gradient_u_coord().to_bits()];
                    for i in 0..4 {
                        for j in 0..4 {
                            w.push(b.get_texture_transform().row[i][j].to_bits());
                        }
                    }
                    expect.push((base_idx, w, vec![true; 19]));
                }
            }
        }
        let _ = tex_set;
        let mut out = vec![0u32; 4096];
        let n = unsafe { ref_dww3dl_mm_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
        assert!(n >= 0, "case {case}: output buffer too small");
        let mut pos = 0usize;
        for (idx, w, mask) in &expect {
            let what = || format!("case {case} composite {composite} inner {has_inner} stage {stage}");
            for (k, (&rw, &m)) in w.iter().zip(mask.iter()).enumerate() {
                let cw = out[pos + k];
                if !m {
                    continue;
                }
                // integer-valued words and float words alike are compared as bit patterns (a Check::eq on u32)
                chk[*idx].f32(f32::from_bits(rw), f32::from_bits(cw), || format!("{} word {k}", what()));
            }
            pos += w.len();
        }
        assert_eq!(pos as i32, n, "case {case}: output length");
    }
    finish(chk);
}
