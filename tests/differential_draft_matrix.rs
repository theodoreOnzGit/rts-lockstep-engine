#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the drafts src/drafts/wwlib/matrix3.rs and matrix4.rs against (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/matrix3.h, matrix3.cpp, matrix4.h, matrix4.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with the additional terms in NOTICE.md.

//! `Matrix3x3` and `Matrix4x4` (the draft methods beside the verified `Matrix4x4`) against the C++ (#215).
//!
//! # Method
//!
//! `differential/wrap_draft_matrix.cpp` includes matrix3.h and matrix4.h and compiles matrix3.cpp and matrix4.cpp
//! UNCHANGED (nothing cut, no stand-ins; Matrix3D and Quaternion are the upstream objects already linked; `sinf`, `cosf`,
//! `tan`, `atan2f` are the interposed petir ones on both sides, `tests/support`). Each case is one operation (69 for
//! `Matrix3x3`, 41 for `Matrix4x4`: every constructor, `Set`/`Init`, the operators, the statics, `Inverse`,
//! `Determinant`, `Is_Orthogonal`, `Re_Orthogonalize`, `Rotate_*`, `Get_*_Rotation` ...) on random inputs from a fixed
//! seed; every output float is compared by bits. Inputs: matrices of every kind (random, small integers so pivots
//! tie, rotations, singular (a zero row, a repeated or scaled row, all zero), near-singular (a row equal to another
//! plus 1e-6 of noise), identity with noise, and the full mixed generator with NaN, infinity and subnormals),
//! angles (including past a turn), unit and non-unit axes and quaternions, scalars (zero a twentieth of the time, so
//! the `/` and `/=` cases divide by zero).
//!
//! Aliased calls the C++ has are run too: `Multiply(r, B, &r)`, `Rotate_Vector(A, v, &v)`, `Transform_Vector(A, v, &v)`.
//! `Swap(Matrix3x3&, ...)` and `Swap(Matrix4x4&, ...)` are declared friends upstream and never defined, so they are
//! not tested (no C++ to compare, nothing calls them). `Matrix4x4::Init_Perspective`/`Init_Ortho` assert their
//! arguments in debug builds only.
//!
//! # Predictions (written before the first run, 2026-11-18)
//!
//! - `Matrix4x4::Inverse`: 0 mismatches only if the draft keeps upstream's Gauss-Jordan; it carries `Vector4`'s `/=`.
//! - everything else: 0 mismatches (same `float` expressions in the same order). The one place I expect a miss is
//!   `Matrix3x3::Set(Quaternion)`, whose `double` and `float` mixture I cannot check by eye, and the `ROWCOL`
//!   statements, whose left-to-right order the draft must keep.
//!
//! # Results (2026-11-18)
//!
//! **0 mismatches on the first run**, 20,000 cases of each of 69 `Matrix3x3` operations, 41 `Matrix4x4` operations and the
//! ten `Matrix3x3` constants (every output float by bits), max ULP 0; some hundreds of both-NaN pairs differ only in
//! payload (`Re_Orthogonalize` 127 of 180,000, a few others), which `tests/support` does not count. **Prediction missed:**
//! I expected a miss in `Set(Quaternion)` or the `ROWCOL` order; there was none (the products are `float`, so the `double`
//! `2.0 *` and `1.0 -` round to the same `float` as `float` arithmetic would). The `Is_Orthogonal` inputs were widened
//! after the first plant run (random unit rows, so a negative dot product with the other checks passing occurs): plant 11268
//! was first missed (0 of 20,000), then caught.
//!
//! Excluded inputs: `Init_Ortho` and `Init_Perspective` are run only on arguments that pass upstream's `assert`s (about
//! half of the random ones): the draft keeps them as `assert!`, which also fires in release, where upstream's `assert` is
//! compiled out (`NDEBUG`).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id> DRAFT_CASES=20000` (the run prints a `PLANTED` line). Mismatching outputs (and operations affected):
//!
//! | id | planted bug | outputs | ops |
//! |---|---|---|---|
//! | 11261 | `Set(axis, s, c)` row 0 column 1 adds the `axis[2]*s` term | 76,140 | 4 |
//! | 11262 | `Determinant` adds the third term | 17,060 | 1 |
//! | 11263 | `Inverse` pivot test `>=` | 6,221 | 1 |
//! | 11264 | `Inverse` swaps the rows of `a` but not of `b` | 105,962 | 1 |
//! | 11265 | `a * b` sums in the other order | 33,585 | 1 |
//! | 11266 | `Set(Quaternion)` `[0][1]` adds where upstream subtracts | 38,794 | 2 |
//! | 11267 | `Re_Orthogonalize` skips `y = z x x` | 52,262 | 1 |
//! | 11268 | `Is_Orthogonal` takes the absolute value of the first dot product | 249 | 1 |
//! | 11269 | `Rotate_Y(s, c)` sign | 110,827 | 2 |
//! | 11270 | `Get_X_Rotation` swaps the `atan2` arguments | 18,588 | 1 |
//! | 11271 | `Matrix4x4 /=` divides (the defect fixed) | 294,952 | 1 |
//! | 11272 | `Init_Ortho` `[0][3]` loses its sign | 19,985 | 1 |
//! | 11273 | `Init_Perspective(hfov, ..)` in `float` | 5,770 | 1 |
//! | 11274 | `Matrix3D * Matrix4x4` last row is 0,0,0,1 | 141,426 | 2 |
//! | 11275 | `Matrix4x4 * Matrix3D` drops `a[i][3]` | 128,466 | 2 |
//! | 11276 | `bits_equal` compares by value | 896 | 2 |
//! | 11277 | `Transform_Vector` drops the translation z | 33,222 | 2 |
//! | 11278 | `Matrix4x4::Inverse` pivot test `>=` | 30,385 | 1 |
//! | 11279 | `Transpose` `[2][3]` | 18,847 | 1 |
//!
//! **19 of 19 caught.** (11266 replaced a first plant, 11266 as an `f64` instead of `f32` product, that cannot differ:
//! the product by 2.0 is exact.)

