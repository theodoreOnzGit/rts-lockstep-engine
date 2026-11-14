#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/linegrp.cpp and linegrp.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #65 (batch e): `LineGroupClass` ([`LineGroup`]) against linegrp.cpp compiled verbatim over a recording device
//! (`differential/wrap_linegrp.cpp`, which lists the stand-ins and the words).
//!
//! # Methodology
//!
//! 20,000 random sequences. Each makes a line group, then (90 %) sets its state (each setter 70 %: a texture or none, a
//! random shader word, tetrahedra or prisms, `TRANSFORM` on (75 %) or off, a size of -1 to 6, a default colour and alpha
//! that are white (1), nearly white (0.9965 or 0.996, either side of upstream's 0.9961 threshold) or -0.2 to 1.2, a U
//! of -2 to 2, a tail colour), then runs 1 to 8 operations: the setters again (15 %), `Set_Arrays` (30 %) or `Render`
//! (55 %). `Set_Arrays`: 1 to 40 lines (1 in 20 100 to 300), each optional array present half the time; without a
//! table the active count is -1 (a third) or 0 to n, with one 0 to n and entries below n; points within 50 of the
//! origin, ends a step of up to 10 away, colours -0.2 to 1.2, sizes -1 to 6, U -2 to 2. `Render`: sorting enabled (80 %)
//! and a camera transform: an axis-aligned signed permutation (20 %), the identity (10 %) or a random rotation
//! (`build_matrix3d` of a random unit quaternion), with a translation of -100 to 100. Compared, word for word (floats as
//! bits, NaN equal to NaN): the draw (the sorting flag, every vertex's position, packed colour, U and V, every
//! triangle's indices, the shader, material and texture, whether the view was set) and after every operation the line
//! group's state and `Get_Polygon_Count()`.
//!
//! # Predictions (written before the first run, `issue65e_predictions.md`)
//!
//! 0 mismatching sequences on the first run (with upstream's in-place `Get_Orthogonal_Inverse` translated as traced:
//! the camera's rotation made symmetric, not inverted); 50 to 400 million words (best guess 150 million); every plant
//! caught.
//!
//! # Results (2026-11-14)
//!
//! - First run: **0 mismatching sequences of 20,000**; **19,652,464 words** compared (0 NaN-only), 21,493 draws,
//!   1,860,992 vertices, 0 undefined. The in-place `Get_Orthogonal_Inverse` quirk holds against the verbatim C++
//!   (plant 10157, the true inverse, mismatches in 4,695 sequences). Prediction misses: the words, 19.7 million against
//!   50 to 400 million (many renders come before any `Set_Arrays` or with a count of 0, and the lines are short).
//! - **Plants**, mismatching sequences of 20,000: 10156 8,806; 10157 4,695; 10158 5,179; 10159 6,562; 10160 6,562;
//!   10161 3,814; 10162 3,814; 10163 201; 10164 431; 10165 3,065; 10166 2,140; 10167 5,327; 10168 5,557; 10169 5,517;
//!   10170 5,519; 10171 6,673; 10172 3,789; 10173 3,845; 10174 6,097; 10175 2,817. **20 of 20 caught.**
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 10156 | the second and third offsets swapped |
//! | 10157 | the camera's rotation truly inverted (not upstream's in-place quirk) |
//! | 10158 | the camera's translation not zeroed before the inverse |
//! | 10159 | a tetrahedron's apex at V 0 |
//! | 10160 | a tetrahedron's first triangle wound the other way |
//! | 10161 | a prism's left side's second triangle wound the other way |
//! | 10162 | a prism's end cap takes the head colour |
//! | 10163 | the white-and-opaque test at 1, not over 0.9961 |
//! | 10164 | a colour array does not force the gradient |
//! | 10165 | alpha testing does not stop sorting |
//! | 10166 | sorting disabled is ignored |
//! | 10167 | the active line table is ignored |
//! | 10168 | the size array is ignored |
//! | 10169 | the U array is ignored |
//! | 10170 | the tail colour array is ignored |
//! | 10171 | without a table the active count is ignored |
//! | 10172 | culling is not turned on |
//! | 10173 | the offsets are turned with `TRANSFORM` off too |
//! | 10174 | a prism counts 6 polygons |
//! | 10175 | the size array is read by line, not by point |

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::w3d::linegrp::{LineGroup, LineGroupChunk, LineModeType, TRANSFORM};
use rts_lockstep_engine::formats::w3d::shader::ShaderClass;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::quat::{build_matrix3d, Quaternion};
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use support::Gen;