mod support;

use rts_lockstep_engine::drafts::wwlib::matrix3::{
    create_x_rotation_matrix3, create_x_rotation_matrix3_rad, create_y_rotation_matrix3, create_y_rotation_matrix3_rad,
    create_z_rotation_matrix3, create_z_rotation_matrix3_rad, Matrix3x3,
};
use rts_lockstep_engine::drafts::wwlib::matrix4::Matrix4x4;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::quat::Quaternion;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use support::{Check, Gen};

extern "C" {
    fn draft_m3_constant(k: i32, out: *mut f32) -> i32;
    fn draft_m3(op: i32, p: *const f32, out: *mut f32) -> i32;
    fn draft_m4(op: i32, p: *const f32, out: *mut f32) -> i32;
}

const CASES: usize = 20_000;

/// `DRAFT_CASES=<n>` overrides the number of cases (the planted-bug runs use fewer than the gated run).
fn cases(default: usize) -> usize {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(default)
}

fn plant_id() -> u32 {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

fn m3(a: &[f32]) -> Matrix3x3 {
    Matrix3x3::from_elements(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8])
}
fn f3(m: &Matrix3x3) -> Vec<f32> {
    let mut v = Vec::new();
    for i in 0..3 {
        for j in 0..3 {
            v.push(m[i][j]);
        }
    }
    v
}
fn m4(a: &[f32]) -> Matrix4x4 {
    Matrix4x4::from_elements(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10], a[11], a[12], a[13], a[14], a[15])
}
fn f4(m: &Matrix4x4) -> Vec<f32> {
    let mut v = Vec::new();
    for i in 0..4 {
        for j in 0..4 {
            v.push(m[i][j]);
        }
    }
    v
}
fn md(a: &[f32]) -> Matrix3D {
    let mut x = [0.0f32; 12];
    x.copy_from_slice(&a[0..12]);
    Matrix3D::from_array(x)
}
fn v3(a: &[f32]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}
fn fv3(v: Vector3) -> Vec<f32> {
    vec![v.x, v.y, v.z]
}
fn fv4(v: Vector4) -> Vec<f32> {
    vec![v.x, v.y, v.z, v.w]
}

/// A square matrix of `n` x `n` (3 or 4): see the module docs for the kinds.
fn square(g: &mut Gen, n: usize) -> Vec<f32> {
    let mut m = vec![0.0f32; n * n];
    match g.below(14) {
        12 | 13 if n == 3 => {
            // three random unit rows: the Is_Orthogonal and Re_Orthogonalize thresholds, with negative dot products
            for r in 0..3 {
                let v = [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)];
                let l = petir::Real::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
                for j in 0..3 {
                    m[r * 3 + j] = if l > 0.0 { v[j] / l } else { v[j] };
                }
            }
        }
        0..=2 => {
            for x in m.iter_mut() {
                *x = g.uniform(-10.0, 10.0);
            }
        }
        3 | 4 => {
            for x in m.iter_mut() {
                *x = (g.below(7) as i32 - 3) as f32;
            }
        }
        5 => {
            // a rotation (orthonormal to rounding), with a translation column and a 0001 row when 4x4
            let (a, b, c) = (g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0));
            let (sa, ca, sb, cb, sc, cc) = (
                petir::Real::sin(a), petir::Real::cos(a), petir::Real::sin(b), petir::Real::cos(b), petir::Real::sin(c),
                petir::Real::cos(c),
            );
            let r = [
                cb * cc, -cb * sc, sb,
                sa * sb * cc + ca * sc, -sa * sb * sc + ca * cc, -sa * cb,
                -ca * sb * cc + sa * sc, ca * sb * sc + sa * cc, ca * cb,
            ];
            for i in 0..3 {
                for j in 0..3 {
                    m[i * n + j] = r[i * 3 + j];
                }
            }
            if n == 4 {
                for i in 0..3 {
                    m[i * 4 + 3] = g.uniform(-50.0, 50.0);
                }
                m[15] = 1.0;
            }
        }
        6 | 7 => {
            // singular: a zero row, a repeated row, a scaled row, or all zero
            for x in m.iter_mut() {
                *x = g.uniform(-10.0, 10.0);
            }
            let (r0, r1) = (g.below(n as u32) as usize, g.below(n as u32) as usize);
            match g.below(4) {
                0 => {
                    for j in 0..n {
                        m[r0 * n + j] = 0.0;
                    }
                }
                1 => {
                    for j in 0..n {
                        m[r0 * n + j] = m[r1 * n + j];
                    }
                }
                2 => {
                    let k = g.uniform(-3.0, 3.0);
                    for j in 0..n {
                        m[r0 * n + j] = m[r1 * n + j] * k;
                    }
                }
                _ => {
                    for x in m.iter_mut() {
                        *x = 0.0;
                    }
                }
            }
        }
        8 => {
            // near-singular
            for x in m.iter_mut() {
                *x = g.uniform(-10.0, 10.0);
            }
            let k = g.uniform(-3.0, 3.0);
            for j in 0..n {
                m[(n - 1) * n + j] = m[j] * k + g.uniform(-1.0e-6, 1.0e-6);
            }
        }
        9 => {
            for i in 0..n {
                for j in 0..n {
                    m[i * n + j] = (if i == j { 1.0 } else { 0.0 }) + g.uniform(-0.05, 0.05);
                }
            }
        }
        10 => {
            for x in m.iter_mut() {
                *x = g.geom();
            }
        }
        _ => {
            for x in m.iter_mut() {
                *x = g.mixed();
            }
        }
    }
    m
}

fn unit_or_not(g: &mut Gen, n: usize) -> Vec<f32> {
    let mut v: Vec<f32> = (0..n).map(|_| g.uniform(-3.0, 3.0)).collect();
    if g.below(10) == 0 {
        for x in v.iter_mut() {
            *x = g.mixed();
        }
    } else if g.below(2) == 0 {
        let l = petir::Real::sqrt(v.iter().map(|x| x * x).sum::<f32>());
        if l > 0.0 {
            for x in v.iter_mut() {
                *x /= l;
            }
        }
    }
    v
}