fn planted() -> u32 {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

fn cases() -> u64 {
    std::env::var("LINEGRP_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

/// One output word, and whether it is a float (compared NaN-equal).
type Word = (u32, bool);

fn wf(out: &mut Vec<Word>, f: f32) {
    out.push((f.to_bits(), true));
}
fn wu(out: &mut Vec<Word>, u: u32) {
    out.push((u, false));
}

fn state_words(out: &mut Vec<Word>, g: &LineGroup) {
    wu(out, g.flags);
    wf(out, g.default_line_size);
    wf(out, g.default_line_color.x);
    wf(out, g.default_line_color.y);
    wf(out, g.default_line_color.z);
    wf(out, g.default_line_alpha);
    wf(out, g.default_line_ucoord);
    wf(out, g.default_tail_diffuse.x);
    wf(out, g.default_tail_diffuse.y);
    wf(out, g.default_tail_diffuse.z);
    wf(out, g.default_tail_diffuse.w);
    wu(out, u32::from(g.line_mode == LineModeType::Prism));
    wu(out, g.line_count as u32);
    wu(out, g.shader.0);
    wu(out, u32::from(g.texture.is_some()));
    wu(out, g.get_polygon_count() as u32);
}

fn chunk_words(out: &mut Vec<Word>, c: &LineGroupChunk) {
    wu(out, 'C' as u32);
    wu(out, u32::from(c.sorting));
    wu(out, c.vertices.len() as u32);
    for v in &c.vertices {
        wf(out, v.position.x);
        wf(out, v.position.y);
        wf(out, v.position.z);
        wu(out, v.diffuse);
        wf(out, v.u);
        wf(out, v.v);
    }
    wu(out, c.triangles.len() as u32);
    for t in &c.triangles {
        for i in t {
            wu(out, u32::from(*i));
        }
    }
    wu(out, c.shader.0);
    wu(out, 0); // PRELIT_DIFFUSE
    wu(out, u32::from(c.texture.is_some()));
    wu(out, u32::from(c.view_identity));
}

fn rotation(g: &mut Gen) -> Matrix3D {
    let mut q = [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)];
    let l = (q.iter().map(|x| x * x).sum::<f32>()).max(1e-6);
    let k = 1.0 / rts_lockstep_engine::ww::wwmath::sqrt(l);
    for x in &mut q {
        *x *= k;
    }
    build_matrix3d(&Quaternion::new(q[0], q[1], q[2], q[3]))
}

fn camera(g: &mut Gen) -> Matrix3D {
    let mut m = match g.below(10) {
        0 => Matrix3D::identity(),
        1 | 2 => {
            // a signed permutation
            let perm = [[0usize, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]][g.below(6) as usize];
            let mut m = Matrix3D::identity();
            for r in 0..3 {
                for c in 0..3 {
                    m.row[r][c] = 0.0;
                }
                m.row[r][perm[r]] = if g.below(2) == 0 { 1.0 } else { -1.0 };
            }
            m
        }
        _ => rotation(g),
    };
    for r in 0..3 {
        m.row[r][3] = g.uniform(-100.0, 100.0);
    }
    m
}

/// A default colour channel: white, nearly white either side of 0.9961, or anything.
fn channel(g: &mut Gen) -> f32 {
    match g.below(6) {
        0 | 1 => 1.0,
        2 => 0.9965,
        3 => 0.996,
        _ => g.uniform(-0.2, 1.2),
    }
}

fn colour4(g: &mut Gen) -> Vector4 {
    Vector4::new(g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2), g.uniform(-0.2, 1.2))
}

fn f4(input: &mut Vec<u32>, v: Vector4) {
    input.extend([v.x.to_bits(), v.y.to_bits(), v.z.to_bits(), v.w.to_bits()]);
}
fn f3(input: &mut Vec<u32>, v: Vector3) {
    input.extend([v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]);
}

fn setters(g: &mut Gen, input: &mut Vec<u32>, lg: &mut LineGroup) {
    let mut mask = 0u32;
    for bit in 0..9 {
        if g.below(10) < 7 {
            mask |= 1 << bit;
        }
    }
    input.push(mask);
    if mask & 0x1 != 0 {
        let t = g.below(2);
        input.push(t);
        lg.set_texture(if t != 0 { Some(b"linegrp".to_vec()) } else { None });
    }
    if mask & 0x2 != 0 {
        let s = g.u32();
        input.push(s);
        lg.set_shader(ShaderClass(s));
    }
    if mask & 0x4 != 0 {
        let m = g.below(2);
        input.push(m);
        lg.set_line_mode(if m != 0 { LineModeType::Prism } else { LineModeType::Tetrahedron });
    }
    if mask & 0x8 != 0 {
        let on = u32::from(g.below(4) != 0);
        input.push(on);
        lg.set_flag(TRANSFORM, on != 0);
    }
    if mask & 0x10 != 0 {
        let s = g.uniform(-1.0, 6.0);
        input.push(s.to_bits());
        lg.set_line_size(s);
    }
    if mask & 0x20 != 0 {
        let c = Vector3::new(channel(g), channel(g), channel(g));
        f3(input, c);
        lg.set_line_color(c);
    }
    if mask & 0x40 != 0 {
        let a = channel(g);
        input.push(a.to_bits());
        lg.set_line_alpha(a);
    }
    if mask & 0x80 != 0 {
        let u = g.uniform(-2.0, 2.0);
        input.push(u.to_bits());
        lg.set_line_ucoord(u);
    }
    if mask & 0x100 != 0 {
        let t = colour4(g);
        f4(input, t);
        lg.set_tail_diffuse(t);
    }
}

fn set_arrays(g: &mut Gen, input: &mut Vec<u32>, lg: &mut LineGroup) {
    let n = if g.below(20) == 0 { 100 + g.below(201) } else { 1 + g.below(40) } as usize;
    let mut mask = 0u32;
    for bit in 0..5 {
        if g.below(2) == 0 {
            mask |= 1 << bit;
        }
    }
    let has_alt = mask & 0x4 != 0;
    let active: i32 = if !has_alt && g.below(3) == 0 { -1 } else { g.below(n as u32 + 1) as i32 };
    input.extend([n as u32, mask, active as u32]);
    let mut start = Vec::with_capacity(n);
    let mut end = Vec::with_capacity(n);
    for _ in 0..n {
        let p = Vector3::new(g.uniform(-50.0, 50.0), g.uniform(-50.0, 50.0), g.uniform(-50.0, 50.0));
        start.push(p);
    }
    for p in &start {
        let e = *p + Vector3::new(g.uniform(-10.0, 10.0), g.uniform(-10.0, 10.0), g.uniform(-10.0, 10.0));
        end.push(e);
    }
    for p in &start {
        f3(input, *p);
    }
    for p in &end {
        f3(input, *p);
    }
    let v4 = |present: bool, g: &mut Gen, input: &mut Vec<u32>| {
        present.then(|| {
            (0..n)
                .map(|_| {
                    let c = colour4(g);
                    f4(input, c);
                    c
                })
                .collect::<Vec<_>>()
        })
    };
    let diffuse = v4(mask & 0x1 != 0, g, input);
    let tail = v4(mask & 0x2 != 0, g, input);
    let alt = has_alt.then(|| {
        (0..n)
            .map(|_| {
                let a = g.below(n as u32);
                input.push(a);
                a
            })
            .collect::<Vec<_>>()
    });
    let fl = |present: bool, lo: f32, hi: f32, g: &mut Gen, input: &mut Vec<u32>| {
        present.then(|| {
            (0..n)
                .map(|_| {
                    let s = g.uniform(lo, hi);
                    input.push(s.to_bits());
                    s
                })
                .collect::<Vec<_>>()
        })
    };
    let sizes = fl(mask & 0x8 != 0, -1.0, 6.0, g, input);
    let ucoords = fl(mask & 0x10 != 0, -2.0, 2.0, g, input);
    lg.set_arrays(start, end, diffuse, tail, alt, sizes, ucoords, active);
}

/// One random sequence: the input words, the Rust's output words, and what it drew.
struct Case {
    input: Vec<u32>,
    rust: Vec<Word>,
    undefined: bool,
    draws: u64,
    verts: u64,
}

fn build_case(g: &mut Gen) -> Case {
    let mut input = Vec::new();
    let mut rust = Vec::new();
    let mut lg = LineGroup::new();
    let (mut draws, mut verts, mut undefined) = (0u64, 0u64, false);
    let mut ops: Vec<u32> = Vec::new();
    if g.below(10) < 9 {
        ops.push(0);
    }
    for _ in 0..1 + g.below(8) {
        let k = g.below(20);
        ops.push(match k {
            0..=2 => 0,
            3..=8 => 1,
            _ => 2,
        });
    }
    for op in ops {
        wu(&mut rust, op);
        input.push(op);
        match op {
            0 => setters(g, &mut input, &mut lg),
            1 => set_arrays(g, &mut input, &mut lg),
            _ => {
                let sorting = u32::from(g.below(5) != 0);
                let cam = camera(g);
                input.push(sorting);
                input.extend(cam.to_array().iter().map(|f| f.to_bits()));
                match lg.render(&cam, sorting != 0) {
                    Ok(Some(c)) => {
                        chunk_words(&mut rust, &c);
                        draws += 1;
                        verts += c.vertices.len() as u64;
                    }
                    Ok(None) => {}
                    Err(_) => undefined = true,
                }
                wu(&mut rust, 'E' as u32);
            }
        }
        state_words(&mut rust, &lg);
    }
    Case { input, rust, undefined, draws, verts }
}

/// *Results (2026-11-14):* 20,000 sequences, 19,652,464 words, 0 mismatching (see the module docs).
#[test]
fn line_group_matches_upstream() {
    let mut g = Gen::new(0x11E_6A0_65E);
    let n = cases();
    let (mut words, mut mismatched, mut undefined, mut nan_only) = (0u64, 0u64, 0u64, 0u64);
    let (mut draws, mut vertices) = (0u64, 0u64);
    let mut examples = Vec::new();
    let mut out = vec![0u32; 1 << 22];
    for case in 0..n {
        let c = build_case(&mut g);
        let len = unsafe { cpp::ref_linegrp_seq(c.input.as_ptr(), c.input.len() as i32, out.as_mut_ptr(), out.len() as i32) } as usize;
        assert!(len <= out.len(), "output buffer too small");
        let cpp_words = &out[..len];
        if c.undefined {
            undefined += 1;
        }
        let mut bad = cpp_words.len() != c.rust.len() || c.undefined;
        for (i, (rw, cw)) in c.rust.iter().zip(cpp_words).enumerate() {
            words += 1;
            if rw.0 != *cw {
                if rw.1 && f32::from_bits(rw.0).is_nan() && f32::from_bits(*cw).is_nan() {
                    nan_only += 1;
                    continue;
                }
                bad = true;
                if examples.len() < 6 {
                    examples.push(format!("case {case} word {i}: rust {:#010x} c++ {:#010x}", rw.0, cw));
                }
                break;
            }
        }
        draws += c.draws;
        vertices += c.verts;
        if bad {
            mismatched += 1;
            if examples.len() < 6 && cpp_words.len() != c.rust.len() {
                examples.push(format!("case {case}: rust {} words, c++ {} words", c.rust.len(), cpp_words.len()));
            }
        }
    }
    let line = format!(
        "linegroup: {n} sequences, {words} words compared ({nan_only} NaN-only), {draws} draws, {vertices} vertices, {undefined} undefined, \
         mismatching sequences {mismatched}"
    );
    if planted() != 0 {
        println!("MEASURED plant {} {line}", planted());
        for e in &examples {
            println!("    e.g. {e}");
        }
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    println!("DIFF {line}");
    assert_eq!(mismatched, 0, "{line}; first: {examples:#?}");
}