fn scalar(g: &mut Gen) -> f32 {
    match g.below(20) {
        0 => 0.0,
        1 => g.mixed(),
        2..=9 => g.uniform(-4.0, 4.0),
        10..=11 => g.uniform(-30.0, 30.0),
        _ => g.uniform(-2.0, 2.0),
    }
}

fn params3(g: &mut Gen) -> [f32; 64] {
    let mut p = [0.0f32; 64];
    p[0..9].copy_from_slice(&square(g, 3));
    p[9..18].copy_from_slice(&square(g, 3));
    p[18..21].copy_from_slice(&unit_or_not(g, 3));
    p[21..25].copy_from_slice(&unit_or_not(g, 4));
    // a Matrix3D: rows of four, mostly a rotation with a translation
    let r = square(g, 4);
    for i in 0..12 {
        p[25 + i] = r[i];
    }
    p[37..53].copy_from_slice(&square(g, 4));
    p[53] = scalar(g);
    p[54] = scalar(g);
    if g.below(2) == 0 {
        // a sine and cosine pair
        let t = g.uniform(-4.0, 4.0);
        p[53] = petir::Real::sin(t);
        p[54] = petir::Real::cos(t);
    }
    p
}

fn params4(g: &mut Gen) -> [f32; 64] {
    let mut p = [0.0f32; 64];
    p[0..16].copy_from_slice(&square(g, 4));
    p[16..32].copy_from_slice(&square(g, 4));
    p[32..35].copy_from_slice(&unit_or_not(g, 3));
    let r = square(g, 4);
    for i in 0..12 {
        p[36 + i] = r[i];
    }
    p[48..52].copy_from_slice(&unit_or_not(g, 4));
    for i in 0..6 {
        p[52 + i] = scalar(g);
    }
    // ortho and perspective want a sensible box now and then (znear < zfar, left < right ...)
    if g.below(2) == 0 {
        p[52] = g.uniform(-100.0, 0.0);
        p[53] = g.uniform(0.5, 100.0);
        p[54] = g.uniform(-100.0, 0.0);
        p[55] = g.uniform(0.5, 100.0);
        p[56] = g.uniform(0.1, 5.0);
        p[57] = p[56] + g.uniform(1.0, 500.0);
    }
    p
}

const M3_NAMES: [&str; 69] = [
    "ctor(true)", "ctor(m11..m33)", "ctor(r0,r1,r2)", "ctor(axis,angle)", "ctor(axis,s,c)", "ctor(Quaternion)",
    "ctor(Matrix3D)", "ctor(Matrix4x4)", "Set(Matrix3D)", "Set(Matrix4x4)", "Set(axis,angle)", "Set(axis,s,c)",
    "Set(Quaternion)", "Transpose", "Inverse", "Determinant", "+=", "-=", "*=", "/=", "Make_Identity", "Rotate_X(t)",
    "Rotate_X(s,c)", "Rotate_Y(t)", "Rotate_Y(s,c)", "Rotate_Z(t)", "Rotate_Z(s,c)", "Get_X_Rotation", "Get_Y_Rotation",
    "Get_Z_Rotation", "Get_X_Vector", "Get_Y_Vector", "Get_Z_Vector", "Get_X_Vector(&)", "Get_Y_Vector(&)",
    "Get_Z_Vector(&)", "-a", "a*d", "d*a", "a/d", "a+b", "Add", "a-b", "Subtract", "a*b", "Matrix3D*Matrix3x3",
    "Matrix3x3*Matrix3D", "Multiply(a,b,&r)", "Multiply(r,b,&r)", "Multiply(Matrix3D,b)", "Multiply(a,Matrix3D)", "a*v",
    "Rotate_Vector", "Transpose_Rotate_Vector", "Rotate_Vector(aliased)", "Transpose_Rotate_Vector(aliased)",
    "Is_Orthogonal", "Re_Orthogonalize", "Rotate_AABox_Extent", "Create_X_Rotation(s,c)", "Create_X_Rotation(rad)",
    "Create_Y_Rotation(s,c)", "Create_Y_Rotation(rad)", "Create_Z_Rotation(s,c)", "Create_Z_Rotation(rad)", "==", "!=",
    "= Matrix3D", "= Matrix4x4",
];

fn rust_m3(op: usize, p: &[f32; 64]) -> Vec<f32> {
    let a = &p[0..9];
    let b = &p[9..18];
    let axis = v3(&p[18..21]);
    let q = Quaternion::new(p[21], p[22], p[23], p[24]);
    let dm = md(&p[25..37]);
    let d4 = m4(&p[37..53]);
    let (s0, s1) = (p[53], p[54]);
    let ma = m3(a);
    let mb = m3(b);
    match op {
        0 => f3(&Matrix3x3::new(true)),
        1 => f3(&m3(a)),
        2 => f3(&Matrix3x3::from_rows(&v3(&a[0..3]), &v3(&a[3..6]), &v3(&a[6..9]))),
        3 => f3(&Matrix3x3::from_axis_angle(&axis, s0)),
        4 => f3(&Matrix3x3::from_axis_sin_cos(&axis, s0, s1)),
        5 => f3(&Matrix3x3::from_quaternion(&q)),
        6 => f3(&Matrix3x3::from_matrix3d(&dm)),
        7 => f3(&Matrix3x3::from_matrix4x4(&d4)),
        8 => {
            let mut m = ma;
            m.set_matrix3d(&dm);
            f3(&m)
        }
        9 => {
            let mut m = ma;
            m.set_matrix4x4(&d4);
            f3(&m)
        }
        10 => {
            let mut m = ma;
            m.set_axis_angle(&axis, s0);
            f3(&m)
        }
        11 => {
            let mut m = ma;
            m.set_axis_sin_cos(&axis, s0, s1);
            f3(&m)
        }
        12 => {
            let mut m = ma;
            m.set_quaternion(&q);
            f3(&m)
        }
        13 => f3(&ma.transpose()),
        14 => f3(&ma.inverse()),
        15 => vec![ma.determinant()],
        16 => {
            let mut m = ma;
            m.add_assign(&mb);
            f3(&m)
        }
        17 => {
            let mut m = ma;
            m.sub_assign(&mb);
            f3(&m)
        }
        18 => {
            let mut m = ma;
            m.mul_assign_scalar(s0);
            f3(&m)
        }
        19 => {
            let mut m = ma;
            m.div_assign_scalar(s0);
            f3(&m)
        }
        20 => {
            let mut m = ma;
            m.make_identity();
            f3(&m)
        }
        21 => {
            let mut m = ma;
            m.rotate_x(s0);
            f3(&m)
        }
        22 => {
            let mut m = ma;
            m.rotate_x_sc(s0, s1);
            f3(&m)
        }
        23 => {
            let mut m = ma;
            m.rotate_y(s0);
            f3(&m)
        }
        24 => {
            let mut m = ma;
            m.rotate_y_sc(s0, s1);
            f3(&m)
        }
        25 => {
            let mut m = ma;
            m.rotate_z(s0);
            f3(&m)
        }
        26 => {
            let mut m = ma;
            m.rotate_z_sc(s0, s1);
            f3(&m)
        }
        27 => vec![ma.get_x_rotation()],
        28 => vec![ma.get_y_rotation()],
        29 => vec![ma.get_z_rotation()],
        30 | 33 => fv3(ma.get_x_vector()),
        31 | 34 => fv3(ma.get_y_vector()),
        32 | 35 => fv3(ma.get_z_vector()),
        36 => f3(&ma.negated()),
        37 | 38 => f3(&ma.scaled(s0)),
        39 => f3(&ma.divided(s0)),
        40 | 41 => f3(&Matrix3x3::add(&ma, &mb)),
        42 | 43 => f3(&Matrix3x3::subtract(&ma, &mb)),
        44 => f3(&Matrix3x3::mul_matrix(&ma, &mb)),
        45 => f3(&Matrix3x3::mul_matrix3d_matrix(&dm, &mb)),
        46 => f3(&Matrix3x3::mul_matrix_matrix3d(&ma, &dm)),
        47 => f3(&Matrix3x3::multiply(&ma, &mb)),
        48 => f3(&Matrix3x3::multiply(&ma, &mb)),
        49 => f3(&Matrix3x3::mul_matrix3d_matrix(&dm, &mb)),
        50 => f3(&Matrix3x3::mul_matrix_matrix3d(&ma, &dm)),
        51 => fv3(ma.mul_vector(&axis)),
        52 | 54 => fv3(Matrix3x3::rotate_vector(&ma, &axis)),
        53 | 55 => fv3(Matrix3x3::transpose_rotate_vector(&ma, &axis)),
        56 => vec![if ma.is_orthogonal() { 1.0 } else { 0.0 }],
        57 => {
            let mut m = ma;
            m.re_orthogonalize();
            f3(&m)
        }
        58 => fv3(ma.rotate_aabox_extent(&axis)),
        59 => f3(&create_x_rotation_matrix3(s0, s1)),
        60 => f3(&create_x_rotation_matrix3_rad(s0)),
        61 => f3(&create_y_rotation_matrix3(s0, s1)),
        62 => f3(&create_y_rotation_matrix3_rad(s0)),
        63 => f3(&create_z_rotation_matrix3(s0, s1)),
        64 => f3(&create_z_rotation_matrix3_rad(s0)),
        65 => vec![if ma == mb { 1.0 } else { 0.0 }],
        66 => vec![if ma != mb { 1.0 } else { 0.0 }],
        67 => {
            let mut m = ma;
            m.set_matrix3d(&dm); // operator = (Matrix3D)
            f3(&m)
        }
        68 => {
            let mut m = ma;
            m.set_matrix4x4(&d4); // operator = (Matrix4x4)
            f3(&m)
        }
        _ => unreachable!(),
    }
}

/// A case for op 8..: a few ops need related inputs (the two multiplications of op 48/49/50 use b; fine as is).
fn run_m3(name: &str, ops: &[usize], seed: u64) {
    let mut g = Gen::new(seed);
    let mut checks: Vec<Check> = (0..69).map(|i| Check::new(&format!("Matrix3x3 {}", M3_NAMES[i]))).collect();
    for _ in 0..cases(CASES) {
        let p = params3(&mut g);
        for &op in ops {
            let r = rust_m3(op, &p);
            let mut out = [0.0f32; 40];
            let n = unsafe { draft_m3(op as i32, p.as_ptr(), out.as_mut_ptr()) };
            assert_eq!(n as usize, r.len(), "{name}: op {op} wrote {n} floats, the Rust {}", r.len());
            let pp = &p;
            checks[op].f32s(&r, &out[..n as usize], || format!("p = {:?}", &pp[..]));
        }
    }
    finish(checks, ops);
}

fn finish(checks: Vec<Check>, ops: &[usize]) {
    let planted = plant_id() != 0;
    let mut bad = 0u64;
    for (i, c) in checks.into_iter().enumerate() {
        if !ops.contains(&i) {
            continue;
        }
        if planted {
            bad += c.report();
        } else {
            c.finish();
        }
    }
    if planted {
        println!("PLANTED {} mismatching outputs over all ops", bad);
        assert!(bad > 0, "plant {} not caught", plant_id());
    }
}

#[test]
fn matrix3x3_matches_the_cpp() {
    let all: Vec<usize> = (0..69).collect();
    // ops 67 is an operator= test written through the constructor; both are plain row copies
    run_m3("m3", &all, 0x215_3333);
}

#[test]
fn matrix3x3_constants_match_the_cpp() {
    let rust = [
        Matrix3x3::IDENTITY, Matrix3x3::ROTATE_X90, Matrix3x3::ROTATE_X180, Matrix3x3::ROTATE_X270, Matrix3x3::ROTATE_Y90,
        Matrix3x3::ROTATE_Y180, Matrix3x3::ROTATE_Y270, Matrix3x3::ROTATE_Z90, Matrix3x3::ROTATE_Z180, Matrix3x3::ROTATE_Z270,
    ];
    let mut c = Check::new("Matrix3x3 constants (Identity, RotateX90 .. RotateZ270)");
    for (k, m) in rust.iter().enumerate() {
        let mut out = [0.0f32; 9];
        let n = unsafe { draft_m3_constant(k as i32, out.as_mut_ptr()) };
        assert_eq!(n, 9);
        c.f32s(&f3(m), &out, || format!("constant {k}"));
    }
    c.finish();
}

const M4_NAMES: [&str; 41] = [
    "ctor(true)", "ctor(m11..m44)", "ctor(r0..r3)", "ctor(Matrix3D)", "Make_Identity", "Init(Matrix3D)", "Init(r0..r3)",
    "Init(m11..m44)", "Init_Ortho", "Init_Perspective(hfov,vfov)", "Init_Perspective(l,r,b,t)", "Transpose", "Inverse", "+=",
    "-=", "*=", "/= (multiplies)", "-a", "a*d", "d*a", "a/d", "a+b", "Add", "a-b", "Subtract", "a*b", "Multiply(a,b)",
    "a*Matrix3D", "Matrix3D*b", "Multiply(a,b,&r)", "Multiply(Matrix3D,b,&r)", "Multiply(a,Matrix3D,&r)", "a*Vector4",
    "a*Vector3", "Transform_Vector(V3->V3)", "Transform_Vector(V3->V4)", "Transform_Vector(V4->V4)",
    "Transform_Vector(V3 aliased)", "Transform_Vector(V4 aliased)", "==", "!=",
];

fn rust_m4(op: usize, p: &[f32; 64]) -> Vec<f32> {
    let a = &p[0..16];
    let b = &p[16..32];
    let v = v3(&p[32..35]);
    let dm = md(&p[36..48]);
    let w = &p[48..52];
    let w4 = Vector4::new(w[0], w[1], w[2], w[3]);
    let s: Vec<f32> = p[52..58].to_vec();
    let ma = m4(a);
    let mb = m4(b);
    match op {
        0 => f4(&Matrix4x4::new(true)),
        1 => f4(&m4(a)),
        2 => f4(&Matrix4x4::from_rows(
            &Vector4::new(a[0], a[1], a[2], a[3]), &Vector4::new(a[4], a[5], a[6], a[7]),
            &Vector4::new(a[8], a[9], a[10], a[11]), &Vector4::new(a[12], a[13], a[14], a[15]),
        )),
        3 => f4(&Matrix4x4::from_matrix3d(&dm)),
        4 => {
            let mut m = ma;
            m.make_identity();
            f4(&m)
        }
        5 => {
            let mut m = ma;
            m.init_matrix3d(&dm);
            f4(&m)
        }
        6 => {
            let mut m = ma;
            m.init_rows(
                &Vector4::new(b[0], b[1], b[2], b[3]), &Vector4::new(b[4], b[5], b[6], b[7]),
                &Vector4::new(b[8], b[9], b[10], b[11]), &Vector4::new(b[12], b[13], b[14], b[15]),
            );
            f4(&m)
        }
        7 => {
            let mut m = ma;
            m.init_elements(b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]);
            f4(&m)
        }
        8 => {
            let mut m = ma;
            m.init_ortho(s[0], s[1], s[2], s[3], s[4], s[5]);
            f4(&m)
        }
        9 => {
            let mut m = ma;
            m.init_perspective_fov(s[0], s[1], s[2], s[3]);
            f4(&m)
        }
        10 => {
            let mut m = ma;
            m.init_perspective(s[0], s[1], s[2], s[3], s[4], s[5]);
            f4(&m)
        }
        11 => f4(&ma.transpose()),
        12 => f4(&ma.inverse()),
        13 => {
            let mut m = ma;
            m.add_assign(&mb);
            f4(&m)
        }
        14 => {
            let mut m = ma;
            m.sub_assign(&mb);
            f4(&m)
        }
        15 => {
            let mut m = ma;
            m.mul_assign_scalar(s[0]);
            f4(&m)
        }
        16 => {
            let mut m = ma;
            m.div_assign_scalar(s[0]);
            f4(&m)
        }
        17 => f4(&ma.negated()),
        18 | 19 => f4(&ma.scaled(s[0])),
        20 => f4(&ma.divided(s[0])),
        21 | 22 => f4(&Matrix4x4::add(&ma, &mb)),
        23 | 24 => f4(&Matrix4x4::subtract(&ma, &mb)),
        25 => f4(&Matrix4x4::mul_matrix(&ma, &mb)),
        26 => f4(&Matrix4x4::multiply(&ma, &mb)),
        27 => f4(&Matrix4x4::mul_matrix_matrix3d(&ma, &dm)),
        28 => f4(&Matrix4x4::mul_matrix3d_matrix(&dm, &mb)),
        29 => f4(&Matrix4x4::multiply(&ma, &mb)),
        30 => f4(&Matrix4x4::multiply_matrix3d_matrix(&dm, &mb)),
        31 => f4(&Matrix4x4::multiply_matrix_matrix3d(&ma, &dm)),
        32 => fv4(ma.mul_vector4(&w4)),
        33 => fv4(ma.mul_vector3(&v)),
        34 | 37 => fv3(Matrix4x4::transform_vector3(&ma, &v)),
        35 => fv4(Matrix4x4::transform_vector3_to_vector4(&ma, &v)),
        36 | 38 => fv4(Matrix4x4::transform_vector4(&ma, &w4)),
        39 => vec![if ma.bits_equal(&mb) { 1.0 } else { 0.0 }],
        40 => vec![if ma.bits_equal(&mb) { 0.0 } else { 1.0 }],
        _ => unreachable!(),
    }
}

/// `Init_Ortho`, `Init_Perspective` assert `znear >= 0` / `> 0` and `zfar > znear` (`> 0`) in debug builds only; the draft
/// keeps them as `assert!`, which also runs in release (a documented deviation: a panic where upstream's release build
/// carries on). So these three ops are tested on arguments that pass the assertions (about half of the random ones do not).
fn fix_for(op: usize, p: &[f32; 64], g: &mut Gen) -> [f32; 64] {
    let mut q = *p;
    match op {
        8 | 10 => {
            let znear = if op == 8 { g.uniform(0.0, 5.0) } else { g.uniform(0.01, 5.0) };
            q[56] = znear;
            q[57] = znear + g.uniform(0.01, 500.0);
            if g.below(2) == 0 {
                q[52] = g.uniform(-100.0, 0.0);
                q[53] = g.uniform(0.5, 100.0);
                q[54] = g.uniform(-100.0, 0.0);
                q[55] = g.uniform(0.5, 100.0);
            }
        }
        9 => {
            q[54] = g.uniform(0.01, 5.0);
            q[55] = q[54] + g.uniform(0.01, 500.0);
            if g.below(2) == 0 {
                q[52] = g.uniform(0.05, 3.0);
                q[53] = g.uniform(0.05, 3.0);
            }
        }
        _ => {}
    }
    q
}

#[test]
fn matrix4x4_matches_the_cpp() {
    let mut g = Gen::new(0x215_4444);
    let ops: Vec<usize> = (0..41).collect();
    let mut checks: Vec<Check> = (0..41).map(|i| Check::new(&format!("Matrix4x4 {}", M4_NAMES[i]))).collect();
    for case in 0..cases(CASES) {
        let mut p = params4(&mut g);
        if case % 7 == 0 {
            // equal matrices (and equal but for a sign of zero) for == and !=
            let (a, _) = p.split_at(16);
            let a: Vec<f32> = a.to_vec();
            let mut b = a.clone();
            if case % 14 == 0 {
                b[(g.below(16)) as usize] = -0.0;
                if a.iter().any(|x| *x == 0.0) {
                    b = a.iter().map(|x| if *x == 0.0 { -*x } else { *x }).collect();
                }
            }
            p[16..32].copy_from_slice(&b);
        }
        for &op in &ops {
            let p = fix_for(op, &p, &mut g);
            let r = rust_m4(op, &p);
            let mut out = [0.0f32; 40];
            let n = unsafe { draft_m4(op as i32, p.as_ptr(), out.as_mut_ptr()) };
            assert_eq!(n as usize, r.len(), "op {op} wrote {n} floats, the Rust {}", r.len());
            let pp = &p;
            checks[op].f32s(&r, &out[..n as usize], || format!("p = {:?}", &pp[..]));
        }
    }
    finish(checks, &ops);
}
